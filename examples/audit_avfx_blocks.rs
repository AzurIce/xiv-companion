//! 诊断探针：全量 avfx 嵌套块频率审计（找出我们解析器未消费但真实文件
//! 常用的字段，按出现文件数排序）。
//! 用法：XIV_GAME_DIR=... cargo run --features game-data --example audit_avfx_blocks

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};
use std::collections::BTreeMap;

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

    // (父块, 子块) -> 出现该组合的 avfx 文件数
    let mut nested: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut files = 0usize;
    let mut tp_files = 0usize;
    let mut tp_blocks = 0usize;
    let mut tp_enabled = 0usize;
    let mut tp_types = BTreeMap::<String, usize>::new();
    let mut tp_filters = BTreeMap::<i32, usize>::new();
    let mut tp_borders = BTreeMap::<i32, usize>::new();
    let mut tp_indexes = BTreeMap::<i32, usize>::new();
    for path in &paths {
        let Some(bytes) = resource.read(path) else {
            continue;
        };
        files += 1;
        let mut seen_here = std::collections::BTreeSet::new();
        walk(&bytes, "AVFX".to_string(), 0, &mut |parent, name| {
            seen_here.insert((parent.to_string(), name.to_string()));
        });
        for key in seen_here {
            *nested.entry(key).or_default() += 1;
        }
        if let Ok(file) = xiv_companion::AvfxFile::parse(&bytes) {
            let mut has_tp = false;
            for particle in &file.particles {
                let Some(tp) = particle.texture_palette.as_ref() else {
                    continue;
                };
                has_tp = true;
                tp_blocks += 1;
                if tp.enabled {
                    tp_enabled += 1;
                    *tp_types
                        .entry(format!("{:?}", particle.particle_type))
                        .or_default() += 1;
                    *tp_filters.entry(tp.texture_filter).or_default() += 1;
                    *tp_borders.entry(tp.texture_border).or_default() += 1;
                    *tp_indexes.entry(tp.texture_index).or_default() += 1;
                }
            }
            tp_files += usize::from(has_tp);
        }
    }
    println!("files={files}");
    println!(
        "texture_palette: files={tp_files} blocks={tp_blocks} enabled={tp_enabled} types={tp_types:?} filters={tp_filters:?} borders={tp_borders:?} indexes={tp_indexes:?}"
    );
    for ((parent, name), count) in &nested {
        println!("{parent}/{name}: {count}");
    }
}

fn walk(bytes: &[u8], parent: String, depth: usize, visit: &mut dyn FnMut(&str, &str)) {
    if depth > 4 {
        return;
    }
    let (mut offset, end) = if depth == 0 {
        (8usize, bytes.len())
    } else {
        (0, bytes.len())
    };
    while offset + 8 <= end {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&bytes[offset..offset + 4]);
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        if offset + 8 + size > end {
            return;
        }
        let mut name = [0u8; 4];
        for (i, c) in raw.iter().rev().take(4).enumerate() {
            name[i] = *c;
        }
        let name = String::from_utf8_lossy(&name)
            .trim_matches(['\0', ' '])
            .to_string();
        if name.is_empty() || !name.bytes().all(|c| c.is_ascii_graphic()) {
            break;
        }
        visit(&parent, &name);
        // 已知容器递归
        if matches!(
            name.as_str(),
            "Schd"
                | "TmLn"
                | "Emit"
                | "Ptcl"
                | "Bind"
                | "Efct"
                | "Modl"
                | "Data"
                | "Life"
                | "PrpS"
                | "Prp1"
                | "Prp2"
                | "PrpG"
                | "Smpl"
        ) {
            walk(
                &bytes[offset + 8..offset + 8 + size],
                name.clone(),
                depth + 1,
                visit,
            );
        }
        offset += 8 + size.div_ceil(4) * 4;
    }
}
