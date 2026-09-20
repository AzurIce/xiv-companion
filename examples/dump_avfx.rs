//! 诊断探针：dump 指定武器 vw avfx 的完整解析结果（JSON）与原始块结构。
//! 用法：XIV_GAME_DIR=... cargo run --features game-data --example dump_avfx -- \
//!   chara/weapon/w0501/obj/body/b0060/vfx/eff/vw0001.avfx [out.json]

#![cfg(feature = "game-data")]

use physis::resource::{Resource, SqPackResource};

fn main() {
    let path = std::env::args().nth(1).expect("avfx sqpack path");
    let out = std::env::args().nth(2);
    let raw_dir =
        std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));
    let bytes = resource.read(&path).expect("read avfx");
    eprintln!("read {} bytes from {path}", bytes.len());
    std::fs::write("/tmp/dump.avfx", &bytes).expect("write raw");

    // 解码全部 Tex 贴图存 PNG（诊断 TC1/TC2 采样内容）。
    let file0 = xiv_companion::AvfxFile::parse(&bytes).expect("parse");
    for (index, texture_path) in file0.texture_paths.iter().enumerate() {
        let Some(tex_bytes) = resource.read(texture_path) else {
            eprintln!("tex {index}: MISSING {texture_path}");
            continue;
        };
        match xiv_companion::decode_atex_rgba(&tex_bytes) {
            Some(tex) => {
                let out = format!("/tmp/avfx-tex-{index}.png");
                image::save_buffer(
                    &out,
                    &tex.rgba,
                    tex.width,
                    tex.height,
                    image::ColorType::Rgba8,
                )
                .expect("write png");
                eprintln!("tex {index}: {}x{} {texture_path} -> {out}", tex.width, tex.height);
            }
            None => eprintln!("tex {index}: DECODE FAIL {texture_path}"),
        }
    }

    // 原始块结构（根直下每个块的 name/size）。
    walk(&bytes, 0, 2);

    let file = xiv_companion::AvfxFile::parse(&bytes).expect("parse");
    // 采样诊断：dump t 时刻的 quad/网格实例数值。
    let runtime = xiv_companion::VfxRuntime::new(&file);
    let t = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.8_f32);
    // 多时刻存活量统计（burst 收敛/常驻稳定性的快速检查）。
    for probe in [0.0_f32, 0.5, 0.8, 2.0, 4.0, 8.0] {
        let mut probe_quads = Vec::new();
        let mut probe_meshes = Vec::new();
        runtime.sample(probe, &mut probe_quads);
        runtime.sample_mesh(probe, &mut probe_meshes);
        eprintln!(
            "t={probe}: quads={} meshes={}",
            probe_quads.len(),
            probe_meshes.len()
        );
    }
    let mut quads = Vec::new();
    runtime.sample(t, &mut quads);
    eprintln!("--- {} quads at t={t} ---", quads.len());
    for (i, q) in quads.iter().enumerate() {
        eprintln!(
            "q{i}: pos={:?} size={:?} rot={:.2} bb={} color={:?} tex={:?} comb={:?} c2a={:?} add={} uv={:?}×{:?}",
            q.position.map(|v| (v * 100.0).round() / 100.0),
            q.size.map(|v| (v * 1000.0).round() / 1000.0),
            q.rotation,
            q.billboard,
            q.color.map(|v| (v * 100.0).round() / 100.0),
            q.texture_indexes,
            q.combine_modes,
            q.color_to_alpha,
            q.blend_add,
            q.uv_origins, q.uv_scales,
        );
    }
    let mut meshes = Vec::new();
    runtime.sample_mesh(t, &mut meshes);
    eprintln!("--- {} mesh instances ---", meshes.len());
    for (i, m) in meshes.iter().enumerate() {
        eprintln!(
            "m{i}: model={} pos={:?} scale={:?} orient={:?} color={:?} tex={:?} add={} uv={:?}×{:?}",
            m.model_index,
            m.position.map(|v| (v * 100.0).round() / 100.0),
            m.scale.map(|v| (v * 100.0).round() / 100.0),
            m.orientation.map(|v| (v * 100.0).round() / 100.0),
            m.color.map(|v| (v * 100.0).round() / 100.0),
            m.texture_indexes,
            m.blend_add,
            m.uv_origins, m.uv_scales,
        );
    }
    let json = serde_json::to_string_pretty(&file).expect("json");
    match out {
        Some(path) => std::fs::write(&path, &json).expect("write json"),
        None => println!("{json}"),
    }
    eprintln!("warnings: {:?}", file.warnings);
    eprintln!("unknown blocks: {:?}", file.unknown_blocks);
}

/// 递归打印块树：name size（深度限制 max_depth）。
fn walk(bytes: &[u8], depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }
    // 根：前 8 字节为 AVFX 头。
    let (mut offset, end) = if depth == 0 { (8usize, bytes.len()) } else { (0, bytes.len()) };
    while offset + 8 <= end {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&bytes[offset..offset + 4]);
        let size =
            u32::from_le_bytes([bytes[offset + 4], bytes[offset + 5], bytes[offset + 6], bytes[offset + 7]])
                as usize;
        if offset + 8 + size > end {
            eprintln!("{:indent$}<truncated size={size}>", "", indent = depth * 2);
            return;
        }
        let mut name = [0u8; 4];
        for (i, b) in raw.iter().rev().take(4).enumerate() {
            name[i] = *b;
        }
        let name = String::from_utf8_lossy(&name).trim_matches(['\0', ' ']).to_string();
        eprintln!("{:indent$}{name} size={size}", "", indent = depth * 2);
        // 容器块递归。
        if matches!(
            name.as_str(),
            "Schd" | "TmLn" | "Emit" | "Ptcl" | "Bind" | "Efct" | "Modl" | "Data" | "TC1" | "TC2"
                | "TC3" | "TC4" | "TN" | "TR" | "TD" | "TP" | "Life" | "Clip"
        ) {
            walk(&bytes[offset + 8..offset + 8 + size], depth + 1, max_depth);
        }
        offset += 8 + size.div_ceil(4) * 4;
    }
}
