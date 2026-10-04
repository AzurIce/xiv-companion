use super::{VfxClientRandomState, VfxClientScalarCurveState, scalar_random};
use crate::avfx::AvfxColorCurve;

/// Shared startup descriptors must be supplied by the compiled environment.
/// No Default: controlled probe values are not proof of live startup values.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VfxClientColorCurveDefaults {
    pub empty_random_type: u32,
    pub empty_rgba: [f32; 4],
}

/// Constructor-owned First bytes for RanR/G/B/A/RBri. The compiled Color
/// resource has no scalar-pair lazy cache. Keep the resource unchanged while
/// this state is alive and share the stream with the caller's other consumers.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VfxClientColorCurveState {
    random_types: [u8; 5],
    first: [i8; 5],
    empty_rgba: [f32; 4],
}

impl VfxClientColorCurveState {
    /// Original 39be90 invokes all five scalar First constructors, including
    /// disabled channels. Missing/empty curves use the shared empty RanT.
    pub fn construct(
        color: &AvfxColorCurve,
        defaults: VfxClientColorCurveDefaults,
        random: &mut VfxClientRandomState,
    ) -> Self {
        let random_types = std::array::from_fn(|i| {
            (color.random[i]
                .as_ref()
                .filter(|curve| !curve.keys.is_empty())
                .map_or(defaults.empty_random_type, |curve| curve.random_type)
                & 7) as u8
        });
        let first = random_types.map(|mode| {
            VfxClientScalarCurveState::construct(u32::from(mode), random).first_percentage()
        });
        Self {
            random_types,
            first,
            empty_rgba: defaults.empty_rgba,
        }
    }

    pub fn first_percentages(self) -> [i8; 5] {
        self.first
    }

    /// Original full Color dispatch: active Always channels draw on every
    /// call, in R/G/B/A then brightness order, even at the same age or zero
    /// sampled amplitude. Single zero and empty channels consume no draw.
    pub fn evaluate(
        self,
        color: &AvfxColorCurve,
        local: f32,
        total: f32,
        random: &mut VfxClientRandomState,
    ) -> [f32; 4] {
        color.compose_at(local, total, true, self.empty_rgba, |i, curve| {
            let mode = u32::from(self.random_types[i]);
            if mode >= 6 {
                return None;
            }
            let amplitude = curve.value_at(local, total, 0.0);
            let draw = if mode >= 3 { random.next_u16() } else { 0 };
            Some(scalar_random::cached_offset(
                mode,
                amplitude,
                self.first[i],
                draw,
            ))
        })
    }
}

pub(super) struct VfxClientColorCurveEnvironment {
    pub(super) particle_construction: bool,
    particle_math: super::VfxClientTrigMode,
    defaults: VfxClientColorCurveDefaults,
    random: super::client_random::VfxClientRandomCell,
    model_skin: Option<super::model_skin_curves::VfxClientModelSkinCurveEnvironment>,
    particle: Option<super::particle_curves::VfxClientParticleCurveEnvironment>,
}

impl Clone for VfxClientColorCurveEnvironment {
    fn clone(&self) -> Self {
        Self {
            defaults: self.defaults,
            particle_construction: self.particle_construction,
            particle_math: self.particle_math,
            random: self.random.share(),
            model_skin: self.model_skin.as_ref().map(|env| env.share()),
            particle: self.particle.as_ref().map(|env| env.share()),
        }
    }
}

impl VfxClientColorCurveEnvironment {
    pub(super) fn new(
        defaults: VfxClientColorCurveDefaults,
        random: &super::client_random::VfxClientRandomCell,
    ) -> Self {
        Self {
            defaults,
            particle_construction: false,
            particle_math: super::VfxClientTrigMode::Sse2,
            random: random.share(),
            model_skin: None,
            particle: None,
        }
    }

    pub(super) fn with_model_skin(
        mut self,
        environment: Option<&super::model_skin_curves::VfxClientModelSkinCurveEnvironment>,
    ) -> Self {
        self.model_skin = environment.map(|env| env.share());
        self
    }

    pub(super) fn with_particle_constructor(
        mut self,
        environment: Option<&super::particle_curves::VfxClientParticleCurveEnvironment>,
    ) -> Self {
        self.particle = environment.map(|env| env.share());
        self.particle_construction = environment.is_some();
        self
    }
    pub(super) fn evaluate_particle_gravity(
        &self,
        index: usize,
        state: super::particle_curves::VfxClientParticleCurveState,
        p: &crate::avfx::AvfxParticle,
        ages: [f32; 2],
    ) -> Option<f32> {
        self.random.with_mut(|random| {
            self.particle
                .as_ref()
                .expect("particle environment retained")
                .gravity(index, state, p, ages, random)
        })
    }
    pub(super) fn with_particle_math(mut self, mode: super::VfxClientTrigMode) -> Self {
        self.particle_math = mode;
        self
    }

    pub(super) fn advance_particle_injection(
        &self,
        index: usize,
        state: super::particle_curves::VfxClientParticleCurveState,
        p: &crate::avfx::AvfxParticle,
        ages: [f32; 2],
        delta: f32,
        cache: &super::particle_curves::VfxClientParticleInjectionCache,
    ) -> Result<(), String> {
        self.random.with_mut(|random| {
            cache.advance_motion(
                self.particle
                    .as_ref()
                    .expect("particle environment retained"),
                index,
                state,
                p,
                ages,
                delta,
                self.particle_math,
                random,
            )
        })
    }

    pub(super) fn construct_particle(
        &self,
        particle: &crate::avfx::AvfxParticle,
    ) -> Result<
        (
            super::particle_curves::VfxClientParticleCurveState,
            VfxClientColorCurveState,
        ),
        String,
    > {
        self.random.with_mut(|random| {
            super::particle_curves::VfxClientParticleCurveState::construct(
                particle,
                self.defaults,
                random,
            )
        })
    }
    pub(super) fn construct_particle_derived(
        &self,
        state: &mut super::particle_curves::VfxClientParticleCurveState,
        particle: &crate::avfx::AvfxParticle,
    ) {
        self.random
            .with_mut(|random| state.construct_derived(particle, self.defaults, random));
    }
    pub(super) fn evaluate_particle_shape_properties(
        &self,
        state: &mut super::particle_curves::VfxClientParticleCurveState,
        particle: &crate::avfx::AvfxParticle,
        ages: [f32; 2],
    ) {
        self.random
            .with_mut(|random| state.evaluate_shape_properties(particle, ages, random));
    }
    pub(super) fn evaluate_particle_textures(
        &self,
        index: usize,
        state: super::particle_curves::VfxClientParticleCurveState,
        particle: &crate::avfx::AvfxParticle,
        ages: [f32; 2],
    ) -> Option<super::particle_curves::VfxClientParticleTextureValues> {
        self.particle.as_ref().map(|env| {
            self.random
                .with_mut(|random| env.textures(index, state, particle, ages, random))
        })
    }

    pub(super) fn evaluate_particle(
        &self,
        state: &mut super::particle_curves::VfxClientParticleCurveState,
        particle: &crate::avfx::AvfxParticle,
        ages: [f32; 2],
    ) {
        let fallback = self
            .model_skin
            .as_ref()
            .map_or([0.0; 3], |env| env.defaults.vector_random_fallback);
        self.random
            .with_mut(|random| state.evaluate(particle, ages, fallback, random));
    }

    pub(super) fn construct_model_skin(
        &self,
        data: &crate::avfx::AvfxParticleDataModelSkin,
    ) -> Result<Option<super::model_skin_curves::VfxClientModelSkinCurveState>, String> {
        self.model_skin
            .as_ref()
            .map(|_| {
                self.random.with_mut(|random| {
                    super::model_skin_curves::VfxClientModelSkinCurveState::construct(
                        data,
                        self.defaults,
                        random,
                    )
                })
            })
            .transpose()
    }

    pub(super) fn construct_model_skin_uv(
        &self,
        sets: &[crate::avfx::AvfxUvSet],
    ) -> Result<Option<Vec<super::uv_curves::VfxClientUvCurveState>>, String> {
        if self.model_skin.is_none() {
            return Ok(None);
        }
        if sets.len() > 4
            || sets
                .iter()
                .any(|set| set.scale.axis_connect as u8 > 2 || set.scroll.axis_connect as u8 > 2)
        {
            return Err(
                "ModelSkin UV replay requires at most four sets and valid byte XY ACT".into(),
            );
        }
        self.random.with_mut(|random| {
            sets.iter()
                .map(|set| {
                    super::uv_curves::VfxClientUvCurveState::construct(
                        set,
                        self.defaults.empty_random_type,
                        random,
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some)
        })
    }

    pub(super) fn evaluate_model_skin_uv(
        &self,
        index: usize,
        states: &mut [super::uv_curves::VfxClientUvCurveState],
        sets: &[crate::avfx::AvfxUvSet],
        ages: [f32; 2],
    ) {
        self.random.with_mut(|random| {
            self.model_skin
                .as_ref()
                .expect("retained ModelSkin environment")
                .evaluate_uv(index, states, sets, ages, random)
        });
    }

    pub(super) fn evaluate_model_skin_properties(
        &self,
        state: &mut super::model_skin_curves::VfxClientModelSkinCurveState,
        data: &crate::avfx::AvfxParticleDataModelSkin,
        ages: [f32; 2],
    ) {
        let environment = self
            .model_skin
            .as_ref()
            .expect("retained ModelSkin environment");
        self.random.with_mut(|random| {
            state.evaluate_properties(
                data,
                ages,
                self.defaults.empty_rgba,
                environment.defaults,
                random,
            )
        });
    }

    pub(super) fn evaluate_model_skin_numeric(
        &self,
        index: usize,
        state: &mut super::model_skin_curves::VfxClientModelSkinCurveState,
        data: &crate::avfx::AvfxParticleDataModelSkin,
        ages: [f32; 2],
    ) {
        self.random.with_mut(|random| {
            self.model_skin
                .as_ref()
                .expect("retained ModelSkin environment")
                .evaluate_numeric(index, state, data, ages, self.defaults.empty_rgba, random)
        });
    }

    pub(super) fn construct(&self, color: &AvfxColorCurve) -> VfxClientColorCurveState {
        self.random
            .with_mut(|random| VfxClientColorCurveState::construct(color, self.defaults, random))
    }

    pub(super) fn evaluate(
        &self,
        state: VfxClientColorCurveState,
        color: &AvfxColorCurve,
        local: f32,
        total: f32,
    ) -> [f32; 4] {
        self.random
            .with_mut(|random| state.evaluate(color, local, total, random))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avfx::{AvfxCurve, AvfxCurveKey};

    fn zero_random(animated: bool) -> AvfxCurve {
        let mut keys = vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
            z: -0.0,
        }];
        if animated {
            keys.push(AvfxCurveKey {
                time: 10,
                ..keys[0]
            });
        }
        AvfxCurve {
            keys,
            random_type: 4,
            ..Default::default()
        }
    }

    #[test]
    fn color_always_draws_for_animated_zero_at_repeated_age_but_skips_single_zero() {
        let defaults = VfxClientColorCurveDefaults {
            empty_random_type: 4,
            empty_rgba: [1.0; 4],
        };
        let mut color = AvfxColorCurve::default();
        color.random[0] = Some(zero_random(false));
        color.random[1] = Some(zero_random(true));
        color.random[4] = Some(zero_random(true));
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let state = VfxClientColorCurveState::construct(&color, defaults, &mut random);
        assert_eq!(random.words(), [1, 2, 3, 4]);
        let mut expected = random;
        for _ in 0..2 {
            expected.next_u16();
            expected.next_u16();
            assert_eq!(state.evaluate(&color, 5.0, 5.0, &mut random), [1.0; 4]);
            assert_eq!(random, expected);
        }
    }

    #[test]
    fn color_first_initializes_disabled_channels_from_shared_empty_descriptor() {
        let mut color = AvfxColorCurve::default();
        // An authored empty Always descriptor is replaced with shared RanT=0.
        color.random[0] = Some(AvfxCurve {
            random_type: 4,
            ..Default::default()
        });
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let state = VfxClientColorCurveState::construct(
            &color,
            VfxClientColorCurveDefaults {
                empty_random_type: 0,
                empty_rgba: [0.75, 0.25, 2.0, 0.5],
            },
            &mut random,
        );
        let after_first = random;
        assert_ne!(random.words(), [1, 2, 3, 4]);
        assert_eq!(
            state.evaluate(&color, f32::NAN, f32::INFINITY, &mut random),
            [0.75, 0.25, 2.0, 0.5]
        );
        assert_eq!(random, after_first);
    }
}
