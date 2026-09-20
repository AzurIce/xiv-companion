//! 诊断探针：扫描审计报告里全部 avfx，统计粒子 TC1 块中 TxNo/TLst 字段的
//! 出现率（判定 TC1 贴图序号语义）。
//! 用法：XIV_GAME_DIR=... cargo run --features game-data --example scan_tc1

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};

fn main() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    let report = std::fs::read_to_string("target/weapon-vfx-audit/report.json")
        .expect("run weapon_vfx_audit first");
    let value: serde_json::Value = serde_json::from_str(&report).expect("json");
    let mut paths = std::collections::BTreeSet::new();
    for model in value["models"].as_array().expect("models") {
        for entry in model["entries"].as_array().expect("entries") {
            if let Some(path) = entry["avfxPath"].as_str() {
                paths.insert(path.to_string());
            }
        }
    }
    let mut files = 0;
    let mut tc1_total = 0;
    let mut tc1_txno = 0;
    let mut tc1_tlst = 0;
    let mut tc1_both = 0;
    let mut tc1_neither_enabled = 0;
    for path in &paths {
        let Some(bytes) = resource.read(path) else {
            continue;
        };
        let Ok(file) = xiv_companion::AvfxFile::parse(&bytes) else {
            continue;
        };
        files += 1;
        for particle in &file.particles {
            let Some(tc1) = particle.texture_color1.as_ref().filter(|t| t.enabled) else {
                continue;
            };
            tc1_total += 1;
            let has_txno = tc1.texture_index >= 0;
            let has_tlst = tc1.mask_texture_index >= 0;
            tc1_txno += has_txno as usize;
            tc1_tlst += has_tlst as usize;
            tc1_both += (has_txno && has_tlst) as usize;
            tc1_neither_enabled += (!has_txno && !has_tlst) as usize;
        }
    }
    println!(
        "files={files} tc1_enabled={tc1_total} has_TxNo={tc1_txno} has_TLst={tc1_tlst} both={tc1_both} neither={tc1_neither_enabled}"
    );
}
