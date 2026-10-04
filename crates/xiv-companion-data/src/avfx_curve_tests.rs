use super::*;
use serde_json::{Value, json};

fn float(value: &Value) -> f32 {
    f32::from_bits(value.as_u64().unwrap() as u32)
}

fn canonical(value: f32) -> Value {
    if value.is_nan() {
        json!("NaN")
    } else {
        json!(value.to_bits())
    }
}

fn scalar(keys: &[(i16, u16, f32)]) -> AvfxCurve {
    AvfxCurve {
        keys: keys
            .iter()
            .map(|&(time, interpolation, z)| AvfxCurveKey {
                time,
                interpolation,
                x: 0.0,
                y: 0.0,
                z,
            })
            .collect(),
        ..Default::default()
    }
}

#[test]
fn animated_spline_keeps_client_single_precision_rounding() {
    let curve = scalar(&[(10, 0, 1.0), (30, 0, 4.0)]);
    // Original 0x1403968f0: f64 polynomial evaluation returned 0x407ffa44.
    assert_eq!(curve.value(29.875, 0.0).to_bits(), 0x407ffa42);
}

#[test]
fn animated_linear_preserves_kernel_overflow_at_exact_keys() {
    let curve = scalar(&[(10, 1, f32::MAX), (30, 1, -f32::MAX)]);
    assert_eq!(curve.value(9.875, 0.0), f32::MAX);
    assert!(curve.value(10.0, 0.0).is_nan());
    assert_eq!(curve.value(20.0, 0.0), -f32::INFINITY);
    assert_eq!(curve.value(30.0, 0.0), -f32::INFINITY);
    assert_eq!(curve.value(30.125, 0.0), -f32::MAX);
}

#[test]
fn animated_scalar_interpolation_aliases_and_zero_sign_match_client() {
    let curve = scalar(&[(10, 5, -0.0), (30, 5, -0.0)]);
    assert_eq!(curve.value(9.0, 0.0).to_bits(), 0x80000000);
    for time in [10.0, 20.0, 30.0] {
        assert_eq!(curve.value(time, 0.0).to_bits(), 0);
    }
    let step = scalar(&[(10, 6, 1.0), (30, 6, 4.0)]);
    assert_eq!(step.value(29.875, 0.0), 1.0);
    assert_eq!(step.value(30.0, 0.0), 4.0);
}

#[test]
fn animated_duplicate_first_keys_execute_zero_span_kernel() {
    let curve = scalar(&[(3, 1, 1.0), (3, 1, 4.0), (18, 1, 2.0)]);
    assert_eq!(curve.value(2.875, 0.0), 1.0);
    assert!(curve.value(3.0, 0.0).is_nan());
    assert!(curve.value(3.125, 0.0).is_finite());
    let step = scalar(&[(3, 2, 1.0), (3, 2, 4.0), (18, 2, 2.0)]);
    assert_eq!(step.value(3.0, 0.0), 4.0);
}

#[test]
fn scalar_pre_behavior_three_selects_final_constant() {
    let mut curve = scalar(&[(10, 1, 1.0), (30, 1, 4.0)]);
    curve.pre_behavior = 7;
    assert_eq!(curve.value(9.875, 0.0), 4.0);
    assert_eq!(curve.value(10.0, 0.0), 1.0);
}

#[test]
fn scalar_infinite_age_uses_boundary_and_selected_clock() {
    let mut curve = scalar(&[(10, 1, 1.0), (30, 1, 4.0)]);
    assert_eq!(curve.value(f32::INFINITY, 0.0), 4.0);
    assert_eq!(curve.value(f32::NEG_INFINITY, 0.0), 1.0);
    curve.pre_behavior = 7;
    assert_eq!(curve.value(f32::NEG_INFINITY, 0.0), 4.0);
    // Repeat selects local age even when the accumulated clock is infinite.
    curve.post_behavior = 5;
    assert_eq!(curve.value_at(20.0, f32::INFINITY, 0.0), 2.5);
    // Add selects accumulated age even when the local clock is infinite.
    curve.post_behavior = 6;
    assert_eq!(curve.value_at(f32::INFINITY, 20.0, 0.0), 2.5);
}

#[test]
fn scalar_repeat_keeps_original_large_age_rounding_and_extrapolation() {
    let mut curve = scalar(&[(10, 1, 1.0), (30, 1, 4.0)]);
    curve.pre_behavior = 5;
    curve.post_behavior = 5;
    // Original Repeat maps both to zero, below the authored first key. The
    // kernel extrapolates; it does not clamp or use an f64 modulo.
    for bits in [0x4effffff, 0xceffffff] {
        assert_eq!(curve.value(f32::from_bits(bits), 0.0).to_bits(), 0xbf000000);
    }
    assert_eq!(
        curve.value(f32::from_bits(0x4b800001), 0.0).to_bits(),
        0x400ccccd
    );
    assert_eq!(
        curve.value(f32::from_bits(0xcb800001), 0.0).to_bits(),
        0x40333334
    );
}

#[test]
fn scalar_overflow_cycle_conversion_preserves_lookup_and_guards_unauthored_key() {
    let mut curve = scalar(&[(10, 1, 1.0), (30, 1, 4.0)]);
    curve.pre_behavior = 5;
    curve.post_behavior = 5;
    for (age_bits, mapped_bits, before) in [
        (0x4f000000, 0x4f800000, false),
        (0xcf000000, 0xcf800000, true),
    ] {
        let age = f32::from_bits(age_bits);
        let mapped = repeat_scalar_curve_time(age, 10.0, 30.0, before).0;
        assert_eq!(mapped.to_bits(), mapped_bits);
        // Original lookup returns the final key as the *left* key, leaving
        // no authored right key. NaN is our guard, not a native value claim.
        assert_eq!(curve.scalar_segment_index(mapped), 1);
        assert!(curve.value(age, 0.0).is_nan());
    }
    assert_eq!(curve.scalar_segment_index(f32::NAN), 1);
    assert!(curve.value(f32::NAN, 0.0).is_nan());
}

#[test]
fn empty_rgb_is_white_in_the_color_container() {
    let color = AvfxColorCurve {
        rgb: Some(AvfxCurve::default()),
        ..Default::default()
    };
    assert_eq!(color.rgba_at(0.0, 23.0), [1.0; 4]);
    assert_eq!(color.rgb.unwrap().color_at(f32::NAN), [1.0; 3]);
}

#[test]
fn rgb_duplicate_times_select_the_original_binary_interval() {
    let curve = scalar(&[
        (3, 1, 1.0),
        (7, 1, 4.0),
        (7, 1, 2.0),
        (18, 1, 8.0),
        (41, 1, -0.5),
    ]);
    assert_eq!(curve.color_at(7.0)[2], 2.0); // original middle interval 2
    let mut zero_span = scalar(&[(0, 1, 0.0), (2, 1, 2.0), (2, 1, 6.0), (4, 1, 8.0)]);
    assert!(zero_span.color_at(2.0).iter().all(|v| v.is_nan()));
    for key in &mut zero_span.keys {
        key.interpolation = 18;
    }
    assert_eq!(zero_span.color_at(2.0)[2], 6.0);
}

#[test]
fn rgb_keeps_local_age_while_scalar_alpha_add_uses_accumulated_age() {
    let mut rgb = scalar(&[(0, 1, 0.0), (10, 1, 1.0)]);
    rgb.post_behavior = BEHAVIOR_ADD;
    for key in &mut rgb.keys {
        key.x = key.z;
        key.y = key.z;
    }
    let mut alpha = scalar(&[(0, 1, 0.0), (10, 1, 1.0)]);
    alpha.post_behavior = BEHAVIOR_ADD;
    let color = AvfxColorCurve {
        rgb: Some(rgb),
        alpha: Some(alpha),
        ..Default::default()
    };
    assert_eq!(color.rgba_at(5.0, 23.0), [0.5, 0.5, 0.5, 2.3]);
}

#[test]
fn rgb_kernel_retains_overflow_zero_sign_and_nonfinite_dispatch() {
    let mut curve = scalar(&[(10, 17, f32::MAX), (30, 17, -f32::MAX)]);
    assert_eq!(curve.color_at(f32::INFINITY)[2], -f32::MAX);
    assert_eq!(curve.color_at(f32::NEG_INFINITY)[2], f32::MAX);
    assert_eq!(curve.color_at(29.875)[2], f32::NEG_INFINITY);
    assert!(curve.color_at(f32::NAN).iter().all(|v| v.is_nan()));
    for key in &mut curve.keys {
        key.interpolation = 18;
    }
    assert_eq!(curve.color_at(f32::NAN)[2], f32::MAX);
    curve.pre_behavior = 7;
    assert_eq!(curve.color_at(f32::NEG_INFINITY)[2], -f32::MAX);
    let zero = scalar(&[(0, 1, -0.0), (1, 1, -0.0)]);
    assert_eq!(zero.color_at(0.0)[2].to_bits(), 0x80000000);
    assert_eq!(zero.color_at(0.5)[2].to_bits(), 0);
    assert_eq!(zero.color_at(1.0)[2].to_bits(), 0x80000000);
}

fn block(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut tag = [b' '; 4];
    tag[..name.len()].copy_from_slice(name.as_bytes());
    tag.reverse();
    let mut bytes = tag.to_vec();
    bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

pub(crate) fn parse_scalar_payload(payload: &[u8]) -> AvfxCurve {
    let bytes = block("AVFX", &block("Gra", payload));
    let root = AvfxNodeView::parse_root(&bytes).unwrap();
    parse_curve(&root.child("Gra").unwrap())
}

fn curve_payload_from_original_case(case: &Value) -> Vec<u8> {
    let raw_keys = case["keys"].as_array().unwrap();
    let mut key_bytes = Vec::new();
    for key in raw_keys {
        key_bytes.extend_from_slice(&(key[0].as_i64().unwrap() as i16).to_le_bytes());
        key_bytes.extend_from_slice(&(key[1].as_u64().unwrap() as u16).to_le_bytes());
        for component in &key.as_array().unwrap()[2..] {
            key_bytes.extend_from_slice(&(component.as_u64().unwrap() as u32).to_le_bytes());
        }
    }
    let mut payload = Vec::new();
    for (tag, value) in [
        ("KeyC", raw_keys.len() as u32),
        ("BvPr", case["pre"].as_u64().unwrap() as u32),
        ("BvPo", case["post"].as_u64().unwrap() as u32),
        ("RanT", case["random"].as_u64().unwrap() as u32),
    ] {
        payload.extend(block(tag, &value.to_le_bytes()));
    }
    payload.extend(block("Keys", &key_bytes));
    payload
}

pub(crate) fn curve_from_original_case(case: &Value) -> AvfxCurve {
    parse_scalar_payload(&curve_payload_from_original_case(case))
}

pub(crate) fn color_from_original_case(case: &Value) -> AvfxColorCurve {
    let mut payload = Vec::new();
    for (name, field) in [
        "RGB", "A", "SclR", "SclG", "SclB", "SclA", "RanR", "RanG", "RanB", "RanA", "Bri", "RBri",
    ]
    .into_iter()
    .zip(case["fields"].as_array().unwrap())
    {
        if !field.is_null() {
            payload.extend(block(name, &curve_payload_from_original_case(field)));
        }
    }
    let bytes = block("AVFX", &block("Col", &payload));
    let root = AvfxNodeView::parse_root(&bytes).unwrap();
    parse_color_curve(&root.child("Col").unwrap())
}

fn compare_color_cases(cases: &[Value]) -> Value {
    use crate::avfx_sim::{
        VfxClientColorCurveDefaults, VfxClientColorCurveState, VfxClientRandomState,
    };
    let mut outputs = 0;
    let mut differences = 0;
    let mut state_checks = 0;
    let mut draws = 0;
    let mut examples = Vec::new();
    for (ordinal, case) in cases.iter().enumerate() {
        let color = color_from_original_case(case);
        let flags = color.client_dispatch_flags();
        assert_eq!(
            flags,
            case["flags"].as_u64().unwrap() as u32,
            "flags {ordinal}"
        );
        let seed = std::array::from_fn(|i| case["seed"][i].as_u64().unwrap() as u32);
        let mut random = VfxClientRandomState::from_words(seed);
        let state = VfxClientColorCurveState::construct(
            &color,
            VfxClientColorCurveDefaults {
                empty_random_type: case["emptyRandomType"].as_u64().unwrap() as u32,
                empty_rgba: std::array::from_fn(|i| float(&case["emptyRgba"][i])),
            },
            &mut random,
        );
        assert_eq!(
            json!(state.first_percentages()),
            case["first"],
            "First {ordinal}"
        );
        assert_eq!(
            json!(random.words()),
            case["afterFirst"],
            "after First {ordinal}"
        );
        let first_draws = color
            .random
            .iter()
            .filter(|c| {
                let mode = c
                    .as_ref()
                    .filter(|c| !c.keys.is_empty())
                    .map_or(case["emptyRandomType"].as_u64().unwrap() as u32, |c| {
                        c.random_type
                    });
                mode & 7 <= 2
            })
            .count();
        assert_eq!(first_draws, case["firstDraws"].as_u64().unwrap() as usize);
        draws += first_draws;
        state_checks += 1;
        for input in case["values"].as_array().unwrap() {
            let actual = state.evaluate(
                &color,
                float(&input["local"]),
                float(&input["total"]),
                &mut random,
            );
            let expected: [f32; 4] = std::array::from_fn(|i| float(&input["rgba"][i]));
            let step_draws = color
                .random
                .iter()
                .enumerate()
                .filter(|(i, c)| {
                    let bit = if *i == 4 { 12 } else { *i + 3 };
                    flags & (1 << bit) != 0
                        && c.as_ref()
                            .is_some_and(|c| (3..=5).contains(&(c.random_type & 7)))
                })
                .count();
            assert_eq!(step_draws, input["draws"].as_u64().unwrap() as usize);
            draws += step_draws;
            state_checks += 1;
            outputs += 1;
            if actual.map(canonical) != expected.map(canonical)
                || json!(random.words()) != input["state"]
            {
                differences += 1;
                if examples.len() < 20 {
                    examples.push(
                        json!({"case":ordinal,"profile":case["profile"],"mode":case["mode"],
                        "input":input,"actual":actual.map(canonical),"state":random.words()}),
                    );
                }
            }
        }
    }
    json!({"cases":cases.len(),"outputs":outputs,"stateChecks":state_checks,"draws":draws,
        "differences":differences,"examples":examples,"rawColorParserExecuted":true,
        "fullColorCompositionExecuted":true,"firstConstructorExecuted":true,
        "finiteInfZeroComparison":"bits","nanComparison":"classification",
        "startupDefaultsCaptured":false,"objectLifecycleProven":false,"gpuExecuted":false})
}

#[test]
fn color_dispatch_and_shared_random_match_original_golden() {
    let original: Value =
        serde_json::from_str(include_str!("avfx_sim/fixtures/color_curves_golden.json")).unwrap();
    let comparison = compare_color_cases(original["cases"].as_array().unwrap());
    assert_eq!(comparison["differences"], 0, "{}", comparison["examples"]);
}

#[test]
#[ignore = "CPU: requires original full Color compiler, First constructor and repeated shared TLS evaluation"]
fn compare_original_color_compilation_first_and_composition() {
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(folder.join("client-color-curves-client-probe.json")).unwrap(),
    )
    .unwrap();
    let cases = original["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 5632);
    let comparison = compare_color_cases(cases);
    assert_eq!(comparison["outputs"], original["outputs"]);
    assert_eq!(comparison["draws"], original["draws"]);
    std::fs::write(
        folder.join("client-color-curves-rust-comparison.json"),
        serde_json::to_vec_pretty(&comparison).unwrap(),
    )
    .unwrap();
    assert_eq!(comparison["differences"], 0, "{}", comparison["examples"]);
}

#[test]
#[ignore = "CPU: requires original raw scalar compiler and animated reader probe"]
fn compare_original_animated_scalar_compilation_and_evaluation() {
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(folder.join("client-animated-curves-client-probe.json")).unwrap(),
    )
    .unwrap();
    let cases = original["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8640);
    let mut differences = 0;
    let mut examples = Vec::new();
    let mut outputs = 0;
    let mut packed_keys = 0;
    let mut differences_by_layout = [0usize; 10];
    for (ordinal, case) in cases.iter().enumerate() {
        let curve = curve_from_original_case(case);
        let count = curve.keys.len();
        let reader = match count {
            0 => 0,
            1 => 128,
            _ => 256,
        };
        let header = if count == 0 {
            0
        } else {
            (count as u32) << 9
                | (curve.random_type & 7) << 4
                | (curve.post_behavior & 3) << 2
                | curve.pre_behavior & 3
                | reader
        };
        assert_eq!(
            header,
            case["header"].as_u64().unwrap() as u32,
            "header {ordinal}"
        );
        for (key, packed) in curve
            .keys
            .iter()
            .zip(case["packedKeys"].as_array().unwrap())
        {
            let pack = |raw: f32| {
                let scaled = raw * 15.0;
                if (-2147483648.0..2147483648.0).contains(&scaled) {
                    scaled as i32 as i8
                } else {
                    0
                }
            };
            let actual = json!([
                key.z.to_bits(),
                key.scalar_time() | (key.interpolation << 14),
                pack(key.x),
                pack(key.y)
            ]);
            assert_eq!(actual, *packed, "packed key {ordinal}:{packed_keys}");
            packed_keys += 1;
        }
        for input in case["values"].as_array().unwrap() {
            let local = float(&input[0]);
            let total = float(&input[1]);
            let actual = curve.value_at(local, total, 0.0);
            let expected = float(&input[2]);
            outputs += 1;
            if canonical(actual) != canonical(expected) {
                differences += 1;
                differences_by_layout[case["layout"].as_u64().unwrap() as usize] += 1;
                if examples.len() < 20 {
                    examples.push(json!({"case": ordinal, "layout": case["layout"],
                        "recipe": case["recipe"], "interpolation": case["interpolation"],
                        "pre": curve.pre_behavior, "post": curve.post_behavior,
                        "local": local, "total": total, "actual": canonical(actual),
                        "expected": canonical(expected)}));
                }
            }
        }
    }
    assert_eq!(outputs, original["outputs"].as_u64().unwrap() as usize);
    let ramp = scalar(&[(0, 1, 0.0), (100, 1, 100.0)]);
    for input in original["linearRamp"].as_array().unwrap() {
        assert_eq!(
            ramp.value(float(&input[0]), 0.0).to_bits(),
            float(&input[2]).to_bits()
        );
    }
    assert_eq!(ramp.value(15.0, 0.0).to_bits(), 0x41700001);
    std::fs::write(
        folder.join("client-animated-curves-rust-comparison.json"),
        serde_json::to_vec_pretty(&json!({"cases": cases.len(), "outputs": outputs,
            "packedKeys": packed_keys, "differences": differences,
            "differencesByLayout": differences_by_layout, "examples": examples,
            "finiteInfZeroComparison": "bits", "nanComparison": "classification",
            "rawScalarParserExecuted": true,
            "nonfiniteAgesProven": false, "fullFileCompilationProven": false}))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(differences, 0, "{examples:?}");
}

#[test]
#[ignore = "CPU: requires guarded original scalar extreme-age probe"]
fn compare_original_extreme_scalar_ages_and_cycle_conversion() {
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(folder.join("client-extreme-curves-client-probe.json")).unwrap(),
    )
    .unwrap();
    let cases = original["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 11232);
    let mut compared = 0usize;
    let mut invalid = 0usize;
    let mut differences = 0usize;
    let mut examples = Vec::new();
    let mut lookup_checks = 0usize;
    let mut mapping_differences = 0usize;
    for (ordinal, case) in cases.iter().enumerate() {
        let curve = curve_from_original_case(case);
        for input in case["values"].as_array().unwrap() {
            let local = float(&input[0]);
            let total = float(&input[1]);
            if input[3].as_i64().unwrap() >= 0 {
                let time = curve.runtime_time(local, total);
                let start = f32::from(curve.keys[0].scalar_time());
                let end = f32::from(curve.keys.last().unwrap().scalar_time());
                let mapped = if time < start {
                    repeat_scalar_curve_time(time, start, end, true).0
                } else if time > end {
                    repeat_scalar_curve_time(time, start, end, false).0
                } else {
                    time
                };
                let index = curve.scalar_segment_index(mapped);
                lookup_checks += 1;
                if canonical(mapped) != canonical(float(&input[4]))
                    || index != input[3].as_u64().unwrap() as usize
                {
                    mapping_differences += 1;
                    if examples.len() < 30 {
                        examples.push(json!({"case":ordinal,"input":input,
                            "mapped":canonical(mapped),"index":index}));
                    }
                }
            }
            if !input[5].as_bool().unwrap() {
                invalid += 1;
                continue;
            }
            let actual = curve.value_at(local, total, 0.0);
            let expected = float(&input[2]);
            compared += 1;
            if canonical(actual) != canonical(expected) {
                differences += 1;
                if examples.len() < 30 {
                    examples.push(json!({"case":ordinal,"layout":case["layout"],
                        "recipe":case["recipe"],"interpolation":case["interpolation"],
                        "pre":case["pre"],"post":case["post"],
                        "localBits":input[0],"totalBits":input[1],
                        "mappedBits":input[4],"segment":input[3],
                        "actual":canonical(actual),"expected":canonical(expected)}));
                }
            }
        }
    }
    assert_eq!(
        compared + invalid,
        original["outputs"].as_u64().unwrap() as usize
    );
    assert_eq!(
        invalid,
        original["invalidSegmentOutputs"].as_u64().unwrap() as usize
    );
    std::fs::write(
        folder.join("client-extreme-curves-rust-comparison.json"),
        serde_json::to_vec_pretty(&json!({"cases":cases.len(),"compared":compared,
            "invalidNativeSegmentsExcluded":invalid,"differences":differences,
            "mappingAndLookupChecks":lookup_checks,"mappingAndLookupDifferences":mapping_differences,
            "examples":examples,"finiteInfZeroComparison":"bits","nanComparison":"classification",
            "invalidNativeValuesProven":false,"zeroPeriodDivisionsProven":false,
            "rawScalarParserExecuted":true}))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(differences, 0, "{examples:?}");
    assert_eq!(mapping_differences, 0, "{examples:?}");
}

#[test]
#[ignore = "CPU: requires original RGB compiler, local color caller and guarded kernels"]
fn compare_original_rgb_compilation_and_local_clock_evaluation() {
    let folder =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(folder.join("client-rgb-curves-client-probe.json")).unwrap(),
    )
    .unwrap();
    let cases = original["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 17136);
    let mut outputs = 0usize;
    let mut differences = 0usize;
    let mut packed_keys = 0usize;
    let mut excluded = 0usize;
    let mut examples = Vec::new();
    for (ordinal, case) in cases.iter().enumerate() {
        let payload = curve_payload_from_original_case(case);
        let bytes = block("AVFX", &block("Col", &block("RGB", &payload)));
        let root = AvfxNodeView::parse_root(&bytes).unwrap();
        let color = parse_color_curve(&root.child("Col").unwrap());
        let curve = color.rgb.unwrap();
        let count = curve.keys.len();
        let header = if count == 0 {
            0
        } else {
            ((count as u32) << 9)
                | ((curve.post_behavior & 3) << 2)
                | (curve.pre_behavior & 3)
                | if count == 1 { 128 } else { 256 }
        };
        assert_eq!(
            header,
            case["header"].as_u64().unwrap() as u32,
            "header {ordinal}"
        );
        for (key, packed) in curve
            .keys
            .iter()
            .zip(case["packedKeys"].as_array().unwrap())
        {
            assert_eq!(
                json!([
                    (i32::from(key.time) as u32 & 0x0fff_ffff)
                        | ((u32::from(key.interpolation) & 15) << 28),
                    key.x.to_bits(),
                    key.y.to_bits(),
                    key.z.to_bits()
                ]),
                *packed,
                "packed key {ordinal}:{packed_keys}"
            );
            packed_keys += 1;
        }
        for input in case["values"].as_array().unwrap() {
            if !input[5].as_bool().unwrap() {
                excluded += 1;
                continue;
            }
            let actual = curve.color_at_times(float(&input[0]), float(&input[1]));
            let expected = std::array::from_fn::<_, 3, _>(|i| float(&input[2][i]));
            outputs += 1;
            if actual.map(canonical) != expected.map(canonical) {
                differences += 1;
                if examples.len() < 25 {
                    examples.push(json!({"case":ordinal,"layout":case["layout"],
                    "recipe":case["recipe"],"interpolation":case["interpolation"],"pre":case["pre"],
                    "post":case["post"],"input":input,"actual":actual.map(canonical),"expected":expected.map(canonical)}));
                }
            }
        }
    }
    assert_eq!(
        outputs + excluded,
        original["outputs"].as_u64().unwrap() as usize
    );
    assert_eq!(
        excluded,
        original["invalidSegmentOutputs"].as_u64().unwrap() as usize
    );
    std::fs::write(folder.join("client-rgb-curves-rust-comparison.json"),
        serde_json::to_vec_pretty(&json!({"cases":cases.len(),"outputs":outputs,"packedKeys":packed_keys,
            "differences":differences,"excludedUnsafeSearches":excluded,"examples":examples,"finiteInfZeroComparison":"bits",
            "nanComparison":"classification","rawColorParserExecuted":true,"localColorCallerExecuted":true,
            "zeroPeriodDivisionsProven":false,"fullColorCompositionProven":false})).unwrap()).unwrap();
    assert_eq!(differences, 0, "{examples:?}");
}
