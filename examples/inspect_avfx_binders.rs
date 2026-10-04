//! Print the complete Binder definitions for one installed AVFX without
//! writing extracted game data to disk.
//! XIV_GAME_DIR=... cargo run --features game-data --example inspect_avfx_binders -- <avfx-path> [mdl-path]

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};
use xiv_companion::game_data::normalize_game_dir;
use xiv_companion_data::AvfxFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("missing AVFX path")?;
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR")?);
    let game_dir = normalize_game_dir(&raw_dir)?;
    let mut resource = SqPackResource::from_existing(game_dir.to_str().ok_or("game dir UTF-8")?);
    let bytes = resource.read(&path).ok_or("missing AVFX")?;
    let file = AvfxFile::parse(&bytes)?;
    if let Some(model_path) = std::env::args().nth(2) {
        let model_bytes = resource.read(&model_path).ok_or("missing MDL")?;
        let points =
            xiv_companion_data::mdl_metadata::mdl_element_ids_from_mdl_bytes(&model_bytes)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "binders": file.binders,
                "bindPoints": points,
            }))?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&file.binders)?);
    }
    Ok(())
}
