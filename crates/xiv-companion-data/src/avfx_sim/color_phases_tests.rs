//! Original Color callbacks and original Common phases versus retained trees.
use super::*;
use crate::avfx::{AvfxCurve, AvfxCurveKey, AvfxEmitter, AvfxParticle, EmitterType, ParticleType};
use crate::avfx_sim::{VfxClientColorCurveDefaults, VfxClientRandomState};
use serde_json::{Value, json};

fn scalar(v: f32) -> AvfxCurve {
    AvfxCurve {
        keys: vec![AvfxCurveKey {
            time: 0,
            interpolation: 1,
            x: 0.0,
            y: 0.0,
            z: v,
        }],
        ..Default::default()
    }
}
fn birth(target: i32, mode: i32) -> AvfxEmitterItem {
    AvfxEmitterItem {
        enabled: true,
        target_index: target,
        create_time: 1,
        create_count: 1,
        create_probability: 100,
        parameter_link: -1,
        parent_influence_color: mode,
        ..Default::default()
    }
}
pub(super) fn file_for(case: &Value) -> AvfxFile {
    let mut file = AvfxFile {
        emitters: (0..3)
            .map(|_| AvfxEmitter {
                emitter_type: Some(EmitterType::Point),
                effector_index: -1,
                create_count: scalar(1.0),
                create_interval: scalar(1000.0),
                ..Default::default()
            })
            .collect(),
        particles: vec![AvfxParticle {
            particle_type: Some(if case["skin"] == true {
                ParticleType::ModelSkin
            } else {
                ParticleType::Quad
            }),
            collision_type: -1,
            ..Default::default()
        }],
        ..Default::default()
    };
    for i in 0..3 {
        file.emitters[i].color =
            crate::avfx::curve_client_tests::color_from_original_case(&case["colors"][i]);
        if i < 2 {
            file.emitters[i].emitter_items.push(birth(
                (i + 1) as i32,
                case[if i == 0 {
                    "childMode"
                } else {
                    "grandchildMode"
                }]
                .as_i64()
                .unwrap() as i32,
            ));
        }
    }
    let mut particle = birth(0, case["particleMode"].as_i64().unwrap() as i32);
    particle.start_frame = case["prewarmFrames"].as_i64().unwrap() as i32;
    particle.start_frame_null_update = true;
    file.emitters[2].particle_items.push(particle);
    file.particles[0].color =
        crate::avfx::curve_client_tests::color_from_original_case(&case["colors"][3]);
    file
}
fn bits(v: &Value) -> u32 {
    v.as_u64().unwrap() as u32
}
fn float(v: &Value) -> f32 {
    f32::from_bits(bits(v))
}
pub(super) fn defaults(case: &Value) -> VfxClientColorCurveDefaults {
    VfxClientColorCurveDefaults {
        empty_random_type: bits(&case["emptyRandomType"]),
        empty_rgba: std::array::from_fn(|i| float(&case["emptyRgba"][i])),
    }
}
fn check_colors(root: &Node, expected: &Value) -> usize {
    let mut node = root;
    for i in 0..4 {
        for axis in 0..4 {
            assert_eq!(
                node.color[axis].to_bits(),
                bits(&expected[i][axis]),
                "node={i}, axis={axis}, color={:?}, expected={expected}",
                node.color
            );
        }
        assert_eq!(node.numeric.alpha.to_bits(), node.color[3].to_bits());
        if i < 3 {
            assert_eq!(node.children.len(), 1);
            node = &node.children[0];
        }
    }
    16
}
fn compare(cases: &[Value]) -> Value {
    let mut components = 0;
    let mut states = 0;
    let mut inputs = 0;
    let mut draws = 0;
    for (ordinal, case) in cases.iter().enumerate() {
        let file = file_for(case);
        for i in 0..4 {
            let curve = if i < 3 {
                &file.emitters[i].color
            } else {
                &file.particles[0].color
            };
            assert_eq!(
                curve.client_dispatch_flags(),
                bits(&case["colors"][i]["flags"])
            );
        }
        let seed =
            VfxClientRandomState::from_words(std::array::from_fn(|i| bits(&case["seed"][i])));
        let random = super::super::client_random::VfxClientRandomCell::new(seed);
        let environment = super::super::color_curves::VfxClientColorCurveEnvironment::new(
            defaults(case),
            &random,
        );
        let mut state = Instances::new_with_roots_continuing(
            &file,
            &[RootInitialization {
                color_seed: None,
                definition: 0,
                life: -1.0,
                start_delay: 0.0,
                binder: false,
                child_age: Some(0.0),
            }],
            None,
            Some(environment),
            None,
            None,
        )
        .unwrap();
        let mut check_state = |expected: &Value, draw_count: usize| {
            // Check the native draw count against its native four-word snapshot;
            // retain cumulative transitions to cover constructor and all phases.
            draws += draw_count;
            let expected_words = std::array::from_fn(|i| bits(&expected[i]));
            assert_eq!(random.get().words(), expected_words, "case={ordinal}");
            states += 1;
        };
        components += check_colors(state.roots[0].child.as_ref().unwrap(), &case["initial"]);
        let initial_draws = case["initialDraws"].as_u64().unwrap() as usize;
        let mut expected_random = seed;
        for _ in 0..initial_draws {
            expected_random.next_u16();
        }
        assert_eq!(random.get(), expected_random, "constructor case={ordinal}");
        check_state(&case["initialState"], initial_draws);
        for (step, original) in case["steps"].as_array().unwrap().iter().enumerate() {
            state.begin_input_update().unwrap();
            if case["retireRoot"] == true && step == 2 {
                state.roots[0].child.as_mut().unwrap().mark_dead();
            }
            let mut context = state.context(&file);
            context.phase = VfxTracePhase::Advance;
            let root = state.roots[0].child.as_mut().unwrap();
            root.advance(float(&original["delta"]), false, &mut context, 0, None)
                .unwrap();
            components += check_colors(root, &original["afterTime"]);
            let n = original["timeDraws"].as_u64().unwrap() as usize;
            for _ in 0..n {
                expected_random.next_u16();
            }
            assert_eq!(
                random.get(),
                expected_random,
                "Time case={ordinal}, step={step}"
            );
            check_state(&original["afterTimeState"], n);
            context.phase = VfxTracePhase::Prepare;
            root.prepare(&mut context, 0).unwrap();
            root.prepare_particles(&mut context).unwrap();
            components += check_colors(root, &original["afterPrepare"]);
            let n = original["prepareDraws"].as_u64().unwrap() as usize;
            for _ in 0..n {
                expected_random.next_u16();
            }
            assert_eq!(
                random.get(),
                expected_random,
                "Prepare case={ordinal}, step={step}"
            );
            check_state(&original["afterPrepareState"], n);
            let particle = state.particles(0).next().unwrap();
            let draw = particle.parent_color.map_or(particle.color, |parent| {
                std::array::from_fn(|axis| parent[axis] * particle.color[axis])
            });
            for axis in 0..4 {
                assert_eq!(
                    draw[axis].to_bits(),
                    bits(&original["drawColor"][axis]),
                    "draw case={ordinal}, step={step}, axis={axis}"
                );
                components += 1;
            }
            check_state(&original["afterDrawState"], 0);
            inputs += 1;
        }
    }
    json!({"cases":cases.len(),"inputs":inputs,"rgbaComponents":components,
        "randomStateChecks":states,"sharedDraws":draws,"differences":0,
        "scope":"original raw Color/First/property/Time/Prepare/Numeric/PICo, controlled non-Color constructors; Quad and controlled ModelSkin prewarm; no Fresnel/complete constructors/GPU"})
}

#[test]
#[ignore = "CPU: original Color + Common phases versus retained instance trees"]
fn compare_original_client_color_phases() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(directory.join("client-color-phases-client-probe.json")).unwrap(),
    )
    .unwrap();
    let report = compare(original["cases"].as_array().unwrap());
    assert_eq!(report["cases"], 14976);
    assert_eq!(report["sharedDraws"], original["draws"]);
    std::fs::write(
        directory.join("client-color-phases-rust-comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[test]
fn client_color_phases_match_original_golden() {
    let original: Value =
        serde_json::from_str(include_str!("fixtures/color_phases_golden.json")).unwrap();
    compare(original["cases"].as_array().unwrap());
}
