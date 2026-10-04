//! Inspect the installed double-Binder definitions behind staged fallbacks.
//! XIV_GAME_DIR=... cargo run --features game-data --example audit_double_binders

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
    let definitions = report["doubleBinderDefinitions"]
        .as_array()
        .ok_or("missing double Binder definitions")?;

    let mut records = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let path = definition["path"].as_str().ok_or("missing AVFX path")?;
        let outer_index = definition["outer"]["index"]
            .as_u64()
            .ok_or("missing outer Binder index")? as usize;
        let inner_index = definition["inner"]["index"]
            .as_u64()
            .ok_or("missing inner Binder index")? as usize;
        let bytes = resource.read(path).ok_or("missing AVFX")?;
        let file = xiv_companion::AvfxFile::parse(&bytes)?;
        let outer = file
            .binders
            .get(outer_index)
            .ok_or("missing outer Binder")?;
        let inner = file
            .binders
            .get(inner_index)
            .ok_or("missing inner Binder")?;
        records.push(json!({
            "path": path,
            "timeline": definition["timeline"],
            "item": definition["item"],
            "outerIndex": outer_index,
            "innerIndex": inner_index,
            "sameBindPoint": outer.bind_point_id == inner.bind_point_id,
            "identicalDefinition": outer == inner,
            "outer": outer,
            "inner": inner,
        }));
    }

    let output = "target/weapon-vfx-audit/double-binder-configs.json";
    serde_json::to_writer_pretty(std::fs::File::create(output)?, &records)?;
    println!(
        "wrote {} double Binder definitions to {output}",
        records.len()
    );
    Ok(())
}
