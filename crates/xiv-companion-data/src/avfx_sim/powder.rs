use super::SplitMix64;
use crate::avfx::AvfxParticleSimple;

pub(super) fn packed_position(point: [f32; 3], has_vertex_binding: bool) -> [f32; 3] {
    let length = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
    if !length.is_finite() || length <= 0.0 {
        return [0.0; 3];
    }
    // Client 0x14040e720 / 0x14040e7f0 pack normalized positions as short / byte.
    let resolution = if has_vertex_binding { 100.0 } else { 32000.0 };
    std::array::from_fn(|axis| {
        let packed = (point[axis] / length * resolution).trunc() as i32;
        let packed = if has_vertex_binding {
            packed as i8 as f32
        } else {
            packed as i16 as f32
        };
        packed / resolution * length
    })
}

pub(super) fn velocity_displacement(velocity: f32, accuracy: f32, age: f32) -> f32 {
    if age <= 0.0 || velocity == 0.0 {
        return 0.0;
    }
    if !age.is_finite() || !velocity.is_finite() || !accuracy.is_finite() {
        return f32::NAN;
    }
    if accuracy == 1.0 {
        return velocity * age;
    }
    // Client 0x1403f931e multiplies velocity before each 0.1-frame move.
    // The absolute-time preview interpolates between these fixed substeps;
    // it cannot reconstruct the client's per-input 10-frame cap or f32 clock.
    let substeps = age as f64 * 10.0;
    let ratio = accuracy as f64;
    let (power, sum) = geometric_power_sum(ratio, substeps.floor());
    let tail = if substeps.fract() == 0.0 {
        0.0
    } else {
        power * ratio * substeps.fract()
    };
    (velocity as f64 * (sum + tail) * 0.1) as f32
}

pub(super) fn bound_velocity_displacement(
    velocity: f32,
    accuracy: f32,
    age: f32,
    flattery_rate: f32,
    flattery_speed: f32,
) -> f32 {
    if age <= 0.0 || velocity == 0.0 {
        return 0.0;
    }
    if !age.is_finite()
        || !velocity.is_finite()
        || !accuracy.is_finite()
        || !flattery_rate.is_finite()
        || !flattery_speed.is_finite()
    {
        return f32::NAN;
    }
    if flattery_speed == 0.0 {
        return if flattery_rate > 0.0 {
            velocity_displacement(velocity, accuracy, age)
        } else {
            0.0
        };
    }

    let substeps = age as f64 * 10.0;
    let whole = substeps.floor();
    let threshold = -f64::from(flattery_rate) / f64::from(flattery_speed) * 10.0;
    let gate = |step: f64| flattery_rate + (step as f32 * 0.1) * flattery_speed > 0.0;
    // 0x1403f936a tests FltR + child_age * FltS after velocity has decayed.
    // For a linear gate, the contributing steps form one contiguous interval.
    let first = if flattery_speed > 0.0 {
        let candidate = (threshold.floor() + 1.0).max(1.0);
        if !gate(candidate) || candidate > 1.0 && gate(candidate - 1.0) {
            let mut lower = 0.0_f64;
            let mut upper = whole + 1.0;
            if !gate(upper) {
                return 0.0;
            }
            while upper - lower > 1.0 {
                let middle = ((lower + upper) * 0.5).floor();
                if middle <= lower || middle >= upper {
                    break;
                }
                if gate(middle) {
                    upper = middle;
                } else {
                    lower = middle;
                }
            }
            upper
        } else {
            candidate
        }
    } else {
        1.0
    };
    let last = if flattery_speed < 0.0 {
        if flattery_rate <= 0.0 {
            return 0.0;
        }
        let candidate = threshold.ceil() - 1.0;
        if !gate(candidate) || gate(candidate + 1.0) {
            let mut lower = 0.0_f64;
            let mut upper = whole + 1.0;
            if gate(upper) {
                upper
            } else {
                while upper - lower > 1.0 {
                    let middle = ((lower + upper) * 0.5).floor();
                    if middle <= lower || middle >= upper {
                        break;
                    }
                    if gate(middle) {
                        lower = middle;
                    } else {
                        upper = middle;
                    }
                }
                lower
            }
        } else {
            candidate
        }
    } else {
        f64::INFINITY
    };
    let count = (whole.min(last) - first + 1.0).max(0.0);
    let tail = if substeps.fract() > 0.0 && first <= whole + 1.0 && whole + 1.0 <= last {
        substeps.fract()
    } else {
        0.0
    };
    if count == 0.0 && tail == 0.0 {
        return 0.0;
    }
    if first == 1.0 && last >= whole + 1.0 {
        return velocity_displacement(velocity, accuracy, age);
    }
    if accuracy == 1.0 {
        return (f64::from(velocity) * (count + tail) * 0.1) as f32;
    }
    let ratio = f64::from(accuracy);
    let (prefix_power, _) = geometric_power_sum(ratio, first - 1.0);
    let (range_power, range_sum) = geometric_power_sum(ratio, count);
    (f64::from(velocity) * prefix_power * (range_sum + range_power * ratio * tail) * 0.1) as f32
}

fn geometric_power_sum(ratio: f64, mut count: f64) -> (f64, f64) {
    let (mut power, mut sum) = (1.0, 0.0);
    let (mut block_power, mut block_sum) = (ratio, ratio);
    // Compose geometric blocks without a division by (1 - ratio). This
    // retains negative/near-unit ratios and bounds work for immortal slots.
    while count >= 1.0 {
        if count % 2.0 != 0.0 {
            sum += power * block_sum;
            power *= block_power;
        }
        count = (count * 0.5).floor();
        if count == 0.0 {
            break;
        }
        block_sum += block_power * block_sum;
        block_power *= block_power;
    }
    (power, sum)
}

/// Persistent non-movement-triggered Smpl slots. One call is one client input
/// update; drawing must only read this state (0x1403f923f..0x1403f96b9).
pub(super) struct SpawnerState {
    pub slots: Vec<SlotState>,
    line_random: Option<SplitMix64>,
}

pub(super) struct SlotState {
    pub definition: SimpleSlot,
    pub phase: SlotPhase,
    pub age: f32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub birth_position: [f32; 3],
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SlotPhase {
    Waiting,
    Active,
    Dead,
}

impl SpawnerState {
    pub fn new(definitions: impl Iterator<Item = SimpleSlot>) -> Self {
        Self {
            line_random: None,
            slots: definitions
                .map(|definition| SlotState {
                    definition,
                    phase: SlotPhase::Waiting,
                    age: 0.0,
                    position: [0.0; 3],
                    velocity: [0.0; 3],
                    birth_position: [0.0; 3],
                    generation: 0,
                })
                .collect(),
        }
    }

    pub fn new_line(definitions: impl Iterator<Item = SimpleSlot>, seed: u64) -> Self {
        let mut state = Self::new(definitions);
        state.line_random = Some(SplitMix64::seeded(seed, 0, 0, 0x4c49_4645));
        state
    }

    pub fn advance(
        &mut self,
        simple: &AvfxParticleSimple,
        frames: f32,
        birth_allowed: bool,
        mut birth: impl FnMut(usize, &SimpleSlot) -> ([f32; 3], [f32; 3]),
    ) {
        // Deliberately preserve the client's f32 subtraction and positive
        // remainder test; ceil(delta * 10) differs near substep boundaries.
        let mut remaining = frames.min(10.0);
        while remaining > 0.0 {
            for (index, slot) in self.slots.iter_mut().enumerate() {
                slot.age += 0.1;
                match slot.phase {
                    SlotPhase::Waiting => {
                        if slot.age >= slot.definition.first_birth {
                            slot.activate(birth_allowed, |definition| birth(index, definition));
                        }
                    }
                    SlotPhase::Active => {
                        for axis in 0..3 {
                            slot.velocity[axis] *= simple.coord_accuracy[axis];
                        }
                        if simple.injection_vertex_bind_model_index as i8 == -1
                            || simple.velocity_flattery_rate
                                + slot.age * simple.velocity_flattery_speed
                                > 0.0
                        {
                            for axis in 0..3 {
                                slot.position[axis] += slot.velocity[axis] * 0.1;
                            }
                        }
                        let life = simple.create_interval_life as i16;
                        // Line draws a fresh CrLR offset on every active substep.
                        // Powder retains the slot's initialization offset.
                        let life_offset = if life > 0 {
                            self.line_random
                                .as_mut()
                                .map_or(slot.definition.life_offset, |rng| {
                                    let range = simple.create_life_random as i16 as i32;
                                    random_integer(rng, -range, range)
                                })
                        } else {
                            0
                        };
                        if life > 0 && slot.age >= f32::from(life) + life_offset as f32 {
                            slot.retire(
                                simple.create_new_after_delete,
                                birth_allowed,
                                |definition| birth(index, definition),
                            );
                        }
                        // Client checks UV retirement after the lifetime
                        // branch, including the age reset by a successful birth.
                        let loops = simple.uv_loop_count as i8;
                        if self.line_random.is_none() && loops > 0 {
                            let step = truncate_i32(slot.age / (simple.uv_interval as i8 as f32))
                                as u16 as u32;
                            let cells = uv_cells(simple);
                            if step + slot.definition.uv_start >= cells[0] * cells[1] * loops as u32
                            {
                                slot.retire(
                                    simple.create_new_after_delete,
                                    birth_allowed,
                                    |definition| birth(index, definition),
                                );
                            }
                        }
                    }
                    SlotPhase::Dead => {}
                }
            }
            remaining -= 0.1;
        }
    }
}

impl SlotState {
    fn activate(&mut self, allowed: bool, birth: impl FnOnce(&SimpleSlot) -> ([f32; 3], [f32; 3])) {
        if allowed {
            let (position, velocity) = birth(&self.definition);
            self.phase = SlotPhase::Active;
            self.age = 0.0;
            self.position = position;
            self.birth_position = position;
            self.generation = self.generation.wrapping_add(1);
            self.velocity = velocity;
        } else {
            self.phase = SlotPhase::Waiting;
        }
    }

    fn retire(
        &mut self,
        recreate: bool,
        allowed: bool,
        birth: impl FnOnce(&SimpleSlot) -> ([f32; 3], [f32; 3]),
    ) {
        if recreate {
            self.activate(allowed, birth);
        } else {
            self.phase = SlotPhase::Dead;
        }
    }
}

pub(super) struct SimpleSlot {
    first_birth: f32,
    life: f32,
    pub life_offset: i32,
    pub birth_offset: [f32; 3],
    pub binding_offset: [f32; 3],
    pub velocity: [f32; 3],
    uv_start: u32,
    uv_flip: bool,
    pub scale: [i8; 2],
    rotation_start: [i16; 3],
    rotation_velocity: [i16; 3],
}

pub(super) fn block_first_slot(simple: &AvfxParticleSimple, slot: u64) -> u64 {
    let count = (simple.block_num as i8).max(1) as u64;
    slot - slot % count
}

pub(super) fn birth_model_ordinal(simple: &AvfxParticleSimple, slot: u64) -> u64 {
    // IJMN's cursor advances only after the head (0x1403e4679); copied
    // siblings jump past that increment to 0x1403e4688.
    slot / (simple.block_num as i8).max(1) as u64
}

impl SimpleSlot {
    /// Line's 0x1403ebc30 update retires by CrIL only; unlike Powder it
    /// has no UvLC/UvIv retirement branch. Keep the shared birth sampling.
    pub fn new_line(simple: &AvfxParticleSimple, seed: u64, parent: u64, slot: u64) -> Self {
        let mut value = Self::new(simple, seed, parent, slot);
        let nominal_life = simple.create_interval_life as i16 as i32;
        value.life = if nominal_life > 0 {
            ((nominal_life + value.life_offset) as f32).max(0.1)
        } else {
            f32::INFINITY
        };
        value
    }

    pub fn new(simple: &AvfxParticleSimple, seed: u64, parent: u64, slot: u64) -> Self {
        Self::new_with_direction(simple, seed, parent, slot, None)
    }

    pub fn new_with_direction(
        simple: &AvfxParticleSimple,
        seed: u64,
        parent: u64,
        slot: u64,
        // The caller selects the block head's model normal, not this slot's.
        model_normal: Option<[f32; 3]>,
    ) -> Self {
        let mut rng = SplitMix64::seeded(seed, parent, slot, 0x7050_5354);
        let amplitude = simple.create_life_random as i16 as i32;
        let life_offset = random_integer(&mut rng, -amplitude, amplitude);
        let cells = uv_cells(simple);
        let uv_start = (random_integer(&mut rng, 0, simple.uv_no_random as i8 as i32) as u32
            % (cells[0] * cells[1]))
            & 0x7fff;

        // Block siblings share their initial delay and U-flip bit. UV start
        // and lifetime offsets remain per-slot (client 0x1403e51c0).
        let block_size = (simple.block_num as i8).max(1) as u64;
        let block = slot / block_size;
        let mut group_rng = SplitMix64::seeded(seed, parent, block, 0x7050_544d);
        let event = block / (simple.create_interval_count as i16).max(1) as u64;
        let delay = (event as i16 as i32 * simple.create_interval as i8 as i32
            + random_integer(
                &mut group_rng,
                0,
                simple.create_interval_random as i8 as i32,
            )) as i16;
        let uv_flip = simple.uv_reverse && random_integer(&mut group_rng, 0, 1) != 0;
        let scale_x = truncate_i32(random_range(&mut rng, simple.scale_rand_x) * 50.0) as i8;
        let scale_y = if simple.scale_random_link {
            scale_x
        } else {
            truncate_i32(random_range(&mut rng, simple.scale_rand_y) * 50.0) as i8
        };
        let mut rotation_start = std::array::from_fn(|axis| {
            pack_angle(
                simple.rotation_start[axis]
                    + random_range(
                        &mut group_rng,
                        [-simple.rotation_base[axis], simple.rotation_base[axis]],
                    ),
            )
        });
        let rotation_velocity = std::array::from_fn(|axis| {
            pack_angle(
                simple.rotation_add[axis]
                    + random_range(
                        &mut group_rng,
                        [
                            -simple.rotation_velocity[axis],
                            simple.rotation_velocity[axis],
                        ],
                    ),
            )
        });
        // 0x1403e5229 uses -1024, then subtracts the truncated block phase.
        rotation_start[2] = rotation_start[2].wrapping_sub(truncate_i32(
            (slot % block_size) as f32 / block_size as f32 * -1024.0,
        ) as i16);
        // Both SIPT=0 (0x1403e4b3b) and model prefill (0x1403e4453)
        // copy the head's packed position, including its birth scatter.
        let source_slot = block_first_slot(simple, slot);
        let mut motion_rng = SplitMix64::seeded(seed, parent, source_slot, 0x7050_5752);
        let birth_offset = if simple.injection_position_type as i8 == 2 {
            [0.0; 3]
        } else {
            std::array::from_fn(|axis| {
                (motion_rng.next_f32() * 2.0 - 1.0) * simple.create_area[axis]
            })
        };
        // VBMN draws scatter independently of IJMN, then copies the head
        // within each block (0x1403e4762). Local RNG differs from client TLS.
        let binding_offset = if (simple.injection_vertex_bind_model_index as i8) >= 0
            && simple.injection_position_type as i8 != 2
        {
            let mut binding_rng = SplitMix64::seeded(seed, parent, source_slot, 0x7050_424e);
            std::array::from_fn(|axis| {
                (binding_rng.next_f32() * 2.0 - 1.0) * simple.create_area[axis]
            })
        } else {
            [0.0; 3]
        };
        // 0x1403e51ef..3e520b copies packed velocity and magnitude from the
        // block head. The motion RNG already uses that head; UV and lifetime
        // offsets keep their separate per-slot inputs.
        let direction = match simple.injection_direction_type as i8 {
            // 0x1403e4aee invokes 0x140404880 before the direction switch:
            // IRD is a polar band around +Y, sampled uniformly in cosine.
            1 => {
                let [start, end] = simple.injection_radial_dir.map(f32::cos);
                let y = start - (start - end) * motion_rng.next_f32();
                polar_direction(y, motion_rng.next_f32())
            }
            2 => [1.0, 0.0, 0.0],
            3 => [0.0, 1.0, 0.0],
            4 => [0.0, 0.0, 1.0],
            5 if model_normal.is_some() => normalize_injection_direction(model_normal.unwrap()),
            _ => {
                // SIDT=0 samples the whole sphere and never consumes IRD.
                let y = 1.0 - 2.0 * motion_rng.next_f32();
                polar_direction(y, motion_rng.next_f32())
            }
        };
        let speed = simple.velocity_min
            + (simple.velocity_max - simple.velocity_min) * motion_rng.next_f32();
        // 0x1403e5660 stores magnitude separately and truncates normalized
        // velocity * 100 into signed bytes. 0x1403fbc78 / 0x1403fbe2b
        // unpack them before any birth-matrix transform, without renormalizing.
        let velocity = packed_position(direction.map(|component| component * speed), true);
        let nominal_life = simple.create_interval_life as i16 as i32;
        let mut life = if nominal_life > 0 {
            (nominal_life + life_offset) as f32
        } else {
            f32::INFINITY
        };
        let loops = simple.uv_loop_count as i8 as i32;
        let interval = simple.uv_interval as i8 as i32;
        if loops > 0 && interval != 0 {
            let steps = cells[0] * cells[1] * loops as u32;
            // 0x1403f94ef gates expiry on UvLC, not UvIv. The signed
            // quotient is truncated to u16: a negative interval reaches
            // 65535 on its first whole tick, even though drawing holds UVs.
            if steps.saturating_sub(uv_start) <= u16::MAX as u32 {
                let uv_life = if interval < 0 {
                    -interval as f32
                } else {
                    (steps as f32 - uv_start as f32) * interval as f32
                };
                life = life.min(uv_life);
            }
        }
        Self {
            first_birth: delay.max(0) as f32,
            life: life.max(0.1),
            life_offset,
            birth_offset,
            binding_offset,
            velocity,
            uv_start,
            uv_flip,
            scale: [scale_x, scale_y],
            rotation_start,
            rotation_velocity,
        }
    }

    pub fn sample(&self, age: f32, recreate: bool) -> Option<(f32, f32)> {
        let elapsed = age - self.first_birth;
        if elapsed < 0.0 || (!recreate && elapsed >= self.life) {
            return None;
        }
        // Absolute-time preview: lifecycle thresholds are continuous. Client
        // update rounding to 0.1 frames and movement-driven births are separate.
        let child_age = if recreate && self.life.is_finite() {
            ((elapsed as f64) % self.life as f64) as f32
        } else {
            elapsed
        };
        Some((age - child_age, child_age))
    }

    pub fn uv(&self, simple: &AvfxParticleSimple, age: f32) -> ([f32; 2], [f32; 2]) {
        let cells = uv_cells(simple);
        let count = cells[0] * cells[1];
        let interval = simple.uv_interval as i8;
        let loops = simple.uv_loop_count as i8;
        let cell = if interval > 0 {
            let step = (age / interval as f32).trunc() as f64 + self.uv_start as f64;
            if loops < 0 && step >= count as f64 * -(loops as f64) {
                count - 1
            } else {
                (step % count as f64) as u32
            }
        } else {
            self.uv_start
        };
        let stride = cells.map(|n| 32767 / n);
        let size = stride.map(|n| n as f32 / 32767.0);
        let origin = [
            (cell % cells[0] * stride[0]) as f32 / 32767.0 + 0.5 * (size[0] - 1.0),
            (cell / cells[0] * stride[1]) as f32 / 32767.0 + 0.5 * (size[1] - 1.0),
        ];
        (
            origin,
            [if self.uv_flip { -size[0] } else { size[0] }, size[1]],
        )
    }

    pub fn angles(&self, age: f32) -> [f32; 3] {
        std::array::from_fn(|axis| {
            let angle = (self.rotation_start[axis] as i32)
                .wrapping_add(truncate_i32(self.rotation_velocity[axis] as f32 * age));
            (angle & 1023) as f32 * std::f32::consts::TAU * (1.0 / 1024.0)
        })
    }
}

fn polar_direction(y: f32, azimuth_sample: f32) -> [f32; 3] {
    // Client 0x140404932..4049e2 stores Y, then X=sin(phi)*sin(acos(Y)),
    // Z=cos(phi)*sin(acos(Y)); 0x1403e4f42 subsequently normalizes it.
    let radius = y.acos().sin();
    let azimuth = azimuth_sample * std::f32::consts::TAU;
    normalize_injection_direction([radius * azimuth.sin(), y, radius * azimuth.cos()])
}

fn normalize_injection_direction(value: [f32; 3]) -> [f32; 3] {
    // 0x1403e4f65 compares the squared length with 0x3727c5ac (1e-5),
    // not its square root. Small finite vectors pass through unchanged.
    let squared = value[1] * value[1] + value[0] * value[0] + value[2] * value[2];
    if !squared.is_finite() {
        return [0.0; 3];
    }
    if squared > 1.0e-5 {
        let inverse_length = 1.0 / squared.sqrt();
        value.map(|component| component * inverse_length)
    } else {
        value
    }
}

fn uv_cells(simple: &AvfxParticleSimple) -> [u32; 2] {
    simple.uv_cell.map(|value| (value as i8).max(1) as u32)
}

fn random_integer(rng: &mut SplitMix64, min: i32, max: i32) -> i32 {
    // Match the inclusive integer range, with this runtime's deterministic RNG.
    min + ((rng.next_u64() as u16 as i64 * (max - min + 1) as i64) / 65536) as i32
}

fn random_range(rng: &mut SplitMix64, [min, max]: [f32; 2]) -> f32 {
    (max - min) * rng.next_u64() as u16 as f32 / 65535.0 + min
}

fn pack_angle(angle: f32) -> i16 {
    truncate_i32(angle * 1024.0 / std::f32::consts::TAU) as i16
}

fn truncate_i32(value: f32) -> i32 {
    // CVTTSS2SI returns INT_MIN for non-finite or out-of-range input.
    if (-2147483648.0..2147483648.0).contains(&value) {
        value as i32
    } else {
        i32::MIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_position_preserves_axis_points_and_skips_invalid_norms() {
        for bound in [false, true] {
            assert_eq!(packed_position([0.0; 3], bound), [0.0; 3]);
            assert_eq!(packed_position([-5.0, 0.0, 0.0], bound), [-5.0, 0.0, 0.0]);
            assert_eq!(packed_position([f32::NAN, 1.0, 0.0], bound), [0.0; 3]);
            assert_eq!(packed_position([f32::MAX, 0.0, 0.0], bound), [0.0; 3]);
        }
    }

    #[test]
    fn velocity_displacement_matches_substep_recurrence() {
        for ratio in [0.0, 0.5, 0.9999, 1.0, 1.0001, 1.25, -0.5, -1.0, -1.25] {
            for steps in [1, 2, 3, 10, 31, 100, 1000] {
                let age = steps as f32 / 10.0;
                let mut velocity = -2.0_f64;
                let mut expected = 0.0_f64;
                for _ in 0..steps {
                    velocity *= ratio as f64;
                    expected += velocity * 0.1;
                }
                let expected = expected as f32;
                let actual = velocity_displacement(-2.0, ratio, age);
                if expected.is_finite() {
                    assert!(
                        (actual - expected).abs() <= 2.0e-6 * expected.abs().max(1.0),
                        "ratio={ratio}, steps={steps}: {actual} != {expected}"
                    );
                } else {
                    assert!(!actual.is_finite());
                }
            }
        }
        assert_eq!(velocity_displacement(2.0, 0.5, 0.25), 0.1625);
        assert_eq!(velocity_displacement(2.0, -0.5, 0.25), -0.0625);
    }

    #[test]
    fn bound_motion_decays_every_step_but_moves_only_when_flattery_is_positive() {
        for accuracy in [0.5, 0.99, 1.0, 1.001, -0.5, -1.0] {
            for (rate, speed) in [
                (-0.5, 0.5),
                (0.3, -0.1),
                (-0.3, 0.1),
                (0.0, 0.0),
                (0.5, 0.0),
            ] {
                for steps in [1, 2, 10, 11, 20, 30, 31, 100] {
                    let mut velocity = 2.0_f64;
                    let mut expected = 0.0_f64;
                    for step in 1..=steps {
                        velocity *= f64::from(accuracy);
                        if rate + (step as f32 * 0.1) * speed > 0.0 {
                            expected += velocity * 0.1;
                        }
                    }
                    let actual = bound_velocity_displacement(
                        2.0,
                        accuracy,
                        steps as f32 / 10.0,
                        rate,
                        speed,
                    );
                    assert!(
                        (f64::from(actual) - expected).abs() <= 2.0e-5 * expected.abs().max(1.0),
                        "C={accuracy}, FltR={rate}, FltS={speed}, steps={steps}: {actual} != {expected}"
                    );
                }
            }
        }
        assert_eq!(bound_velocity_displacement(2.0, 0.5, 1.0e9, -1.0, 0.0), 0.0);
        assert_eq!(bound_velocity_displacement(0.0, 2.0, 1.0e9, -1.0, 1.0), 0.0);
    }

    #[test]
    fn velocity_displacement_bounds_work_without_clamping_coefficients() {
        for age in [1.0e9, f32::MAX] {
            assert_eq!(velocity_displacement(2.0, 0.5, age), 0.2);
            assert_eq!(velocity_displacement(3.0, -0.5, age), -0.1);
            assert_eq!(velocity_displacement(1.0, -1.0, age), 0.0);
            assert_eq!(velocity_displacement(0.0, 2.0, age), 0.0);
            assert_eq!(velocity_displacement(2.0, 0.0, age), 0.0);
            assert_eq!(velocity_displacement(1.0, 1.0, age), age);
            assert!(!velocity_displacement(1.0, 2.0, age).is_finite());
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(velocity_displacement(1.0, value, 1.0).is_nan());
        }
    }

    #[test]
    fn powder_motion_preview_cannot_reconstruct_a_capped_input_from_age() {
        // Client 0x1403f923f..0x1403f9285 caps each input at 10 frames;
        // 0x1403f92f0..0x1403f9688 then advances in f32 0.1-frame steps.
        let client_position = |inputs: &[f32]| {
            let (mut velocity, mut position) = (2.0_f32, 0.0_f32);
            let mut steps = 0;
            for &input in inputs {
                let mut remaining = input.min(10.0);
                while remaining > 0.0 {
                    velocity *= 0.99;
                    position += velocity * 0.1;
                    remaining -= 0.1;
                    steps += 1;
                }
            }
            (position, steps)
        };
        let (single, single_steps) = client_position(&[20.0]);
        let (split, split_steps) = client_position(&[10.0, 10.0]);
        let absolute = velocity_displacement(2.0, 0.99, 20.0);

        assert!((100..=101).contains(&single_steps));
        assert_eq!(split_steps, single_steps * 2);
        assert!(split - single > 4.0);
        assert!((absolute - single).abs() > 4.0);
        assert!((absolute - split).abs() < 0.1);
    }

    #[test]
    fn line_lifetime_is_redrawn_per_substep_and_ignores_uv_retirement() {
        let simple = AvfxParticleSimple {
            create_interval_life: 2,
            create_life_random: 1,
            create_new_after_delete: true,
            uv_loop_count: 1,
            uv_interval: 1,
            coord_accuracy: [1.0; 3],
            ..Default::default()
        };
        let run = || {
            let mut state =
                SpawnerState::new_line(std::iter::once(SimpleSlot::new_line(&simple, 7, 0, 0)), 13);
            let mut previous = 0;
            let mut births = Vec::new();
            for step in 0..400 {
                state.advance(&simple, 0.1, true, |_, _| ([0.0; 3], [1.0, 0.0, 0.0]));
                if state.slots[0].generation != previous {
                    births.push(step);
                    previous = state.slots[0].generation;
                }
            }
            births
        };
        let births = run();
        assert_eq!(births, run());
        assert!(births.len() > 10);
        let durations: std::collections::HashSet<_> =
            births.windows(2).map(|pair| pair[1] - pair[0]).collect();
        assert!(
            durations.len() > 1,
            "Line cannot retain one fixed lifetime offset"
        );
        assert!(
            durations.iter().any(|&duration| duration > 10),
            "UvLC must not force retirement at one frame"
        );
    }

    #[test]
    fn incremental_slots_wait_activate_move_reject_rebirth_and_remain_dead() {
        let simple = AvfxParticleSimple {
            create_interval_life: 1,
            coord_accuracy: [1.0; 3],
            injection_vertex_bind_model_index: -1,
            create_new_after_delete: true,
            ..Default::default()
        };
        let mut state = SpawnerState::new(std::iter::once(SimpleSlot::new(&simple, 1, 2, 0)));
        let birth = |_, _: &SimpleSlot| ([3.0, 0.0, 0.0], [2.0, 0.0, 0.0]);
        state.advance(&simple, 0.0, true, birth);
        assert_eq!(state.slots[0].phase, SlotPhase::Waiting);
        state.advance(&simple, 0.1, false, birth);
        assert_eq!(state.slots[0].age, 0.1);
        assert_eq!(state.slots[0].phase, SlotPhase::Waiting);
        state.advance(&simple, 0.1, true, birth);
        assert_eq!(state.slots[0].age, 0.0);
        assert_eq!(state.slots[0].position[0], 3.0);
        state.advance(&simple, 0.1, true, birth);
        assert_eq!(state.slots[0].position[0], 3.2);
        state.advance(&simple, 0.9, false, birth);
        assert_eq!(state.slots[0].phase, SlotPhase::Waiting);
        assert!(state.slots[0].age >= 1.0);
        state.advance(&simple, 0.1, true, birth);
        assert_eq!(state.slots[0].age, 0.0);
        assert_eq!(state.slots[0].position[0], 3.0);
        let mut final_life = simple.clone();
        final_life.create_new_after_delete = false;
        state.advance(&final_life, 1.0, true, birth);
        assert_eq!(state.slots[0].phase, SlotPhase::Dead);
        let position = state.slots[0].position;
        state.advance(&simple, 1.0, true, birth);
        assert_eq!(state.slots[0].phase, SlotPhase::Dead);
        assert_eq!(state.slots[0].position, position);
    }

    #[test]
    fn incremental_input_cap_and_substep_remainders_are_per_call() {
        let simple = AvfxParticleSimple {
            coord_accuracy: [1.0; 3],
            injection_vertex_bind_model_index: -1,
            ..Default::default()
        };
        let make = || SpawnerState::new(std::iter::once(SimpleSlot::new(&simple, 1, 2, 0)));
        let birth = |_, _: &SimpleSlot| ([0.0; 3], [1.0; 3]);
        let mut capped = make();
        let mut ten = make();
        capped.advance(&simple, 100.0, true, birth);
        ten.advance(&simple, 10.0, true, birth);
        assert_eq!(capped.slots[0].age, ten.slots[0].age);
        assert_eq!(capped.slots[0].position, ten.slots[0].position);
        let before = capped.slots[0].age;
        capped.advance(&simple, 10.0, true, birth);
        assert!(capped.slots[0].age > before + 9.9);
        let mut fractional = make();
        fractional.advance(&simple, 0.01, true, birth);
        assert_eq!(fractional.slots[0].age, 0.0);
        fractional.advance(&simple, 0.01, true, birth);
        assert_eq!(fractional.slots[0].age, 0.1);
        assert_eq!(fractional.slots[0].position, [0.1; 3]);
    }

    #[test]
    fn incremental_decay_continues_while_binding_gate_is_closed() {
        let simple = AvfxParticleSimple {
            coord_accuracy: [0.5; 3],
            injection_vertex_bind_model_index: 0,
            velocity_flattery_rate: -0.15,
            velocity_flattery_speed: 1.0,
            ..Default::default()
        };
        let mut state = SpawnerState::new(std::iter::once(SimpleSlot::new(&simple, 1, 2, 0)));
        let birth = |_, _: &SimpleSlot| ([0.0; 3], [4.0; 3]);
        state.advance(&simple, 0.1, true, birth);
        state.advance(&simple, 0.1, true, birth);
        assert_eq!(state.slots[0].position, [0.0; 3]);
        assert_eq!(state.slots[0].velocity, [2.0; 3]);
        state.advance(&simple, 0.1, true, birth);
        assert_eq!(state.slots[0].position, [0.1; 3]);
        assert_eq!(state.slots[0].velocity, [1.0; 3]);
    }

    #[test]
    fn incremental_lifetime_rebirth_resets_age_before_uv_retirement() {
        let simple = AvfxParticleSimple {
            create_interval_life: 1,
            create_new_after_delete: true,
            uv_cell: [1, 1],
            uv_interval: -1,
            uv_loop_count: 1,
            ..Default::default()
        };
        let mut state = SpawnerState::new(std::iter::once(SimpleSlot::new(&simple, 1, 2, 0)));
        let mut births = 0;
        let mut birth = |_, _: &SimpleSlot| {
            births += 1;
            ([0.0; 3], [0.0; 3])
        };
        state.advance(&simple, 0.1, true, &mut birth);
        state.advance(&simple, 1.0, true, &mut birth);
        assert_eq!(state.slots[0].age, 0.0);
        assert_eq!(state.slots[0].phase, SlotPhase::Active);
        assert_eq!(births, 2); // no third birth from checking the expired old age
    }

    #[test]
    fn positive_uv_loops_retire_negative_intervals_at_first_wrapped_tick() {
        for raw_interval in [-1, -3, -128, 255, 128] {
            let simple = AvfxParticleSimple {
                uv_cell: [2, 2],
                uv_interval: raw_interval,
                uv_loop_count: 1,
                ..Default::default()
            };
            let slot = SimpleSlot::new(&simple, 1, 2, 0);
            let boundary = -(raw_interval as i8 as f32);
            assert!(slot.sample(boundary - 0.1, false).is_some());
            assert!(slot.sample(boundary, false).is_none());
            assert_eq!(slot.sample(boundary, true), Some((boundary, 0.0)));
            // Drawing does not advance UVs for nonpositive UvIv, although
            // the separate lifetime branch does perform the signed division.
            assert_eq!(slot.uv(&simple, 0.0), slot.uv(&simple, boundary - 0.1));
        }
        let simple = AvfxParticleSimple {
            uv_cell: [127, 127],
            uv_interval: -1,
            uv_loop_count: 127,
            ..Default::default()
        };
        let slot = SimpleSlot::new(&simple, 1, 2, 0);
        // Even the maximum wrapped quotient cannot reach this loop count.
        assert!(slot.sample(1.0e6, false).is_some());
    }

    #[test]
    fn uv_start_shortens_positive_loop_lifetime_and_survives_rebirth() {
        let simple = AvfxParticleSimple {
            uv_cell: [4, 1],
            uv_interval: 2,
            uv_loop_count: 1,
            uv_no_random: 3,
            create_interval_life: 100,
            ..Default::default()
        };
        let mut starts = [false; 4];
        for index in 0..64 {
            let slot = SimpleSlot::new(&simple, 123, 456, index);
            starts[slot.uv_start as usize] = true;
            let end = (4 - slot.uv_start) as f32 * 2.0;
            assert!(slot.sample(end - 0.25, false).is_some());
            assert!(slot.sample(end, false).is_none());
            assert_eq!(slot.sample(end + 0.25, true), Some((end, 0.25)));
            assert_eq!(slot.sample(end * 100_000.0 + 0.25, true).unwrap().1, 0.25);
        }
        assert_eq!(starts, [true; 4]);
    }

    #[test]
    fn block_birth_and_binding_scatter_copy_the_head_independently() {
        for position_type in [0, 1, 2] {
            for binding in [-1, 0] {
                let simple = AvfxParticleSimple {
                    block_num: 4,
                    injection_position_type: position_type,
                    injection_vertex_bind_model_index: binding,
                    create_area: [1.0, 2.0, 3.0],
                    ..Default::default()
                };
                let head = SimpleSlot::new(&simple, 123, 456, 0);
                let sibling = SimpleSlot::new(&simple, 123, 456, 1);
                assert_eq!(sibling.birth_offset, head.birth_offset);
                if position_type == 2 {
                    assert_eq!(head.birth_offset, [0.0; 3]);
                } else if binding >= 0 {
                    assert_eq!(sibling.binding_offset, head.binding_offset);
                    assert_ne!(head.birth_offset, head.binding_offset);
                }
            }
        }
    }

    #[test]
    fn block_siblings_copy_the_first_slots_packed_velocity() {
        for direction in [0, 1, 2, 3, 4] {
            for radial in [[0.0; 2], [0.3, 1.2]] {
                let simple = AvfxParticleSimple {
                    block_num: 4,
                    injection_direction_type: direction,
                    injection_radial_dir: radial,
                    create_area: [1.0, 2.0, 3.0],
                    velocity_min: 1.0,
                    velocity_max: 4.0,
                    ..Default::default()
                };
                let mut heads = Vec::new();
                for block in 0..4 {
                    let head = SimpleSlot::new(&simple, 123, 456, block * 4);
                    heads.push(head.velocity);
                    for index in 1..4 {
                        let sibling = SimpleSlot::new(&simple, 123, 456, block * 4 + index);
                        assert_eq!(
                            sibling.velocity, head.velocity,
                            "SIDT={direction}, radial={radial:?}, block={block}, sibling={index}"
                        );
                    }
                }
                assert!(heads.windows(2).any(|pair| pair[0] != pair[1]));
            }
        }
    }

    #[test]
    fn blocks_share_delay_and_flip_but_keep_individual_uv_starts() {
        let simple = AvfxParticleSimple {
            block_num: 4,
            create_interval_count: 2,
            create_interval: 10,
            create_interval_random: 2,
            create_interval_life: 100,
            uv_cell: [4, 1],
            uv_no_random: 3,
            uv_reverse: true,
            ..Default::default()
        };
        let mut starts_differ = false;
        for block in 0..16 {
            let first = SimpleSlot::new(&simple, 123, 456, block * 4);
            let base = (block / 2 * 10) as f32;
            assert!((base..=base + 2.0).contains(&first.first_birth));
            assert_eq!(first.first_birth.fract(), 0.0);
            for sibling in 1..4 {
                let slot = SimpleSlot::new(&simple, 123, 456, block * 4 + sibling);
                assert_eq!(slot.first_birth, first.first_birth);
                assert_eq!(slot.uv_flip, first.uv_flip);
                starts_differ |= slot.uv_start != first.uv_start;
            }
        }
        assert!(starts_differ);
    }

    #[test]
    fn fixed_sidts_use_client_axis_directions() {
        for (sidt, expected) in [
            (2, [2.0, 0.0, 0.0]),
            (3, [0.0, 2.0, 0.0]),
            (4, [0.0, 0.0, 2.0]),
        ] {
            let simple = AvfxParticleSimple {
                injection_direction_type: sidt,
                velocity_min: 2.0,
                velocity_max: 2.0,
                ..Default::default()
            };
            let slot = SimpleSlot::new(&simple, 123, 456, 0);
            assert_eq!(slot.velocity, expected);
        }
    }

    #[test]
    fn sidt_one_zero_angles_emit_up() {
        let simple = AvfxParticleSimple {
            injection_direction_type: 1,
            velocity_min: 2.0,
            velocity_max: 2.0,
            ..Default::default()
        };
        let slot = SimpleSlot::new(&simple, 123, 456, 0);
        assert_eq!(slot.velocity, [0.0, 2.0, 0.0]);
    }

    #[test]
    fn sidt_zero_ignores_ird_and_sidt_one_uses_cosine_band() {
        let base = AvfxParticleSimple {
            injection_direction_type: 0,
            velocity_min: 2.0,
            velocity_max: 2.0,
            ..Default::default()
        };
        let mut variant = base.clone();
        variant.injection_radial_dir = [0.3, 1.2];
        for slot in 0..32 {
            assert_eq!(
                SimpleSlot::new(&base, 123, 456, slot).velocity,
                SimpleSlot::new(&variant, 123, 456, slot).velocity
            );
        }
        variant.injection_direction_type = 1;
        variant.injection_radial_dir = [0.0, std::f32::consts::FRAC_PI_3];
        let mut min_y = 2.0_f32;
        let mut max_y = 0.0_f32;
        let mut sum_y = 0.0;
        for slot in 0..256 {
            let velocity = SimpleSlot::new(&variant, 123, 456, slot).velocity;
            assert!((0.98..=2.0).contains(&velocity[1]), "{velocity:?}");
            min_y = min_y.min(velocity[1]);
            max_y = max_y.max(velocity[1]);
            sum_y += velocity[1];
        }
        assert!(min_y < 1.1 && max_y > 1.9);
        // Cosine-uniform Y averages 1.5 before quantization; uniform angle
        // would average about 1.65, and sampling around scatter is unrelated.
        assert!((sum_y / 256.0 - 1.49).abs() < 0.06);
        variant.injection_radial_dir = [std::f32::consts::PI; 2];
        assert_eq!(
            SimpleSlot::new(&variant, 123, 456, 0).velocity,
            [0.0, -2.0, 0.0]
        );
    }

    #[test]
    fn velocity_roundtrips_signed_byte_direction_before_birth_transform() {
        for speed in [2.0, -2.0, 0.0] {
            let simple = AvfxParticleSimple {
                injection_direction_type: 5,
                velocity_min: speed,
                velocity_max: speed,
                ..Default::default()
            };
            let slot = SimpleSlot::new_with_direction(&simple, 123, 456, 0, Some([1.0, 2.0, -3.0]));
            // Unit direction * 100 truncates to [26, 53, -80]. The
            // magnitude remains separate; do not renormalize the bytes.
            let expected = [0.26 * speed, 0.53 * speed, -0.80 * speed];
            for axis in 0..3 {
                assert!(
                    (slot.velocity[axis] - expected[axis]).abs() < 1e-6,
                    "speed={speed}: {:?} != {expected:?}",
                    slot.velocity
                );
            }
        }
        let simple = AvfxParticleSimple {
            injection_direction_type: 5,
            velocity_min: 2.0,
            velocity_max: 2.0,
            ..Default::default()
        };
        let slot = SimpleSlot::new_with_direction(&simple, 123, 456, 0, Some([0.001, -0.001, 1.0]));
        assert_eq!(slot.velocity[0], 0.0);
        assert_eq!(slot.velocity[1], 0.0);
        assert!((slot.velocity[2] - 1.98).abs() < 1e-6);
    }

    #[test]
    fn model_normal_below_client_squared_threshold_keeps_its_magnitude() {
        let simple = AvfxParticleSimple {
            injection_direction_type: 5,
            velocity_min: 2.0,
            velocity_max: 2.0,
            ..Default::default()
        };
        for component in [0.0_f32, 1.0e-9, -1.0e-9, 0.001, -0.003, 0.004, -0.004] {
            let slot =
                SimpleSlot::new_with_direction(&simple, 123, 456, 0, Some([component, 0.0, 0.0]));
            let expected = if component.abs() < 0.004 {
                component * 2.0
            } else {
                component.signum() * 2.0
            };
            assert!(
                (slot.velocity[0] - expected).abs() < 1.0e-6 * expected.abs().max(1.0e-6),
                "normal={component}: {:?}, expected {expected}",
                slot.velocity
            );
            assert_eq!(&slot.velocity[1..], &[0.0, 0.0]);
        }
    }

    #[test]
    fn sidt_five_uses_the_selected_model_normal() {
        let simple = AvfxParticleSimple {
            injection_direction_type: 5,
            velocity_min: 2.0,
            velocity_max: 2.0,
            ..Default::default()
        };
        let slot = SimpleSlot::new_with_direction(&simple, 123, 456, 0, Some([0.0, 3.0, 4.0]));
        assert_eq!(slot.velocity, [0.0, 1.2, 1.6]);
    }
}
