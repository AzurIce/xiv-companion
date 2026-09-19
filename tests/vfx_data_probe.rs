#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};

#[test]
#[ignore = "probe game data paths"]
fn probe_weapon_imc_paths() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8 game dir"));
    for path in [
        "exd/root.exl",
        "chara/weapon/w0001/obj/body/b0001/model/w0001b0001.mdl",
        "chara/weapon/w0001/obj/body/b0001/b0001.imc",
        "chara/weapon/w2001/obj/body/b0001/b0001.imc",
        "chara/weapon/w2001/obj/body/b0001/model/w2001b0001.mdl",
        "chara/weapon/w2001/obj/body/b0001/vfx/eff/vw0001.avfx",
        // 装备侧对照（Set 型 imc，e####.imc）
        "chara/equipment/e0067/e0067.imc",
    ] {
        let bytes = resource.read(path);
        println!(
            "{path} -> {}",
            bytes
                .as_ref()
                .map(|b| format!(
                    "{} bytes, head {:02x?}{}",
                    b.len(),
                    &b[..b.len().min(8)],
                    if b.len() >= 4 && &b[..4] == b"atex" {
                        " ATEX"
                    } else {
                        ""
                    }
                ))
                .unwrap_or_else(|| "MISSING".to_string())
        );
    }
}

#[test]
#[ignore = "scan shared vw avfx files for textured particles"]
fn scan_shared_vw_files() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    // 诊断：手动走 vw0001 的 Ptcl 子块名，确认真实块命名。
    {
        let path = "chara/weapon/w0105/obj/body/b0001/vfx/eff/vw0001.avfx";
        let bytes = resource.read(path).expect("vw0001");
        let mut ptcl_index = 0;
        walk_blocks(&bytes[8..], 0, &mut |name, payload, depth| {
            if depth == 1 && name == "Ptcl" {
                if ptcl_index == 0 {
                    println!("-- particle 0 child blocks:");
                    let mut print_child = |name: &str, payload: &[u8], depth: usize| {
                        if depth == 3 {
                            println!("   {name} ({}B)", payload.len());
                        }
                    };
                    walk_blocks(payload, depth + 1, &mut print_child);
                }
                ptcl_index += 1;
            }
        });
        println!("total particles visited: {ptcl_index}");
    }

    // 从审计报告抽一批不同武器的 vw 路径，找带贴图粒子的特效。
    let report = std::fs::read_to_string("target/weapon-vfx-audit/report.json")
        .expect("run weapon_vfx_audit first");
    let value: serde_json::Value = serde_json::from_str(&report).expect("json");
    let mut seen = std::collections::HashSet::new();
    let mut scanned = 0;
    for model in value["models"].as_array().expect("models") {
        for entry in model["entries"].as_array().expect("entries") {
            let Some(path) = entry["avfxPath"].as_str() else {
                continue;
            };
            if !seen.insert(path.to_string()) {
                continue;
            }
            if scanned >= 400 {
                return;
            }
            scanned += 1;
            let Some(bytes) = resource.read(path) else {
                continue;
            };
            let Ok(file) = xiv_companion::AvfxFile::parse(&bytes) else {
                println!("{path} PARSE FAIL");
                continue;
            };
            let textured = file
                .particles
                .iter()
                .filter(|p| p.texture_color1.as_ref().is_some_and(|t| t.enabled))
                .count();
            if textured > 0 {
                println!(
                    "{path}: emitters={} particles={textured}/{} tex={:?}",
                    file.emitters.len(),
                    file.particles.len(),
                    file.texture_paths
                );
            }
        }
    }
}

/// 独立的小块遍历器（仅探针用）：访问每个直接/嵌套块的 (名, 负载, 深度)。
fn walk_blocks(bytes: &[u8], base_depth: usize, visit: &mut dyn FnMut(&str, &[u8], usize)) {
    let mut offset = 0;
    while offset + 8 <= bytes.len() {
        let mut raw = [0_u8; 4];
        raw.copy_from_slice(&bytes[offset..offset + 4]);
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        if offset + 8 + size > bytes.len() {
            return;
        }
        let mut name = [0_u8; 4];
        for (index, byte) in raw.iter().rev().take(4).enumerate() {
            name[index] = *byte;
        }
        let name = String::from_utf8_lossy(&name)
            .trim_matches(['\0', ' '])
            .to_string();
        let payload = &bytes[offset + 8..offset + 8 + size];
        visit(&name, payload, base_depth + 1);
        if name == "Ptcl" || name == "Emit" {
            walk_blocks(payload, base_depth + 1, visit);
        }
        offset += 8 + size.div_ceil(4) * 4;
    }
}
