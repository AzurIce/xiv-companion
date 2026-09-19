#![cfg(all(feature = "game-data", feature = "render-test-support"))]

//! 真实客户端数据的武器 VFX 全链路冒烟：catalog → 模型 → imc VfxId →
//! vw####.avfx 解析 → 确定性采样 → GPU 快照（固定时间，可复现）。
//! 运行：XIV_GAME_DIR=... cargo test --features game-data,render-test-support
//!   --test weapon_vfx_real_smoke render_installed_weapon_vfx_smoke -- --ignored --nocapture

use physis::resource::{Resource, SqPackResource};
use xiv_companion::{
    AVFX_FPS, WeaponModelLoadRequest,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    load_weapon_model_from_resource_request, load_weapon_vfx_from_resource,
    renderer::test_support::{
        WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
    },
};

#[test]
#[ignore = "renders an installed glowing weapon with its mounted VFX into target/weapon-render-snapshots"]
fn render_installed_weapon_vfx_smoke() {
    let raw_dir = std::path::PathBuf::from(
        std::env::var("XIV_GAME_DIR").expect("set XIV_GAME_DIR to the local game install"),
    );
    let game_dir = normalize_game_dir(&raw_dir).expect("normalize game dir");
    let game_dir_text = game_dir.to_str().expect("utf8 game dir");

    let mut resource = SqPackResource::from_existing(game_dir_text);
    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir_text),
        game_dir.display().to_string(),
        game_version(&game_dir),
        "local-vfx-smoke".to_string(),
    )
    .expect("export weapon catalog");

    // 15264 圣母盾（w0105b0001 variant1，审计确认该 variant vfx=1 → vw0001.avfx）。
    // 注意：VfxId 挂在 IMC 子集上，必须选 item 变体与 vfx 子集对齐的条目
    // （如 1671 火神刀 variant1 无特效，特效在其 variant2）。
    let item = catalog
        .items
        .iter()
        .find(|item| item.id == 15264)
        .expect("item 15264 圣母盾 in catalog");
    let request = WeaponModelLoadRequest::from(item);
    let model = load_weapon_model_from_resource_request(&mut resource, &request)
        .expect("load weapon model");

    let packed = request.primary_model();
    eprintln!(
        "packed raw={:#x} model={} body={} variant={}",
        packed.raw, packed.model_id, packed.body_id, packed.variant_id
    );
    for body_id in xiv_companion::weapon_body_ids(packed) {
        let imc_path = xiv_companion::weapon_body_imc_path(packed.model_id, body_id);
        let bytes = resource.read(&imc_path);
        eprintln!("imc {imc_path} -> {:?}", bytes.as_ref().map(|b| b.len()));
        if let Some(bytes) = bytes {
            if let Ok(imc) = xiv_companion::ImcFile::parse(&bytes) {
                eprintln!(
                    "  subsets={} default_vfx={} v1_vfx={}",
                    imc.subset_count,
                    imc.default_subset.first().map(|e| e.vfx).unwrap_or(255),
                    imc.subsets
                        .first()
                        .and_then(|s| s.first())
                        .map(|e| e.vfx)
                        .unwrap_or(255)
                );
            }
        }
    }
    let vfx = load_weapon_vfx_from_resource(&mut resource, &request)
        .expect("mounted vfx must resolve for 15264");
    eprintln!(
        "vfx: {} (id {}) schedulers={} timelines={} emitters={} particles={} binders={} textures={:?}",
        vfx.avfx_path,
        vfx.vfx_id,
        vfx.file.schedulers.len(),
        vfx.file.timelines.len(),
        vfx.file.emitters.len(),
        vfx.file.particles.len(),
        vfx.file.binders.len(),
        vfx.file.texture_paths,
    );
    eprintln!("vfx diagnostics: {:?}", vfx.diagnostics);
    eprintln!("vfx warnings: {:?}", vfx.file.warnings);

    // 文件级 Tex 引用应已解码（atex）；全部失败说明路径/头假设有误。
    assert!(
        vfx.file.texture_paths.is_empty() || vfx.textures.iter().any(|texture| texture.is_some()),
        "no vfx texture decoded; atex path/header assumption wrong: {:?}",
        vfx.file.texture_paths
    );

    let runtime = vfx.runtime();
    let mut quads = Vec::new();
    let sample_seconds = 0.8;
    runtime.sample(sample_seconds, &mut quads);
    eprintln!(
        "sampled {} quads at t={sample_seconds}s (frame {})",
        quads.len(),
        sample_seconds * AVFX_FPS
    );
    // 真实特效必须有粒子产出；为 0 说明 scheduler→timeline→emitter 链路断。
    assert!(
        !quads.is_empty(),
        "vfx sampled zero quads; check scheduler/timeline wiring: {:#?}",
        vfx.file
    );

    // 基线（无 VFX）对照，用于确认画面中哪些元素来自特效。
    let baseline = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("installed-vfx-15264-baseline")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0]),
        &model,
    )
    .expect("render baseline");
    eprintln!("baseline: {}", baseline.png_path.display());

    // 放大诊断：确认粒子几何位置与武器遮挡关系（正式尺寸见上方采样）。
    let magnified = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("installed-vfx-15264-magnified")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_quads((0..24).map(|index| {
                // 合成大环：把粒子挪出盾面爆白区域，验证渲染路径。
                let angle = index as f32 / 24.0 * std::f32::consts::TAU;
                xiv_companion::VfxQuad {
                    position: [angle.cos() * 0.7, 0.15, angle.sin() * 0.7],
                    size: [0.14, 0.14],
                    rotation: 0.0,
                    color: [3.0, 1.8, 0.6, 1.0],
                    uv_origin: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                    texture_index: -1,
                }
            })),
        &model,
    )
    .expect("render magnified vfx");
    eprintln!("magnified: {}", magnified.png_path.display());
    // 渲染路径有效性：真实模型 + 合成环的快照必须与基线不同。
    let baseline_bytes = std::fs::read(&baseline.png_path).expect("read baseline png");
    let ring_bytes = std::fs::read(&magnified.png_path).expect("read ring png");
    assert_ne!(
        baseline_bytes, ring_bytes,
        "vfx particles produced no pixel change on the real model"
    );

    let options = WeaponModelSnapshotOptions::new("installed-vfx-15264-holyshield")
        .with_output_dir("target/weapon-render-snapshots")
        .with_viewport(1024, 1024)
        .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
        .with_vfx_quads(quads);
    let snapshot =
        render_weapon_model_snapshot_with_options(options, &model).expect("render weapon with vfx");
    eprintln!("snapshot: {}", snapshot.png_path.display());
}
