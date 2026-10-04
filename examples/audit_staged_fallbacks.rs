//! Inspect non-Binder staged fallbacks in the installed weapon AVFX corpus.
//! XIV_GAME_DIR=... cargo run --features game-data --example audit_staged_fallbacks

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR")?);
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir)?;
    let mut resource = SqPackResource::from_existing(game_dir.to_str().ok_or("game dir UTF-8")?);
    let report: Value = serde_json::from_slice(&std::fs::read(
        "target/weapon-vfx-audit/staged-playback-installed.json",
    )?)?;
    let fallbacks = report["fallbackExamples"]
        .as_object()
        .ok_or("missing fallback examples")?;
    let mut entries = Vec::new();
    for (reason, paths) in fallbacks {
        if reason.starts_with("staged playback:") {
            continue;
        }
        for path in paths.as_array().ok_or("invalid fallback examples")? {
            let path = path.as_str().ok_or("invalid fallback path")?;
            let bytes = resource.read(path).ok_or("missing AVFX")?;
            let file = xiv_companion::AvfxFile::parse(&bytes)?;
            let emitters = file
                .emitters
                .iter()
                .enumerate()
                .map(|(index, emitter)| {
                    let particles = emitter
                        .particle_items
                        .iter()
                        .enumerate()
                        .filter(|(_, item)| item.enabled)
                        .map(|(item_index, item)| {
                            let particle = usize::try_from(item.target_index)
                                .ok()
                                .and_then(|index| file.particles.get(index));
                            json!({
                                "item": item_index,
                                "particleIndex": item.target_index,
                                "type": particle.and_then(|particle| particle.particle_type),
                                "collisionType": particle.map(|particle| particle.collision_type),
                                "data": particle.map(|particle| &particle.data),
                            })
                        })
                        .collect::<Vec<_>>();
                    json!({
                        "index": index,
                        "type": emitter.emitter_type,
                        "anyDirection": emitter.any_direction,
                        "shapeData": emitter.data,
                        "particles": particles,
                    })
                })
                .collect::<Vec<_>>();
            entries.push(json!({ "path": path, "reason": reason, "emitters": emitters }));
        }
    }
    serde_json::to_writer_pretty(std::io::stdout().lock(), &entries)?;
    Ok(())
}
