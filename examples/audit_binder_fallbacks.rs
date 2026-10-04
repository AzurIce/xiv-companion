//! Summarize installed single-Binder staged fallbacks without copying raw Data payloads.
//! XIV_GAME_DIR=... cargo run --features game-data --example audit_binder_fallbacks

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use xiv_companion_data::avfx::AvfxBinder;
use xiv_companion_data::{AvfxFile, VfxPlayback, VfxRuntime};

fn binder_summary(index: i32, binders: &[AvfxBinder]) -> Value {
    let Some(binder) = usize::try_from(index)
        .ok()
        .and_then(|index| binders.get(index))
    else {
        return Value::Null;
    };
    let properties = binder.properties_start.as_ref();
    let data = binder.data.as_ref();
    json!({
        "index": index,
        "type": binder.binder_type,
        "life": binder.life,
        "bindPointId": binder.bind_point_id,
        "transformScale": binder.transform_scale,
        "rotationType": binder.rotation_type,
        "startToGlobalDirection": binder.start_to_global_direction,
        "followingTargetOrientation": binder.following_target_orientation,
        "documentScaleEnabled": binder.document_scale_enabled,
        "adjustToScreenEnabled": binder.adjust_to_screen_enabled,
        "ify": binder.ify,
        "bet": binder.bet,
        "vfxScaleEnabled": binder.vfx_scale_enabled,
        "vfxScaleBias": binder.vfx_scale_bias,
        "vfxScaleDepthOffset": binder.vfx_scale_depth_offset,
        "vfxScaleInterpolation": binder.vfx_scale_interpolation,
        "transformScaleDepthOffset": binder.transform_scale_depth_offset,
        "transformScaleInterpolation": binder.transform_scale_interpolation,
        "bindPointType": properties.map(|properties| properties.bind_point_type),
        "bindTargetPointType": properties.map(|properties| properties.bind_target_point_type),
        "binderName": properties.map(|properties| &properties.binder_name),
        "coordUpdateFrame": properties.map(|properties| properties.coord_update_frame),
        "ringEnabled": properties.map(|properties| properties.ring_enabled),
        "bct": properties.map(|properties| properties.bct),
        "springStrength": data.and_then(|data| data.spring_strength.as_ref()).map(|curve| &curve.keys),
        "springStrengthRandom": data.and_then(|data| data.spring_strength_random.as_ref()).map(|curve| &curve.keys),
        "generateDelay": properties.map(|properties| properties.generate_delay),
        "position": properties.map(|properties| &properties.position),
        "hasIntermediateProperties": binder.properties_1.is_some()
            || binder.properties_2.is_some(),
        "goalBindPointId": binder.properties_goal.as_ref().map(|goal| goal.bind_point_id),
        "carryOverFactor": data.and_then(|data| data.carry_over_factor.as_ref()).map(|curve| &curve.keys),
        "carryOverFactorRandom": data.and_then(|data| data.carry_over_factor_random.as_ref()).map(|curve| &curve.keys),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR")?);
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir)?;
    let mut resource = SqPackResource::from_existing(game_dir.to_str().ok_or("game dir UTF-8")?);
    let report: Value =
        serde_json::from_slice(&std::fs::read("target/weapon-vfx-audit/report.json")?)?;
    let paths: BTreeSet<_> = report["models"]
        .as_array()
        .ok_or("missing models")?
        .iter()
        .flat_map(|model| model["entries"].as_array().into_iter().flatten())
        .filter_map(|entry| entry["avfxPath"].as_str())
        .collect();

    let mut records = Vec::new();
    for path in paths {
        let file = AvfxFile::parse(&resource.read(path).ok_or("missing AVFX")?)?;
        let playback = VfxPlayback::new(VfxRuntime::new(&file));
        let Some(
            reason @ ("staged playback: unsupported Timeline Binder"
            | "staged playback: unsupported Item Binder"),
        ) = playback.fallback_reason()
        else {
            continue;
        };
        let mut references = Vec::new();
        for (timeline_index, timeline) in file.timelines.iter().enumerate() {
            for (item_index, item) in timeline.items.iter().enumerate() {
                if !item.enabled
                    || (item.effector_index as i8) >= 0
                    || (item.emitter_index as i8) < 0
                {
                    continue;
                }
                let outer = i32::from(timeline.binder_index as i8);
                let inner = i32::from(item.binder_index as i8);
                let selected = if reason.ends_with("Timeline Binder") {
                    outer
                } else {
                    inner
                };
                if selected < 0 {
                    continue;
                }
                references.push(json!({
                    "timeline": timeline_index,
                    "item": item_index,
                    "outerIndex": outer,
                    "innerIndex": inner,
                    "binder": binder_summary(selected, &file.binders),
                }));
            }
        }
        records.push(json!({ "path": path, "reason": reason, "references": references }));
    }

    let output = "target/weapon-vfx-audit/binder-fallback-configs.json";
    serde_json::to_writer_pretty(std::fs::File::create(output)?, &records)?;
    println!("wrote {} Binder fallback files to {output}", records.len());
    Ok(())
}
