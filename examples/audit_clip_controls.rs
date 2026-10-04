//! Installed Clip parameter inventory. No Clip execution or GPU admission.
//! XIV_GAME_DIR=... cargo run --features game-data --example audit_clip_controls
#![cfg(feature = "game-data")]

use anyhow::{Context, Result, ensure};
use physis::resource::{Resource, SqPackResource};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
use xiv_companion_data::{AvfxFile, AvfxTimelineClipParameters as Parameters};

fn main() -> Result<()> {
    let started = std::time::Instant::now();
    let dir = xiv_companion::game_data::normalize_game_dir(&std::path::PathBuf::from(
        std::env::var("XIV_GAME_DIR")?,
    ))?;
    let mut resource = SqPackResource::from_existing(dir.to_str().context("UTF-8 game directory")?);
    let prior: Value = serde_json::from_slice(&fs::read("target/weapon-vfx-audit/report.json")?)?;
    let paths = prior["models"]
        .as_array()
        .context("models")?
        .iter()
        .flat_map(|model| model["entries"].as_array().unwrap())
        .filter_map(|entry| entry["avfxPath"].as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        paths.len() == 1464,
        "inventory changed; expected 1464 installed effects"
    );
    let mut types = BTreeMap::<String, usize>::new();
    let mut files = Vec::new();
    let mut controls = 0;
    let mut automatic_controls = 0;
    let mut warnings = 0;
    let mut warning_counts = BTreeMap::<String, usize>::new();
    for path in paths {
        let bytes = resource
            .read(path)
            .with_context(|| format!("missing {path}"))?;
        let file = AvfxFile::parse(&bytes).with_context(|| path.to_owned())?;
        warnings += file.warnings.len();
        for warning in &file.warnings {
            *warning_counts.entry(warning.clone()).or_default() += 1;
        }
        let mut clips = Vec::new();
        for (timeline_index, timeline) in file.timelines.iter().enumerate() {
            for (clip_index, clip) in timeline.clips.iter().enumerate() {
                let kind = format!("{:?}", clip.clip_type);
                *types.entry(kind).or_default() += 1;
                ensure!(
                    clip.clip_type.is_none() || clip.parameters.is_some(),
                    "{path} Timeline{timeline_index}/Clip{clip_index} missing runtime fields"
                );
                let control = !matches!(
                    clip.parameters,
                    Some(Parameters::Trigger { .. } | Parameters::RandomTrigger { .. }) | None
                );
                if !control {
                    continue;
                }
                controls += 1;
                let scheduler_sources=file.schedulers.iter().enumerate().filter_map(|(index,scheduler)| {
                    let automatic=scheduler.items.iter().enumerate().filter(|(_,item)|item.enabled && i32::from(item.timeline_index as i16)==timeline_index as i32).map(|(i,_)|i).collect::<Vec<_>>();
                    let external=scheduler.triggers.iter().enumerate().filter(|(_,item)|i32::from(item.timeline_index as i16)==timeline_index as i32).map(|(i,_)|i).collect::<Vec<_>>();
                    if automatic.is_empty() && external.is_empty(){None}else{Some(json!({"scheduler":index,"automaticItems":automatic,"externalMappingSlots":external}))}
                }).collect::<Vec<_>>();
                let item_references=timeline.items.iter().enumerate().filter(|(_,item)|item.enabled && (item.effector_index as i8)<0 && (item.emitter_index as i8)<0 && i32::from(item.clip_index as i8)==clip_index as i32).map(|(index,item)|json!({"item":index,"start":item.start_time,"end":item.end_time,"binder":item.binder_index})).collect::<Vec<_>>();
                if !item_references.is_empty()
                    && scheduler_sources
                        .iter()
                        .any(|source| !source["automaticItems"].as_array().unwrap().is_empty())
                {
                    automatic_controls += 1;
                }
                clips.push(json!({"timeline":timeline_index,"clip":clip_index,"type":clip.clip_type,"runtimeParameters":clip.parameters,
                    "rawInts":clip.raw_ints,"rawFloats":clip.raw_floats,"enabledItemReferences":item_references,"schedulerSources":scheduler_sources}));
            }
        }
        if !clips.is_empty() {
            files.push(json!({"path":path,"controls":clips}));
        }
    }
    let summary = json!({"files":1464,"filesWithControls":files.len(),"controlDefinitions":controls,"directAutomaticControlDefinitions":automatic_controls,
        "clipTypes":types,"parseWarnings":warnings,"elapsedSeconds":started.elapsed().as_secs_f64(),
        "scope":"Unmodified installed Clip parameter inventory and direct structural Scheduler/Item references only; not transitive reachability, live target presence, integrated control execution, staged admission or GPU/client pixels. Existing parser/runtime diagnostics are counted and preserved; this is not a zero-warning corpus claim."});
    fs::write(
        "target/weapon-vfx-audit/clip-control-installed.json",
        serde_json::to_vec_pretty(
            &json!({"summary":summary,"files":files,"warningCounts":warning_counts}),
        )?,
    )?;
    println!("{summary}");
    Ok(())
}
