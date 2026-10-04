//! CPU audit of real IMC mounts, owner geometry/materials and Aura arrays.
//! XIV_GAME_DIR=... cargo run --features game-data,render-test-support --example audit_model_skin_surfaces
#![cfg(all(feature = "game-data", feature = "render-test-support"))]

use anyhow::{Context, Result, ensure};
use physis::resource::SqPackResource;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
use xiv_companion::{
    WeaponModelLoadRequest,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    load_weapon_model_from_resource_request, load_weapon_vfx_attachments_from_resource,
};
use xiv_companion_data::{
    SkeletonPose, VFX_IDENTITY_BASIS, VfxBinderCameraSnapshot, VfxBinderObjectSnapshot,
    VfxBinderTargetSnapshot, VfxModelSkinCharacterTargets, VfxModelSkinSurface,
    VfxModelSkinTargetInput, VfxModelSkinTargetSnapshot, VfxPlayback, VfxRuntime,
};
use xiv_companion_render::{
    PreparedModelOptions, WeaponVfxAuraBindings, WeaponVfxAuraInput, WeaponVfxPlayback,
    prepare_weapon_vfx_aura_inputs,
};

fn no_nonfinite_json(value: &Value) -> bool {
    match value {
        Value::Null => false, // Serde JSON represents nonfinite f32 as null.
        Value::Array(values) => values.iter().all(no_nonfinite_json),
        Value::Object(values) => values.values().all(no_nonfinite_json),
        _ => true,
    }
}

fn audit_default_preview(
    mounts: &xiv_companion_data::WeaponVfxAttachments,
    inputs: &[WeaponVfxAuraInput],
    camera: VfxBinderCameraSnapshot,
    owner: &str,
    effect: &str,
    external: bool,
) -> Result<Value> {
    // This is the CPU object held by WeaponVfxParticles, not a separately
    // configured VfxRuntime. Inputs come from actual surface preparation.
    let mut playback =
        WeaponVfxPlayback::from_prepared_aura_inputs(mounts, inputs, Some(camera), None, |_| {
            Some([0.0; 3])
        });
    let mut bindings = WeaponVfxAuraBindings::from_prepared_inputs(inputs);
    let mut expected_targets = BTreeMap::<String, (u8, u64, usize, u64)>::new();
    let mut selected_resources = std::collections::BTreeSet::new();
    let mut selected_observations = 0;
    let mut selection_checks = 0;
    let mut initial_selection = Vec::new();
    let mut frozen_selection = Vec::new();
    let mut observed = std::collections::BTreeSet::new();
    let mut observations = 0;
    let mut packet_observations = 0;
    let mut frozen = Vec::new();
    let mut failure_checks = 0;
    let mut initial_state = None;
    for input in 0..=300 {
        if input == 80 {
            frozen = playback.observed_aura_instances().to_vec();
            frozen_selection = bindings.selected_instances();
            ensure!(!frozen.is_empty(), "no default Aura before failure window");
            playback.refresh_model_skin_targets(|_| false);
        }
        if input == 90 {
            playback.refresh_model_skin_targets(|_| true);
        }
        if input == 120 {
            for attachment in &mounts.attachments {
                if let Some(skeleton) = &attachment.data.skeleton {
                    let mut pose = SkeletonPose::rest_pose(skeleton);
                    pose.set_translation(
                        0,
                        std::array::from_fn(|axis| {
                            skeleton.rest_pose[0].translation[axis] + [0.25, -0.125, 0.5][axis]
                        }),
                    )?;
                    playback
                        .set_attachment_pose(&attachment.model_path, &pose)
                        .map_err(anyhow::Error::msg)?;
                }
            }
        }
        playback.sample(input as f32 / 60.0, 16384, |_| Some([0.0; 3]));
        ensure!(
            playback.fallback_reasons().is_empty(),
            "default preview input {input}: {:?}",
            playback.fallback_reasons()
        );
        bindings.update(input as f32 / 60.0, &playback);
        // Independent target-key history: APri occupies the high byte;
        // earlier production constructors have the larger inverted order.
        for active in playback.observed_aura_instances() {
            let resource = &inputs[active.resource_index];
            let serial = active
                .instance
                .aura_creation_order
                .context("default Aura has no shared creation order")?;
            let next = (
                resource.priority,
                u64::MAX - serial,
                active.resource_index,
                active.instance.instance_id,
            );
            expected_targets
                .entry(resource.target_model_path.clone())
                .and_modify(|old| {
                    if (next.0, next.1) > (old.0, old.1) {
                        *old = next;
                    }
                })
                .or_insert(next);
        }
        let selected = bindings.selected_instances();
        ensure!(
            selected.len() == expected_targets.len(),
            "missing resolved target at input {input}"
        );
        for active in &selected {
            let resource = &inputs[active.resource_index];
            let expected = expected_targets
                .get(&resource.target_model_path)
                .context("unexpected selected target")?;
            ensure!(
                (active.resource_index, active.instance.instance_id) == (expected.2, expected.3),
                "wrong selected target at input {input}"
            );
            ensure!(
                no_nonfinite_json(&serde_json::to_value(active.instance)?),
                "nonfinite selected Aura"
            );
            selected_resources.insert(active.resource_index);
        }
        selected_observations += selected.len();
        selection_checks += 1;
        if input == 0 {
            initial_state = Some(playback.random_state());
            initial_selection = selected
                .iter()
                .map(|i| (i.resource_index, i.instance.aura_creation_order))
                .collect::<Vec<_>>();
            initial_selection.sort_unstable();
        }
        if (80..90).contains(&input) {
            ensure!(
                playback.observed_aura_instances() == frozen,
                "default preview failed query changed complete packet at input {input}"
            );
            ensure!(
                selected == frozen_selection,
                "failed query changed selected bindings at input {input}"
            );
            failure_checks += 1;
        }
        for active in playback.aura_instances() {
            let resource = inputs
                .get(active.resource_index)
                .context("output refers to absent prepared resource")?;
            ensure!(
                resource.particle_index == active.instance.particle_index,
                "resource routed to wrong definition"
            );
            ensure!(
                no_nonfinite_json(&serde_json::to_value(active.instance)?),
                "nonfinite default Aura"
            );
            observed.insert((resource.attachment_index, resource.particle_index));
            observations += 1;
        }
        ensure!(
            playback.quads().iter().all(|quad| quad
                .position
                .into_iter()
                .chain(quad.orientation)
                .chain(quad.size)
                .chain(quad.parent_basis.into_iter().flatten())
                .chain(quad.color)
                .all(f32::is_finite)),
            "nonfinite default Quad"
        );
        ensure!(
            playback.meshes().iter().all(|mesh| mesh
                .position
                .into_iter()
                .chain(mesh.orientation)
                .chain(mesh.scale)
                .chain(mesh.parent_basis.into_iter().flatten())
                .chain(mesh.color)
                .all(f32::is_finite)),
            "nonfinite default mesh"
        );
        packet_observations += playback.quads().len() + playback.meshes().len();
        if external
            && input == 150
            && effect == "chara/weapon/w5741/obj/body/b0003/vfx/eff/vw0002.avfx"
        {
            ensure!(
                playback
                    .trigger_attachment_document(owner, 0, 10)
                    .map_err(anyhow::Error::msg)?,
                "default external target not attached"
            );
        }
    }
    let frozen = playback.observed_aura_instances().to_vec();
    let selected_before_pause = bindings.selected_instances();
    playback.refresh_model_skin_targets(|_| false);
    playback.sample(5.0, 16384, |_| Some([0.0; 3]));
    ensure!(
        playback.observed_aura_instances() == frozen,
        "paused default missing host changed parameters"
    );
    bindings.update(5.0, &playback);
    ensure!(
        bindings.selected_instances() == selected_before_pause,
        "paused target loss changed selected resource/parameters"
    );
    let paused_state = playback.random_state();
    playback.sample(5.0, 16384, |_| Some([0.0; 3]));
    ensure!(
        playback.random_state() == paused_state,
        "default read consumed random values"
    );
    playback.refresh_model_skin_targets(|_| true);
    playback.sample(5.0, 16384, |_| Some([0.0; 3]));
    ensure!(
        playback.fallback_reasons().is_empty(),
        "paused default recovery fallback"
    );
    playback.sample(0.0, 16384, |_| Some([0.0; 3]));
    bindings.update(0.0, &playback);
    let mut replay_selection = bindings
        .selected_instances()
        .iter()
        .map(|i| (i.resource_index, i.instance.aura_creation_order))
        .collect::<Vec<_>>();
    replay_selection.sort_unstable();
    ensure!(
        replay_selection == initial_selection,
        "group rewind changed resolved target creation keys"
    );
    ensure!(
        Some(playback.random_state()) == initial_state,
        "default group rewind changed constructor/input draw order"
    );
    let missing = inputs
        .iter()
        .map(|input| (input.attachment_index, input.particle_index))
        .filter(|key| !observed.contains(key))
        .collect::<std::collections::BTreeSet<_>>();
    ensure!(
        !external || missing.is_empty(),
        "default external window missed prepared definitions: {missing:?}"
    );
    Ok(
        json!({"path":effect,"defaultPreview":true,"attachmentCount":mounts.attachments.len(),
        "preparedDefinitions":inputs.iter().map(|i|(i.attachment_index,i.particle_index)).collect::<std::collections::BTreeSet<_>>(),
        "observedDefinitions":observed,"unobservedPreparedDefinitions":missing,
        "samples":301,"auraObservations":observations,"packetObservations":packet_observations,
        "selectedResourceIndexes":selected_resources,"selectedAuraObservations":selected_observations,
        "selectionChecks":selection_checks,"selectionReplayChecks":1,
        "hostFailureChecks":failure_checks,"pausedHostChecks":3,"replayStateChecks":1,
        "previewInitialRandomState":initial_state,"previewReplayedRandomState":playback.random_state(),
        "scope":"Actual WeaponVfxPlayback used by GPU mounts with its default shared random/Color/ModelSkin environments; loaded prepared surfaces, entire mount group, production Aura binding arbitration/retention, moving bone pose and explicit target loss/recovery. Camera/owner offsets are controlled; no GPU, ModelInstance query or complete client TLS/provider replay."}),
    )
}

fn main() -> Result<()> {
    let started = std::time::Instant::now();
    let args = std::env::args().collect::<Vec<_>>();
    let preview_curves = args.iter().any(|arg| arg == "--preview-curves");
    let model_skin_host = args.iter().any(|arg| arg == "--model-skin-host");
    let argument = |name: &str| -> Result<Option<&str>> {
        ensure!(
            !args.last().is_some_and(|arg| arg == name),
            "{name} requires a path"
        );
        Ok(args
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].as_str()))
    };
    let requested_output = argument("--output")?;
    let mounted_report = argument("--mounted-report")?
        .unwrap_or("target/weapon-vfx-audit/object-target-mounted-playback.json");
    let external = std::env::args().any(|arg| arg == "--external-trigger");
    let dir = normalize_game_dir(&std::path::PathBuf::from(std::env::var("XIV_GAME_DIR")?))?;
    let path = dir.to_str().context("UTF-8 game directory")?;
    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(path),
        dir.display().to_string(),
        game_version(&dir),
        "model-skin-surface-audit".into(),
    )?;
    let prior: Value = serde_json::from_slice(&fs::read(mounted_report)?)?;
    let manifest: Value =
        serde_json::from_slice(&fs::read("target/weapon-vfx-audit/report.json")?)?;
    let mut selected = BTreeMap::new();
    for record in prior["records"].as_array().context("mounted records")? {
        if !record["fallback"]
            .as_str()
            .is_some_and(|reason| reason.contains("ModelSkin"))
        {
            continue;
        }
        let effect = record["path"].as_str().unwrap();
        let entry = manifest["models"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|model| model["entries"].as_array().unwrap())
            .find(|entry| entry["avfxPath"].as_str() == Some(effect))
            .context("missing authored IMC mount")?;
        let raw = entry["modelId"].as_u64().unwrap()
            | entry["bodyId"].as_u64().unwrap() << 16
            | entry["variant"].as_u64().unwrap() << 32;
        selected.insert(effect.to_string(), raw);
    }
    ensure!(
        selected.len() == 7,
        "expected the seven currently missing surfaces"
    );
    let mut resource = SqPackResource::from_existing(path);
    let mut records = Vec::new();
    for (effect, authored_raw) in selected {
        let result = (|| -> Result<Value> {
            let item = catalog
                .items
                .iter()
                .find(|item| item.model_main == authored_raw || item.model_sub == authored_raw);
            let request =
                item.map(WeaponModelLoadRequest::from)
                    .unwrap_or(WeaponModelLoadRequest {
                        item_id: 0,
                        item_name: "authored IMC variant (no catalog item)".into(),
                        model_main: authored_raw,
                        model_sub: 0,
                        stain_ids: [0, 0],
                    });
            let model = load_weapon_model_from_resource_request(&mut resource, &request)?;
            let mounts = load_weapon_vfx_attachments_from_resource(&mut resource, &model)?
                .context("no IMC-mounted effect")?;
            let attachment_index = mounts
                .attachments
                .iter()
                .position(|mount| mount.data.avfx_path == effect)
                .context("authored variant did not load the expected effect")?;
            let attachment = &mounts.attachments[attachment_index];
            let (all_inputs, diagnostics) =
                prepare_weapon_vfx_aura_inputs(&model, &mounts, PreparedModelOptions::default());
            let inputs = all_inputs
                .iter()
                .filter(|input| input.attachment_index == attachment_index)
                .collect::<Vec<_>>();
            ensure!(
                !inputs.is_empty(),
                "no prepared compatible Aura resources: {diagnostics:?}"
            );
            let indexes = inputs
                .iter()
                .map(|input| input.particle_index)
                .collect::<Vec<_>>();
            let skeleton = attachment
                .data
                .skeleton
                .as_ref()
                .context("missing owner skeleton")?;
            let targets = attachment
                .data
                .binder_targets_for_pose(&SkeletonPose::rest_pose(skeleton))
                .map_err(anyhow::Error::msg)?;
            let camera = VfxBinderCameraSnapshot {
                basis: VFX_IDENTITY_BASIS,
                parallel_direction: [0.0, 0.0, 1.0],
                position: [0.0, 0.0, 3.0],
            };
            if preview_curves {
                let mut report = audit_default_preview(
                    &mounts,
                    &all_inputs,
                    camera,
                    &attachment.model_path,
                    &effect,
                    external,
                )?;
                report["authoredModelRaw"] = json!(authored_raw);
                report["catalogItem"] =
                    json!(item.map(|item| json!({"id":item.id,"name":item.name})));
                report["loadedPaths"] = json!(model.loaded_paths);
                report["diagnostics"] = json!(diagnostics);
                return Ok(report);
            }
            let source = attachment
                .data
                .runtime()
                .with_binder_target_snapshot(&targets)
                .and_then(|v| v.with_binder_object_snapshot(VfxBinderObjectSnapshot::IDENTITY))
                .and_then(|v| v.with_binder_camera_snapshot(camera))
                .map_err(anyhow::Error::msg)?;
            let surface = |path: Option<&str>, identity| {
                path.filter(|path| inputs.iter().any(|input| input.target_model_path == *path))
                    .map(|_| VfxModelSkinSurface {
                        identity,
                        type_code: 3,
                    })
            };
            let host_input = VfxModelSkinTargetInput {
                listener_present: true,
                character: Some(VfxModelSkinCharacterTargets {
                    generation: 1,
                    surfaces: [
                        None,
                        surface(mounts.model_skin_targets.weapon.as_deref(), 2),
                        surface(mounts.model_skin_targets.off_hand.as_deref(), 4),
                        None,
                    ],
                }),
                ..Default::default()
            };
            let install_host = |source: VfxRuntime| {
                if model_skin_host {
                    source.with_model_skin_target_snapshot(VfxModelSkinTargetSnapshot {
                        selector: -1,
                        input: host_input,
                    })
                } else {
                    source
                }
            };
            let mut playback = VfxPlayback::new_with_model_skin(install_host(source), &indexes);
            ensure!(
                playback.fallback_reason().is_none(),
                "{:?}",
                playback.fallback_reason()
            );
            let mut neutral = attachment.data.file.clone();
            for binder in &mut neutral.binders {
                if binder.binder_type == 0 {
                    binder.start_to_global_direction = false;
                }
            }
            let control_source =
                VfxRuntime::with_bind_points(&neutral, &attachment.data.bind_points)
                    .with_binder_target_snapshot(&targets)
                    .and_then(|v| v.with_binder_object_snapshot(VfxBinderObjectSnapshot::IDENTITY))
                    .and_then(|v| v.with_binder_camera_snapshot(camera))
                    .map_err(anyhow::Error::msg)?;
            let mut control =
                VfxPlayback::new_with_model_skin(install_host(control_source), &indexes);
            let mut pose = SkeletonPose::rest_pose(skeleton);
            pose.set_translation(
                0,
                std::array::from_fn(|a| {
                    skeleton.rest_pose[0].translation[a] + [0.25, -0.125, 0.5][a]
                }),
            )?;
            let moved_targets = attachment
                .data
                .binder_targets_for_pose(&pose)
                .map_err(anyhow::Error::msg)?;
            let mut observations = 0;
            let mut observed_definitions = std::collections::BTreeSet::new();
            let mut quads = Vec::new();
            let mut meshes = Vec::new();
            let mut control_quads = Vec::new();
            let mut control_meshes = Vec::new();
            let mut packet_observations = 0;
            let mut frozen = Vec::new();
            let mut host_failure_checks = 0;
            for input in 0..=300 {
                if model_skin_host && input == 80 {
                    frozen = playback.observed_model_skin_instances();
                    ensure!(
                        !frozen.is_empty(),
                        "no successful Aura before host-loss check"
                    );
                    for value in [&mut playback, &mut control] {
                        value
                            .set_model_skin_target_input(VfxModelSkinTargetInput {
                                listener_present: false,
                                ..host_input
                            })
                            .map_err(anyhow::Error::msg)?;
                    }
                }
                if model_skin_host && input == 90 {
                    for value in [&mut playback, &mut control] {
                        value
                            .set_model_skin_target_input(host_input)
                            .map_err(anyhow::Error::msg)?;
                    }
                }
                let object = if input >= 180 {
                    VfxBinderObjectSnapshot {
                        position: [0.125, 0.25, -0.125],
                        quaternion: [0.0, 0.0, 0.6, 0.8],
                        scale: [1.25, 0.75, 1.5],
                    }
                } else {
                    VfxBinderObjectSnapshot::IDENTITY
                };
                let bones = if input >= 120 {
                    &moved_targets
                } else {
                    &targets
                };
                let placed = bones
                    .iter()
                    .map(|target| VfxBinderTargetSnapshot {
                        id: target.id,
                        matrix: object.matrix().transform_matrix(target.matrix),
                    })
                    .collect::<Vec<_>>();
                for value in [&mut playback, &mut control] {
                    value
                        .update_preview_binder_sources_to(
                            input as f32 / 60.0,
                            Some(&placed),
                            Some(object),
                            Some(camera),
                        )
                        .map_err(anyhow::Error::msg)?;
                }
                ensure!(
                    playback.fallback_reason().is_none(),
                    "input {input}: {:?}",
                    playback.fallback_reason()
                );
                ensure!(
                    control.fallback_reason().is_none(),
                    "control input {input}: {:?}",
                    control.fallback_reason()
                );
                if model_skin_host && (80..90).contains(&input) {
                    ensure!(
                        playback.observed_model_skin_instances() == frozen,
                        "failed target query changed complete observed Aura packet at input {input}"
                    );
                    host_failure_checks += 1;
                }
                let instances = playback.model_skin_instances();
                ensure!(
                    instances == control.model_skin_instances(),
                    "Point bStG changed Aura parameters"
                );
                for instance in instances {
                    ensure!(
                        indexes.contains(&instance.particle_index),
                        "unprepared definition"
                    );
                    ensure!(
                        no_nonfinite_json(&serde_json::to_value(instance)?),
                        "nonfinite Aura parameters"
                    );
                    observations += 1;
                    observed_definitions.insert(instance.particle_index);
                }
                playback.sample(&mut quads, &mut meshes);
                control.sample(&mut control_quads, &mut control_meshes);
                ensure!(
                    quads == control_quads && meshes == control_meshes,
                    "Point bStG changed geometry"
                );
                ensure!(
                    quads.iter().all(|q| q
                        .position
                        .into_iter()
                        .chain(q.orientation)
                        .chain(q.size)
                        .chain(q.parent_basis.into_iter().flatten())
                        .chain(q.color)
                        .all(f32::is_finite)),
                    "nonfinite Quad"
                );
                ensure!(
                    meshes.iter().all(|m| m
                        .position
                        .into_iter()
                        .chain(m.orientation)
                        .chain(m.scale)
                        .chain(m.parent_basis.into_iter().flatten())
                        .chain(m.color)
                        .all(f32::is_finite)),
                    "nonfinite mesh"
                );
                packet_observations += quads.len() + meshes.len();
                if external
                    && input == 150
                    && effect == "chara/weapon/w5741/obj/body/b0003/vfx/eff/vw0002.avfx"
                {
                    // Call between inputs: no Time or Prepare is implicit.
                    // Original Document converts external number 10 to slot 9.
                    for value in [&mut playback, &mut control] {
                        ensure!(
                            value.trigger_document(0, 10).map_err(anyhow::Error::msg)?,
                            "external target not attached"
                        );
                    }
                }
            }
            let mut paused_host_checks = 0;
            if model_skin_host {
                let frozen = playback.observed_model_skin_instances();
                playback
                    .set_model_skin_target_input(VfxModelSkinTargetInput {
                        listener_present: false,
                        ..host_input
                    })
                    .map_err(anyhow::Error::msg)?;
                ensure!(
                    playback.update_to(5.0),
                    "paused missing-host update rejected"
                );
                ensure!(
                    playback.observed_model_skin_instances() == frozen,
                    "paused missing host changed parameters"
                );
                ensure!(
                    playback.update_to(5.0),
                    "repeated paused timestamp rejected"
                );
                ensure!(
                    playback.observed_model_skin_instances() == frozen,
                    "read changed missing-host parameters"
                );
                playback
                    .set_model_skin_target_input(host_input)
                    .map_err(anyhow::Error::msg)?;
                ensure!(playback.update_to(5.0), "paused host recovery rejected");
                ensure!(
                    playback.fallback_reason().is_none(),
                    "paused recovery caused fallback"
                );
                ensure!(
                    playback
                        .model_skin_instances()
                        .iter()
                        .all(|instance| no_nonfinite_json(
                            &serde_json::to_value(instance).unwrap()
                        )),
                    "nonfinite recovered Aura packet"
                );
                paused_host_checks = 3;
            }
            ensure!(observations > 0, "no active Aura instances observed");
            let unobserved = indexes
                .iter()
                .copied()
                .filter(|index| !observed_definitions.contains(index))
                .collect::<Vec<_>>();
            ensure!(
                !external || unobserved.is_empty(),
                "external window missed prepared Aura definitions"
            );
            if !unobserved.is_empty() {
                fs::write(
                    "target/weapon-vfx-audit/model-skin-unobserved-definition.json",
                    serde_json::to_vec_pretty(&json!({"path":effect,"prepared":indexes,
                        "observed":observed_definitions,"file":attachment.data.file}))?,
                )?;
                // This unmodified file has a second Aura exclusively behind
                // an external Scheduler trigger, absent from normal startup.
                // Verify that exact graph rather than equating resource
                // preparation with active playback or editing its Timeline.
                let file = &attachment.data.file;
                ensure!(
                    effect == "chara/weapon/w5741/obj/body/b0003/vfx/eff/vw0002.avfx"
                        && unobserved == [11],
                    "unexpected unobserved Aura definition"
                );
                ensure!(
                    file.schedulers.len() == 1
                        && file.schedulers[0]
                            .items
                            .iter()
                            .filter(|item| item.enabled)
                            .all(|item| item.timeline_index == 0)
                        && file.timelines[0].clips.is_empty()
                        && file
                            .emitters
                            .iter()
                            .all(|emitter| emitter.emitter_items.is_empty()),
                    "external-only source graph changed"
                );
                let origins = file
                    .emitters
                    .iter()
                    .enumerate()
                    .flat_map(|(index, emitter)| {
                        emitter
                            .particle_items
                            .iter()
                            .filter(|item| item.enabled && item.target_index == 11)
                            .map(move |_| index)
                    })
                    .collect::<Vec<_>>();
                ensure!(
                    origins == [2]
                        && file.timelines[0]
                            .items
                            .iter()
                            .all(|item| !item.enabled || item.emitter_index != 2),
                    "Aura 11 acquired a normal startup source"
                );
                ensure!(
                    file.schedulers[0].triggers[9].timeline_index == 4
                        && file.timelines[4]
                            .items
                            .iter()
                            .any(|item| item.enabled && item.emitter_index == 2),
                    "external trigger 9 target changed"
                );
            }
            Ok(json!({
                "path": effect, "authoredModelRaw": authored_raw,
                "catalogItem": item.map(|item| json!({"id": item.id,"name":item.name})),
                "loadedPaths":model.loaded_paths, "modelDiagnostics":model.load_diagnostics,
                "ownerModel":attachment.model_path, "modelMeshes":model.meshes.len(),
                "materials":model.materials.iter().map(|m| json!({"slot":m.slot,"path":m.path,"shader":m.shader_package_name})).collect::<Vec<_>>(),
                "auraInputs":inputs.iter().map(|i| json!({"particle":i.particle_index,"target":i.target_model_path,
                    "width":i.packed.width,"height":i.packed.height,"slotLayers":i.packed.slot_layers,
                    "mips":i.packed.mips.len(),"bytes":i.packed.mips.iter().map(|m|m.rgba.len()).sum::<usize>(),
                    "border":i.border,"filter":i.filter})).collect::<Vec<_>>(),
                "externalTrigger":if external && effect == "chara/weapon/w5741/obj/body/b0003/vfx/eff/vw0002.avfx" {Some(json!({"scheduler":0,"externalNumber":10,"mappingSlot":9,"timeline":4,"afterInput":150,"beforeInput":151}))} else {None},
                "modelSkinHostRequested":model_skin_host,"hostFailureChecks":host_failure_checks,"pausedHostChecks":paused_host_checks,
                "diagnostics":diagnostics,"samples":301,"auraObservations":observations,
                "observedDefinitions":observed_definitions,"packetObservations":packet_observations,
                "unobservedPreparedDefinitions":unobserved,
                "unobservedScope":"When nonempty: unchanged w5741 Ptcl11 is sourced only by Emit2 in Timeline4 behind external Scheduler trigger9; resource prepared, external playback not exercised",
            }))
        })();
        records.push(match result {
            Ok(value) => value,
            Err(error) => json!({"path":effect,"error":format!("{error:#}")}),
        });
        eprintln!(
            "ModelSkin surface audit: {} / 7, {:.1}s",
            records.len(),
            started.elapsed().as_secs_f64()
        );
    }
    let errors = records
        .iter()
        .filter(|record| record.get("error").is_some())
        .count();
    let output = if external {
        "target/weapon-vfx-audit/scheduler-input-real-surfaces.json"
    } else {
        "target/weapon-vfx-audit/model-skin-surfaces.json"
    };
    let output = requested_output.unwrap_or(output);
    let summary = json!({"defaultPreviewRequested":preview_curves,"modelSkinHostRequested":model_skin_host,
        "defaultPreviewScope":"When requested: production WeaponVfxPlayback and default shared stream/Color/ModelSkin environments, all loaded mounts, 301 inputs, pose movement, query loss/paused recovery and group replay RNG equality. Not a captured live TLS stream, full original Aura comparison or GPU pixels.",
        "replayStateChecks":records.iter().filter_map(|r|r["replayStateChecks"].as_u64()).sum::<u64>(),
        "selectionChecks":records.iter().filter_map(|r|r["selectionChecks"].as_u64()).sum::<u64>(),
        "selectionReplayChecks":records.iter().filter_map(|r|r["selectionReplayChecks"].as_u64()).sum::<u64>(),
        "hostFailureChecks":records.iter().filter_map(|r| r["hostFailureChecks"].as_u64()).sum::<u64>(),
        "pausedHostChecks":records.iter().filter_map(|r| r["pausedHostChecks"].as_u64()).sum::<u64>(),
        "modelSkinHostScope":"When requested: preview caster selector -1, generation1 and fixed weapon/off-hand identities selected from prepared resources; inputs80..89 missing listener preserve complete observed packets; paused loss/read/recovery at5s. This is explicit CPU host input, not the ModelInstance GPU adapter or full game IFY.",
        "externalTriggerRequested":external,"externalScope":"When requested: unchanged w5741 Scheduler0 external number10 maps to slot9/Timeline4/Ptcl11 between inputs150 and151, no implicit Time/Prepare; other mounts use ordinary startup.","files":records.len(),"errors":errors,"elapsedSeconds":started.elapsed().as_secs_f64(),
        "scope":if preview_curves { "Actual authored IMC variant groups and production default WeaponVfxPlayback; 301 CPU samples, moving bone pose at input120, missing prepared targets at80..89, paused loss/read/recovery and replay RNG equality. No independent object transform or bStG comparison in this mode, GPU upload, pixels or live client host equivalence." } else { "Actual authored IMC variant mounts; catalog main/off-hand when available; loaded owner meshes/materials and production Aura target/texture/sampler checks; 301 CPU samples, skeleton root translation at input120, independent object transform plus composed bone targets at input180, exact Point-bStG on/off geometry/Aura equivalence and finite values. No GPU upload, pixels or live client host equivalence." }});
    fs::write(
        output,
        serde_json::to_vec_pretty(&json!({"summary":summary,"records":records}))?,
    )?;
    println!("{summary}");
    ensure!(errors == 0, "surface audit errors; see {output}");
    Ok(())
}
