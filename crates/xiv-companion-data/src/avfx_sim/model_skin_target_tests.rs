use super::*;
use serde_json::{Value, json};

fn surface(identity: u64, type_code: u8) -> VfxModelSkinSurface {
    VfxModelSkinSurface {
        identity,
        type_code,
    }
}
fn compare(cases: &[Value]) -> Value {
    let mut boundaries = 0;
    for case in cases {
        let host = case["host"].as_u64().unwrap();
        let selector = case["selector"].as_i64().unwrap() as i32;
        let target = case["target"].as_u64().unwrap() as u32;
        let mut state = VfxModelSkinTargetState {
            selector,
            cached_generation: case["initialGeneration"].as_u64().unwrap() as u32,
        };
        let mut flags = case["initialFlags"].as_u64().unwrap() as u32;
        for step in case["steps"].as_array().unwrap() {
            let types = step["types"].as_array().unwrap();
            let surfaces = std::array::from_fn(|index| {
                if (host == 6 && index == 0) || (host == 7 && index == 2) {
                    None
                } else {
                    Some(surface(
                        index as u64 + 1,
                        types[index].as_u64().unwrap() as u8,
                    ))
                }
            });
            let generation = step["generation"].as_u64().unwrap() as u32;
            let input = VfxModelSkinTargetInput {
                listener_present: step["listener"].as_bool().unwrap(),
                character: step["character"].as_bool().unwrap().then_some(
                    VfxModelSkinCharacterTargets {
                        generation,
                        surfaces,
                    },
                ),
                fallback_enabled: step["fallbackEnabled"].as_bool().unwrap(),
                fallback: step["fallback"].as_bool().unwrap().then_some((
                    surface(5, types[4].as_u64().unwrap() as u8),
                    generation as u16,
                )),
            };
            let result = state.query(target, input);
            let status = match result.status {
                VfxModelSkinTargetStatus::Missing => 0,
                VfxModelSkinTargetStatus::Ready => 1,
                VfxModelSkinTargetStatus::GenerationChanged => 2,
            };
            assert_eq!(status, step["status"], "{case}");
            assert_eq!(state.cached_generation, step["cachedGeneration"]);
            assert_eq!(state.cached_generation, step["numericGeneration"]);
            let run_aura = result.begin_numeric(&mut flags);
            assert_eq!(flags, step["flags"]);
            let native = step["numericCalls"].as_array().unwrap();
            assert_eq!(native[7], 1); // Actual wrapper always invokes base first.
            assert_eq!(native[8], u32::from(run_aura));
            let successful_aura = run_aura && step["auraSuccess"].as_bool().unwrap();
            let plan = result.registrations(step["auraSuccess"].as_bool().unwrap());
            assert_eq!(native[9], u32::from(successful_aura && target == 16));
            assert_eq!(
                native[11],
                if successful_aura && target == 16 {
                    2
                } else {
                    0
                }
            );
            if let Some(list) = result.list {
                let ids = list
                    .surfaces
                    .map(|surface| surface.map_or(0, |surface| surface.identity));
                assert_eq!(json!(ids), step["list"]);
                assert_eq!(list.body_filter, step["filter"]);
                let registered = if successful_aura {
                    ids.into_iter().filter(|&id| id != 0).count()
                } else {
                    0
                };
                assert_eq!(native[10], registered);
                let registrations = plan
                    .into_iter()
                    .map(|request| {
                        let VfxModelSkinRegistration::Surface {
                            surface,
                            material_filters,
                        } = request
                        else {
                            panic!("surface query registered through Document");
                        };
                        json!([surface.identity, material_filters])
                    })
                    .collect::<Vec<_>>();
                assert_eq!(json!(registrations), step["registrations"]);
            } else {
                assert!(step["list"].is_null());
                assert_eq!(native[10], 0);
                assert_eq!(step["registrations"], json!([]));
                assert_eq!(
                    plan,
                    if successful_aura {
                        vec![VfxModelSkinRegistration::Document]
                    } else {
                        Vec::new()
                    }
                );
            }
            for index in 0..7 {
                assert_eq!(native[index], step["queryCalls"][index]);
            }
            boundaries += 1;
        }
    }
    json!({"cases":cases.len(),"boundaries":boundaries,"differences":0,
        "scope":"Actual original target-query and outer +100 dispatch versus explicit resolved-host target state and numeric gate; listener callbacks, base Numeric and Aura are controlled; not connected to retained particles/full host/Aura parameters/GPU"})
}

#[test]
fn modelskin_target_state_matches_original_golden() {
    let original: Value =
        serde_json::from_str(include_str!("fixtures/modelskin_target_golden.json")).unwrap();
    compare(original["cases"].as_array().unwrap());
}

#[test]
#[ignore = "CPU: actual original ModelSkin target query and outer numeric branches"]
fn compare_original_modelskin_target_query() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/weapon-vfx-audit");
    let original: Value = serde_json::from_slice(
        &std::fs::read(directory.join("modelskin-target-client-probe.json")).unwrap(),
    )
    .unwrap();
    let report = compare(original["cases"].as_array().unwrap());
    assert_eq!(report["cases"], 16128);
    std::fs::write(
        directory.join("modelskin-target-rust-comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}

#[test]
fn modelskin_target_keeps_generation_from_an_empty_surface_query() {
    let mut state = VfxModelSkinTargetState::new(-1);
    let mut input = VfxModelSkinTargetInput {
        listener_present: true,
        character: Some(VfxModelSkinCharacterTargets {
            generation: 7,
            surfaces: [None; 4],
        }),
        ..Default::default()
    };
    assert_eq!(
        state.query(1, input).status,
        VfxModelSkinTargetStatus::Missing
    );
    assert_eq!(state.cached_generation, 7);
    input.character.as_mut().unwrap().generation = 8;
    input.character.as_mut().unwrap().surfaces[0] = Some(surface(10, 3));
    let result = state.query(1, input);
    assert_eq!(result.status, VfxModelSkinTargetStatus::GenerationChanged);
    let mut flags = 0x3f000000;
    assert!(!result.begin_numeric(&mut flags));
    assert_eq!(flags, 0x40000);
    flags = 0x3f040000;
    assert!(!result.begin_numeric(&mut flags));
    assert_eq!(flags, 0x3f040000);
}

#[test]
fn modelskin_target_stops_filtering_after_first_compatible_slot() {
    let mut state = VfxModelSkinTargetState::new(0);
    let surfaces = [
        Some(surface(1, 2)),
        Some(surface(2, 0x13)),
        Some(surface(3, 2)),
        Some(surface(4, 3)),
    ];
    let input = VfxModelSkinTargetInput {
        listener_present: true,
        character: Some(VfxModelSkinCharacterTargets {
            generation: 7,
            surfaces,
        }),
        ..Default::default()
    };
    let query = state.query(0x3ef, input);
    assert_eq!(query.status, VfxModelSkinTargetStatus::Ready);
    assert_eq!(
        query.list.unwrap().surfaces,
        [None, surfaces[1], surfaces[2], surfaces[3]]
    );
    assert_eq!(query.list.unwrap().body_filter, 0x3e0);
}

#[test]
fn modelskin_target_exact_special_value_bypasses_listener_and_keeps_output_unwritten() {
    let mut state = VfxModelSkinTargetState {
        selector: -1,
        cached_generation: 7,
    };
    let query = state.query(16, VfxModelSkinTargetInput::default());
    assert_eq!(query.status, VfxModelSkinTargetStatus::Ready);
    assert_eq!(query.list, None);
    assert_eq!(state.cached_generation, 7);
    assert_eq!(
        state
            .query(0x410, VfxModelSkinTargetInput::default())
            .status,
        VfxModelSkinTargetStatus::Missing
    );
}

#[test]
fn modelskin_target_fallback_requires_negative_selector_and_retains_zero_generation() {
    let input = VfxModelSkinTargetInput {
        listener_present: true,
        fallback_enabled: true,
        fallback: Some((surface(5, 3), 0)),
        ..Default::default()
    };
    let mut current = VfxModelSkinTargetState::new(-2);
    assert_eq!(
        current.query(1, input).status,
        VfxModelSkinTargetStatus::Ready
    );
    assert_eq!(current.cached_generation, 0);
    let mut next = input;
    next.fallback.as_mut().unwrap().1 = 65535;
    assert_eq!(
        current.query(1, next).status,
        VfxModelSkinTargetStatus::Ready
    );
    assert_eq!(current.cached_generation, 65535);
    assert_eq!(
        VfxModelSkinTargetState::new(0).query(1, input).status,
        VfxModelSkinTargetStatus::Missing
    );
}
