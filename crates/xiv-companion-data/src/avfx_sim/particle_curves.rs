//! Particle First prefix, XYZ refresh, gravity and joint VR/ARs Numeric.
//! 3e0f10: TC1/TP, conditional Gra/ARs/VR, Col, Scl/Rot/Pos.
//! TC1 and TP draw readers retain their constructor First values and pairs.
//! Laser derives Len/Wdt First after the base refresh; Draw samples Wdt, Len, then textures.
//! Disc derives nine geometry First values and two edge colors; normal properties
//! refresh edge colors, while Draw samples geometry before textures. Polygon
//! derives its count First and samples count before textures.
//! Line retains LenR and endpoint-color First values; properties update colors,
//! ordinary Draw samples length, and Smpl Draw skips length. Neither draws TC1/TP.
//! Polyline retains six Color First groups before seven scalar First values;
//! Draw reads Len/CF/Sft, Wd/WdB/WdE, textures, then conditional WdC.
//! This does not reproduce Common/geometry factories or resource providers.
use super::VfxClientScalarPairState;
use super::{
    VfxClientColorCurveDefaults, VfxClientColorCurveState, VfxClientRandomState,
    VfxClientScalarCurveState, VfxClientVectorCurveState,
};
use crate::avfx::{AvfxCurve, AvfxParticle};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

/// Lazy scalar dispatch belongs to the compiled definition, not the particle.
#[derive(Debug, Default)]
pub(super) struct VfxClientParticleCurveEnvironment {
    pairs: Arc<Mutex<HashMap<(usize, u8), VfxClientScalarPairState>>>,
}
impl Clone for VfxClientParticleCurveEnvironment {
    fn clone(&self) -> Self {
        Self {
            pairs: Arc::new(Mutex::new(
                self.pairs
                    .lock()
                    .expect("particle pair cache poisoned")
                    .clone(),
            )),
        }
    }
}
impl VfxClientParticleCurveEnvironment {
    pub(super) fn textures(
        &self,
        index: usize,
        state: VfxClientParticleCurveState,
        p: &AvfxParticle,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) -> VfxClientParticleTextureValues {
        let mut pairs = self.pairs.lock().expect("particle pair cache poisoned");
        if let (Some(state), crate::avfx::AvfxParticleData::Line(data)) = (state.line, &p.data) {
            // 3ecd40 reads Len, whereas Smpl 3ed140 uses slot endpoints.
            // Neither Line Draw path invokes the TC1/TD/TP callbacks.
            let length = (!p.simple_anim_enable).then(|| {
                state.length.evaluate(
                    pairs.entry((index, 19)).or_default(),
                    (!data.length.keys.is_empty())
                        .then(|| data.length.value_at(ages[0], ages[1], 0.0)),
                    (!data.length_random.keys.is_empty())
                        .then(|| data.length_random.value_at(ages[0], ages[1], 0.0)),
                    random,
                )
            });
            return VfxClientParticleTextureValues {
                line_geometry: Some(VfxClientLineDrawValues {
                    length,
                    colors: state.cache,
                }),
                ..Default::default()
            };
        }
        let mut polyline_geometry = state.polyline.and_then(|state| {
            let crate::avfx::AvfxParticleData::Polyline(data) = &p.data else {
                return None;
            };
            let curves = super::polyline::scalar_curves(data);
            let mut values = [0.0; 8];
            // 3f8480: Len, CF, Sft, PnDs, Wd, WdB, WdE. WdC runs
            // after the TC1/TD/TP callbacks and only for a valid center.
            for (output, input) in [(0, 5), (1, 0), (2, 6), (4, 1), (5, 2), (7, 4)] {
                let (main, rand) = curves[input];
                values[output] = state.scalars[input].evaluate(
                    pairs.entry((index, 20 + input as u8)).or_default(),
                    (!main.keys.is_empty()).then(|| main.value_at(ages[0], ages[1], 0.0)),
                    (!rand.keys.is_empty()).then(|| rand.value_at(ages[0], ages[1], 0.0)),
                    random,
                );
            }
            values[3] = data.point_distortion.value_at(ages[0], ages[1], 0.0);
            Some(VfxClientPolylineDrawValues {
                values,
                colors: state.cache,
            })
        });
        // Laser's derived constructor retains LenR before WdtR, but its Draw
        // samples Wdt before Len (3eafdf..3eb061), ahead of TC1/TP.
        let laser_dimensions = state.laser.and_then(|states| {
            let crate::avfx::AvfxParticleData::Laser(data) = &p.data else {
                return None;
            };
            let mut evaluate =
                |slot, state: VfxClientScalarCurveState, main: &AvfxCurve, rand: &AvfxCurve| {
                    state.evaluate(
                        pairs.entry((index, slot)).or_default(),
                        (!main.keys.is_empty()).then(|| main.value_at(ages[0], ages[1], 0.0)),
                        (!rand.keys.is_empty()).then(|| rand.value_at(ages[0], ages[1], 0.0)),
                        random,
                    )
                };
            let width = evaluate(8, states[1], &data.width, &data.width_random);
            let length = evaluate(7, states[0], &data.length, &data.length_random);
            Some([length, width])
        });
        let polygon_count = state.polygon.and_then(|state| {
            let crate::avfx::AvfxParticleData::Polygon(data) = &p.data else {
                return None;
            };
            Some(
                state.evaluate(
                    pairs.entry((index, 9)).or_default(),
                    (!data.count.keys.is_empty())
                        .then(|| data.count.value_at(ages[0], ages[1], 0.0)),
                    (!data.count_random.keys.is_empty())
                        .then(|| data.count_random.value_at(ages[0], ages[1], 0.0)),
                    random,
                ),
            )
        });
        let disc_geometry = state.disc.and_then(|state| {
            let crate::avfx::AvfxParticleData::Disc(data) = &p.data else {
                return None;
            };
            let curves = super::disc::scalar_curves(data);
            let scalars = std::array::from_fn(|i| {
                let (main, rand) = curves[i];
                state.scalars[i].evaluate(
                    pairs.entry((index, 10 + i as u8)).or_default(),
                    (!main.keys.is_empty()).then(|| main.value_at(ages[0], ages[1], 0.0)),
                    (!rand.keys.is_empty()).then(|| rand.value_at(ages[0], ages[1], 0.0)),
                    random,
                )
            });
            Some(VfxClientDiscDrawValues {
                scalars,
                colors: state.cache,
            })
        });
        let texture_index = p.texture_color1.as_ref().filter(|t| t.enabled).map(|t| {
            // Builtin sources and empty TLst return before the pair callback.
            if let Some(source) = t.tc1_builtin_source() {
                return source;
            }
            let count = (t.texture_list.len() & 255) as i32;
            if count == 0 {
                return -1;
            }
            let value = state.texture_index.evaluate(
                pairs.entry((index, 5)).or_default(),
                t.tex_n
                    .as_ref()
                    .filter(|c| !c.keys.is_empty())
                    .map(|c| c.value_at(ages[0], ages[1], 0.0)),
                t.tex_n_random
                    .as_ref()
                    .filter(|c| !c.keys.is_empty())
                    .map(|c| c.value_at(ages[0], ages[1], 0.0)),
                random,
            );
            let slot = (count.wrapping_shl(16).wrapping_add(super::cvttss2si(value))) % count;
            usize::try_from(slot)
                .ok()
                .and_then(|slot| t.texture_list.get(slot))
                .map_or(-1, |value| i32::from(*value as u8 as i8))
        });
        let palette_offset = p.texture_palette.as_ref().filter(|t| t.enabled).map(|t| {
            // The callback runs even when TxNo fails resource lookup.
            let value = state.palette_offset.evaluate(
                pairs.entry((index, 6)).or_default(),
                (!t.offset.keys.is_empty()).then(|| t.offset.value_at(ages[0], ages[1], 0.0)),
                (!t.offset_random.keys.is_empty())
                    .then(|| t.offset_random.value_at(ages[0], ages[1], 0.0)),
                random,
            );
            super::particle_palette_offset(p, value)
        });
        if let (Some(value), Some(state), crate::avfx::AvfxParticleData::Polyline(data)) =
            (&mut polyline_geometry, state.polyline, &p.data)
        {
            if super::polyline::has_center(data) {
                value.values[6] = state.scalars[3].evaluate(
                    pairs.entry((index, 23)).or_default(),
                    (!data.width_center.keys.is_empty())
                        .then(|| data.width_center.value_at(ages[0], ages[1], 0.0)),
                    (!data.width_center_random.keys.is_empty())
                        .then(|| data.width_center_random.value_at(ages[0], ages[1], 0.0)),
                    random,
                );
            }
        }
        VfxClientParticleTextureValues {
            texture_index,
            palette_offset,
            laser_dimensions,
            polygon_count,
            disc_geometry,
            line_geometry: None,
            polyline_geometry,
        }
    }

    pub(super) fn share(&self) -> Self {
        Self {
            pairs: Arc::clone(&self.pairs),
        }
    }
    pub(super) fn reset(&self) {
        self.pairs
            .lock()
            .expect("particle pair cache poisoned")
            .clear();
    }
    pub(super) fn gravity(
        &self,
        index: usize,
        state: VfxClientParticleCurveState,
        p: &AvfxParticle,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) -> Option<f32> {
        let state = state.gravity?;
        let mut pairs = self.pairs.lock().expect("particle pair cache poisoned");
        Some(
            state.evaluate(
                pairs.entry((index, 0)).or_default(),
                (!p.gravity.keys.is_empty()).then(|| p.gravity.value_at(ages[0], ages[1], 0.0)),
                (!p.gravity_random.keys.is_empty())
                    .then(|| p.gravity_random.value_at(ages[0], ages[1], 0.0)),
                random,
            ),
        )
    }

    pub(super) fn resistance(
        &self,
        index: usize,
        state: VfxClientParticleCurveState,
        p: &AvfxParticle,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) -> Option<f32> {
        let state = state.resistance?;
        let mut pairs = self.pairs.lock().expect("particle pair cache poisoned");
        Some(
            state.evaluate(
                pairs.entry((index, 1)).or_default(),
                (!p.air_resistance.keys.is_empty())
                    .then(|| p.air_resistance.value_at(ages[0], ages[1], 0.0)),
                (!p.air_resistance_random.keys.is_empty())
                    .then(|| p.air_resistance_random.value_at(ages[0], ages[1], 0.0)),
                random,
            ),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct VfxClientParticleTextureValues {
    pub texture_index: Option<i32>,
    pub palette_offset: Option<f32>,
    pub laser_dimensions: Option<[f32; 2]>,
    pub polygon_count: Option<f32>,
    pub disc_geometry: Option<VfxClientDiscDrawValues>,
    pub line_geometry: Option<VfxClientLineDrawValues>,
    pub polyline_geometry: Option<VfxClientPolylineDrawValues>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VfxClientPolylineDrawValues {
    /// Len, CF, Sft, PnDs, Wd, WdB, WdC, WdE, in original buffer order.
    pub values: [f32; 8],
    /// ColB, ColC, ColE, CoEB, CoEC, CoEE property caches.
    pub colors: [[f32; 4]; 6],
}

#[derive(Clone, Copy, Debug)]
struct VfxClientPolylineCurveState {
    scalars: [VfxClientScalarCurveState; 7],
    colors: [VfxClientColorCurveState; 6],
    cache: [[f32; 4]; 6],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VfxClientLineDrawValues {
    pub length: Option<f32>,
    pub colors: [[f32; 4]; 2],
}

#[derive(Clone, Copy, Debug)]
struct VfxClientLineCurveState {
    length: VfxClientScalarCurveState,
    colors: [VfxClientColorCurveState; 2],
    cache: [[f32; 4]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VfxClientDiscDrawValues {
    pub scalars: [f32; 9],
    pub colors: [[f32; 4]; 2],
}

#[derive(Clone, Copy, Debug)]
struct VfxClientDiscCurveState {
    scalars: [VfxClientScalarCurveState; 9],
    colors: [VfxClientColorCurveState; 2],
    cache: [[f32; 4]; 2],
}

/// Numeric owns accumulated displacement. Birth geometry supplies +18c once;
/// callbacks before that resolution retain their exact operation order.
#[derive(Clone, Debug, Default)]
pub(super) struct VfxClientParticleInjectionCache(Arc<Mutex<InjectionCache>>);

#[derive(Clone, Debug, Default)]
struct InjectionCache {
    input: Arc<OnceLock<VfxClientParticleInjectionInput>>,
    birth: Arc<OnceLock<[f32; 3]>>,
    velocity: Option<[f32; 3]>,
    displacement: [f32; 3],
    pending: Option<Arc<DeferredInjection>>,
}

#[derive(Debug)]
struct DeferredInjection {
    previous: Option<Arc<Self>>,
    delta: f32,
    ars: Option<f32>,
    displacement: OnceLock<[f32; 3]>,
}

impl Drop for DeferredInjection {
    fn drop(&mut self) {
        // Long undrawn histories must not recurse through Arc destructors.
        let mut previous = self.previous.take();
        while let Some(node) = previous {
            match Arc::try_unwrap(node) {
                Ok(mut node) => previous = node.previous.take(),
                Err(_) => break,
            }
        }
    }
}

fn resolve_injection(cache: &mut InjectionCache, velocity: [f32; 3]) {
    cache.velocity = Some(velocity);
    let mut pending = cache.pending.take();
    let mut steps = Vec::new();
    while let Some(node) = pending {
        if let Some(saved) = node.displacement.get() {
            cache.displacement = *saved;
            break;
        }
        pending = node.previous.clone();
        steps.push(node);
    }
    for node in steps.into_iter().rev() {
        injection_step(&mut cache.displacement, velocity, node.delta, node.ars);
        // Prewarm snapshots share prefixes and their one birth velocity.
        // Each prefix is integrated once even if snapshots are read out of order.
        cache.displacement = *node.displacement.get_or_init(|| cache.displacement);
    }
}

fn injection_step(displacement: &mut [f32; 3], velocity: [f32; 3], delta: f32, ars: Option<f32>) {
    injection_add(displacement, velocity.map(|v| delta * v), ars);
}

fn injection_add(displacement: &mut [f32; 3], change: [f32; 3], ars: Option<f32>) {
    for axis in 0..3 {
        displacement[axis] = match ars {
            Some(value) => change[axis] * value + displacement[axis],
            None if axis == 0 => displacement[axis] + change[axis],
            None => change[axis] + displacement[axis],
        };
    }
}

/// Explicit original +18c/+104/+198 inputs. The joint VR kernel currently has
/// controlled CPU proof for finite angles within [-floatPI,floatPI]; production
/// must supply birth data before Numeric decides whether VR consumes random.
#[derive(Clone, Copy, Debug)]
pub(super) struct VfxClientParticleInjectionInput {
    pub velocity: [f32; 3],
    pub basis: [[f32; 3]; 3],
    pub local_direction: bool,
}

fn rotated_injection(
    mut change: [f32; 3],
    length: f32,
    angles: [f32; 3],
    input: VfxClientParticleInjectionInput,
    mode: super::VfxClientTrigMode,
) -> [f32; 3] {
    if angles == [0.0; 3] {
        return change;
    }
    let [cx, sx] = super::client_trig::spline_transverse(angles[0], mode);
    let [cy, sy] = super::client_trig::spline_transverse(angles[1], mode);
    let [cz, sz] = super::client_trig::spline_transverse(angles[2], mode);
    // The original rotates (0,0,L) sequentially, retaining zero products and
    // each scalar operation; the simplified trigonometric formula rounds apart.
    let a = length * 0.0 + 0.0;
    let b = (cx * 0.0 + 0.0) - sx * length;
    let c = cx * length + (sx * 0.0 + 0.0);
    let x = (cy * a + b * 0.0) + c * sy;
    let y = (a * 0.0 + b) + c * 0.0;
    let z = (b * 0.0 - a * sy) + cy * c;
    let local = [
        (cz * x - y * sz) + z * 0.0,
        (x * sz + cz * y) + z * 0.0,
        (x * 0.0 + y * 0.0) + z,
    ];
    change = std::array::from_fn(|axis| {
        (input.basis[0][axis] * local[0] + input.basis[1][axis] * local[1])
            + input.basis[2][axis] * local[2]
    });
    if input.local_direction {
        // The transpose branch sums Y before X, unlike the forward multiply.
        change = std::array::from_fn(|axis| {
            (input.basis[axis][1] * change[1] + input.basis[axis][0] * change[0])
                + input.basis[axis][2] * change[2]
        });
    }
    change
}

impl VfxClientParticleCurveEnvironment {
    pub(super) fn advance_motion(
        &self,
        index: usize,
        state: VfxClientParticleCurveState,
        p: &AvfxParticle,
        ages: [f32; 2],
        delta: f32,
        input: VfxClientParticleInjectionInput,
        mode: super::VfxClientTrigMode,
        displacement: &mut [f32; 3],
        random: &mut VfxClientRandomState,
    ) -> Result<(), String> {
        let mut change = input.velocity.map(|value| delta * value);
        if let Some(states) = state.injection_rotation {
            let length =
                ((change[1] * change[1] + change[0] * change[0]) + change[2] * change[2]).sqrt();
            if length > 0.0 {
                let mut pairs = self.pairs.lock().expect("particle pair cache poisoned");
                let angles = std::array::from_fn(|axis| {
                    states[axis].evaluate(
                        pairs.entry((index, 2 + axis as u8)).or_default(),
                        (!p.rotation_velocity[axis].keys.is_empty())
                            .then(|| p.rotation_velocity[axis].value_at(ages[0], ages[1], 0.0)),
                        (!p.rotation_velocity_random[axis].keys.is_empty()).then(|| {
                            p.rotation_velocity_random[axis].value_at(ages[0], ages[1], 0.0)
                        }),
                        random,
                    )
                });
                if angles
                    .iter()
                    .any(|angle: &f32| !angle.is_finite() || angle.abs() > std::f32::consts::PI)
                {
                    return Err("staged VR exceeds verified client CRT angle domain".into());
                }
                change = rotated_injection(change, length, angles, input, mode);
            }
        }
        let ars = self.resistance(index, state, p, ages, random);
        injection_add(displacement, change, ars);
        Ok(())
    }
}

impl VfxClientParticleInjectionCache {
    pub(super) fn bind_input(&self, input: VfxClientParticleInjectionInput) {
        let mut cache = self.0.lock().expect("particle motion cache poisoned");
        let input = *cache.input.get_or_init(|| input);
        let velocity = *cache.birth.get_or_init(|| input.velocity);
        if cache.velocity.is_none() {
            resolve_injection(&mut cache, velocity);
        }
    }

    pub(super) fn advance_motion(
        &self,
        environment: &VfxClientParticleCurveEnvironment,
        index: usize,
        state: VfxClientParticleCurveState,
        particle: &AvfxParticle,
        ages: [f32; 2],
        delta: f32,
        mode: super::VfxClientTrigMode,
        random: &mut VfxClientRandomState,
    ) -> Result<(), String> {
        let input = self
            .0
            .lock()
            .expect("particle motion cache poisoned")
            .input
            .get()
            .copied();
        if let Some(input) = input {
            let mut cache = self.0.lock().expect("particle motion cache poisoned");
            environment.advance_motion(
                index,
                state,
                particle,
                ages,
                delta,
                input,
                mode,
                &mut cache.displacement,
                random,
            )
        } else if state.injection_rotation.is_none() {
            let ars = environment.resistance(index, state, particle, ages, random);
            self.advance(delta, ars);
            Ok(())
        } else {
            Err("staged VR Numeric requires resolved birth velocity and basis".into())
        }
    }

    pub(super) fn snapshot(&self) -> Self {
        Self(Arc::new(Mutex::new(
            self.0
                .lock()
                .expect("particle motion cache poisoned")
                .clone(),
        )))
    }

    pub(super) fn needs_birth(&self) -> bool {
        self.0
            .lock()
            .expect("particle motion cache poisoned")
            .birth
            .get()
            .is_none()
    }

    pub(super) fn advance(&self, delta: f32, ars: Option<f32>) {
        let mut cache = self.0.lock().expect("particle motion cache poisoned");
        if cache.velocity.is_none() {
            if let Some(velocity) = cache.birth.get().copied() {
                resolve_injection(&mut cache, velocity);
            }
        }
        if let Some(velocity) = cache.velocity {
            injection_step(&mut cache.displacement, velocity, delta, ars);
        } else {
            cache.pending = Some(Arc::new(DeferredInjection {
                previous: cache.pending.take(),
                delta,
                ars,
                displacement: OnceLock::new(),
            }));
        }
    }

    pub(super) fn displacement(&self, birth_velocity: [f32; 3]) -> [f32; 3] {
        let mut cache = self.0.lock().expect("particle motion cache poisoned");
        if cache.velocity.is_none() {
            let velocity = *cache.birth.get_or_init(|| birth_velocity);
            resolve_injection(&mut cache, velocity);
        }
        cache.displacement
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VfxClientParticleCurveState {
    pub texture_index: VfxClientScalarCurveState,
    pub palette_offset: VfxClientScalarCurveState,
    pub gravity: Option<VfxClientScalarCurveState>,
    pub resistance: Option<VfxClientScalarCurveState>,
    pub injection_rotation: Option<[VfxClientScalarCurveState; 3]>,
    laser: Option<[VfxClientScalarCurveState; 2]>,
    polygon: Option<VfxClientScalarCurveState>,
    disc: Option<VfxClientDiscCurveState>,
    line: Option<VfxClientLineCurveState>,
    polyline: Option<VfxClientPolylineCurveState>,
    vectors: [VfxClientVectorCurveState; 3],
    pub cache: [[f32; 3]; 3],
}
fn constant_or_empty(curve: &AvfxCurve, value: f32) -> bool {
    curve.keys.is_empty() || (curve.keys.len() == 1 && curve.keys[0].z == value)
}
fn mode(curve: Option<&AvfxCurve>, empty: u32) -> u32 {
    curve
        .filter(|c| !c.keys.is_empty())
        .map_or(empty, |c| c.random_type)
}
impl VfxClientParticleCurveState {
    pub(super) fn construct(
        p: &AvfxParticle,
        defaults: VfxClientColorCurveDefaults,
        random: &mut VfxClientRandomState,
    ) -> Result<(Self, VfxClientColorCurveState), String> {
        let first = |curve: Option<&AvfxCurve>, random: &mut VfxClientRandomState| {
            VfxClientScalarCurveState::construct(mode(curve, defaults.empty_random_type), random)
        };
        // Both unconditional First calls run even for absent/disabled textures.
        let texture_index = first(
            p.texture_color1
                .as_ref()
                .and_then(|t| t.tex_n_random.as_ref()),
            random,
        );
        let palette_offset = first(p.texture_palette.as_ref().map(|t| &t.offset_random), random);
        let gravity = (!(constant_or_empty(&p.gravity, 0.0)
            && constant_or_empty(&p.gravity_random, 0.0)))
        .then(|| first(Some(&p.gravity_random), random));
        let resistance = (!(constant_or_empty(&p.air_resistance, 1.0)
            && constant_or_empty(&p.air_resistance_random, 0.0)))
        .then(|| first(Some(&p.air_resistance_random), random));
        let injection_rotation = (!(p
            .rotation_velocity
            .iter()
            .all(|c| constant_or_empty(c, 0.0))
            && p.rotation_velocity_random
                .iter()
                .all(|c| constant_or_empty(c, 0.0))))
        .then(|| {
            p.rotation_velocity_random
                .each_ref()
                .map(|curve| first(Some(curve), random))
        });
        let color = VfxClientColorCurveState::construct(&p.color, defaults, random);
        let vectors = [&p.scale, &p.rotation, &p.position].map(|curve| {
            VfxClientVectorCurveState::construct(curve, defaults.empty_random_type, random)
        });
        let [Some(scale), Some(rotation), Some(position)] = vectors else {
            return Err("particle constructor requires valid XYZ connection codes".into());
        };
        Ok((
            Self {
                texture_index,
                palette_offset,
                gravity,
                resistance,
                injection_rotation,
                laser: None,
                polygon: None,
                disc: None,
                line: None,
                polyline: None,
                vectors: [scale, rotation, position],
                cache: [[1.0; 3], [0.0; 3], [0.0; 3]],
            },
            color,
        ))
    }
    pub(super) fn construct_derived(
        &mut self,
        p: &AvfxParticle,
        defaults: VfxClientColorCurveDefaults,
        random: &mut VfxClientRandomState,
    ) {
        // This runs after the base's initial XYZ/Col callback and alpha reset,
        // before the derived constructor refreshes properties a second time.
        if let crate::avfx::AvfxParticleData::Laser(data) = &p.data {
            self.laser = Some([&data.length_random, &data.width_random].map(|curve| {
                VfxClientScalarCurveState::construct(
                    mode(Some(curve), defaults.empty_random_type),
                    random,
                )
            }));
        }
        if let crate::avfx::AvfxParticleData::Polygon(data) = &p.data {
            self.polygon = Some(VfxClientScalarCurveState::construct(
                mode(Some(&data.count_random), defaults.empty_random_type),
                random,
            ));
        }
        if let crate::avfx::AvfxParticleData::Disc(data) = &p.data {
            let scalars = super::disc::scalar_curves(data).map(|(_, rand)| {
                VfxClientScalarCurveState::construct(
                    mode(Some(rand), defaults.empty_random_type),
                    random,
                )
            });
            let colors = [&data.color_edge_inner, &data.color_edge_outer]
                .map(|color| VfxClientColorCurveState::construct(color, defaults, random));
            self.disc = Some(VfxClientDiscCurveState {
                scalars,
                colors,
                cache: [defaults.empty_rgba; 2],
            });
            // 3e88f0 reads CEI/CEO once before its second base XYZ/Col refresh.
            self.evaluate_shape_properties(p, [0.0; 2], random);
        }
        if let crate::avfx::AvfxParticleData::Line(data) = &p.data {
            self.line = Some(VfxClientLineCurveState {
                length: VfxClientScalarCurveState::construct(
                    mode(Some(&data.length_random), defaults.empty_random_type),
                    random,
                ),
                colors: [&data.color_begin, &data.color_end]
                    .map(|color| VfxClientColorCurveState::construct(color, defaults, random)),
                cache: [defaults.empty_rgba; 2],
            });
            // 3eb890 evaluates both endpoint colors before second XYZ/Col.
            self.evaluate_shape_properties(p, [0.0; 2], random);
        }
        if let crate::avfx::AvfxParticleData::Polyline(data) = &p.data {
            let colors = super::polyline::color_curves(data)
                .map(|color| VfxClientColorCurveState::construct(color, defaults, random));
            let scalars = super::polyline::scalar_curves(data).map(|(_, rand)| {
                VfxClientScalarCurveState::construct(
                    mode(Some(rand), defaults.empty_random_type),
                    random,
                )
            });
            self.polyline = Some(VfxClientPolylineCurveState {
                scalars,
                colors,
                cache: [[0.0; 4]; 6],
            });
            self.evaluate_shape_properties(p, [0.0; 2], random);
        }
    }

    pub(super) fn evaluate_shape_properties(
        &mut self,
        p: &AvfxParticle,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) {
        if let (Some(state), crate::avfx::AvfxParticleData::Disc(data)) = (&mut self.disc, &p.data)
        {
            for (i, color) in [&data.color_edge_inner, &data.color_edge_outer]
                .into_iter()
                .enumerate()
            {
                state.cache[i] = state.colors[i].evaluate(color, ages[0], ages[1], random);
            }
        }
        if let (Some(state), crate::avfx::AvfxParticleData::Line(data)) = (&mut self.line, &p.data)
        {
            for (i, color) in [&data.color_begin, &data.color_end].into_iter().enumerate() {
                state.cache[i] = state.colors[i].evaluate(color, ages[0], ages[1], random);
            }
        }
        if let (Some(state), crate::avfx::AvfxParticleData::Polyline(data)) =
            (&mut self.polyline, &p.data)
        {
            let colors = super::polyline::color_curves(data);
            let mut evaluate = |i: usize| {
                state.cache[i] = state.colors[i].evaluate(colors[i], ages[0], ages[1], random);
            };
            evaluate(0);
            evaluate(2);
            if data.use_edge {
                evaluate(3);
                evaluate(5);
            }
            if super::polyline::has_center(data) {
                evaluate(1);
                if data.use_edge {
                    evaluate(4);
                }
            }
        }
    }
    pub(super) fn evaluate(
        &mut self,
        p: &AvfxParticle,
        ages: [f32; 2],
        fallback: [f32; 3],
        random: &mut VfxClientRandomState,
    ) {
        // 3e1cb0 evaluates all XYZ channels before Col, including random axes
        // whose value is later overwritten by the connection operation.
        for (index, curve) in [&p.scale, &p.rotation, &p.position].into_iter().enumerate() {
            self.cache[index] = self.vectors[index].evaluate(
                curve,
                ages,
                if index == 0 { 1.0 } else { 0.0 },
                fallback,
                random,
            );
        }
    }
    #[cfg(test)]
    fn first_bytes(self, color: VfxClientColorCurveState) -> [i8; 21] {
        let mut values = [0; 21];
        values[0] = self.texture_index.first_percentage();
        values[1] = self.palette_offset.first_percentage();
        values[2] = self
            .gravity
            .map_or(0, VfxClientScalarCurveState::first_percentage);
        values[3] = self
            .resistance
            .map_or(0, VfxClientScalarCurveState::first_percentage);
        values[4..7].copy_from_slice(&self.injection_rotation.map_or([0; 3], |v| {
            v.map(VfxClientScalarCurveState::first_percentage)
        }));
        values[7..12].copy_from_slice(&color.first_percentages());
        for (i, v) in self.vectors.into_iter().enumerate() {
            values[12 + i * 3..15 + i * 3].copy_from_slice(&v.first_percentages());
        }
        values
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::avfx::{
        AvfxCurve3Axis, AvfxCurveKey, AvfxParticleDataModelSkin, AvfxParticleTexture,
        AvfxParticleTexturePalette,
    };
    fn scalar(mode: u32, present: bool, value: f32) -> AvfxCurve {
        AvfxCurve {
            random_type: mode,
            keys: if present {
                vec![AvfxCurveKey {
                    time: 0,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: value,
                }]
            } else {
                vec![]
            },
            ..Default::default()
        }
    }
    fn fixture(mode: u32, profile: u32, color: crate::avfx::AvfxColorCurve) -> AvfxParticle {
        let vectors = std::array::from_fn::<_, 3, _>(|index| {
            let v = |axis: u32| {
                Some(scalar(
                    mode + index as u32 + axis,
                    true,
                    if axis < 3 {
                        if index == 0 { 1.0 } else { 0.0 }
                    } else if profile & 8 != 0 {
                        0.125 * (axis - 2) as f32
                    } else {
                        0.0
                    },
                ))
            };
            AvfxCurve3Axis {
                x: v(0),
                y: v(1),
                z: v(2),
                random_x: v(3),
                random_y: v(4),
                random_z: v(5),
                ..Default::default()
            }
        });
        AvfxParticle {
            color,
            texture_color1: Some(AvfxParticleTexture {
                tex_n_random: Some(scalar(mode, profile & 1 != 0, 0.25)),
                ..Default::default()
            }),
            texture_palette: Some(AvfxParticleTexturePalette {
                offset_random: scalar(mode + 1, profile & 1 != 0, 0.25),
                ..Default::default()
            }),
            gravity: scalar(0, profile & 2 != 0, 0.5),
            gravity_random: scalar(mode + 2, profile & 2 != 0, 0.25),
            air_resistance: scalar(0, profile & 4 != 0, 0.75),
            air_resistance_random: scalar(mode + 3, profile & 4 != 0, 0.25),
            rotation_velocity: std::array::from_fn(|_| scalar(0, profile & 8 != 0, 0.125)),
            rotation_velocity_random: std::array::from_fn(|i| {
                scalar(mode + 4 + i as u32, profile & 8 != 0, 0.25)
            }),
            scale: vectors[0].clone(),
            rotation: vectors[1].clone(),
            position: vectors[2].clone(),
            ..Default::default()
        }
    }
    fn compare_case(case: &serde_json::Value, ordinal: usize) -> (usize, usize) {
        let mut components = 0;
        let mut states = 0;
        let mode = case["mode"].as_u64().unwrap() as u32;
        let profile = case["profile"].as_u64().unwrap() as u32;
        let p = fixture(
            mode,
            profile,
            crate::avfx::curve_client_tests::color_from_original_case(&case["color"]),
        );
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        let (mut state, color) =
            VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
        state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
        color.evaluate(&p.color, 0.0, 0.0, &mut random); // Base property, then alpha reset.
        let uv_count = case["uvCount"].as_u64().unwrap() as usize;
        let mut uv_first = Vec::new();
        for uv in 0..uv_count {
            let r = |axis| Some(scalar(mode + uv as u32 + axis, profile & 1 != 0, 0.25));
            let set = crate::avfx::AvfxUvSet {
                scale: crate::avfx::AvfxCurve2Axis {
                    random_x: r(0),
                    random_y: r(1),
                    ..Default::default()
                },
                scroll: crate::avfx::AvfxCurve2Axis {
                    random_x: r(2),
                    random_y: r(3),
                    ..Default::default()
                },
                rotation_random: r(4).unwrap(),
                ..Default::default()
            };
            uv_first.extend(
                super::super::uv_curves::VfxClientUvCurveState::construct(
                    &set,
                    defaults.empty_random_type,
                    &mut random,
                )
                .unwrap()
                .first_percentages(),
            );
        }
        assert_eq!(
            serde_json::json!(uv_first),
            case["uvFirst"],
            "case {ordinal} UV First"
        );
        let mut derived = super::super::model_skin_curves::VfxClientModelSkinCurveState::construct(
            &AvfxParticleDataModelSkin::default(),
            defaults,
            &mut random,
        )
        .unwrap();
        derived.evaluate_properties(
            &AvfxParticleDataModelSkin::default(),
            [0.0; 2],
            [1.0; 4],
            super::super::VfxClientModelSkinCurveDefaults {
                vector_random_fallback: [0.0; 3],
            },
            &mut random,
        );
        state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
        let rgba = color.evaluate(&p.color, 0.0, 0.0, &mut random);
        assert_eq!(
            serde_json::json!(state.first_bytes(color)),
            case["first"],
            "case {ordinal} first"
        );
        assert_eq!(
            serde_json::json!(derived.first_percentages()),
            case["derivedFirst"],
            "case {ordinal} derived"
        );
        assert_eq!(
            serde_json::json!(rgba.map(f32::to_bits)),
            case["colorCache"],
            "case {ordinal} color"
        );
        let mut check = |row: &serde_json::Value,
                         state: VfxClientParticleCurveState,
                         random: VfxClientRandomState| {
            assert_eq!(
                serde_json::json!(random.words()),
                row["state"],
                "case {ordinal} stream"
            );
            states += 1;
            let values: Vec<_> = state
                .cache
                .into_iter()
                .flatten()
                .map(f32::to_bits)
                .collect();
            assert_eq!(
                serde_json::json!(values),
                row["xyzCache"],
                "case {ordinal} XYZ"
            );
            components += 9;
        };
        check(case, state, random);
        for step in case["repeatedProperties"].as_array().unwrap() {
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            check(step, state, random);
        }
        (components, states)
    }
    #[test]
    fn particle_constructor_and_same_age_properties_match_captured_client_state() {
        let case: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/particle-construction-first.json"))
                .unwrap();
        assert_eq!(compare_case(&case, 0), (36, 4));
    }
    fn gravity_fixture(case: &serde_json::Value) -> AvfxParticle {
        let profile = case["profile"].as_u64().unwrap() as usize;
        let value = case["value"].as_u64().unwrap();
        let c = |count: usize, mode, base| AvfxCurve {
            random_type: mode,
            pre_behavior: 4,
            post_behavior: 4,
            keys: (0..count)
                .map(|i| AvfxCurveKey {
                    time: (i * 10) as i16,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: base + 0.125 * i as f32,
                })
                .collect(),
        };
        AvfxParticle {
            gravity: c(
                profile / 3,
                0,
                if value == 0 {
                    0.0
                } else if value == 1 {
                    0.5
                } else if value == 2 {
                    -0.5
                } else {
                    1.0
                },
            ),
            gravity_random: c(
                profile % 3,
                case["mode"].as_u64().unwrap() as u32,
                if value == 0 {
                    0.0
                } else if value == 1 {
                    0.25
                } else if value == 2 {
                    -0.25
                } else {
                    0.0
                },
            ),
            ..Default::default()
        }
    }
    fn compare_gravity_case(case: &serde_json::Value) -> usize {
        let p = gravity_fixture(case);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let mut steps = 0;
        for expected in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            for _ in 0..2 {
                state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, 0.0, 0.0, &mut random);
            }
            assert_eq!(
                serde_json::json!(random.words()),
                expected["constructionState"]
            );
            assert_eq!(
                serde_json::json!(state.gravity.is_some()),
                expected["enabled"]
            );
            assert_eq!(
                serde_json::json!(
                    state
                        .gravity
                        .map_or(0, VfxClientScalarCurveState::first_percentage)
                ),
                expected["first"]
            );
            let (mut velocity, mut offset) = (0.0f32, 0.0f32);
            for row in expected["steps"].as_array().unwrap() {
                let n = |name| f32::from_bits(row[name].as_u64().unwrap() as u32);
                if let Some(value) = env.gravity(0, state, &p, [n("age"), n("total")], &mut random)
                {
                    velocity += value * n("delta");
                    offset += velocity * n("delta");
                }
                assert_eq!(serde_json::json!(velocity.to_bits()), row["velocity"]);
                assert_eq!(serde_json::json!(offset.to_bits()), row["offset"]);
                assert_eq!(serde_json::json!(random.words()), row["state"]);
                let dispatch = env
                    .pairs
                    .lock()
                    .unwrap()
                    .get(&(0, 0))
                    .copied()
                    .unwrap_or_default()
                    .dispatch_code();
                assert_eq!(serde_json::json!(dispatch), row["dispatch"]);
                steps += 1;
            }
        }
        steps
    }
    #[test]
    fn gravity_pair_dispatch_is_shared_by_instances_and_matches_original_capture() {
        let case: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/particle-gravity-shared.json")).unwrap();
        assert_eq!(compare_gravity_case(&case), 12);
    }
    #[test]
    #[ignore = "original particle construction and gravity Numeric pair dispatch"]
    fn compare_original_particle_gravity_numeric() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let native: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("particle-gravity-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut steps = 0;
        for (index, case) in native["cases"].as_array().unwrap().iter().enumerate() {
            let result = std::panic::catch_unwind(|| compare_gravity_case(case));
            steps += result.unwrap_or_else(|_| panic!("original gravity case {index}"));
        }
        assert_eq!(steps, 5184);
        std::fs::write(folder.join("particle-gravity-rust-comparison.json"),serde_json::to_string_pretty(&serde_json::json!({"definitions":432,"instances":864,"numericSteps":steps,"gravityComponentsCompared":steps*2,"statesCompared":steps+864,"differences":0,"scope":"Original complete base constructor and 3e6000 gravity Numeric, actual compiled scalar readers, shared Gra pair across two instances; concrete second property callback/parent Color/Common/geometry controlled. Eight random modes, empty globals0/4, zero/one/two-key main/random pairs, positive/negative/zero amplitudes, repeated clocks and nonuniform deltas. Not full emitter, ARs/VR motion, host or GPU."})).unwrap()+"\n").unwrap();
    }
    fn compare_ars_case(case: &serde_json::Value) -> usize {
        let mut p = gravity_fixture(case);
        p.air_resistance = std::mem::take(&mut p.gravity);
        p.air_resistance_random = std::mem::take(&mut p.gravity_random);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let mut steps = 0;
        for expected in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            assert!(state.injection_rotation.is_none());
            for _ in 0..2 {
                state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, 0.0, 0.0, &mut random);
            }
            assert_eq!(
                serde_json::json!(random.words()),
                expected["constructionState"]
            );
            assert_eq!(
                serde_json::json!(state.resistance.is_some()),
                expected["enabled"]
            );
            assert_eq!(
                serde_json::json!(
                    state
                        .resistance
                        .map_or(0, VfxClientScalarCurveState::first_percentage)
                ),
                expected["first"]
            );
            let velocity: [f32; 3] = std::array::from_fn(|i| {
                f32::from_bits(expected["velocity"][i].as_u64().unwrap() as u32)
            });
            let cache = VfxClientParticleInjectionCache::default();
            let late_birth = VfxClientParticleInjectionCache::default();
            let mut prewarm = Vec::new();
            for row in expected["steps"].as_array().unwrap() {
                let n = |name| f32::from_bits(row[name].as_u64().unwrap() as u32);
                let ars = env.resistance(0, state, &p, [n("age"), n("total")], &mut random);
                cache.advance(n("delta"), ars);
                late_birth.advance(n("delta"), ars);
                prewarm.push((late_birth.snapshot(), row["offset"].clone()));
                let offset = cache.displacement(velocity);
                assert_eq!(serde_json::json!(offset.map(f32::to_bits)), row["offset"]);
                assert_eq!(
                    serde_json::json!(velocity.map(f32::to_bits)),
                    row["velocity"]
                );
                assert_eq!(serde_json::json!(random.words()), row["state"]);
                let dispatch = env
                    .pairs
                    .lock()
                    .unwrap()
                    .get(&(0, 1))
                    .copied()
                    .unwrap_or_default()
                    .dispatch_code();
                assert_eq!(serde_json::json!(dispatch), row["dispatch"]);
                assert_eq!(
                    cache.displacement([99.0; 3]),
                    offset,
                    "later velocity must not replace birth"
                );
                steps += 1;
            }
            assert_eq!(
                late_birth.displacement(velocity),
                cache.displacement(velocity)
            );
            for (snapshot, expected) in prewarm.into_iter().rev() {
                assert_eq!(
                    serde_json::json!(snapshot.displacement([99.0; 3]).map(f32::to_bits)),
                    expected,
                    "prewarm prefix and birth velocity"
                );
            }
            let historical = cache.snapshot();
            let captured = historical.displacement(velocity);
            cache.advance(1.0, None);
            assert_eq!(historical.displacement(velocity), captured);
        }
        steps
    }

    #[test]
    fn particle_ars_shared_dispatch_and_deferred_birth_match_original() {
        let case: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/particle-ars-shared.json")).unwrap();
        assert_eq!(compare_ars_case(&case), 12);
    }

    #[test]
    #[ignore = "original particle construction and straight-injection ARs Numeric"]
    fn compare_original_particle_ars_numeric() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let native: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("particle-ars-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut steps = 0;
        for (index, case) in native["cases"].as_array().unwrap().iter().enumerate() {
            steps += std::panic::catch_unwind(|| compare_ars_case(case))
                .unwrap_or_else(|_| panic!("original ARs case {index}"));
        }
        assert_eq!(steps, 6912);
        std::fs::write(folder.join("particle-ars-rust-comparison.json"),serde_json::to_string_pretty(&serde_json::json!({"definitions":576,"instances":1152,"numericSteps":steps,"positionComponentsCompared":steps*3,"statesCompared":steps+1152,"differences":0,"scope":"Complete base constructor, original straight injection3e6130 and ARs callbacks3e6070/3e60b0, actual compiled scalar readers. Two instances share ARs dispatch; controlled velocities, parent Color/Common/geometry. Eight modes, empty globals0/4, zero/one/two keys and neutral/positive/negative/zero amplitudes. Deferred birth and snapshot checks use original accumulated positions. No VR/full emitter/provider/GPU."})).unwrap()+"\n").unwrap();
    }

    fn compare_vr_case(case: &serde_json::Value) -> usize {
        let mut p =
            gravity_fixture(&serde_json::json!({"profile":4,"value":1,"mode":case["mode"]}));
        p.air_resistance = std::mem::take(&mut p.gravity);
        p.air_resistance_random = std::mem::take(&mut p.gravity_random);
        let profile = case["profile"].as_u64().unwrap();
        let mode = case["mode"].as_u64().unwrap() as u32;
        for axis in 0..3 {
            for random in [false, true] {
                let count = if profile <= 1 {
                    1
                } else if profile == 4 {
                    usize::from(random)
                } else {
                    2
                };
                let base = if profile == 0 || profile == 3 {
                    0.0
                } else if profile == 1 {
                    if random {
                        0.125
                    } else {
                        [0.25, -0.5, 0.75][axis]
                    }
                } else if profile == 4 {
                    0.125
                } else {
                    [0.0, 0.125, -0.125][axis]
                };
                let curve = AvfxCurve {
                    random_type: if random { mode } else { 0 },
                    pre_behavior: 4,
                    post_behavior: 4,
                    keys: (0..count)
                        .map(|i| AvfxCurveKey {
                            time: (10 * i) as i16,
                            interpolation: 1,
                            x: 0.0,
                            y: 0.0,
                            z: base + 0.125 * i as f32,
                        })
                        .collect(),
                };
                if random {
                    p.rotation_velocity_random[axis] = curve;
                } else {
                    p.rotation_velocity[axis] = curve;
                }
            }
        }
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let crt = if case["crt"] == 0 {
            super::super::VfxClientTrigMode::Sse2
        } else {
            super::super::VfxClientTrigMode::AvxFma
        };
        let velocities = [
            [0.0; 3],
            [0.03125, -0.0625, 0.125],
            [-0.25, 0.125, -0.5],
            [1e-30, 0.0, 0.0],
        ];
        let bases = [
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
            [[-1.0, 0.25, 0.0], [0.0, 2.0, 0.5], [0.125, 0.0, 3.0]],
        ];
        let input = VfxClientParticleInjectionInput {
            velocity: velocities[case["velocityProfile"].as_u64().unwrap() as usize],
            basis: bases[case["basisProfile"].as_u64().unwrap() as usize],
            local_direction: case["localDirection"] == 1,
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let mut steps = 0;
        for instance in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            assert_eq!(
                serde_json::json!(state.injection_rotation.is_some()),
                instance["vrEnabled"]
            );
            let first = [
                state
                    .resistance
                    .map_or(0, VfxClientScalarCurveState::first_percentage),
                state
                    .injection_rotation
                    .map_or(0, |s| s[0].first_percentage()),
                state
                    .injection_rotation
                    .map_or(0, |s| s[1].first_percentage()),
                state
                    .injection_rotation
                    .map_or(0, |s| s[2].first_percentage()),
            ];
            assert_eq!(serde_json::json!(first), instance["first"]);
            for _ in 0..2 {
                state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, 0.0, 0.0, &mut random);
            }
            assert_eq!(
                serde_json::json!(random.words()),
                instance["constructionState"]
            );
            let mut displacement = [0.0; 3];
            for row in instance["steps"].as_array().unwrap() {
                let n = |name| f32::from_bits(row[name].as_u64().unwrap() as u32);
                env.advance_motion(
                    0,
                    state,
                    &p,
                    [n("age"), n("total")],
                    n("delta"),
                    input,
                    crt,
                    &mut displacement,
                    &mut random,
                )
                .unwrap();
                assert_eq!(
                    serde_json::json!(displacement.map(f32::to_bits)),
                    row["offset"]
                );
                assert_eq!(serde_json::json!(random.words()), row["state"]);
                let pairs = env.pairs.lock().unwrap();
                let flags: [u8; 4] = std::array::from_fn(|i| {
                    pairs
                        .get(&(0, 1 + i as u8))
                        .copied()
                        .unwrap_or_default()
                        .dispatch_code()
                });
                assert_eq!(serde_json::json!(flags), row["dispatch"]);
                steps += 1;
            }
        }
        steps
    }

    #[test]
    fn vr_numeric_clock_length_gate_and_transpose_match_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/particle-vr-numeric.json")).unwrap();
        for case in cases.as_array().unwrap() {
            assert_eq!(compare_vr_case(case), 12);
        }
    }

    #[test]
    #[ignore = "original base construction and full VR/ARs Numeric with both CRT paths"]
    fn compare_original_particle_vr_numeric() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let native: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("particle-vr-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut steps = 0;
        for (index, case) in native["cases"].as_array().unwrap().iter().enumerate() {
            steps += std::panic::catch_unwind(|| compare_vr_case(case))
                .unwrap_or_else(|_| panic!("original VR case {index}"));
        }
        assert_eq!(steps, 46080);
        std::fs::write(folder.join("particle-vr-rust-comparison.json"),serde_json::to_string_pretty(&serde_json::json!({"definitions":3840,"instances":7680,"numericSteps":steps,"positionComponentsCompared":steps*3,"statesCompared":steps+7680,"differences":0,"scope":"Original full VR3e61a0/ARs helper and base constructor; compiled scalar readers, both real CRT paths. Five curve profiles, eight modes, empty defaults0/4, four velocities including zero/underflow, three controlled bases and LoDr0/1; two instances share dispatch, positive/negative/zero deltas. Birth velocity/basis, parent Color/Common/geometry controlled. Core comparison, not production VR integration, shape/provider or GPU."})).unwrap()+"\n").unwrap();
    }

    #[test]
    #[ignore = "complete original particle/ModelSkin constructor First, XYZ/color and shared stream"]
    fn compare_original_particle_and_modelskin_constructors() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let data: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("particle-construction-client-probe.json")).unwrap(),
        )
        .unwrap();
        let mut components = 0;
        let mut states = 0;
        for (ordinal, case) in data["cases"].as_array().unwrap().iter().enumerate() {
            let (c, n) = compare_case(case, ordinal);
            components += c;
            states += n;
        }
        assert_eq!(states, 2048);
        std::fs::write(folder.join("particle-construction-rust-comparison.json"),serde_json::to_string_pretty(&serde_json::json!({"cases":512,"statesCompared":states,"xyzComponentsCompared":components,"firstBytesCompared":512*40+256*10,"differences":0,"scope":"Actual complete 3e0f10 and 400430 construction flow versus common particle First + XYZ/Col and derived ModelSkin First/properties. Common initialization, geometry, QPC/resource allocator controlled; zero/two UV sets, empty ModelSkin derived curves; native XYZ animated factory/provider not proven. Three same-age property repeats per case; no GPU."})).unwrap()+"\n").unwrap();
    }
    fn texture_fixture(case: &serde_json::Value) -> AvfxParticle {
        let profile = case["profile"].as_u64().unwrap() as usize;
        let mode = case["mode"].as_u64().unwrap() as u32;
        let curve = |count: usize, base: f32, random_type| AvfxCurve {
            random_type,
            keys: (0..count)
                .map(|i| AvfxCurveKey {
                    time: (i * 10) as i16,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: base + 0.125 * i as f32,
                })
                .collect(),
            ..Default::default()
        };
        let source = case["source"].as_u64().unwrap();
        let mut particle = AvfxParticle {
            particle_type: case["shape"].as_u64().map(|shape| {
                use crate::avfx::ParticleType;
                match shape {
                    0 => ParticleType::Disc,
                    1 => ParticleType::Laser,
                    2 => ParticleType::Polygon,
                    3 => ParticleType::Windmill,
                    _ => panic!("unknown original texture sequence"),
                }
            }),
            texture_color1: Some(AvfxParticleTexture {
                enabled: source != 4,
                use_screen_copy: source == 1 || source == 2,
                previous_frame_copy: source == 2,
                use_chara_portrait: source == 3,
                texture_list: vec![0, 1, 255, 254],
                tex_n: Some(curve(profile / 3, 1.25, 0)),
                tex_n_random: Some(curve(profile % 3, 0.5, mode)),
                ..Default::default()
            }),
            texture_palette: Some(AvfxParticleTexturePalette {
                enabled: case["palette"] == true,
                texture_index: -1,
                offset: curve(profile / 3, -0.5, 0),
                offset_random: curve(profile % 3, 0.25, mode),
                ..Default::default()
            }),
            ..Default::default()
        };
        if case["geometry"] == 9 {
            use crate::avfx::{
                AvfxParticleData, AvfxParticleDataDisc, AvfxParticleDataLaser,
                AvfxParticleDataPolygon,
            };
            particle.data = match case["shape"].as_u64() {
                Some(0) => AvfxParticleData::Disc(AvfxParticleDataDisc {
                    parts_count: 1,
                    parts_count_u: 2,
                    parts_count_v: 2,
                    scaling_scale: 100,
                    angle: curve(1, 1.0, 0),
                    ..Default::default()
                }),
                Some(1) => AvfxParticleData::Laser(AvfxParticleDataLaser {
                    length: curve(1, 1.0, 0),
                    width: curve(1, 1.0, 0),
                    ..Default::default()
                }),
                Some(2) => AvfxParticleData::Polygon(AvfxParticleDataPolygon {
                    count: curve(1, 3.0, 0),
                    ..Default::default()
                }),
                _ => particle.data,
            };
        }
        particle
    }
    fn compare_texture_case(case: &serde_json::Value) -> usize {
        let p = texture_fixture(case);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let mut samples = 0;
        for instance in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            assert_eq!(
                serde_json::json!([
                    state.texture_index.first_percentage(),
                    state.palette_offset.first_percentage()
                ]),
                instance["first"]
            );
            for pass in 0..2 {
                state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, 0.0, 0.0, &mut random);
                // Renewed neutral-geometry captures include full derived
                // construction. The older texture-only probe intentionally
                // executes just the base constructor (except Windmill).
                if pass == 0 && case["geometry"].is_number() {
                    state.construct_derived(&p, defaults, &mut random);
                }
            }
            assert_eq!(
                serde_json::json!(random.words()),
                instance["constructionState"]
            );
            for row in instance["steps"].as_array().unwrap() {
                let ages =
                    ["age", "total"].map(|key| f32::from_bits(row[key].as_u64().unwrap() as u32));
                let value = env.textures(0, state, &p, ages, &mut random);
                assert_eq!(
                    serde_json::json!(value.texture_index.is_some()),
                    instance["tc1Callback"]
                );
                assert_eq!(
                    serde_json::json!(value.palette_offset.is_some()),
                    instance["tpCallback"]
                );
                assert_eq!(
                    serde_json::json!(value.texture_index.unwrap_or(-1)),
                    row["texture"],
                    "{case}"
                );
                assert_eq!(
                    serde_json::json!(value.palette_offset.unwrap_or(0.0).to_bits()),
                    serde_json::json!(
                        (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits()
                    ),
                    "{case}"
                );
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                samples += 1;
            }
        }
        samples
    }
    #[test]
    fn texture_first_shared_pairs_and_builtin_gates_match_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/particle-textures.json")).unwrap();
        assert_eq!(
            cases
                .as_array()
                .unwrap()
                .iter()
                .map(compare_texture_case)
                .sum::<usize>(),
            96
        );
    }
    #[test]
    #[ignore = "requires scripts/probe-particle-textures.py original CPU capture"]
    fn compare_original_particle_texture_callbacks() {
        let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/weapon-vfx-audit");
        let capture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(output.join("particle-textures-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = capture["cases"].as_array().unwrap();
        let steps: usize = cases.iter().map(compare_texture_case).sum();
        assert_eq!((cases.len(), steps), (1440, 17280));
        std::fs::write(output.join("particle-textures-rust-comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({"definitions":cases.len(),"instances":cases.len()*2,"drawCallbacks":steps,"textureIndexesCompared":steps,"paletteBytesCompared":steps,"statesCompared":steps+cases.len()*2,"differences":0,"scope":"Full original base construction, TC1 selector and TP callback; empty parent Color and second XYZ/Col callback controlled; original scalar compilation, shared pair dispatch and RNG. Common/geometry/texture resource lookup controlled; no full Draw/factory/provider or GPU."})).unwrap()).unwrap();
    }

    fn laser_fixture(case: &serde_json::Value) -> AvfxParticle {
        let mut p = texture_fixture(case);
        p.particle_type = Some(crate::avfx::ParticleType::Laser);
        let mode = case["mode"].as_u64().unwrap() as u32;
        let profile = case["geometry"].as_u64().unwrap() as usize;
        let curve = |n: usize, base: f32, random_type| AvfxCurve {
            random_type,
            keys: (0..n)
                .map(|i| AvfxCurveKey {
                    time: (i * 10) as i16,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: base + 0.125 * i as f32,
                })
                .collect(),
            ..Default::default()
        };
        let w = if profile == 9 { 3 } else { (profile + 4) % 9 };
        let l = if profile == 9 { 3 } else { profile };
        p.data = crate::avfx::AvfxParticleData::Laser(crate::avfx::AvfxParticleDataLaser {
            length: curve(l / 3, if profile == 9 { 1.0 } else { 2.0 }, 0),
            length_random: curve(l % 3, 0.75, mode),
            width: curve(w / 3, 1.0, 0),
            width_random: curve(w % 3, -0.25, (mode + 3) & 7),
            ..Default::default()
        });
        if case["activeXYZ"] == true {
            let vectors: [_; 3] = std::array::from_fn(|index| AvfxCurve3Axis {
                x: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                y: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                z: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                random_x: Some(curve(1, 0.125, mode + index as u32 + 3)),
                random_y: Some(curve(1, 0.25, mode + index as u32 + 4)),
                random_z: Some(curve(1, 0.375, mode + index as u32 + 5)),
                ..Default::default()
            });
            [p.scale, p.rotation, p.position] = vectors;
        }
        p
    }
    fn compare_laser_case(case: &serde_json::Value) -> usize {
        let p = laser_fixture(case);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let mut rows = 0;
        for instance in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            state.construct_derived(&p, defaults, &mut random);
            assert_eq!(
                serde_json::json!(
                    state
                        .laser
                        .unwrap()
                        .map(VfxClientScalarCurveState::first_percentage)
                ),
                instance["derivedFirst"],
                "{case}"
            );
            assert_eq!(
                serde_json::json!([
                    state.texture_index.first_percentage(),
                    state.palette_offset.first_percentage()
                ]),
                instance["first"]
            );
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            assert_eq!(
                serde_json::json!(
                    state
                        .cache
                        .into_iter()
                        .flatten()
                        .map(f32::to_bits)
                        .collect::<Vec<_>>()
                ),
                instance["xyz"],
                "{case}"
            );
            assert_eq!(
                serde_json::json!(random.words()),
                instance["constructionState"],
                "{case}"
            );
            for row in instance["steps"].as_array().unwrap() {
                let ages =
                    ["age", "total"].map(|k| f32::from_bits(row[k].as_u64().unwrap() as u32));
                let values = env.textures(0, state, &p, ages, &mut random);
                assert_eq!(
                    serde_json::json!(values.laser_dimensions.unwrap().map(f32::to_bits)),
                    row["dimensions"],
                    "{case}"
                );
                assert_eq!(
                    serde_json::json!(values.texture_index.unwrap_or(-1)),
                    row["texture"],
                    "{case}"
                );
                assert_eq!(
                    values.palette_offset.unwrap_or(0.0).to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits(),
                    "{case}"
                );
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                rows += 1;
            }
        }
        rows
    }
    #[test]
    fn laser_geometry_before_textures_matches_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/laser-geometry-draw.json")).unwrap();
        assert_eq!(
            cases
                .as_array()
                .unwrap()
                .iter()
                .map(compare_laser_case)
                .sum::<usize>(),
            192
        );
    }
    #[test]
    #[ignore = "requires scripts/probe-laser-geometry-draw.py original CPU capture"]
    fn compare_original_laser_geometry_draw() {
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let capture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(output.join("laser-geometry-draw-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = capture["cases"].as_array().unwrap();
        let rows = cases.iter().map(compare_laser_case).sum::<usize>();
        assert_eq!(cases.len(), 14400);
        assert_eq!(rows, 172800);
        std::fs::write(output.join("laser-geometry-draw-rust-comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "definitions": cases.len(), "instances": cases.len()*2, "drawStatesCompared": rows,
            "dimensionComponentsCompared": rows*2, "statesCompared": rows+cases.len()*2,
            "differences": 0,
            "scope": "Complete original Laser constructor, width then length then TC1/TD/TP Draw slice, compiled scalar readers and shared RNG/pairs across two instances. Eight modes, empty descriptors0/4, independent 0-2 key geometry/texture pairs, repeated/negative clocks; active XYZ cases verify derived First between base and second property refresh. Common, transform providers, fade and resources controlled, TD disabled/no-op. Not complete Draw, live providers or GPU."
        })).unwrap()).unwrap();
    }

    pub(crate) fn disc_polygon_fixture(case: &serde_json::Value) -> AvfxParticle {
        use crate::avfx::{
            AvfxColorCurve, AvfxColorScaleRgb, AvfxParticleData, AvfxParticleDataDisc,
            AvfxParticleDataPolygon, ParticleType,
        };
        let mut texture_case = case.clone();
        texture_case["shape"] = serde_json::Value::Null;
        texture_case["geometry"] = serde_json::Value::Null;
        let mut p = texture_fixture(&texture_case);
        p.collision_type = -1;
        let shape = case["shape"].as_u64().unwrap();
        let mode = case["mode"].as_u64().unwrap() as u32;
        let profile = case["geometry"].as_u64().unwrap() as usize;
        let curve = |count: usize, base: f32, mode| AvfxCurve {
            random_type: mode,
            keys: (0..count)
                .map(|i| AvfxCurveKey {
                    time: (i * 10) as i16,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: base + 0.125 * i as f32,
                })
                .collect(),
            ..Default::default()
        };
        let pair = |axis: usize| {
            let profile = if profile == 9 {
                3
            } else {
                (profile + axis) % 9
            };
            let main = if shape == 0 {
                if axis == 0 { 1.0 } else { 0.125 * axis as f32 }
            } else {
                4.0
            };
            let main = if case["geometry"] == 9 {
                if shape == 0 {
                    if axis == 0 { 1.0 } else { 0.0 }
                } else {
                    3.0
                }
            } else {
                main
            };
            (
                curve(profile / 3, main, 0),
                curve(
                    profile % 3,
                    if axis % 2 == 0 { 0.5 } else { -0.25 },
                    (mode + axis as u32) & 7,
                ),
            )
        };
        let edge_color = |edge: u32| {
            let variant = case["color"].as_u64().unwrap();
            if variant == 0 {
                return AvfxColorCurve::default();
            }
            let count = if variant == 2 { 2 } else { 1 };
            let field = |slot: u32| {
                let random = slot >= 6 && slot != 10;
                let value = if random {
                    if variant == 3 {
                        0.0
                    } else if edge == 0 {
                        0.125
                    } else {
                        -0.125
                    }
                } else if slot == 0 {
                    0.5
                } else if slot == 1 {
                    0.75
                } else {
                    1.0
                };
                let mut c = curve(
                    count,
                    value,
                    if random {
                        (mode + edge * 2 + slot) & 7
                    } else {
                        0
                    },
                );
                if slot == 0 {
                    for k in &mut c.keys {
                        k.interpolation = 17;
                        k.x = k.z;
                        k.y = k.z;
                    }
                }
                c
            };
            AvfxColorCurve {
                rgb: Some(field(0)),
                alpha: Some(field(1)),
                brightness: Some(field(10)),
                scale_alpha: Some(field(5)),
                scale_rgb: Some(AvfxColorScaleRgb {
                    r: Some(field(2)),
                    g: Some(field(3)),
                    b: Some(field(4)),
                }),
                random: [6, 7, 8, 9, 11].map(|slot| Some(field(slot))),
            }
        };
        if shape == 0 {
            let pairs: [_; 9] = std::array::from_fn(pair);
            p.particle_type = Some(ParticleType::Disc);
            p.data = AvfxParticleData::Disc(AvfxParticleDataDisc {
                parts_count: 1,
                parts_count_u: 2,
                parts_count_v: 2,
                scaling_scale: 100,
                angle: pairs[0].0.clone(),
                angle_random: pairs[0].1.clone(),
                height_begin_inner: pairs[1].0.clone(),
                height_begin_inner_random: pairs[1].1.clone(),
                height_end_inner: pairs[2].0.clone(),
                height_end_inner_random: pairs[2].1.clone(),
                height_begin_outer: pairs[3].0.clone(),
                height_begin_outer_random: pairs[3].1.clone(),
                height_end_outer: pairs[4].0.clone(),
                height_end_outer_random: pairs[4].1.clone(),
                width_begin: pairs[5].0.clone(),
                width_begin_random: pairs[5].1.clone(),
                width_end: pairs[6].0.clone(),
                width_end_random: pairs[6].1.clone(),
                radius_begin: pairs[7].0.clone(),
                radius_begin_random: pairs[7].1.clone(),
                radius_end: pairs[8].0.clone(),
                radius_end_random: pairs[8].1.clone(),
                color_edge_inner: edge_color(0),
                color_edge_outer: edge_color(1),
                ..Default::default()
            });
        } else {
            let (count, count_random) = pair(0);
            p.particle_type = Some(ParticleType::Polygon);
            p.data = AvfxParticleData::Polygon(AvfxParticleDataPolygon {
                count,
                count_random,
                ..Default::default()
            });
        }
        if case["activeXYZ"] == true {
            let vectors: [_; 3] = std::array::from_fn(|index| AvfxCurve3Axis {
                x: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                y: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                z: Some(curve(1, if index == 0 { 1.0 } else { 0.0 }, 0)),
                random_x: Some(curve(1, 0.125, mode + index as u32 + 3)),
                random_y: Some(curve(1, 0.25, mode + index as u32 + 4)),
                random_z: Some(curve(1, 0.375, mode + index as u32 + 5)),
                ..Default::default()
            });
            [p.scale, p.rotation, p.position] = vectors;
        }
        p
    }
    pub(crate) fn line_fixture(case: &serde_json::Value) -> AvfxParticle {
        use crate::avfx::{AvfxParticleData, AvfxParticleDataLine, ParticleType};
        let mut shared = case.clone();
        shared["shape"] = 0.into();
        shared["profile"] = 4.into();
        shared["palette"] = true.into();
        let mut p = disc_polygon_fixture(&shared);
        let AvfxParticleData::Disc(colors) = p.data else {
            unreachable!()
        };
        let geometry = case["geometry"].as_u64().unwrap() as usize;
        let curve = |count: usize, base, random_type| AvfxCurve {
            random_type,
            keys: (0..count)
                .map(|i| AvfxCurveKey {
                    time: (i * 10) as i16,
                    interpolation: 1,
                    x: 0.0,
                    y: 0.0,
                    z: base + 0.125 * i as f32,
                })
                .collect(),
            ..Default::default()
        };
        p.particle_type = Some(ParticleType::Line);
        p.simple_anim_enable = case["simple"].as_bool().unwrap();
        p.data = AvfxParticleData::Line(AvfxParticleDataLine {
            line_count: 1,
            length: curve(geometry / 3, 2.0, 0),
            length_random: curve(geometry % 3, 0.75, case["mode"].as_u64().unwrap() as u32),
            color_begin: colors.color_edge_inner,
            color_end: colors.color_edge_outer,
            ..Default::default()
        });
        p
    }

    pub(crate) fn polyline_fixture(case: &serde_json::Value) -> AvfxParticle {
        use crate::avfx::{AvfxParticleData, AvfxParticleDataPolyline, ParticleType};
        let mut shared = case.clone();
        shared["simple"] = false.into();
        let mut p = line_fixture(&shared);
        let AvfxParticleData::Line(line) = &p.data else {
            unreachable!()
        };
        let colors: [_; 6] = std::array::from_fn(|i| {
            let mut color = if i % 2 == 0 {
                line.color_begin.clone()
            } else {
                line.color_end.clone()
            };
            for curve in color.random.iter_mut().flatten() {
                curve.random_type = (curve.random_type + 2 * (i - i % 2) as u32) & 7;
            }
            color
        });
        let profile = case["geometry"].as_u64().unwrap() as usize;
        let mode = case["mode"].as_u64().unwrap() as u32;
        let pairs: [_; 7] = std::array::from_fn(|axis| {
            let profile = (profile + axis) % 9;
            let curve = |count: usize, base, random_type| AvfxCurve {
                random_type,
                keys: (0..count)
                    .map(|i| AvfxCurveKey {
                        time: (i * 10) as i16,
                        interpolation: 1,
                        x: 0.0,
                        y: 0.0,
                        z: base + 0.125 * i as f32,
                    })
                    .collect(),
                ..Default::default()
            };
            (
                curve(
                    profile / 3,
                    if axis == 5 {
                        2.0
                    } else if axis == 0 || axis == 6 {
                        0.0
                    } else {
                        1.0
                    },
                    0,
                ),
                curve(
                    profile % 3,
                    if axis % 2 == 0 { 0.5 } else { -0.25 },
                    (mode + axis as u32) & 7,
                ),
            )
        });
        let [
            color_begin,
            color_center,
            color_end,
            color_edge_begin,
            color_edge_center,
            color_edge_end,
        ] = colors;
        p.particle_type = Some(ParticleType::Polyline);
        p.rotation_direction_base = 0;
        p.data = AvfxParticleData::Polyline(AvfxParticleDataPolyline {
            create_line_type: 1,
            point_count: 4,
            point_count_center: case["center"].as_i64().unwrap() as i32,
            use_edge: case["edge"].as_bool().unwrap(),
            cf: pairs[0].0.clone(),
            cf_random: pairs[0].1.clone(),
            width: pairs[1].0.clone(),
            width_random: pairs[1].1.clone(),
            width_begin: pairs[2].0.clone(),
            width_begin_random: pairs[2].1.clone(),
            width_center: pairs[3].0.clone(),
            width_center_random: pairs[3].1.clone(),
            width_end: pairs[4].0.clone(),
            width_end_random: pairs[4].1.clone(),
            length: pairs[5].0.clone(),
            length_random: pairs[5].1.clone(),
            softness: pairs[6].0.clone(),
            softness_random: pairs[6].1.clone(),
            color_begin,
            color_center,
            color_end,
            color_edge_begin,
            color_edge_center,
            color_edge_end,
            ..Default::default()
        });
        p
    }

    fn compare_polyline_case(case: &serde_json::Value) -> usize {
        let p = polyline_fixture(case);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let bits = |colors: [[f32; 4]; 6]| {
            serde_json::json!(
                colors
                    .into_iter()
                    .flatten()
                    .map(f32::to_bits)
                    .collect::<Vec<_>>()
            )
        };
        let mut rows = 0;
        for instance in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            state.construct_derived(&p, defaults, &mut random);
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            let polyline = state.polyline.unwrap();
            let mut first: Vec<_> = polyline
                .colors
                .into_iter()
                .flat_map(|c| c.first_percentages())
                .collect();
            first.extend(polyline.scalars.map(|s| s.first_percentage()));
            assert_eq!(serde_json::json!(first), instance["derivedFirst"], "{case}");
            assert_eq!(bits(polyline.cache), instance["initialColors"], "{case}");
            assert_eq!(
                serde_json::json!(random.words()),
                instance["constructionState"],
                "{case}"
            );
            for warm in instance["prewarm"].as_array().unwrap() {
                let age = warm["age"].as_u64().unwrap() as f32;
                state.evaluate(&p, [age; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, age, age, &mut random);
                assert_eq!(
                    bits(state.polyline.unwrap().cache),
                    warm["colors"],
                    "{case}"
                );
                assert_eq!(serde_json::json!(random.words()), warm["state"], "{case}");
            }
            for row in instance["steps"].as_array().unwrap() {
                let read = |key| f32::from_bits(row[key].as_u64().unwrap() as u32);
                let ages = [read("propertyAge"), read("propertyTotal")];
                state.evaluate_shape_properties(&p, ages, &mut random);
                state.evaluate(&p, ages, [0.0; 3], &mut random);
                color.evaluate(&p.color, ages[0], ages[1], &mut random);
                assert_eq!(bits(state.polyline.unwrap().cache), row["colors"], "{case}");
                assert_eq!(
                    serde_json::json!(random.words()),
                    row["propertyState"],
                    "{case}"
                );
                let draw = env.textures(0, state, &p, [read("age"), read("total")], &mut random);
                let polyline = draw.polyline_geometry.unwrap();
                assert_eq!(
                    serde_json::json!(polyline.values.map(f32::to_bits)),
                    row["geometryValues"],
                    "{case}"
                );
                assert_eq!(bits(polyline.colors), row["colors"]);
                assert_eq!(
                    serde_json::json!(draw.texture_index.unwrap_or(-1)),
                    row["texture"]
                );
                assert_eq!(
                    serde_json::json!((draw.palette_offset.unwrap() * 255.0) as u8),
                    row["paletteByte"]
                );
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                assert_eq!(row["submissions"], 1);
                rows += 1;
            }
        }
        rows
    }

    #[test]
    fn polyline_constructor_properties_and_draw_callbacks_match_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/polyline-geometry.json")).unwrap();
        assert_eq!(
            cases
                .as_array()
                .unwrap()
                .iter()
                .map(compare_polyline_case)
                .sum::<usize>(),
            4608
        );
    }

    #[test]
    #[ignore = "requires scripts/probe-polyline-geometry.py original CPU capture"]
    fn compare_original_polyline_constructor_properties_and_draw_callbacks() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/weapon-vfx-audit");
        let capture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out.join("polyline-geometry-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = capture["cases"].as_array().unwrap();
        let rows = cases.iter().map(compare_polyline_case).sum::<usize>();
        assert_eq!((cases.len(), rows), (13824, 165888));
        std::fs::write(out.join("polyline-geometry-rust-comparison.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "definitions":13824,"instances":27648,"drawStatesCompared":rows,"geometryComponentsCompared":rows*8,"propertyStatesCompared":rows,"prewarmStatesCompared":41472,"statesCompared":rows*2+27648+41472,"differences":0,
            "scope":"Original complete Polyline constructor, properties/prewarm and whole Draw entry, six Color First groups then seven scalar First values; conditional center/edge colors; Len/CF/Sft/PnDs then Wd/WdB/WdE then TC1/TD/TP then valid-center WdC. Original bytes unmodified; Common, successful allocation, UV, transform/position and geometry submission controlled. No original geometry writers/history perturbation, provider/LOD/resource budget/failure, GPU or client pixels."
        })).unwrap()).unwrap();
    }

    fn compare_line_case(case: &serde_json::Value) -> usize {
        let p = line_fixture(case);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let bits = |colors: [[f32; 4]; 2]| {
            serde_json::json!(
                colors
                    .into_iter()
                    .flatten()
                    .map(f32::to_bits)
                    .collect::<Vec<_>>()
            )
        };
        let mut rows = 0;
        for instance in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            state.construct_derived(&p, defaults, &mut random);
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            let line = state.line.unwrap();
            let mut first = vec![line.length.first_percentage()];
            first.extend(line.colors.into_iter().flat_map(|c| c.first_percentages()));
            assert_eq!(serde_json::json!(first), instance["derivedFirst"], "{case}");
            assert_eq!(
                serde_json::json!([
                    state.texture_index.first_percentage(),
                    state.palette_offset.first_percentage()
                ]),
                instance["first"]
            );
            assert_eq!(bits(line.cache), instance["initialColors"], "{case}");
            assert_eq!(
                serde_json::json!(random.words()),
                instance["constructionState"],
                "{case}"
            );
            for warm in instance["prewarm"].as_array().unwrap() {
                let age = warm["age"].as_u64().unwrap() as f32;
                state.evaluate(&p, [age; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, age, age, &mut random);
                assert_eq!(bits(state.line.unwrap().cache), warm["colors"], "{case}");
                assert_eq!(serde_json::json!(random.words()), warm["state"], "{case}");
            }
            for row in instance["steps"].as_array().unwrap() {
                let read = |key| f32::from_bits(row[key].as_u64().unwrap() as u32);
                let ages = [read("propertyAge"), read("propertyTotal")];
                state.evaluate_shape_properties(&p, ages, &mut random);
                state.evaluate(&p, ages, [0.0; 3], &mut random);
                color.evaluate(&p.color, ages[0], ages[1], &mut random);
                assert_eq!(bits(state.line.unwrap().cache), row["colors"], "{case}");
                assert_eq!(
                    serde_json::json!(random.words()),
                    row["propertyState"],
                    "{case}"
                );
                let draw = env.textures(0, state, &p, [read("age"), read("total")], &mut random);
                let geometry = draw.line_geometry.unwrap();
                assert_eq!(
                    serde_json::json!(geometry.length.map(f32::to_bits)),
                    row["length"],
                    "{case}"
                );
                assert_eq!(bits(geometry.colors), row["colors"], "{case}");
                assert_eq!(draw.texture_index, None);
                assert_eq!(draw.palette_offset, None);
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                rows += 1;
            }
        }
        rows
    }

    #[test]
    fn line_constructor_properties_and_length_match_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/line-geometry.json")).unwrap();
        assert_eq!(
            cases
                .as_array()
                .unwrap()
                .iter()
                .map(compare_line_case)
                .sum::<usize>(),
            1536
        );
    }

    #[test]
    #[ignore = "requires scripts/probe-line-geometry.py original CPU capture"]
    fn compare_original_line_constructor_properties_and_length() {
        let out = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/weapon-vfx-audit");
        let capture: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out.join("line-geometry-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = capture["cases"].as_array().unwrap();
        let rows = cases.iter().map(compare_line_case).sum::<usize>();
        assert_eq!((cases.len(), rows), (4608, 55296));
        std::fs::write(out.join("line-geometry-rust-comparison.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "definitions":4608,"instances":9216,"drawStatesCompared":rows,"lengthsCompared":27648,"propertyStatesCompared":55296,"prewarmStatesCompared":13824,"statesCompared":133632,"differences":0,
            "scope":"Original complete ordinary/Smpl Line construction with successful pool/controlled Smpl heap, original endpoint-color properties and prewarm wrapper, ordinary length Draw slice. Two instances share compiled pairs/stream; eight modes, defaults0/4, 0-2 keys, four color configurations, active XYZ, repeated/negative clocks. Smpl Draw length intentionally skipped; no TC1/TP Draw readers. Common, providers, Smpl motion/birth/slots, render buffers and resource handles controlled; not full Draw, allocation failure, full Smpl simulation, UV, GPU or client pixels."
        })).unwrap()).unwrap();
    }

    fn compare_disc_polygon_case(case: &serde_json::Value) -> usize {
        let p = disc_polygon_fixture(case);
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
            empty_rgba: [1.0; 4],
        };
        let env = VfxClientParticleCurveEnvironment::default();
        let mut random =
            VfxClientRandomState::from_words([123456789, 362436069, 521288629, 88675123]);
        VfxClientColorCurveState::construct(&Default::default(), defaults, &mut random);
        let mut rows = 0;
        for instance in case["instances"].as_array().unwrap() {
            let (mut state, color) =
                VfxClientParticleCurveState::construct(&p, defaults, &mut random).unwrap();
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            state.construct_derived(&p, defaults, &mut random);
            let first: Vec<_> = if let Some(disc) = state.disc {
                disc.scalars
                    .map(VfxClientScalarCurveState::first_percentage)
                    .into_iter()
                    .chain(
                        disc.colors
                            .into_iter()
                            .flat_map(VfxClientColorCurveState::first_percentages),
                    )
                    .collect()
            } else {
                vec![state.polygon.unwrap().first_percentage()]
            };
            assert_eq!(serde_json::json!(first), instance["derivedFirst"], "{case}");
            state.evaluate(&p, [0.0; 2], [0.0; 3], &mut random);
            color.evaluate(&p.color, 0.0, 0.0, &mut random);
            assert_eq!(
                serde_json::json!([
                    state.texture_index.first_percentage(),
                    state.palette_offset.first_percentage()
                ]),
                instance["first"]
            );
            assert_eq!(
                serde_json::json!(
                    state
                        .cache
                        .into_iter()
                        .flatten()
                        .map(f32::to_bits)
                        .collect::<Vec<_>>()
                ),
                instance["xyz"],
                "{case}"
            );
            let colors = |state: VfxClientParticleCurveState| {
                state
                    .disc
                    .map(|s| {
                        serde_json::json!(
                            s.cache
                                .into_iter()
                                .flatten()
                                .map(f32::to_bits)
                                .collect::<Vec<_>>()
                        )
                    })
                    .unwrap_or(serde_json::Value::Null)
            };
            assert_eq!(colors(state), instance["initialColors"], "{case}");
            assert_eq!(
                serde_json::json!(random.words()),
                instance["constructionState"],
                "{case}"
            );
            for warm in instance["prewarm"].as_array().unwrap() {
                let age = warm["age"].as_u64().unwrap() as f32;
                state.evaluate(&p, [age; 2], [0.0; 3], &mut random);
                color.evaluate(&p.color, age, age, &mut random);
                assert_eq!(colors(state), warm["colors"]);
                assert_eq!(serde_json::json!(random.words()), warm["state"], "{case}");
            }
            for row in instance["steps"].as_array().unwrap() {
                let ages =
                    |keys: [&str; 2]| keys.map(|k| f32::from_bits(row[k].as_u64().unwrap() as u32));
                let property = ages(["propertyAge", "propertyTotal"]);
                state.evaluate_shape_properties(&p, property, &mut random);
                state.evaluate(&p, property, [0.0; 3], &mut random);
                color.evaluate(&p.color, property[0], property[1], &mut random);
                assert_eq!(colors(state), row["colors"], "{case}");
                assert_eq!(
                    serde_json::json!(random.words()),
                    row["propertyState"],
                    "{case}"
                );
                let values = env.textures(0, state, &p, ages(["age", "total"]), &mut random);
                let geometry = values
                    .disc_geometry
                    .map(|v| serde_json::json!(v.scalars.map(f32::to_bits)))
                    .unwrap_or_else(|| {
                        serde_json::json!([values.polygon_count.unwrap().trunc() as i32])
                    });
                assert_eq!(geometry, row["geometryValues"], "{case}");
                assert_eq!(
                    serde_json::json!(values.texture_index.unwrap_or(-1)),
                    row["texture"],
                    "{case}"
                );
                assert_eq!(
                    values.palette_offset.unwrap_or(0.0).to_bits(),
                    (row["paletteByte"].as_u64().unwrap() as f32 / 255.0).to_bits(),
                    "{case}"
                );
                assert_eq!(serde_json::json!(random.words()), row["state"], "{case}");
                rows += 1;
            }
        }
        rows
    }
    #[test]
    fn disc_polygon_properties_geometry_and_textures_match_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/disc-polygon-geometry.json")).unwrap();
        assert_eq!(
            cases
                .as_array()
                .unwrap()
                .iter()
                .map(compare_disc_polygon_case)
                .sum::<usize>(),
            960
        );
    }
    // Consume one definition at a time: the complete capture is hundreds of MB,
    // but a single definition contains all state needed for its comparisons.
    fn compare_streamed_capture(
        path: &std::path::Path,
        compare: fn(&serde_json::Value) -> usize,
    ) -> (usize, usize) {
        use serde::Deserializer;
        use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
        struct Rows(fn(&serde_json::Value) -> usize);
        impl<'de> DeserializeSeed<'de> for Rows {
            type Value = (usize, usize);
            fn deserialize<D: serde::Deserializer<'de>>(
                self,
                d: D,
            ) -> Result<Self::Value, D::Error> {
                d.deserialize_seq(self)
            }
        }
        impl<'de> Visitor<'de> for Rows {
            type Value = (usize, usize);
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("original definitions")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut counts = (0, 0);
                while let Some(case) = a.next_element::<serde_json::Value>()? {
                    counts.0 += 1;
                    counts.1 += (self.0)(&case);
                }
                Ok(counts)
            }
        }
        struct Capture(fn(&serde_json::Value) -> usize);
        impl<'de> Visitor<'de> for Capture {
            type Value = (usize, usize);
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("original capture")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut counts = None;
                while let Some(key) = a.next_key::<String>()? {
                    if key == "cases" {
                        counts = Some(a.next_value_seed(Rows(self.0))?);
                    } else {
                        a.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                counts.ok_or_else(|| serde::de::Error::missing_field("cases"))
            }
        }
        let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        serde_json::Deserializer::from_reader(file)
            .deserialize_map(Capture(compare))
            .unwrap()
    }
    #[test]
    #[ignore = "requires scripts/probe-disc-polygon-geometry.py original CPU capture"]
    fn compare_original_disc_polygon_geometry() {
        let output =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
        let (definitions, rows) = compare_streamed_capture(
            &output.join("disc-polygon-geometry-client-probe.json"),
            compare_disc_polygon_case,
        );
        assert_eq!((definitions, rows), (72000, 864000));
        std::fs::write(output.join("disc-polygon-geometry-rust-comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "definitions": definitions, "instances": definitions*2, "drawStatesCompared": rows,
            "geometryComponentsCompared": 6393600, "propertyStatesCompared": rows,
            "statesCompared": rows*2+definitions*2+34560,
            "differences": 0,
            "scope": "Original complete Disc/Polygon constructors with successful pool, zero UV sets, Common ordinary branch; Disc Color First/initial evaluation, original ordinary properties and prewarm wrapper, unchanged geometry->TC1/TD/TP Draw slices and actual scalar/Color compilers/readers. Shared pairs and stream across two instances. Eight modes, empty defaults0/4, independent 0-2 key geometry/texture pairs, four Disc Color configurations, repeated/negative clocks, active XYZ, retained edge cache during prewarm. Common, transform/provider callbacks, fade and handles controlled; UV callback controlled, TD disabled. Not allocation failure, complete Draw, live providers or GPU."
        })).unwrap()).unwrap();
    }

    #[test]
    fn four_shape_texture_sequences_match_original_capture() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/shape-texture-draw.json")).unwrap();
        assert_eq!(
            cases
                .as_array()
                .unwrap()
                .iter()
                .map(compare_texture_case)
                .sum::<usize>(),
            384
        );
    }

    #[test]
    #[ignore = "requires scripts/probe-shape-texture-draw.py original CPU capture"]
    fn compare_original_four_shape_texture_sequences() {
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/weapon-vfx-audit");
        let native: serde_json::Value = serde_json::from_slice(
            &std::fs::read(folder.join("shape-texture-draw-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = native["cases"].as_array().unwrap();
        let samples: usize = cases.iter().map(compare_texture_case).sum();
        assert_eq!((cases.len(), samples), (5760, 69120));
        std::fs::write(folder.join("shape-texture-draw-rust-comparison.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "definitions": cases.len(), "instances": cases.len()*2,
            "drawStatesCompared": samples, "statesCompared": samples+cases.len()*2, "differences": 0,
            "scope": "Actual original TC1-TD-TP instruction sequences from Disc, Laser, Polygon and Windmill Draw, full base and Windmill constructors and actual resource-binding TC1/TP byte callbacks. ABI shim supplies registers/buffers and returns immediately after the sequence; original sequence bytes preserved. Eight modes, empty defaults0/4, five sources, TP switches and 0-2 key pairs, repeated/negative ages; shared pair across two instances. Preceding shape geometry/property phases, Common and texture resources controlled; not complete Disc/Laser/Polygon derived constructors, full Draw, live providers or GPU. TD disabled/no-op; enabled TD uniforms were not compared."
        })).unwrap()).unwrap();
    }
}
