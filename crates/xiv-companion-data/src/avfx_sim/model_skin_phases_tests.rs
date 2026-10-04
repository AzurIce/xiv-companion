use super::*;
use crate::avfx::curve_client_tests::{color_from_original_case, curve_from_original_case};
use crate::avfx::{AvfxCurve3Axis, AvfxParticleData, AvfxParticleDataModelSkin};
use crate::avfx_sim::{VfxClientModelSkinCurveDefaults, VfxClientRandomState};
use serde_json::{Value, json};
fn bits(v: &Value) -> u32 {
    v.as_u64().unwrap() as u32
}
fn float(v: &Value) -> f32 {
    f32::from_bits(bits(v))
}
fn vector(v: &Value) -> AvfxCurve3Axis {
    let fields = v["fields"].as_array().unwrap();
    AvfxCurve3Axis {
        axis_connect: bits(&v["act"]),
        axis_connect_random: bits(&v["actr"]),
        x: Some(curve_from_original_case(&fields[0])),
        y: Some(curve_from_original_case(&fields[1])),
        z: Some(curve_from_original_case(&fields[2])),
        random_x: Some(curve_from_original_case(&fields[3])),
        random_y: Some(curve_from_original_case(&fields[4])),
        random_z: Some(curve_from_original_case(&fields[5])),
    }
}
pub(in crate::avfx_sim) fn file_for(case: &Value) -> AvfxFile {
    let mut file = super::color_phases_tests::file_for(case);
    file.emitters[2].particle_items[0].create_count = 2;
    file.particles[0].data = AvfxParticleData::ModelSkin(AvfxParticleDataModelSkin {
        fresnel_type: bits(&case["fresnelType"]) as i32,
        color_begin: color_from_original_case(&case["colors"][1]),
        color_end: color_from_original_case(&case["colors"][2]),
        fresnel_curve: curve_from_original_case(&case["fresnel"]["main"]),
        fresnel_curve_random: curve_from_original_case(&case["fresnel"]["random"]),
        fresnel_rotation: vector(&case["rotation"]),
        sem: curve_from_original_case(&case["sem"]["main"]),
        sem_random: curve_from_original_case(&case["sem"]["random"]),
        eem: curve_from_original_case(&case["eem"]["main"]),
        eem_random: curve_from_original_case(&case["eem"]["random"]),
        uv_point_density: vector(&case["uvDensity"]),
        ..Default::default()
    });
    file
}
fn compare(cases: &[Value]) -> Value {
    let mut values = 0;
    let mut first_bytes = 0;
    let mut states = 0;
    let mut inputs = 0;
    let mut draws = 0;
    for (ordinal, case) in cases.iter().enumerate() {
        let file = file_for(case);
        let seed =
            VfxClientRandomState::from_words(std::array::from_fn(|i| bits(&case["seed"][i])));
        let random = super::super::client_random::VfxClientRandomCell::new(seed);
        let model_skin = super::super::model_skin_curves::VfxClientModelSkinCurveEnvironment::new(
            VfxClientModelSkinCurveDefaults {
                vector_random_fallback: std::array::from_fn(|i| float(&case["fallback"][i])),
            },
            1,
        );
        let environment = super::super::color_curves::VfxClientColorCurveEnvironment::new(
            super::color_phases_tests::defaults(case),
            &random,
        )
        .with_model_skin(Some(&model_skin));
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
        let mut expected_random = seed;
        let mut check_stream = |expected: &Value, n: usize| {
            for _ in 0..n {
                expected_random.next_u16();
            }
            draws += n;
            assert_eq!(random.get(), expected_random, "case={ordinal}");
            assert_eq!(
                random.get().words(),
                std::array::from_fn(|i| bits(&expected[i])),
                "case={ordinal}"
            );
            states += 1;
        };
        let mut check = |root: &Node, records: &Value| {
            let parent = &root.children[0].children[0];
            assert_eq!(parent.children.len(), 2);
            for (i, particle) in parent.children.iter().enumerate() {
                let original = &records[i];
                let actual = particle.client_model_skin.unwrap();
                let first = actual.first_percentages();
                for k in 0..19 {
                    assert_eq!(
                        first[k] as i64,
                        original["first"][k].as_i64().unwrap(),
                        "First case={ordinal}, owner={i}, byte={k}"
                    );
                    first_bytes += 1;
                }
                for axis in 0..4 {
                    assert_eq!(
                        particle.color[axis].to_bits(),
                        bits(&original["color"][axis]),
                        "Col case={ordinal}, owner={i}, axis={axis}"
                    );
                    values += 1;
                }
                let c = actual.cache;
                let mut all = Vec::from(c.begin);
                all.extend(c.end);
                all.extend(c.rotation);
                all.extend([
                    c.fresnel.unwrap_or(0.0),
                    c.sem.unwrap_or(0.0),
                    c.eem.unwrap_or(0.0),
                ]);
                all.extend(c.uv_density.unwrap_or([0.0; 3]));
                for k in 0..17 {
                    assert_eq!(
                        all[k].to_bits(),
                        bits(&original["values"][k]),
                        "cache case={ordinal}, owner={i}, field={k}, all={all:?}"
                    );
                    values += 1;
                }
                assert_eq!(
                    model_skin.dispatch_codes(0).map(u64::from),
                    std::array::from_fn::<_, 3, _>(|k| original["pairs"][k].as_u64().unwrap()),
                    "pair dispatch case={ordinal}, owner={i}"
                );
            }
        };
        check(state.roots[0].child.as_ref().unwrap(), &case["initial"]);
        check_stream(
            &case["initialState"],
            case["initialDraws"].as_u64().unwrap() as usize,
        );
        for original in case["steps"].as_array().unwrap() {
            state.begin_input_update().unwrap();
            let mut context = state.context(&file);
            let root = state.roots[0].child.as_mut().unwrap();
            context.phase = VfxTracePhase::Advance;
            root.advance(float(&original["delta"]), false, &mut context, 0, None)
                .unwrap();
            check(root, &original["afterTime"]);
            check_stream(
                &original["afterTimeState"],
                original["timeDraws"].as_u64().unwrap() as usize,
            );
            context.phase = VfxTracePhase::Prepare;
            root.prepare(&mut context, 0).unwrap();
            root.prepare_particles(&mut context).unwrap();
            check(root, &original["afterNumeric"]);
            check_stream(
                &original["afterNumericState"],
                original["numericDraws"].as_u64().unwrap() as usize,
            );
            inputs += 1;
        }
    }
    json!({"cases":cases.len(),"inputs":inputs,"components":values,"firstByteChecks":first_bytes,
        "randomStateChecks":states,"sharedDraws":draws,"differences":0,
        "scope":"original raw Color/scalar/XYZ, 19 First, actual ModelSkin f0/e8/Common phases; controlled successful numeric curve order and original PICo; two shared definition owners; no full constructors/Aura matrices/provider/packing/GPU"})
}
#[test]
#[ignore = "CPU: original ModelSkin First/property/numeric-curve phases versus retained playback tree"]
fn compare_original_modelskin_curve_phases() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(dir.join("modelskin-curves-client-probe.json")).unwrap(),
    )
    .unwrap();
    let report = compare(original["cases"].as_array().unwrap());
    assert_eq!(report["cases"], 6144);
    assert_eq!(report["sharedDraws"], original["draws"]);
    std::fs::write(
        dir.join("modelskin-curves-rust-comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
#[test]
fn modelskin_curve_phases_match_original_golden() {
    let original: Value =
        serde_json::from_str(include_str!("fixtures/modelskin_curves_golden.json")).unwrap();
    compare(original["cases"].as_array().unwrap());
}
