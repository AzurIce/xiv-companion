use super::{
    VfxClientRandomState, VfxClientScalarCurveState, VfxClientScalarPairState,
    VfxClientVectorCurveState, VfxLinearBinderCurves,
};
use crate::avfx::{AvfxBinder, AvfxCurve, AvfxCurve3Axis};

/// Resolved compiled-client defaults. Empty scalar RanT is read by First
/// even without keys; XYZ's absent/disabled random axes use a separate global
/// vector. Supply these from the compiled environment, not editor UI defaults.
/// This type deliberately has no inferred Default implementation.
#[derive(Clone, Copy, Debug)]
pub struct VfxClientBinderCurveDefaults {
    pub empty_scalar_random_type: u32,
    pub vector_random_fallback: [f32; 3],
}

/// Compiled COF reader dispatch is shared by owners of the same Binder.
/// Runtime clones copy the current cache; reset starts a fresh compiled cache.
#[derive(Debug)]
pub(super) struct VfxClientBinderCurveEnvironment {
    pub(super) defaults: VfxClientBinderCurveDefaults,
    pairs: std::sync::Mutex<Vec<VfxClientScalarPairState>>,
}

impl Clone for VfxClientBinderCurveEnvironment {
    fn clone(&self) -> Self {
        Self {
            defaults: self.defaults,
            pairs: std::sync::Mutex::new(
                self.pairs
                    .lock()
                    .expect("Binder curve cache poisoned")
                    .clone(),
            ),
        }
    }
}

impl VfxClientBinderCurveEnvironment {
    pub(super) fn new(defaults: VfxClientBinderCurveDefaults, count: usize) -> Self {
        Self {
            defaults,
            pairs: std::sync::Mutex::new(vec![VfxClientScalarPairState::default(); count]),
        }
    }

    pub(super) fn reset(&self) {
        self.pairs
            .lock()
            .expect("Binder curve cache poisoned")
            .fill(VfxClientScalarPairState::default());
    }

    pub(super) fn evaluate(
        &self,
        index: usize,
        state: VfxClientDualBinderCurveState,
        binder: &AvfxBinder,
        ages: [f32; 2],
        random: &mut VfxClientRandomState,
    ) -> VfxLinearBinderCurves {
        let mut pairs = self.pairs.lock().expect("Binder curve cache poisoned");
        state.evaluate(binder, ages, self.defaults, &mut pairs[index], random)
    }

    #[cfg(test)]
    pub(super) fn dispatch_code(&self, index: usize) -> u8 {
        self.pairs.lock().expect("Binder curve cache poisoned")[index].dispatch_code()
    }
}

/// Owner-local First bytes for dual-target Linear/Spline. The scalar COF
/// dispatch cache belongs to the shared compiled Binder and is supplied to
/// evaluation separately. Does not own the global random stream.
#[derive(Clone, Copy, Debug)]
pub struct VfxClientDualBinderCurveState {
    positions: [VfxClientVectorCurveState; 2],
    factor: VfxClientScalarCurveState,
}

impl VfxClientDualBinderCurveState {
    /// Six endpoint bytes precede COF's byte, before queries/control draws.
    /// Controlled compiled-descriptor adapter: unlike authored compilation,
    /// keyless random descriptors retain their explicit RanT and random-present
    /// is supplied by key presence. Use construct_from_authored for AVFX files.
    #[cfg(test)]
    pub(super) fn construct_compiled(
        binder: &AvfxBinder,
        defaults: VfxClientBinderCurveDefaults,
        random: &mut VfxClientRandomState,
    ) -> Option<Self> {
        Self::construct_inner(binder, defaults, random, true)
    }

    /// Known AVFX scalar/XYZ compilation before First construction. Keyless
    /// scalar blocks collapse to the shared empty descriptor; single zero
    /// random keys can disable the vector random reader. Animated reading and
    /// full file/resource compilation are still separate work.
    pub fn construct_from_authored(
        binder: &AvfxBinder,
        defaults: VfxClientBinderCurveDefaults,
        random: &mut VfxClientRandomState,
    ) -> Option<Self> {
        Self::construct_inner(binder, defaults, random, false)
    }

    fn construct_inner(
        binder: &AvfxBinder,
        defaults: VfxClientBinderCurveDefaults,
        random: &mut VfxClientRandomState,
        compiled: bool,
    ) -> Option<Self> {
        if !matches!(binder.binder_type, 1 | 2 | 4) {
            return None;
        }
        let endpoints = [
            binder.properties_start.as_ref()?,
            binder.properties_goal.as_ref()?,
        ];
        let data = binder.data.as_ref()?;
        if endpoints
            .iter()
            .any(|p| p.position.axis_connect & 15 > 9 || p.position.axis_connect_random & 15 > 9)
        {
            return None;
        }
        let positions = endpoints.map(|p| {
            if compiled {
                VfxClientVectorCurveState::construct_compiled(
                    &p.position,
                    defaults.empty_scalar_random_type,
                    random,
                )
            } else {
                VfxClientVectorCurveState::construct(
                    &p.position,
                    defaults.empty_scalar_random_type,
                    random,
                )
            }
            .expect("validated XYZ connection codes")
        });
        let factor = VfxClientScalarCurveState::construct(
            data.carry_over_factor_random
                .as_ref()
                .filter(|c| compiled || !c.keys.is_empty())
                .map_or(defaults.empty_scalar_random_type, |c| c.random_type),
            random,
        );
        Some(Self { positions, factor })
    }

    pub fn first_percentages(self) -> [i8; 7] {
        std::array::from_fn(|i| {
            if i == 6 {
                self.factor.first_percentage()
            } else {
                self.positions[i / 3].first_percentages()[i % 3]
            }
        })
    }

    /// COF, start XYZ, goal XYZ, then start XYZ again. Call only when the
    /// native update actually reaches curve evaluation (after initialization
    /// succeeds, or on an initialized ordinary update). Query providers in
    /// the current numeric adapter must not consume this random stream.
    pub fn evaluate(
        self,
        binder: &AvfxBinder,
        ages: [f32; 2],
        defaults: VfxClientBinderCurveDefaults,
        factor_pair: &mut VfxClientScalarPairState,
        random: &mut VfxClientRandomState,
    ) -> VfxLinearBinderCurves {
        let data = binder.data.as_ref().expect("validated dual Binder curves");
        let factor = self.factor.evaluate(
            factor_pair,
            value(data.carry_over_factor.as_ref(), ages),
            value(data.carry_over_factor_random.as_ref(), ages),
            random,
        );
        let start = &binder.properties_start.as_ref().unwrap().position;
        let goal = &binder.properties_goal.as_ref().unwrap().position;
        let start_position = self.vector(start, 0, ages, defaults, random);
        let goal_position = self.vector(goal, 1, ages, defaults, random);
        let start_position_after = self.vector(start, 0, ages, defaults, random);
        VfxLinearBinderCurves {
            factor,
            start_position,
            goal_position,
            start_position_after,
        }
    }

    fn vector(
        self,
        curve: &AvfxCurve3Axis,
        endpoint: usize,
        ages: [f32; 2],
        defaults: VfxClientBinderCurveDefaults,
        random: &mut VfxClientRandomState,
    ) -> [f32; 3] {
        self.positions[endpoint].evaluate(curve, ages, 0.0, defaults.vector_random_fallback, random)
    }
}

fn value(curve: Option<&AvfxCurve>, ages: [f32; 2]) -> Option<f32> {
    curve
        .filter(|c| !c.keys.is_empty())
        .map(|c| c.value_at(ages[0], ages[1], 0.0))
}

#[cfg(test)]
pub(in crate::avfx_sim) mod tests {
    use super::*;
    use crate::avfx::{AvfxBinderData, AvfxBinderProperties, AvfxCurveKey};
    use crate::avfx_sim::{
        VfxBinderMatrix, VfxBinderTarget, VfxClientTrigMode, VfxLinearBinderFrame,
        VfxSplineBinderInstance, VfxSplineBinderState,
    };
    use serde_json::{Value, json};
    use std::cell::RefCell;

    fn scalar(mode: u32, present: bool, z: f32) -> AvfxCurve {
        AvfxCurve {
            random_type: mode,
            keys: if present {
                vec![AvfxCurveKey {
                    time: 0,
                    z,
                    x: 0.0,
                    y: 0.0,
                    interpolation: 1,
                }]
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    }

    pub(in crate::avfx_sim) fn binder(
        scenario: u32,
        start: i32,
        goal: i32,
        delay: i32,
    ) -> AvfxBinder {
        let curve = |slot: u32, random: bool| {
            let mode = if random { (scenario + slot * 2) & 7 } else { 0 };
            let present = scenario >= 16 || (scenario + slot) % 4 & if random { 2 } else { 1 } != 0;
            let z = if random {
                ((slot % 3) as f32 - 1.0) * 0.25
            } else if slot == 6 {
                0.5
            } else {
                (slot as f32 - 3.0) * 0.125
            };
            (scenario < 16 || !random).then(|| scalar(mode, present, z))
        };
        let position = |endpoint: u32| AvfxCurve3Axis {
            x: curve(endpoint * 3, false),
            y: curve(endpoint * 3 + 1, false),
            z: curve(endpoint * 3 + 2, false),
            random_x: curve(endpoint * 3, true),
            random_y: curve(endpoint * 3 + 1, true),
            random_z: curve(endpoint * 3 + 2, true),
            ..Default::default()
        };
        AvfxBinder {
            binder_type: 2,
            life: 30,
            properties_start: Some(AvfxBinderProperties {
                bind_point_type: start,
                generate_delay: delay,
                coord_update_frame: -1,
                position: position(0),
                ..Default::default()
            }),
            properties_goal: Some(AvfxBinderProperties {
                bind_point_type: goal,
                coord_update_frame: -1,
                position: position(1),
                ..Default::default()
            }),
            properties_1: Some(AvfxBinderProperties {
                ring_enabled: scenario & 1 != 0,
                ring_position: [0.125, -0.25, 0.3],
                ring_radius: 2.0,
                ring_progress_time: 1,
                ..Default::default()
            }),
            properties_2: Some(AvfxBinderProperties {
                ring_enabled: scenario & 2 != 0,
                ring_position: [-0.375, 0.5, 0.7],
                ring_radius: 1.25,
                ring_progress_time: 3,
                ..Default::default()
            }),
            data: Some(AvfxBinderData {
                carry_over_factor: curve(6, false),
                carry_over_factor_random: curve(6, true),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn dual_binder_missing_random_descriptors_consume_seven_first_draws_before_queries() {
        let b = binder(16, 0, 0, 0);
        let defaults = VfxClientBinderCurveDefaults {
            empty_scalar_random_type: 0,
            vector_random_fallback: [0.0; 3],
        };
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let mut expected = random;
        for _ in 0..7 {
            expected.next_u16();
        }
        let state =
            VfxClientDualBinderCurveState::construct_compiled(&b, defaults, &mut random).unwrap();
        assert_eq!(random, expected);
        let mut pair = VfxClientScalarPairState::default();
        let first = state.evaluate(&b, [0.0; 2], defaults, &mut pair, &mut random);
        assert_eq!(random, expected);
        assert_eq!(pair.dispatch_code(), 1);
        assert_eq!(first.factor, 0.5);
        assert_eq!(first.start_position, [-0.375, -0.25, -0.125]);
    }

    #[test]
    fn xyz_empty_always_axes_do_not_take_scalar_lazy_draws() {
        let b = binder(17, 0, 0, 0);
        let defaults = VfxClientBinderCurveDefaults {
            empty_scalar_random_type: 4,
            vector_random_fallback: [0.75; 3],
        };
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let mut expected = random;
        let state =
            VfxClientDualBinderCurveState::construct_compiled(&b, defaults, &mut random).unwrap();
        assert_eq!(state.first_percentages(), [0; 7]);
        assert_eq!(random, expected);
        let mut shared_pair = VfxClientScalarPairState::default();
        state.evaluate(&b, [0.0; 2], defaults, &mut shared_pair, &mut random);
        expected.next_u16();
        assert_eq!(
            random, expected,
            "only lazy COF draws; XYZ uses a different reader"
        );
        let second =
            VfxClientDualBinderCurveState::construct_compiled(&b, defaults, &mut random).unwrap();
        second.evaluate(&b, [0.0; 2], defaults, &mut shared_pair, &mut random);
        assert_eq!(
            random, expected,
            "COF cache belongs to the shared compiled Binder"
        );
    }

    #[test]
    fn xyz_skipped_random_axis_uses_resolved_vector_fallback() {
        let mut b = binder(16, 0, 0, 0);
        b.properties_start.as_mut().unwrap().position.random_x = Some(scalar(1, true, 0.0));
        let defaults = VfxClientBinderCurveDefaults {
            empty_scalar_random_type: 0,
            vector_random_fallback: [0.75; 3],
        };
        let mut random = VfxClientRandomState::from_words([1, 2, 3, 4]);
        let state =
            VfxClientDualBinderCurveState::construct_compiled(&b, defaults, &mut random).unwrap();
        let v = state.evaluate(
            &b,
            [0.0; 2],
            defaults,
            &mut VfxClientScalarPairState::default(),
            &mut random,
        );
        assert_eq!(v.start_position, [-0.375, 0.5, 0.625]);
        assert_eq!(v.goal_position, [0.0, 0.125, 0.25]);
    }

    pub(in crate::avfx_sim) fn authored_binder(case: &Value) -> AvfxBinder {
        let mut b = binder(
            case["scenario"].as_u64().unwrap() as u32,
            case["startMode"].as_i64().unwrap() as i32,
            case["goalMode"].as_i64().unwrap() as i32,
            case["generateDelay"].as_i64().unwrap() as i32,
        );
        let raw = case["rawCurves"].as_array().unwrap();
        b.bind_point_id = 4;
        assert_eq!(raw.len(), 14);
        let parsed = |slot: usize| {
            let bytes = raw[slot]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
                .collect::<Vec<_>>();
            Some(crate::avfx::curve_client_tests::parse_scalar_payload(
                &bytes,
            ))
        };
        for (endpoint, p) in [
            b.properties_start.as_mut().unwrap(),
            b.properties_goal.as_mut().unwrap(),
        ]
        .into_iter()
        .enumerate()
        {
            p.bind_target_point_type = 3;
            p.bind_point_id = if endpoint == 0 { 4 } else { 7 };
            let i = endpoint * 6;
            p.position = AvfxCurve3Axis {
                axis_connect: case["connections"][endpoint][0].as_u64().unwrap() as u32,
                axis_connect_random: case["connections"][endpoint][1].as_u64().unwrap() as u32,
                x: parsed(i),
                random_x: parsed(i + 1),
                y: parsed(i + 2),
                random_y: parsed(i + 3),
                z: parsed(i + 4),
                random_z: parsed(i + 5),
            };
        }
        let data = b.data.as_mut().unwrap();
        data.carry_over_factor = parsed(12);
        data.carry_over_factor_random = parsed(13);
        b
    }

    fn matrix(m: VfxBinderMatrix) -> Vec<u32> {
        m.basis
            .into_iter()
            .flatten()
            .chain(m.position)
            .map(f32::to_bits)
            .collect()
    }

    fn snapshot(
        objects: &[(
            super::super::point_factory::LinearTargetBirth,
            VfxClientDualBinderCurveState,
            VfxSplineBinderInstance,
        )],
        random: &RefCell<VfxClientRandomState>,
        pair: &RefCell<VfxClientScalarPairState>,
        queries: &RefCell<Vec<[i32; 2]>>,
    ) -> Value {
        let children = objects.iter().map(|(birth, _, instance)| {
            let c = instance.lifecycle.clock;
            let s = &instance.state;
            json!({"targets":birth.target_indices,"lifeEnabled":instance.lifecycle.life_limit_enabled,"initialized":instance.initialized,
                "clocks":([c.local_age,c.total_age,instance.lifecycle.scaled_delta,c.previous_age,c.nominal_life].map(f32::to_bits)),"life":[c.nominal_life.to_bits()],"delay":[c.delay.to_bits()],
                "main":matrix(s.common.matrix),"auxiliary":matrix(s.common.auxiliary_matrix),"scale":s.common.scale.map(f32::to_bits),
                "offsets":s.controls().into_iter().flat_map(|p|p.offset.map(f32::to_bits)).collect::<Vec<_>>(),"parameters":s.controls().map(|p|p.parameter.to_bits()),
                "knots":s.path().knots().iter().map(|p|json!({"parameter":p.parameter.to_bits(),"position":p.position.map(f32::to_bits)})).collect::<Vec<_>>()})
        }).collect::<Vec<_>>();
        json!({"factory":{"children":children,"randomState":random.borrow().words(),"queryTargets":*queries.borrow()},
            "factorDispatch":pair.borrow().dispatch_code(),"coefficients":objects.iter().map(|(_,c,_)|c.first_percentages()).collect::<Vec<_>>()})
    }

    pub(in crate::avfx_sim) fn scoped_expected(mut v: Value) -> Value {
        let f = v["factory"].as_object_mut().unwrap();
        for k in [
            "parentFlags",
            "freeCount",
            "countGetters",
            "allocatorFunctionCalls",
            "tlsGetterCalls",
        ] {
            f.remove(k);
        }
        for c in f["children"].as_array_mut().unwrap() {
            let c = c.as_object_mut().unwrap();
            c.remove("flags");
            c.remove("inner");
        }
        v
    }

    pub(in crate::avfx_sim) fn canonicalize(v: &mut Value) {
        match v {
            Value::Object(o) => {
                for (key, item) in o {
                    if matches!(
                        key.as_str(),
                        "clocks"
                            | "life"
                            | "delay"
                            | "main"
                            | "auxiliary"
                            | "scale"
                            | "offsets"
                            | "parameters"
                            | "position"
                    ) {
                        for x in item.as_array_mut().unwrap() {
                            if f32::from_bits(x.as_u64().unwrap() as u32).is_nan() {
                                *x = json!("NaN");
                            }
                        }
                    } else if key == "parameter" {
                        if f32::from_bits(item.as_u64().unwrap() as u32).is_nan() {
                            *item = json!("NaN");
                        }
                    } else {
                        canonicalize(item);
                    }
                }
            }
            Value::Array(a) => {
                for x in a {
                    canonicalize(x);
                }
            }
            _ => {}
        }
    }

    #[test]
    #[ignore = "CPU: original Spline factories/own phases with unhooked First and original scalar/XYZ readers"]
    fn compare_original_spline_random_curves_with_persistent_numeric_instances() {
        let folder = std::path::Path::new("../../target/weapon-vfx-audit");
        let original: Value = serde_json::from_slice(
            &std::fs::read(folder.join("spline-random-curves-client-probe.json")).unwrap(),
        )
        .unwrap();
        let cases = original["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 6912);
        let mut boundaries = 0usize;
        let mut object_count = 0usize;
        for (ordinal, case) in cases.iter().enumerate() {
            let b = binder(
                case["scenario"].as_u64().unwrap() as u32,
                case["startMode"].as_i64().unwrap() as i32,
                case["goalMode"].as_i64().unwrap() as i32,
                case["generateDelay"].as_i64().unwrap() as i32,
            );
            let defaults = VfxClientBinderCurveDefaults {
                empty_scalar_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
                vector_random_fallback: [f32::from_bits(case["fallback"].as_u64().unwrap() as u32);
                    3],
            };
            let random = RefCell::new(VfxClientRandomState::from_words(std::array::from_fn(|i| {
                case["seed"][i].as_u64().unwrap() as u32
            })));
            let pair = RefCell::new(VfxClientScalarPairState::default());
            let queries = RefCell::new(Vec::<[i32; 2]>::new());
            let plans = super::super::point_factory::dual_target_births(
                &b,
                case["targetCount"].as_i64().unwrap() as i32,
            );
            let mode = if case["trigMode"] == 0 {
                VfxClientTrigMode::Sse2
            } else {
                VfxClientTrigMode::AvxFma
            };
            let frame =
                |birth: super::super::point_factory::LinearTargetBirth| VfxLinearBinderFrame {
                    age: 0.0,
                    query_deadlines: [-1.0; 2],
                    camera_position: [0.0; 3],
                    camera: None,
                    document_scale: [1.0; 3],
                    self_targets: birth.target_indices.map(|i| i < 0),
                    root_ags: false,
                };
            let target =
                |birth: super::super::point_factory::LinearTargetBirth, success: u64, e: usize| {
                    let index = birth.target_indices[e];
                    queries
                        .borrow_mut()
                        .push([if e == 0 { 0 } else { 3 }, index]);
                    (success & (1 << e) != 0).then_some(VfxBinderTarget {
                        position: [
                            (if index < 0 {
                                100.0
                            } else {
                                index as f32 * 10.0
                            }) + if e == 1 { 3.0 } else { 0.0 },
                            0.0,
                            0.0,
                        ],
                        basis: VfxBinderMatrix::IDENTITY.basis,
                        scale: [1.0; 3],
                    })
                };
            let mut objects = Vec::new();
            let initial = case["initialSuccessMask"].as_u64().unwrap();
            for birth in plans {
                let curves = VfxClientDualBinderCurveState::construct_compiled(
                    &b,
                    defaults,
                    &mut random.borrow_mut(),
                )
                .unwrap();
                let constructed = VfxSplineBinderState::construct_with_client_trig(
                    &b,
                    frame(birth),
                    birth.delay,
                    |e| target(birth, initial, e),
                    |_, v| {
                        *v = 1.0;
                        true
                    },
                    || random.borrow_mut().next_u16(),
                    mode,
                    || {
                        curves.evaluate(
                            &b,
                            [0.0; 2],
                            defaults,
                            &mut pair.borrow_mut(),
                            &mut random.borrow_mut(),
                        )
                    },
                    VfxBinderMatrix::IDENTITY,
                );
                let child = constructed
                    .initialization
                    .and_then(|i| i.child_direction)
                    .is_some();
                objects.push((
                    birth,
                    curves,
                    constructed.into_instance(u16::from(child), mode),
                ));
                object_count += 1;
            }
            let compare = |expected: Value, objects: &[_], boundary: usize| {
                let mut expected = scoped_expected(expected);
                let mut actual = snapshot(objects, &random, &pair, &queries);
                canonicalize(&mut expected);
                canonicalize(&mut actual);
                assert_eq!(actual, expected, "case {ordinal} boundary {boundary}");
            };
            compare(case["afterFactory"].clone(), &objects, 0);
            boundaries += 1;
            for (tick, input) in case["inputs"].as_array().unwrap().iter().enumerate() {
                let delta = f32::from_bits(input["delta"].as_u64().unwrap() as u32);
                for (_, _, instance) in &mut objects {
                    instance.advance_time(delta);
                }
                let success = if tick >= 2 { 3 } else { initial };
                for (birth, curves, instance) in &mut objects {
                    instance.prepare_self(
                        &b,
                        frame(*birth),
                        |e| target(*birth, success, e),
                        |_, v| {
                            *v = 1.0;
                            true
                        },
                        || random.borrow_mut().next_u16(),
                        |age, total| {
                            curves.evaluate(
                                &b,
                                [age, total],
                                defaults,
                                &mut pair.borrow_mut(),
                                &mut random.borrow_mut(),
                            )
                        },
                        VfxBinderMatrix::IDENTITY,
                        |_, _, _| true,
                    );
                }
                compare(input["afterPrepare"].clone(), &objects, tick + 1);
                boundaries += 1;
            }
        }
        assert_eq!(boundaries, 41472);
        std::fs::write(folder.join("spline-random-curves-rust-comparison.json"),serde_json::to_string_pretty(&json!({
            "cases":cases.len(),"boundaries":boundaries,"constructedObjects":object_count,"differences":0,
            "scope":"Original Scheduler/Item Spline factories, First generation, scalar lazy/cached and XYZ readers, both CRT paths and persistent own Time/Prepare versus production dual-birth plans, owner-local curve state, shared COF cache and persistent numeric instances. Ordered targets, initialized/life gates, clocks, matrices/scales, controls/knots, First bytes, COF dispatch, query order and four RNG words compared at construction and five complete inputs. Constant/empty compiled descriptors, pure queries, Document/TLS and global vector/empty-mode inputs controlled. Children/resource callbacks inert; outer registry/pool/flags/child phases, actual playback integration, animated readers, connected XYZ, live default BSS/global stream and GPU not proven."
        })).unwrap()+"\n").unwrap();
    }
}
