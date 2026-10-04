//! Unmodified installed END/KILL/ULLP/REST target admission with real MDL/rest-pose sources.
//! CPU only; co-referenced unsupported controls remain errors, not disabled.
//! XIV_GAME_DIR=... cargo run --features game-data --example audit_end_controls
#![cfg(feature = "game-data")]

use anyhow::{Context, Result, ensure};
use physis::resource::{Resource, SqPackResource};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};
use xiv_companion_data::{
    AvfxFile, PackedModelId, SkeletonPose, VFX_IDENTITY_BASIS, VfxBindPoint,
    VfxBinderCameraSnapshot, VfxBinderObjectSnapshot, VfxPlayback, VfxRuntime,
    load_skeleton_from_sklb_bytes, mdl_metadata::mdl_element_ids_from_mdl_bytes,
    weapon_model_candidate_paths, weapon_skeleton_path,
};

fn main() -> Result<()> {
    let started = std::time::Instant::now();
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let mut kind = "end";
    let mut output = None;
    let mut index = 0;
    while index < arguments.len() {
        let value = arguments.get(index + 1).context("argument value")?;
        match arguments[index].as_str() {
            "--kind" => {
                ensure!(
                    matches!(value.as_str(), "end" | "kill" | "unlockLoopPoint" | "reset"),
                    "control kind"
                );
                kind = value;
            }
            "--output" => output = Some(value.as_str()),
            _ => anyhow::bail!("unknown argument"),
        }
        index += 2;
    }
    let default_output = format!("target/weapon-vfx-audit/{kind}-clip-installed.json");
    let output = output.unwrap_or(&default_output);
    let dir = xiv_companion::game_data::normalize_game_dir(&std::path::PathBuf::from(
        std::env::var("XIV_GAME_DIR")?,
    ))?;
    let mut resource = SqPackResource::from_existing(dir.to_str().context("game directory UTF-8")?);
    let inventory: Value = serde_json::from_slice(&fs::read(
        "target/weapon-vfx-audit/clip-control-installed.json",
    )?)?;
    let manifest: Value =
        serde_json::from_slice(&fs::read("target/weapon-vfx-audit/report.json")?)?;
    let mut owners = BTreeMap::new();
    for model in manifest["models"].as_array().context("models")? {
        for entry in model["entries"].as_array().context("entries")? {
            if let Some(path) = entry["avfxPath"].as_str() {
                owners.insert(
                    path,
                    (
                        entry["modelId"].as_u64().unwrap() as u16,
                        entry["bodyId"].as_u64().unwrap() as u16,
                    ),
                );
            }
        }
    }
    let mut records = Vec::new();
    let mut errors = BTreeMap::<String, usize>::new();
    let mut definitions = 0;
    let mut admitted = 0;
    let mut input_cases = 0;
    let mut samples = 0;
    let mut warnings = 0;
    for entry in inventory["files"].as_array().context("control files")? {
        let controls = entry["controls"]
            .as_array()
            .context("controls")?
            .iter()
            .filter(|control| control["type"] == kind)
            .collect::<Vec<_>>();
        if controls.is_empty() {
            continue;
        }
        let path = entry["path"].as_str().context("AVFX path")?;
        let bytes = resource
            .read(path)
            .with_context(|| format!("missing {path}"))?;
        let file = AvfxFile::parse(&bytes)?;
        warnings += file.warnings.len();
        let (model_id, body_id) = owners[path];
        let mdl = weapon_model_candidate_paths(PackedModelId::from_raw(
            u64::from(model_id) | (u64::from(body_id) << 16),
        ))
        .into_iter()
        .next()
        .context("owner model path")?;
        let points = mdl_element_ids_from_mdl_bytes(&resource.read(&mdl).context("owner model")?)?
            .into_iter()
            .map(|p| VfxBindPoint {
                id: p.id,
                parent_bone: p.parent_bone,
                translate: p.translate,
                rotate: p.rotate,
            })
            .collect::<Vec<_>>();
        let skeleton = load_skeleton_from_sklb_bytes(
            &resource
                .read(&weapon_skeleton_path(model_id))
                .context("owner skeleton")?,
        )?;
        let targets = VfxRuntime::bind_point_pose_targets(
            &points,
            &skeleton,
            &SkeletonPose::rest_pose(&skeleton),
        )
        .map_err(anyhow::Error::msg)?;
        let runtime = VfxRuntime::with_bind_points(&file, &points)
            .with_binder_target_snapshot(&targets)
            .and_then(|v| v.with_binder_object_snapshot(VfxBinderObjectSnapshot::IDENTITY))
            .and_then(|v| {
                v.with_binder_camera_snapshot(VfxBinderCameraSnapshot {
                    basis: VFX_IDENTITY_BASIS,
                    parallel_direction: [0.0, 0.0, 1.0],
                    position: [0.0, 0.0, 3.0],
                })
            })
            .map_err(anyhow::Error::msg)?;
        for control in controls {
            definitions += 1;
            let co_referenced_controls = file.timelines
                [control["timeline"].as_u64().unwrap() as usize]
                .items
                .iter()
                .filter(|item| item.enabled)
                .filter_map(|item| {
                    // Compiled target priority and signed-byte references.
                    if (item.effector_index as i8) >= 0
                        || (item.emitter_index as i8) >= 0
                        || (item.clip_index as i8) < 0
                    {
                        return None;
                    }
                    let index = item.clip_index as i8 as usize;
                    file.timelines[control["timeline"].as_u64().unwrap() as usize]
                        .clips
                        .get(index)
                        .and_then(|clip| clip.clip_type)
                        .map(|kind| format!("{kind:?}"))
                })
                .collect::<Vec<_>>();
            for source in control["schedulerSources"]
                .as_array()
                .context("END sources")?
            {
                for slot in source["externalMappingSlots"]
                    .as_array()
                    .context("END slots")?
                {
                    input_cases += 1;
                    let scheduler = source["scheduler"].as_u64().unwrap() as i32;
                    let number = slot.as_u64().unwrap() as i32 + 1;
                    let mut playback = VfxPlayback::new(runtime.clone());
                    if let Some(reason) = playback.fallback_reason() {
                        *errors.entry(reason.to_owned()).or_default() += 1;
                        records.push(json!({"path":path,"timeline":control["timeline"],
                            "scheduler":scheduler,"number":number,"initialFallback":reason,
                            "coReferencedControls":co_referenced_controls}));
                        continue;
                    }
                    playback.advance(0.0).map_err(anyhow::Error::msg)?;
                    ensure!(
                        playback.fallback_reason().is_none(),
                        "ordinary playback failed"
                    );
                    let mut before_quads = Vec::new();
                    let mut before_meshes = Vec::new();
                    playback.sample(&mut before_quads, &mut before_meshes);
                    match playback.trigger_document(scheduler, number) {
                        Err(reason) => {
                            let mut after_quads = Vec::new();
                            let mut after_meshes = Vec::new();
                            playback.sample(&mut after_quads, &mut after_meshes);
                            ensure!(
                                serde_json::to_value((&before_quads, &before_meshes))?
                                    == serde_json::to_value((&after_quads, &after_meshes))?,
                                "rejected END target mutated playback"
                            );
                            *errors.entry(reason.clone()).or_default() += 1;
                            records.push(json!({"path":path,"timeline":control["timeline"],
                                "scheduler":scheduler,"number":number,"targetRejected":reason,
                                "ordinaryPackets":before_quads.len()+before_meshes.len(),"unchanged":true,
                                "coReferencedControls":co_referenced_controls}));
                        }
                        Ok(attached) => {
                            ensure!(attached, "END mapping did not attach");
                            admitted += 1;
                            let end_frame = control["enabledItemReferences"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|item| item["start"].as_i64().unwrap())
                                .max()
                                .context("END Item")?;
                            let end_document = kind == "end"
                                || (kind == "kill"
                                    && control["rawFloats"][0].as_f64().is_some_and(|v| v < 0.0));
                            let duration = if kind == "kill" {
                                control["rawInts"][0].as_i64().context("KILL duration")?
                            } else {
                                0
                            };
                            let deadline = end_frame + duration.max(0);
                            let terminal_deadline =
                                end_document && duration >= 0 && deadline <= 600;
                            let initial_generation = playback.document_generation().unwrap();
                            let reset_before_end = file.timelines
                                [control["timeline"].as_u64().unwrap() as usize]
                                .items
                                .iter()
                                .filter(|item| item.enabled)
                                .any(|item| {
                                    if (item.effector_index as i8) >= 0
                                        || (item.emitter_index as i8) >= 0
                                        || (item.clip_index as i8) < 0
                                        || item.start_time != 0
                                    {
                                        return false;
                                    }
                                    file.timelines[control["timeline"].as_u64().unwrap() as usize]
                                        .clips
                                        .get(item.clip_index as i8 as usize)
                                        .is_some_and(|clip| {
                                            clip.clip_type
                                                == Some(
                                                    xiv_companion_data::AvfxTimelineClipType::Reset,
                                                )
                                                && clip
                                                    .raw_floats
                                                    .is_some_and(|floats| floats[0] >= 0.0)
                                        })
                                });
                            let mut reset_checked = false;
                            let window = (deadline + 6).clamp(301, 601);
                            for input in 0..window {
                                playback.advance(1.0 / 30.0).map_err(anyhow::Error::msg)?;
                                ensure!(
                                    playback.fallback_reason().is_none(),
                                    "END playback fell back"
                                );
                                samples += 1;
                                if (kind == "reset" || reset_before_end) && input == 0 {
                                    ensure!(
                                        playback.document_generation() == Some(initial_generation),
                                        "REST reconstructed during the request input"
                                    );
                                }
                                if (kind == "reset" || reset_before_end) && input == 1 {
                                    // Installed REST Items all start at zero:
                                    // first Prepare requests, next manager input replaces.
                                    ensure!(
                                        playback.document_generation()
                                            == Some(initial_generation + 1),
                                        "REST was not processed before the next input"
                                    );
                                    reset_checked = true;
                                }
                            }
                            let mut quads = Vec::new();
                            let mut meshes = Vec::new();
                            playback.sample(&mut quads, &mut meshes);
                            ensure!(
                                quads.iter().all(|quad| quad
                                    .position
                                    .into_iter()
                                    .chain(quad.orientation)
                                    .chain(quad.size)
                                    .chain(quad.parent_basis.into_iter().flatten())
                                    .chain(quad.color)
                                    .all(f32::is_finite))
                                    && meshes.iter().all(|mesh| mesh
                                        .position
                                        .into_iter()
                                        .chain(mesh.orientation)
                                        .chain(mesh.scale)
                                        .chain(mesh.parent_basis.into_iter().flatten())
                                        .chain(mesh.color)
                                        .all(f32::is_finite)),
                                "nonfinite control output"
                            );
                            let preempted_by_reset =
                                playback.document_generation().unwrap() > initial_generation;
                            let complete_deadline = terminal_deadline && !preempted_by_reset;
                            if terminal_deadline && preempted_by_reset {
                                ensure!(
                                    reset_before_end && reset_checked,
                                    "unexpected Document reset preempted END: {path}, Scheduler{scheduler}, input{number}"
                                );
                                ensure!(
                                    playback
                                        .trigger_document(scheduler, number)
                                        .map_err(anyhow::Error::msg)?,
                                    "reconstructed Scheduler did not accept a target: {path}"
                                );
                            }
                            if complete_deadline {
                                ensure!(
                                    quads.is_empty() && meshes.is_empty(),
                                    "END left drawing objects: {path}, Scheduler{scheduler}, input{number}"
                                );
                                ensure!(
                                    !playback
                                        .trigger_document(scheduler, number)
                                        .map_err(anyhow::Error::msg)?,
                                    "retired Scheduler accepted a target"
                                );
                            }
                            records.push(json!({"path":path,"timeline":control["timeline"],
                                "scheduler":scheduler,"number":number,"admitted":true,"controlType":kind,"endFrame":end_frame,"deadline":deadline,"endDocumentRequested":end_document,"completeDeadlineChecked":complete_deadline,"endPreemptedByDocumentReset":terminal_deadline && preempted_by_reset,"documentResetDeadlineChecked":reset_checked,"documentGeneration":playback.document_generation(),"windowSamples":window,"finalPackets":quads.len()+meshes.len()}));
                        }
                    }
                }
            }
        }
    }
    ensure!(
        definitions
            == match kind {
                "end" => 104,
                "kill" => 3312,
                "unlockLoopPoint" => 2,
                "reset" => 1465,
                _ => unreachable!(),
            },
        "installed control inventory changed"
    );
    let summary = json!({"controlType":kind,"definitions":definitions,"externalInputCases":input_cases,
        "admitted":admitted,"samples":samples,"errors":errors,"parseWarnings":warnings,
        "elapsedSeconds":started.elapsed().as_secs_f64(),
        "scope":"Unmodified installed END/KILL/ULLP/REST definitions, actual owner MDL ElementId/rest-pose plus object/camera inputs. Co-referenced unsupported controls and providers are preserved, not disabled; rejected targets checked atomic. Admitted targets receive a 301..601 CPU-sample window and finite final geometry/color checks. Nonnegative KILL lifetime or END deadlines within 600 frames require no draws and rejected subsequent external input, except explicitly verified authored zero-start REST: unchanged generation at request input, increment before next input and reconstructed Scheduler accepts target. Those later ENDs belong to a destroyed Timeline and are recorded as preempted, not completed; negative or longer deadlines not verified as completed. Complete native Document lifecycle excluded. No GPU, live host, complete lifecycle or client pixels."});
    fs::write(
        output,
        serde_json::to_vec_pretty(&json!({"summary":summary,"records":records}))?,
    )?;
    println!("{summary}");
    Ok(())
}
