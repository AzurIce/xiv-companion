//! Classify unmodified installed AVFX with real owner ElementIds/rest poses.
//! This is a CPU admission audit with a controlled preview camera, not client
//! fidelity or the GPU attachment/resource-loading path.
//! XIV_GAME_DIR=... cargo run --features game-data --example audit_mounted_vfx

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use xiv_companion_data::{
    AvfxFile, ModelSkeleton, PackedModelId, SkeletonPose, VfxBindPoint, VfxBinderCameraSnapshot,
    VfxBinderObjectSnapshot, VfxPlayback, VfxRuntime, load_skeleton_from_sklb_bytes,
    mdl_metadata::mdl_element_ids_from_mdl_bytes, weapon_model_candidate_paths,
    weapon_skeleton_path,
};

fn main() -> anyhow::Result<()> {
    let started = std::time::Instant::now();
    let args = std::env::args().collect::<Vec<_>>();
    let requested_output = args
        .windows(2)
        .find(|pair| pair[0] == "--output")
        .map(|pair| pair[1].as_str());
    anyhow::ensure!(
        !args.last().is_some_and(|arg| arg == "--output"),
        "--output requires a path"
    );
    let verify_playback = std::env::args().any(|arg| arg == "--playback");
    let object_source = std::env::args().any(|arg| arg == "--object-source");
    let initial_object = VfxBinderObjectSnapshot::IDENTITY;
    let moved_object = VfxBinderObjectSnapshot {
        position: [0.125, 0.25, -0.125],
        quaternion: [0.0, 0.0, 0.6, 0.8],
        scale: [1.25, 0.75, 1.5],
    };
    let with_object = |runtime: VfxRuntime| {
        if object_source {
            runtime.with_binder_object_snapshot(initial_object)
        } else {
            Ok(runtime)
        }
    };
    let raw = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR")?);
    let dir = xiv_companion::game_data::normalize_game_dir(&raw)?;
    let mut resource = SqPackResource::from_existing(dir.to_str().unwrap());
    let report: Value =
        serde_json::from_slice(&std::fs::read("target/weapon-vfx-audit/report.json")?)?;
    let mut entries = BTreeMap::new();
    for model in report["models"].as_array().unwrap() {
        for entry in model["entries"].as_array().unwrap() {
            if let Some(path) = entry["avfxPath"].as_str() {
                entries.insert(
                    path.to_string(),
                    (
                        entry["modelId"].as_u64().unwrap() as u16,
                        entry["bodyId"].as_u64().unwrap() as u16,
                    ),
                );
            }
        }
    }
    let camera = VfxBinderCameraSnapshot {
        basis: xiv_companion_data::VFX_IDENTITY_BASIS,
        parallel_direction: [0.0, 0.0, 1.0],
        position: [0.0, 0.0, 3.0],
    };
    let mut skeletons = HashMap::<u16, ModelSkeleton>::new();
    let mut points = HashMap::<(u16, u16), Vec<VfxBindPoint>>::new();
    let mut records = Vec::new();
    let mut fallbacks = BTreeMap::<String, usize>::new();
    let mut admitted = 0;
    let mut playback_verified = 0;
    let mut playback_failures = Vec::new();
    let mut parse_warnings = 0;
    for (path, (model_id, body_id)) in entries {
        let result = (|| -> anyhow::Result<Value> {
            let bytes = resource
                .read(&path)
                .ok_or_else(|| anyhow::anyhow!("missing AVFX {path}"))?;
            let file = AvfxFile::parse(&bytes)?;
            parse_warnings += file.warnings.len();
            let model_path = weapon_model_candidate_paths(PackedModelId::from_raw(
                u64::from(model_id) | (u64::from(body_id) << 16),
            ))
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing model path"))?;
            if !points.contains_key(&(model_id, body_id)) {
                let bytes = resource
                    .read(&model_path)
                    .ok_or_else(|| anyhow::anyhow!("missing owner {model_path}"))?;
                points.insert(
                    (model_id, body_id),
                    mdl_element_ids_from_mdl_bytes(&bytes)?
                        .into_iter()
                        .map(|p| VfxBindPoint {
                            id: p.id,
                            parent_bone: p.parent_bone,
                            translate: p.translate,
                            rotate: p.rotate,
                        })
                        .collect(),
                );
            }
            if !skeletons.contains_key(&model_id) {
                let skeleton_path = weapon_skeleton_path(model_id);
                let bytes = resource
                    .read(&skeleton_path)
                    .ok_or_else(|| anyhow::anyhow!("missing skeleton {skeleton_path}"))?;
                skeletons.insert(model_id, load_skeleton_from_sklb_bytes(&bytes)?);
            }
            let points = &points[&(model_id, body_id)];
            let skeleton = &skeletons[&model_id];
            let targets = VfxRuntime::bind_point_pose_targets(
                points,
                skeleton,
                &SkeletonPose::rest_pose(skeleton),
            )
            .map_err(anyhow::Error::msg)?;
            let runtime = VfxRuntime::with_bind_points(&file, points)
                .with_binder_target_snapshot(&targets)
                .and_then(|value| value.with_binder_camera_snapshot(camera))
                .and_then(with_object)
                .map_err(anyhow::Error::msg)?;
            let mut playback = VfxPlayback::new(runtime);
            let reason = playback.fallback_reason().map(str::to_string);
            let mut sampled_packets = 0;
            if let Some(reason) = &reason {
                *fallbacks.entry(reason.clone()).or_default() += 1;
            } else {
                admitted += 1;
                if verify_playback {
                    let verification = (|| -> anyhow::Result<()> {
                        let mut neutral = file.clone();
                        for binder in &mut neutral.binders {
                            if binder.binder_type == 0 {
                                binder.start_to_global_direction = false;
                            }
                        }
                        let mut control = VfxPlayback::new(
                            VfxRuntime::with_bind_points(&neutral, points)
                                .with_binder_target_snapshot(&targets)
                                .and_then(|v| v.with_binder_camera_snapshot(camera))
                                .and_then(with_object)
                                .map_err(anyhow::Error::msg)?,
                        );
                        anyhow::ensure!(
                            control.fallback_reason().is_none(),
                            "neutral control fell back"
                        );
                        anyhow::ensure!(skeleton.bone_count() > 0, "empty owner skeleton");
                        let mut moved_pose = SkeletonPose::rest_pose(skeleton);
                        // Preserve the authored root translation, then add a
                        // controlled owner pose change at input 120.
                        moved_pose.set_translation(
                            0,
                            std::array::from_fn(|a| {
                                skeleton.rest_pose[0].translation[a] + [0.25, -0.125, 0.5][a]
                            }),
                        )?;
                        let moved =
                            VfxRuntime::bind_point_pose_targets(points, skeleton, &moved_pose)
                                .map_err(anyhow::Error::msg)?;
                        let mut quads = Vec::new();
                        let mut meshes = Vec::new();
                        let mut control_quads = Vec::new();
                        let mut control_meshes = Vec::new();
                        for input in 0..=300 {
                            let object = if input >= 180 {
                                moved_object
                            } else {
                                initial_object
                            };
                            let targets = if input >= 120 { &moved } else { &targets };
                            let placed_targets = targets
                                .iter()
                                .map(|target| xiv_companion_data::VfxBinderTargetSnapshot {
                                    id: target.id,
                                    matrix: object.matrix().transform_matrix(target.matrix),
                                })
                                .collect::<Vec<_>>();
                            let targets = if object_source {
                                &placed_targets
                            } else {
                                targets
                            };
                            for value in [&mut playback, &mut control] {
                                value
                                    .update_preview_binder_sources_to(
                                        input as f32 / 60.0,
                                        Some(targets),
                                        object_source.then_some(object),
                                        Some(camera),
                                    )
                                    .map_err(anyhow::Error::msg)?;
                                anyhow::ensure!(
                                    value.fallback_reason().is_none(),
                                    "input {input}: {:?}",
                                    value.fallback_reason()
                                );
                            }
                            playback.sample(&mut quads, &mut meshes);
                            control.sample(&mut control_quads, &mut control_meshes);
                            anyhow::ensure!(
                                quads == control_quads && meshes == control_meshes,
                                "input {input}: toggling Point bStG changed packets"
                            );
                            anyhow::ensure!(
                                quads.iter().all(|q| q
                                    .position
                                    .into_iter()
                                    .chain(q.orientation)
                                    .chain(q.size)
                                    .chain(q.parent_basis.into_iter().flatten())
                                    .chain(q.color)
                                    .all(f32::is_finite)),
                                "input {input}: nonfinite Quad geometry/color"
                            );
                            anyhow::ensure!(
                                meshes.iter().all(|m| m
                                    .position
                                    .into_iter()
                                    .chain(m.orientation)
                                    .chain(m.scale)
                                    .chain(m.parent_basis.into_iter().flatten())
                                    .chain(m.color)
                                    .all(f32::is_finite)),
                                "input {input}: nonfinite mesh geometry/color"
                            );
                            sampled_packets += quads.len() + meshes.len();
                        }
                        Ok(())
                    })();
                    match verification {
                        Ok(()) => playback_verified += 1,
                        Err(error) => {
                            playback_failures.push(json!({"path":path, "error":error.to_string()}))
                        }
                    }
                }
            }
            Ok(json!({
                "path": path, "ownerModel": model_path,
                "targets": targets.len(), "bones": skeleton.bone_count(),
                "fallback": reason, "sampledPackets": sampled_packets,
            }))
        })();
        records.push(match result {
            Ok(record) => record,
            Err(error) => json!({"path": path, "error": error.to_string()}),
        });
        if records.len() % 25 == 0 {
            eprintln!(
                "mounted audit: {} files, {} admitted, {} playback verified, {} playback failures, {:.1}s",
                records.len(),
                admitted,
                playback_verified,
                playback_failures.len(),
                started.elapsed().as_secs_f64()
            );
        }
    }
    let errors = records
        .iter()
        .filter(|record| record.get("error").is_some())
        .count();
    let summary = json!({
        "files": records.len(), "admitted": admitted,
        "fallbackCounts": fallbacks, "resourceErrors": errors,
        "parseWarnings": parse_warnings,
        "scope": "unmodified files, real owner MDL ElementIds and base-skeleton rest pose, controlled identity camera at (0,0,3), default listener scale; admission and bounded CPU playback when requested, no GPU resource upload or client equivalence",
        "camera": camera, "objectSource": object_source,
        "objectSourceScope": "--object-source supplies independent identity object; at input 180 uses controlled position/raw quaternion/nonuniform scale, composing bone targets in that same space; does not reconstruct live game object ownership",
        "elapsedSeconds": started.elapsed().as_secs_f64(),
        "playbackRequested": verify_playback, "playbackVerified": playback_verified,
        "playbackFailures": playback_failures,
        "playbackScope": "301 samples per admitted file, skeleton root translation after input 120, optional independent object transform and composed bone targets after input 180, persistent preview updates and exact Point-bStG on/off packet equivalence; finite geometry/color only, no GPU textures or client visual proof",
    });
    let output = if object_source && verify_playback {
        "target/weapon-vfx-audit/object-target-mounted-playback.json"
    } else if object_source {
        "target/weapon-vfx-audit/object-target-mounted-admission.json"
    } else if verify_playback {
        "target/weapon-vfx-audit/mounted-vfx-playback.json"
    } else {
        "target/weapon-vfx-audit/mounted-vfx-admission.json"
    };
    let output = requested_output.unwrap_or(output);
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&json!({"summary": summary, "records": records}))?,
    )?;
    println!("{summary}");
    anyhow::ensure!(
        errors == 0 && playback_failures.is_empty(),
        "mounted audit has failures; see {output}"
    );
    Ok(())
}
