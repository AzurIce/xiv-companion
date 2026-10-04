use super::{
    VfxClientColorCurveDefaults, VfxClientColorCurveState, VfxClientRandomState,
    VfxClientScalarCurveState, VfxClientScalarPairState, VfxClientVectorCurveState,
};
use crate::avfx::{AvfxCurve, AvfxParticleDataModelSkin};
use std::sync::{Arc, Mutex};

/// Resolved XYZ random fallback from the compiled environment. This is not an
/// authored editor default. Col's explicit defaults supply the shared empty RanT.
#[derive(Clone, Copy, Debug)]
pub struct VfxClientModelSkinCurveDefaults {
    pub vector_random_fallback: [f32; 3],
}

#[derive(Debug)]
pub(super) struct VfxClientModelSkinCurveEnvironment {
    pub defaults: VfxClientModelSkinCurveDefaults,
    pairs: Arc<Mutex<Vec<[VfxClientScalarPairState; 3]>>>,
    uv_pairs: Arc<Mutex<Vec<Vec<VfxClientScalarPairState>>>>,
}
impl Clone for VfxClientModelSkinCurveEnvironment {
    fn clone(&self) -> Self {
        Self {
            defaults: self.defaults,
            uv_pairs: Arc::new(Mutex::new(
                self.uv_pairs.lock().expect("UV cache poisoned").clone(),
            )),
            pairs: Arc::new(Mutex::new(
                self.pairs
                    .lock()
                    .expect("ModelSkin curve cache poisoned")
                    .clone(),
            )),
        }
    }
}
impl VfxClientModelSkinCurveEnvironment {
    pub(super) fn new(defaults: VfxClientModelSkinCurveDefaults, count: usize) -> Self {
        Self {
            defaults,
            uv_pairs: Arc::new(Mutex::new(vec![Vec::new(); count])),
            pairs: Arc::new(Mutex::new(vec![
                [VfxClientScalarPairState::default(); 3];
                count
            ])),
        }
    }
    pub(super) fn share(&self) -> Self {
        Self {
            defaults: self.defaults,
            uv_pairs: Arc::clone(&self.uv_pairs),
            pairs: Arc::clone(&self.pairs),
        }
    }
    #[cfg(test)]
    pub(super) fn dispatch_codes(&self, index: usize) -> [u8; 3] {
        self.pairs.lock().unwrap()[index].map(VfxClientScalarPairState::dispatch_code)
    }
    pub(super) fn reset(&self) {
        for pairs in self.uv_pairs.lock().expect("UV cache poisoned").iter_mut() {
            pairs.fill(VfxClientScalarPairState::default());
        }
        self.pairs
            .lock()
            .expect("ModelSkin curve cache poisoned")
            .fill([VfxClientScalarPairState::default(); 3]);
    }
    pub(super) fn evaluate_numeric(
        &self,
        index: usize,
        state: &mut VfxClientModelSkinCurveState,
        data: &AvfxParticleDataModelSkin,
        ages: [f32; 2],
        empty_rgba: [f32; 4],
        random: &mut VfxClientRandomState,
    ) {
        let mut pairs = self.pairs.lock().expect("ModelSkin curve cache poisoned");
        state.evaluate_numeric(
            data,
            ages,
            empty_rgba,
            self.defaults,
            &mut pairs[index],
            random,
        );
    }

    pub(super) fn evaluate_uv(
        &self,
        index: usize,
        states: &mut [super::uv_curves::VfxClientUvCurveState],
        sets: &[crate::avfx::AvfxUvSet],
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) {
        let mut all_pairs = self.uv_pairs.lock().expect("UV cache poisoned");
        let pairs = &mut all_pairs[index];
        pairs.resize(states.len(), VfxClientScalarPairState::default());
        for ((state, set), pair) in states.iter_mut().zip(sets).zip(pairs) {
            state.evaluate(set, pair, ages, random);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VfxClientModelSkinCurveState {
    fresnel: VfxClientScalarCurveState,
    begin: VfxClientColorCurveState,
    end: VfxClientColorCurveState,
    rotation: VfxClientVectorCurveState,
    sem: VfxClientScalarCurveState,
    eem: VfxClientScalarCurveState,
    uv_density: VfxClientVectorCurveState,
    pub cache: VfxClientModelSkinCurveCache,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct VfxClientModelSkinCurveCache {
    pub begin: [f32; 4],
    pub end: [f32; 4],
    pub rotation: [f32; 3],
    pub fresnel: Option<f32>,
    pub sem: Option<f32>,
    pub eem: Option<f32>,
    pub uv_density: Option<[f32; 3]>,
}
fn mode(curve: &AvfxCurve, empty: u32) -> u32 {
    if curve.keys.is_empty() {
        empty
    } else {
        curve.random_type
    }
}
fn scalar(
    state: VfxClientScalarCurveState,
    pair: &mut VfxClientScalarPairState,
    main: &AvfxCurve,
    random_curve: &AvfxCurve,
    ages: [f32; 2],
    random: &mut VfxClientRandomState,
) -> f32 {
    state.evaluate(
        pair,
        (!main.keys.is_empty()).then(|| main.value_at(ages[0], ages[1], 0.0)),
        (!random_curve.keys.is_empty()).then(|| random_curve.value_at(ages[0], ages[1], 0.0)),
        random,
    )
}
impl VfxClientModelSkinCurveState {
    pub(super) fn construct(
        data: &AvfxParticleDataModelSkin,
        color_defaults: VfxClientColorCurveDefaults,
        random: &mut VfxClientRandomState,
    ) -> Result<Self, String> {
        for axes in [&data.fresnel_rotation, &data.uv_point_density] {
            if axes.axis_connect & 15 > 9 || axes.axis_connect_random & 15 > 9 {
                return Err("ModelSkin curve replay requires valid XYZ connection codes".into());
            }
        }
        let empty = color_defaults.empty_random_type;
        // Exact derived constructor order, between the base Col callback and
        // the final Col refresh. The shared particle UV helper has already
        // initialized its five states per set. Every derived First runs even
        // when FrsT is disabled; other base constructor consumers are separate.
        let fresnel =
            VfxClientScalarCurveState::construct(mode(&data.fresnel_curve_random, empty), random);
        let begin = VfxClientColorCurveState::construct(&data.color_begin, color_defaults, random);
        let end = VfxClientColorCurveState::construct(&data.color_end, color_defaults, random);
        let rotation =
            VfxClientVectorCurveState::construct(&data.fresnel_rotation, empty, random).unwrap();
        let sem = VfxClientScalarCurveState::construct(mode(&data.sem_random, empty), random);
        let eem = VfxClientScalarCurveState::construct(mode(&data.eem_random, empty), random);
        let uv_density =
            VfxClientVectorCurveState::construct(&data.uv_point_density, empty, random).unwrap();
        Ok(Self {
            fresnel,
            begin,
            end,
            rotation,
            sem,
            eem,
            uv_density,
            cache: VfxClientModelSkinCurveCache {
                begin: color_defaults.empty_rgba,
                end: color_defaults.empty_rgba,
                rotation: [0.0; 3],
                fresnel: None,
                sem: None,
                eem: None,
                uv_density: None,
            },
        })
    }
    #[cfg(test)]
    pub(super) fn first_percentages(self) -> [i8; 19] {
        let mut out = [0; 19];
        out[0] = self.fresnel.first_percentage();
        out[1..6].copy_from_slice(&self.begin.first_percentages());
        out[6..11].copy_from_slice(&self.end.first_percentages());
        out[11..14].copy_from_slice(&self.rotation.first_percentages());
        out[14] = self.sem.first_percentage();
        out[15] = self.eem.first_percentage();
        out[16..19].copy_from_slice(&self.uv_density.first_percentages());
        out
    }
    pub(super) fn evaluate_properties(
        &mut self,
        data: &AvfxParticleDataModelSkin,
        ages: [f32; 2],
        empty_rgba: [f32; 4],
        defaults: VfxClientModelSkinCurveDefaults,
        random: &mut VfxClientRandomState,
    ) {
        if data.fresnel_type & 3 == 0 {
            self.cache.begin = empty_rgba;
            self.cache.end = empty_rgba;
        } else {
            self.cache.begin = self
                .begin
                .evaluate(&data.color_begin, ages[0], ages[1], random);
            self.cache.end = self.end.evaluate(&data.color_end, ages[0], ages[1], random);
            self.cache.rotation = self.rotation.evaluate(
                &data.fresnel_rotation,
                ages,
                0.0,
                defaults.vector_random_fallback,
                random,
            );
        }
    }
    fn evaluate_numeric(
        &mut self,
        data: &AvfxParticleDataModelSkin,
        ages: [f32; 2],
        empty_rgba: [f32; 4],
        defaults: VfxClientModelSkinCurveDefaults,
        pairs: &mut [VfxClientScalarPairState; 3],
        random: &mut VfxClientRandomState,
    ) {
        self.cache.fresnel = Some(if data.fresnel_type & 3 == 0 {
            empty_rgba[3]
        } else {
            scalar(
                self.fresnel,
                &mut pairs[0],
                &data.fresnel_curve,
                &data.fresnel_curve_random,
                ages,
                random,
            )
        });
        self.cache.sem = Some(scalar(
            self.sem,
            &mut pairs[1],
            &data.sem,
            &data.sem_random,
            ages,
            random,
        ));
        self.cache.eem = Some(scalar(
            self.eem,
            &mut pairs[2],
            &data.eem,
            &data.eem_random,
            ages,
            random,
        ));
        self.cache.uv_density = Some(self.uv_density.evaluate(
            &data.uv_point_density,
            ages,
            0.0,
            defaults.vector_random_fallback,
            random,
        ));
    }
}
