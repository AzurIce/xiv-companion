//! 诊断探针：dump MDL 的 ElementId（武器 bind point 数据）。
#![cfg(feature = "game-data")]
use physis::resource::{Resource, SqPackResource};
use physis::ReadableFile;

fn main() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    let path = std::env::args().nth(1).expect("mdl path");
    let bytes = resource.read(&path).expect("read mdl");
    let model = physis::model::MDL::from_existing(physis::Platform::Win32, &bytes)
        .expect("parse mdl");
    let debug = format!("{model:#?}");
    for line in debug.lines() {
        if line.contains("element_id") || line.contains("ElementId") {
            eprintln!("{line}");
        }
    }
    eprintln!("debug len {}", debug.len());
    // element_ids 区段整段打印
    if let Some(start) = debug.find("element_ids") {
        eprintln!("{}", &debug[start..(start + 2000).min(debug.len())]);
    }
}
