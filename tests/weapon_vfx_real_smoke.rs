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

    // 15264 圣母盾 + 16053 屠龙戟·灵光（w0501b0060 variant1 → vw0001）。
    // 注意：VfxId 挂在 IMC 子集上，必须选 item 变体与 vfx 子集对齐的条目
    // （如 1671 火神刀 variant1 无特效，特效在其 variant2）。
    for (item_id, label) in [(15264_u32, "holyshield"), (16053, "reikan-lance")] {
        render_weapon_vfx_item(
            &mut resource,
            &catalog,
            item_id,
            label,
            sample_seconds_label(item_id),
        );
    }
}

fn sample_seconds_label(item_id: u32) -> &'static str {
    let _ = item_id;
    "default"
}

fn render_weapon_vfx_item(
    resource: &mut SqPackResource,
    catalog: &xiv_companion::WeaponCatalogPackage,
    item_id: u32,
    label: &str,
    _tag: &str,
) {
    let item = catalog
        .items
        .iter()
        .find(|item| item.id == item_id)
        .unwrap_or_else(|| panic!("item {item_id} in catalog"));
    let request = WeaponModelLoadRequest::from(item);
    let model = load_weapon_model_from_resource_request(resource, &request)
        .unwrap_or_else(|error| panic!("load weapon model {item_id}: {error:#}"));

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
    let vfx = load_weapon_vfx_from_resource(resource, &request)
        .unwrap_or_else(|| panic!("mounted vfx must resolve for {item_id}"));
    eprintln!(
        "[{label}] models(parsed)={} textures={}",
        vfx.file.models.len(),
        vfx.file.texture_paths.len()
    );
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

    for (mi, model) in vfx.file.models.iter().enumerate() {
        if let (Some(min), Some(max)) = (
            model.emit_vertices.iter().fold(None::<[f32; 3]>, |acc, v| {
                let p = v.position;
                Some(match acc {
                    None => p,
                    Some(a) => [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])],
                })
            }),
            model.emit_vertices.iter().fold(None::<[f32; 3]>, |acc, v| {
                let p = v.position;
                Some(match acc {
                    None => p,
                    Some(a) => [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])],
                })
            }),
        ) {
            eprintln!(
                "[{label}] model {mi}: {} emit verts bbox min={min:?} max={max:?}",
                model.emit_vertices.len()
            );
        }
    }
    for (ti, timeline) in vfx.file.timelines.iter().enumerate() {
        eprintln!(
            "[{label}] timeline {ti}: loop {}..{} items {:?}",
            timeline.loop_start,
            timeline.loop_end,
            timeline
                .items
                .iter()
                .map(|i| (i.enabled, i.start_time, i.end_time, i.emitter_index))
                .collect::<Vec<_>>()
        );
    }
    for (ei, emitter) in vfx.file.emitters.iter().enumerate() {
        eprintln!(
            "[{label}] emitter {ei}: type={:?} data={:?} items={} targets={:?} life={:?}",
            emitter.emitter_type,
            emitter.data,
            emitter.particle_items.len(),
            emitter
                .particle_items
                .iter()
                .map(|i| (i.enabled, i.target_index, i.create_time, i.create_count))
                .collect::<Vec<_>>(),
            emitter.life.value
        );
    }
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
    let qmin = quads.iter().fold([f32::MAX; 3], |a, q| {
        [
            a[0].min(q.position[0]),
            a[1].min(q.position[1]),
            a[2].min(q.position[2]),
        ]
    });
    let qmax = quads.iter().fold([f32::MIN; 3], |a, q| {
        [
            a[0].max(q.position[0]),
            a[1].max(q.position[1]),
            a[2].max(q.position[2]),
        ]
    });
    eprintln!("[{label}] quads bbox min={qmin:?} max={qmax:?}");
    for quad in quads.iter().take(4) {
        eprintln!(
            "[{label}] quad uv_o={:?} uv_s={:?} size={:?} color={:?} tex={:?}",
            quad.uv_origins, quad.uv_scales, quad.size, quad.color, quad.texture_indexes
        );
    }
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
        WeaponModelSnapshotOptions::new(format!("installed-vfx-{item_id}-baseline"))
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0]),
        &model,
    )
    .expect("render baseline");
    eprintln!("baseline: {}", baseline.png_path.display());

    let mut mesh_instances = Vec::new();
    runtime.sample_mesh(sample_seconds, &mut mesh_instances);
    eprintln!(
        "[{label}] mesh instances: {} (file draw models: {})",
        mesh_instances.len(),
        vfx.file.models.iter().filter(|m| m.draw.is_some()).count()
    );

    for (mi, model) in vfx.file.models.iter().enumerate() {
        if let Some(draw) = &model.draw {
            let mins = draw.vertices.iter().fold([f32::MAX; 3], |a, v| {
                [
                    a[0].min(v.position[0]),
                    a[1].min(v.position[1]),
                    a[2].min(v.position[2]),
                ]
            });
            let maxs = draw.vertices.iter().fold([f32::MIN; 3], |a, v| {
                [
                    a[0].max(v.position[0]),
                    a[1].max(v.position[1]),
                    a[2].max(v.position[2]),
                ]
            });
            eprintln!(
                "[{label}] draw model {mi}: verts={} idx={} bbox min={mins:?} max={maxs:?} uv0={:?} color0={:?}",
                draw.vertices.len(),
                draw.indices.len(),
                draw.vertices.first().map(|v| v.uv),
                draw.vertices.first().map(|v| v.color)
            );
        }
    }
    // 保持文件序号对齐（实例 model_index 指文件 Modl 序号）。
    let meshes: Vec<xiv_companion::VfxDrawModel> = vfx
        .file
        .models
        .iter()
        .map(|model| model.draw.clone().unwrap_or_default())
        .collect();
    // 文件 Tex 序号对齐的贴图输入（解码失败项由渲染端回退）。
    let textures: Vec<Option<xiv_companion::renderer::VfxTextureInput>> = vfx
        .textures
        .iter()
        .map(|texture| {
            texture
                .as_ref()
                .map(|texture| xiv_companion::renderer::VfxTextureInput {
                    rgba: texture.rgba.clone(),
                    width: texture.width,
                    height: texture.height,
                })
        })
        .collect();

    // 增亮诊断：同位置粒子放大提亮，验证粒子几何/遮挡（正式强度走上面采样值）。
    let magnified = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new(format!("installed-vfx-{item_id}-ring"))
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_quads(quads.iter().enumerate().map(|(index, quad)| {
                let mut boosted = *quad;
                // 二分诊断：偶数序号 → 环位置（已知可见）；奇数序号 → 原位置。
                // 尺寸/颜色统一放大增亮。
                if index % 2 == 0 {
                    let angle = index as f32 / 24.0 * std::f32::consts::TAU;
                    boosted.position = [angle.cos() * 0.7, 0.15, angle.sin() * 0.7];
                }
                boosted.size = [0.2, 0.2];
                boosted.color = [3.0, 2.0, 1.0, 1.0];
                boosted
            }))
            .with_vfx_textures(textures.iter().cloned()),
        &model,
    )
    .unwrap_or_else(|error| panic!("render boosted vfx {item_id}: {error:#}"));
    eprintln!("magnified: {}", magnified.png_path.display());
    // 渲染路径有效性：增亮粒子必须产生像素变化。
    let baseline_bytes = std::fs::read(&baseline.png_path).expect("read baseline png");
    let boosted_bytes = std::fs::read(&magnified.png_path).expect("read boosted png");
    assert_ne!(
        baseline_bytes, boosted_bytes,
        "boosted vfx particles produced no pixel change"
    );

    // 正式渲染：真实采样值（尺寸/亮度为 v1 近似参数）。
    let snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new(format!("installed-vfx-{item_id}-vfx"))
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_quads(quads)
            .with_vfx_mesh_instances(mesh_instances)
            .with_vfx_meshes(meshes.clone())
            .with_vfx_textures(textures.iter().cloned()),
        &model,
    )
    .unwrap_or_else(|error| panic!("render weapon with vfx {item_id}: {error:#}"));
    eprintln!("snapshot: {}", snapshot.png_path.display());

    // 稳态对照：burst 结束后（t=4s）的常驻特效形态。
    let mut late_quads = Vec::new();
    let mut late_meshes = Vec::new();
    runtime.sample(4.0, &mut late_quads);
    runtime.sample_mesh(4.0, &mut late_meshes);
    let steady = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new(format!("installed-vfx-{item_id}-steady"))
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_quads(late_quads)
            .with_vfx_mesh_instances(late_meshes)
            .with_vfx_meshes(meshes)
            .with_vfx_textures(textures),
        &model,
    )
    .unwrap_or_else(|error| panic!("render steady vfx {item_id}: {error:#}"));
    eprintln!("steady: {}", steady.png_path.display());
}
