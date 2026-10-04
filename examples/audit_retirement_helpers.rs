//! Inventory unchanged installed emitter termination-helper definitions.
//! No playback, transitive reachability or GPU claim.
#![cfg(feature = "game-data")]
use anyhow::{Context, Result, ensure};
use physis::resource::{Resource, SqPackResource};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs};
use xiv_companion_data::AvfxFile;

fn main() -> Result<()> {
    let started = std::time::Instant::now();
    let dir = xiv_companion::game_data::normalize_game_dir(&std::path::PathBuf::from(
        std::env::var("XIV_GAME_DIR")?,
    ))?;
    let mut resource = SqPackResource::from_existing(dir.to_str().context("UTF-8 game path")?);
    let prior: Value = serde_json::from_slice(&fs::read("target/weapon-vfx-audit/report.json")?)?;
    let paths = prior["models"]
        .as_array()
        .context("models")?
        .iter()
        .flat_map(|model| model["entries"].as_array().unwrap())
        .filter_map(|entry| entry["avfxPath"].as_str())
        .collect::<BTreeSet<_>>();
    ensure!(paths.len() == 1464, "installed inventory changed");
    let mut files = Vec::new();
    let mut total = 0;
    let mut unlinked = 0;
    let mut warnings = 0;
    for path in &paths {
        let bytes = resource
            .read(path)
            .with_context(|| format!("missing {path}"))?;
        let file = AvfxFile::parse(&bytes).with_context(|| path.to_string())?;
        warnings += file.warnings.len();
        let mut helpers = Vec::new();
        for (emitter_index, emitter) in file.emitters.iter().enumerate() {
            for (kind, items) in [
                ("particle", &emitter.particle_items),
                ("emitter", &emitter.emitter_items),
            ] {
                for (item_index, item) in items.iter().enumerate() {
                    if !item.enabled
                        || item.create_time != 2
                        || item.generate_delay == 0
                        || item.create_count <= 1
                    {
                        continue;
                    }
                    total += 1;
                    unlinked += usize::from(item.parameter_link == -1 && item.generate_delay > 0);
                    helpers.push(json!({"emitter":emitter_index,"kind":kind,"item":item_index,"target":item.target_index,
                        "delay":item.generate_delay,"count":item.create_count,"byOne":item.generate_delay_by_one,"parameterLink":item.parameter_link}));
                }
            }
        }
        if !helpers.is_empty() {
            files.push(json!({"path":path,"helpers":helpers}));
        }
    }
    let summary = json!({"files":paths.len(),"filesWithTerminationHelpers":files.len(),"terminationHelperDefinitions":total,
        "positiveUnlinkedTerminationHelperDefinitions":unlinked,"parseWarnings":warnings,"elapsedSeconds":started.elapsed().as_secs_f64(),
        "scope":"Unmodified 1464 installed files; structural enabled emitter/particle items with CrTm=2, GenD!=0 and CrCn>1 only. Not transitive reachability, execution, whole-effect playback, GPU or client pixels. Diagnostic count preserved."});
    fs::write(
        "target/weapon-vfx-audit/retirement-helper-installed.json",
        serde_json::to_vec_pretty(&json!({"summary":summary,"files":files}))?,
    )?;
    println!("{summary}");
    Ok(())
}
