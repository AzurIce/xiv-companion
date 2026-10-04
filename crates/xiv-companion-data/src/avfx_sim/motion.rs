use super::{
    EmitterBase, InstanceClock, SpawnContext, VFX_IDENTITY_BASIS, basis_mul, basis_transform,
    coordinate_mode, coordinate_translation, cross, curve_value_seeded_at, integration,
    particle_coordinate_orientation, particle_coordinate_scale,
};
use crate::avfx::{AvfxCurve, AvfxEmitter, AvfxEmitterItem, AvfxParticle};

#[derive(Clone, Copy)]
pub(super) enum InjectionDirection {
    World([f32; 3]),
    // Point forwards the parent's stored +174 vector without a PICd conversion.
    Inherited([f32; 3]),
}

#[derive(Clone, Copy)]
pub(super) struct InjectionMotion {
    origin: [f32; 3],
    velocity: [f32; 3],
    pub(super) direction: [f32; 3],
    direction_basis: [[f32; 3]; 3],
    local_direction: bool,
    parent_coord: i32,
    auxiliary_coordinates: bool,
    follow_position: bool,
}

impl InjectionMotion {
    pub(super) fn new(
        item: &AvfxEmitterItem,
        parent: EmitterBase,
        origin: [f32; 3],
        direction: InjectionDirection,
        velocity: [f32; 3],
    ) -> Self {
        let parent_coord = coordinate_mode(item.parent_influence_coord);
        let (origin, direction, velocity, up, fallback_up) = if matches!(parent_coord, 2 | 3 | 8) {
            let inverse = inverse_basis(parent.linear);
            (
                basis_transform(
                    inverse,
                    std::array::from_fn(|axis| origin[axis] - parent.position[axis]),
                ),
                match direction {
                    InjectionDirection::World(value) => basis_transform(inverse, value),
                    InjectionDirection::Inherited(value) => value,
                },
                basis_transform(inverse, velocity),
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            )
        } else {
            (
                origin,
                match direction {
                    InjectionDirection::World(value) | InjectionDirection::Inherited(value) => {
                        value
                    }
                },
                velocity,
                parent.linear[1],
                parent.linear[2],
            )
        };
        let direction_basis = if matches!(parent_coord, 0 | 1 | 2 | 3 | 8) {
            injection_basis(direction, up, fallback_up)
        } else {
            VFX_IDENTITY_BASIS
        };
        let local_direction = item.local_direction == 1;
        Self {
            origin,
            direction,
            velocity: if local_direction {
                transpose_transform(direction_basis, velocity)
            } else {
                velocity
            },
            direction_basis,
            local_direction,
            parent_coord,
            auxiliary_coordinates: !matches!(item.parent_influence_coord, 2 | 3 | 8),
            follow_position: parent_coord == 1 && item.influence_coord_pos,
        }
    }

    fn has_matrix_coordinates(self) -> bool {
        matches!(self.parent_coord, 0 | 1 | 2 | 3 | 8)
    }

    pub(super) fn has_optional_components(self) -> bool {
        self.parent_coord == 1
    }

    pub(super) fn has_auxiliary_coordinates(self) -> bool {
        self.auxiliary_coordinates
    }

    pub(super) fn parent(self, birth: EmitterBase, now: EmitterBase) -> EmitterBase {
        match self.parent_coord {
            2 | 8 => now,
            3 => birth,
            _ => EmitterBase::root([0.0; 3]),
        }
    }

    pub(super) fn drawing_parent(self, parent: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
        let parent = if matches!(self.parent_coord, 0 | 1) {
            VFX_IDENTITY_BASIS
        } else {
            parent
        };
        if self.has_matrix_coordinates() && self.local_direction {
            basis_mul(parent, self.direction_basis)
        } else {
            parent
        }
    }

    pub(super) fn drawing_parent_with_auxiliary(
        self,
        parent: [[f32; 3]; 3],
        auxiliary: [[f32; 3]; 3],
    ) -> [[f32; 3]; 3] {
        if !self.has_auxiliary_coordinates() {
            return self.drawing_parent(parent);
        }
        let local = basis_mul(auxiliary, self.drawing_parent(VFX_IDENTITY_BASIS));
        if matches!(self.parent_coord, 0 | 1) {
            local
        } else {
            // Raw ground modes 4/5/9 enable the auxiliary getter before
            // their normalized PICd parent matrix, not after it.
            basis_mul(parent, local)
        }
    }

    /// Instance +0x174 is consumed as a world-space direction by RBDT 3/7.
    /// Matrix-coordinate PICd modes store it in the selected parent's space.
    pub(super) fn drawing_direction(self, parent: [[f32; 3]; 3]) -> [f32; 3] {
        if matches!(self.parent_coord, 2 | 3 | 8) {
            basis_transform(parent, self.direction)
        } else {
            self.direction
        }
    }

    /// Convert the stored parent velocity back to world space for `IPbV`.
    /// The stored value is in the same PICd/LoDr space used by the instance
    /// constructor; using the drawing basis mirrors the client's final basis
    /// conversion without applying translation or scale a second time.
    pub(super) fn world_velocity(self, parent: [[f32; 3]; 3]) -> [f32; 3] {
        basis_transform(self.drawing_parent(parent), self.velocity)
    }

    pub(super) fn client_input(self) -> super::particle_curves::VfxClientParticleInjectionInput {
        super::particle_curves::VfxClientParticleInjectionInput {
            velocity: self.velocity,
            basis: self.direction_basis,
            local_direction: self.local_direction,
        }
    }

    pub(super) fn stored_velocity(self) -> [f32; 3] {
        self.velocity
    }

    pub(super) fn position(
        self,
        ctx: &SpawnContext,
        particle: &AvfxParticle,
        curve_age: f32,
        local: [f32; 3],
        parent: [[f32; 3]; 3],
    ) -> [f32; 3] {
        let mut displacement = ctx.client_injection.unwrap_or_else(|| {
            self.displacement_with_clock(particle, ctx.age, ctx.clock, ctx.seed)
        });
        if self.has_matrix_coordinates() {
            // Client +0xa4 contains Pos + accumulated motion. CCOT precedes
            // LoDr; the creation point and PICd parent matrix are added later.
            displacement = std::array::from_fn(|axis| local[axis] + displacement[axis]);
            if matches!(particle.coord_compute_order, 1..=4) {
                displacement = coordinate_translation(
                    particle.coord_compute_order,
                    displacement,
                    particle_coordinate_orientation(ctx, particle, curve_age),
                    particle_coordinate_scale(ctx, particle, curve_age),
                );
            }
        }
        let mut position = self.translate(
            displacement,
            ctx.emitter.world(),
            ctx.emitter_now.world(),
            ctx.bound_position(),
            ctx.binder_base,
        );
        if !self.has_matrix_coordinates() {
            // Unknown coordinate modes retain the preview's fallback.
            let local = basis_transform(parent, local);
            for axis in 0..3 {
                position[axis] += local[axis];
            }
        }
        position
    }

    /// CCOT has already combined local Pos and motion. LoDr, the birth point,
    /// and the selected parent matrix apply in this order to both instance kinds.
    pub(super) fn translate(
        self,
        mut displacement: [f32; 3],
        birth: EmitterBase,
        now: EmitterBase,
        bound: Option<[f32; 3]>,
        binder: EmitterBase,
    ) -> [f32; 3] {
        if self.local_direction {
            displacement = basis_transform(self.direction_basis, displacement);
        }
        if self.has_auxiliary_coordinates() {
            displacement = binder.auxiliary_matrix.transform_point(displacement);
        }
        let origin = if self.follow_position {
            bound.unwrap_or(self.origin)
        } else {
            self.origin
        };
        let point = std::array::from_fn(|axis| origin[axis] + displacement[axis]);
        match self.parent_coord {
            // PICd=8 still approximates the client's unstickiness with full follow.
            2 | 8 => now.transform_point(point),
            3 => birth.transform_point(point),
            _ => point,
        }
    }

    pub(super) fn emitter_displacement(
        self,
        emitter: &AvfxEmitter,
        frame: f32,
        clock: InstanceClock,
        seed: u64,
    ) -> [f32; 3] {
        let direction_zero = emitter
            .rotation_velocity
            .iter()
            .chain(&emitter.rotation_velocity_random)
            .all(|curve| curve.keys.iter().all(|key| key.z == 0.0));
        if direction_zero {
            return self.straight_displacement(
                &emitter.air_resistance,
                &emitter.air_resistance_random,
                clock.motion_range(frame),
                clock,
                seed,
            );
        }
        let speed = length(self.velocity);
        if speed == 0.0 {
            return [0.0; 3];
        }
        let range = clock.motion_range(frame);
        let resistance_neutral =
            neutral(&emitter.air_resistance, 1.0) && neutral(&emitter.air_resistance_random, 0.0);
        let at = |ages| {
            let angles = std::array::from_fn(|axis| {
                curve_value_seeded_at(
                    &emitter.rotation_velocity[axis],
                    &emitter.rotation_velocity_random[axis],
                    ages,
                    0.0,
                    seed ^ (0x7010 + axis as u64),
                )
            });
            let velocity = if angles == [0.0; 3] {
                self.velocity
            } else {
                let velocity = basis_transform(self.direction_basis, direction_angles(angles))
                    .map(|value| value * speed);
                if self.local_direction {
                    transpose_transform(self.direction_basis, velocity)
                } else {
                    velocity
                }
            };
            let resistance = if resistance_neutral {
                1.0
            } else {
                curve_value_seeded_at(
                    &emitter.air_resistance,
                    &emitter.air_resistance_random,
                    ages,
                    0.0,
                    seed ^ 0xA125,
                )
            };
            velocity.map(|value| f64::from(value) * f64::from(resistance))
        };
        let curves = [
            (&emitter.rotation_velocity[0], false),
            (&emitter.rotation_velocity[1], false),
            (&emitter.rotation_velocity[2], false),
            (&emitter.rotation_velocity_random[0], true),
            (&emitter.rotation_velocity_random[1], true),
            (&emitter.rotation_velocity_random[2], true),
            (&emitter.air_resistance, false),
            (&emitter.air_resistance_random, true),
        ];
        integration::looped_vector_range(
            range,
            f64::from(clock.loop_start),
            f64::from(clock.loop_end),
            &curves,
            &at,
        )
        .map(|value| value as f32)
    }

    fn straight_displacement(
        self,
        resistance: &AvfxCurve,
        random: &AvfxCurve,
        range: [f32; 2],
        clock: InstanceClock,
        seed: u64,
    ) -> [f32; 3] {
        let travel = if neutral(resistance, 1.0) && neutral(random, 0.0) {
            f64::from(range[1]) - f64::from(range[0])
        } else {
            integration::looped_curve_range(
                resistance,
                random,
                range,
                f64::from(clock.loop_start),
                f64::from(clock.loop_end),
                seed ^ 0xA125,
            )
            .value
        };
        self.velocity
            .map(|value| (f64::from(value) * travel) as f32)
    }

    #[cfg(test)]
    fn displacement(self, particle: &AvfxParticle, age: f32, seed: u64) -> [f32; 3] {
        self.displacement_with_clock(
            particle,
            age,
            InstanceClock::new(-1.0, 1.0, particle.loop_start, particle.loop_end),
            seed,
        )
    }

    fn displacement_with_clock(
        self,
        particle: &AvfxParticle,
        frame: f32,
        clock: InstanceClock,
        seed: u64,
    ) -> [f32; 3] {
        let range = clock.motion_range(frame);
        let resistance_neutral =
            neutral(&particle.air_resistance, 1.0) && neutral(&particle.air_resistance_random, 0.0);
        let direction_zero = particle
            .rotation_velocity
            .iter()
            .chain(&particle.rotation_velocity_random)
            .all(|curve| curve.keys.iter().all(|key| key.z == 0.0));
        if direction_zero {
            return self.straight_displacement(
                &particle.air_resistance,
                &particle.air_resistance_random,
                range,
                clock,
                seed,
            );
        }
        let speed = length(self.velocity);
        if speed == 0.0 {
            return [0.0; 3];
        }
        let at = |ages| {
            let angles = std::array::from_fn(|axis| {
                curve_value_seeded_at(
                    &particle.rotation_velocity[axis],
                    &particle.rotation_velocity_random[axis],
                    ages,
                    0.0,
                    seed ^ (0x7010 + axis as u64),
                )
            });
            // Zero angles preserve the original velocity, including its sign
            // and any component that differs from the injection direction.
            let velocity = if angles == [0.0; 3] {
                self.velocity
            } else {
                let velocity = basis_transform(self.direction_basis, direction_angles(angles))
                    .map(|value| value * speed);
                if self.local_direction {
                    transpose_transform(self.direction_basis, velocity)
                } else {
                    velocity
                }
            };
            let resistance = if resistance_neutral {
                1.0
            } else {
                curve_value_seeded_at(
                    &particle.air_resistance,
                    &particle.air_resistance_random,
                    ages,
                    0.0,
                    seed ^ 0xA125,
                )
            };
            velocity.map(|value| f64::from(value) * f64::from(resistance))
        };
        let curves = [
            (&particle.rotation_velocity[0], false),
            (&particle.rotation_velocity[1], false),
            (&particle.rotation_velocity[2], false),
            (&particle.rotation_velocity_random[0], true),
            (&particle.rotation_velocity_random[1], true),
            (&particle.rotation_velocity_random[2], true),
            (&particle.air_resistance, false),
            (&particle.air_resistance_random, true),
        ];
        integration::looped_vector_range(
            range,
            f64::from(clock.loop_start),
            f64::from(clock.loop_end),
            &curves,
            &at,
        )
        .map(|value| value as f32)
    }
}

fn neutral(curve: &AvfxCurve, value: f32) -> bool {
    match curve.keys.as_slice() {
        [] => true,
        [key] => key.z == value,
        _ => false,
    }
}

fn direction_angles([x, y, z]: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = x.sin_cos();
    let (sy, cy) = y.sin_cos();
    let (sz, cz) = z.sin_cos();
    [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy]
}

fn length(vector: [f32; 3]) -> f32 {
    vector.iter().map(|value| value * value).sum::<f32>().sqrt()
}

pub(super) fn normalized(vector: [f32; 3]) -> [f32; 3] {
    let length = length(vector);
    if length > 0.0 && length.is_finite() {
        vector.map(|value| value / length)
    } else {
        // Collapsed emitter axes must not inject NaNs into submitted instances.
        [0.0; 3]
    }
}

fn injection_basis(direction: [f32; 3], up: [f32; 3], fallback: [f32; 3]) -> [[f32; 3]; 3] {
    // Client 0x14037cc80 leaves the constructor's identity basis below this bound.
    if direction.iter().map(|value| value * value).sum::<f32>() < 1e-4 {
        return VFX_IDENTITY_BASIS;
    }
    let z = normalized(direction);
    let cosine: f32 = normalized(up).iter().zip(z).map(|(a, b)| a * b).sum();
    let up = if cosine.abs() > 0.99 { fallback } else { up };
    let x = normalized(cross(up, z));
    if x == [0.0; 3] || z == [0.0; 3] {
        VFX_IDENTITY_BASIS
    } else {
        [x, cross(z, x), z]
    }
}

fn transpose_transform(basis: [[f32; 3]; 3], vector: [f32; 3]) -> [f32; 3] {
    basis.map(|column| column.iter().zip(vector).map(|(a, b)| a * b).sum())
}

fn inverse_basis([x, y, z]: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let rows = [cross(y, z), cross(z, x), cross(x, y)];
    let determinant: f32 = x.iter().zip(rows[0]).map(|(a, b)| a * b).sum();
    // Client 0x14037b2b0 uses identity linear and negative translation when
    // determinant lies inside [-1e-5, 1e-5]. Translation is handled by the caller.
    if determinant.abs() <= 1e-5 || !determinant.is_finite() {
        VFX_IDENTITY_BASIS
    } else {
        std::array::from_fn(|column| std::array::from_fn(|row| rows[row][column] / determinant))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurveKey, BEHAVIOR_ADD, BEHAVIOR_REPEAT};

    fn curve(keys: &[(i16, f32)]) -> AvfxCurve {
        AvfxCurve {
            keys: keys
                .iter()
                .map(|&(time, z)| AvfxCurveKey {
                    time,
                    z,
                    x: 0.0,
                    y: 0.0,
                    interpolation: AvfxCurveKey::INTERPOLATION_LINEAR,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn assert_vector(actual: [f32; 3], expected: [f32; 3]) {
        for (a, b) in actual.into_iter().zip(expected) {
            assert!(
                (a - b).abs() < 2e-5 * b.abs().max(1.0),
                "{actual:?} != {expected:?}"
            );
        }
    }

    fn world_motion(direction: [f32; 3], velocity: [f32; 3]) -> InjectionMotion {
        InjectionMotion::new(
            &Default::default(),
            EmitterBase::root([0.0; 3]),
            [0.0; 3],
            InjectionDirection::World(direction),
            velocity,
        )
    }

    #[test]
    fn vr_basis_uses_picd_space_and_paired_local_direction_transform() {
        let parent = EmitterBase {
            linear: [[0.0, 2.0, 0.0], [-3.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
            ..EmitterBase::root([0.0; 3])
        };
        let mut particle = AvfxParticle::default();
        particle.rotation_velocity[0] = curve(&[(0, std::f32::consts::FRAC_PI_2)]);
        for mode in [0, 1, 2, 3] {
            for local in [0, 1] {
                let motion = InjectionMotion::new(
                    &AvfxEmitterItem {
                        parent_influence_coord: mode,
                        local_direction: local,
                        ..Default::default()
                    },
                    parent,
                    [0.0; 3],
                    InjectionDirection::World([0.0, 0.0, 1.0]),
                    [0.0, 0.0, 2.0],
                );
                let base = if mode < 2 {
                    VFX_IDENTITY_BASIS
                } else {
                    parent.linear
                };
                let expected_parent = if local == 1 {
                    basis_mul(base, motion.direction_basis)
                } else {
                    base
                };
                for (actual, expected) in motion
                    .drawing_parent(parent.linear)
                    .into_iter()
                    .flatten()
                    .zip(expected_parent.into_iter().flatten())
                {
                    assert!((actual - expected).abs() < 2e-5);
                }
                let mut displacement = motion.displacement(&particle, 2.0, 0);
                if local == 1 {
                    displacement = basis_transform(motion.direction_basis, displacement);
                }
                if mode == 2 || mode == 3 {
                    displacement = basis_transform(parent.linear, displacement);
                }
                assert_vector(displacement, [if mode < 2 { 4.0 } else { 3.0 }, 0.0, 0.0]);
            }
        }
    }

    #[test]
    fn point_direction_keeps_raw_components_across_coordinate_modes() {
        let parent = EmitterBase {
            position: [1.0, 2.0, 3.0],
            linear: [[0.0, -2.0, 0.0], [-3.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
            ..EmitterBase::root([0.0; 3])
        };
        for mode in [0, 1, 2, 3, 8] {
            for local in [0, 1] {
                let item = AvfxEmitterItem {
                    parent_influence_coord: mode,
                    local_direction: local,
                    ..Default::default()
                };
                let inherited = InjectionMotion::new(
                    &item,
                    parent,
                    [4.0, 6.0, 11.0],
                    InjectionDirection::Inherited([2.0, 3.0, 4.0]),
                    [3.0, 4.0, 8.0],
                );
                assert_eq!(inherited.direction, [2.0, 3.0, 4.0]);
                let inherited_world = if mode < 2 {
                    [2.0, 3.0, 4.0]
                } else {
                    basis_transform(parent.linear, [2.0, 3.0, 4.0])
                };
                assert_vector(inherited.drawing_direction(parent.linear), inherited_world);
                let world = InjectionMotion::new(
                    &item,
                    parent,
                    [4.0, 6.0, 11.0],
                    InjectionDirection::World([2.0, 3.0, 4.0]),
                    [3.0, 4.0, 8.0],
                );
                let expected = if mode < 2 {
                    [2.0, 3.0, 4.0]
                } else {
                    [-1.5, -2.0 / 3.0, 1.0]
                };
                assert_vector(world.direction, expected);
                assert_vector(world.drawing_direction(parent.linear), [2.0, 3.0, 4.0]);
                assert_eq!(inherited.origin, world.origin);
                if local == 0 {
                    assert_eq!(inherited.velocity, world.velocity);
                }
            }
        }
    }

    #[test]
    fn short_point_direction_leaves_identity_basis_at_client_threshold() {
        let up = [0.0, 1.0, 0.0];
        let fallback = [0.0, 0.0, 1.0];
        for y in [0.0, 0.00999, -0.00999] {
            assert_eq!(
                injection_basis([0.0, y, 0.0], up, fallback),
                VFX_IDENTITY_BASIS
            );
        }
        let basis = [[-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]];
        for y in [0.01, 0.01001] {
            assert_eq!(injection_basis([0.0, y, 0.0], up, fallback), basis);
        }
    }

    #[test]
    fn zero_vr_preserves_signed_velocity_instead_of_reconstructing_direction() {
        let motion = world_motion([0.0, 0.0, 1.0], [-3.0, 4.0, 0.0]);
        let mut particle = AvfxParticle::default();
        particle.rotation_velocity[1] = curve(&[(0, 0.0), (2, std::f32::consts::FRAC_PI_2)]);
        particle.rotation_velocity[1].keys[1].interpolation = AvfxCurveKey::INTERPOLATION_STEP;
        assert_vector(motion.displacement(&particle, 2.0, 0), [-6.0, 8.0, 0.0]);
        assert_vector(motion.displacement(&particle, 3.0, 0), [-1.0, 8.0, 0.0]);
        let stopped = world_motion([0.0, 0.0, 1.0], [0.0; 3]);
        assert_vector(stopped.displacement(&particle, 3.0, 0), [0.0; 3]);
    }

    #[test]
    fn vr_integrates_local_loops_repeats_and_long_constant_tails() {
        let motion = world_motion([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        let mut particle = AvfxParticle::default();
        let w = std::f32::consts::PI / 8.0;
        particle.rotation_velocity[1] = curve(&[(0, 0.0), (4, w * 4.0)]);
        let primitive = |from: f32, to: f32| {
            [
                ((w * from).cos() - (w * to).cos()) / w,
                0.0,
                ((w * to).sin() - (w * from).sin()) / w,
            ]
        };
        assert_vector(
            motion.displacement(&particle, 1_000_000.0, 0),
            [
                999_996.0 * (4.0 * w).sin() + 1.0 / w,
                0.0,
                999_996.0 * (4.0 * w).cos() + 1.0 / w,
            ],
        );
        particle.loop_start = 2;
        particle.loop_end = 4;
        let intro = primitive(0.0, 4.0);
        let cycle = primitive(2.0, 4.0);
        assert_vector(
            motion.displacement(&particle, 10.0, 0),
            std::array::from_fn(|i| intro[i] + 3.0 * cycle[i]),
        );
        particle.loop_start = 0;
        particle.loop_end = 0;
        particle.rotation_velocity[1].post_behavior = BEHAVIOR_REPEAT;
        assert_vector(
            motion.displacement(&particle, 40_000.0, 0),
            intro.map(|v| v * 10_000.0),
        );
    }

    #[test]
    fn emitter_vr_integrates_parent_injected_motion_with_resistance() {
        let motion = world_motion([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        let mut emitter = AvfxEmitter::default();
        let angular_rate = std::f32::consts::PI / 8.0;
        emitter.rotation_velocity[1] = curve(&[(0, 0.0), (4, angular_rate * 4.0)]);
        emitter.air_resistance = curve(&[(0, 1.0), (4, 1.0)]);
        let actual =
            motion.emitter_displacement(&emitter, 4.0, InstanceClock::new(-1.0, 1.0, 0, 0), 0);
        assert_vector(
            actual,
            [
                (1.0 - (angular_rate * 4.0).cos()) / angular_rate,
                0.0,
                (angular_rate * 4.0).sin() / angular_rate,
            ],
        );
    }

    #[test]
    fn add_vr_uses_total_age_while_resistance_uses_local_loop_age() {
        let motion = world_motion([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        let mut particle = AvfxParticle {
            loop_start: 0,
            loop_end: 2,
            ..Default::default()
        };
        let angular_rate = std::f32::consts::FRAC_PI_2;
        particle.rotation_velocity[1] = curve(&[(0, 0.0), (2, angular_rate * 2.0)]);
        particle.rotation_velocity[1].post_behavior = BEHAVIOR_ADD;
        particle.air_resistance = curve(&[(0, 1.0), (1, 2.0), (2, 1.0)]);

        let actual = motion.displacement(&particle, 3.0, 0);
        let steps = 30_000;
        let step = 3.0 / steps as f32;
        let mut expected = [0.0; 3];
        for index in 0..steps {
            let total = (index as f32 + 0.5) * step;
            let local = if total >= 2.0 { total - 2.0 } else { total };
            let resistance = if local <= 1.0 {
                1.0 + local
            } else {
                3.0 - local
            };
            let angle = angular_rate * total;
            expected[0] += angle.sin() * resistance * step;
            expected[2] += angle.cos() * resistance * step;
        }
        assert_vector(actual, expected);

        particle.air_resistance = curve(&[(0, 1.0)]);
        let expected = [
            (1.0 - (angular_rate * 3.0).cos()) / angular_rate,
            0.0,
            (angular_rate * 3.0).sin() / angular_rate,
        ];
        assert_vector(motion.displacement(&particle, 3.0, 0), expected);
    }

    #[test]
    fn random_direction_and_resistance_share_each_frame_of_the_integral() {
        let motion = world_motion([0.0, 0.0, 1.0], [0.0, 0.0, 2.0]);
        let mut particle = AvfxParticle::default();
        particle.rotation_velocity_random[1] = AvfxCurve {
            random_type: 4,
            ..curve(&[(0, 1.0)])
        };
        particle.air_resistance = curve(&[(0, 0.5)]);
        particle.air_resistance_random = AvfxCurve {
            random_type: 3,
            ..curve(&[(0, 1.0)])
        };
        let seed = 123;
        // Single-key Always curves are constant within each integer frame.
        let mut expected = [0.0; 3];
        for frame in 0..8 {
            let angle = super::super::curve_random_value(
                &particle.rotation_velocity_random[1],
                frame as f32,
                seed ^ 0x7011,
            );
            let resistance = 0.5
                + super::super::curve_random_value(
                    &particle.air_resistance_random,
                    frame as f32,
                    seed ^ 0xA125,
                );
            expected[0] += 2.0 * angle.sin() * resistance;
            expected[2] += 2.0 * angle.cos() * resistance;
        }
        assert_vector(motion.displacement(&particle, 8.0, seed), expected);
        let again = motion.displacement(&particle, 8.0, seed);
        motion.displacement(&particle, 100.0, seed);
        assert_eq!(motion.displacement(&particle, 8.0, seed), again);
    }

    #[test]
    fn inverse_retains_shear_mirroring_and_client_singular_fallback() {
        let basis = [[-2.0, 0.0, 0.0], [1.0, 3.0, 0.0], [0.0, 1.0, 4.0]];
        let inverse = inverse_basis(basis);
        assert_vector(basis_transform(inverse, [-1.0, 6.0, 12.0]), [1.0, 1.0, 3.0]);
        for x in [0.0, 0.5e-5, -0.5e-5] {
            assert_eq!(
                inverse_basis([[x, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
                VFX_IDENTITY_BASIS
            );
        }
        assert_eq!(
            injection_basis([0.0; 3], [0.0; 3], [0.0; 3]),
            VFX_IDENTITY_BASIS
        );
        assert_eq!(normalized([0.0; 3]), [0.0; 3]);
    }
}
