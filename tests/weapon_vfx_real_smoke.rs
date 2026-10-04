#![cfg(all(feature = "game-data", feature = "render-test-support"))]

//! 真实客户端数据的武器 VFX 全链路冒烟：catalog → 模型 → imc VfxId →
//! vw####.avfx 解析 → 确定性采样 → GPU 快照（固定时间，可复现）。
//! 运行：XIV_GAME_DIR=... cargo test --features game-data,render-test-support
//!   --test weapon_vfx_real_smoke render_installed_weapon_vfx_smoke -- --ignored --nocapture
//! VFX_SMOKE_ITEM selects comma-separated items; VFX_SMOKE_DISTANCE changes diagnostic framing.

use physis::resource::{Resource, SqPackResource};
use xiv_companion::{
    AVFX_FPS, WeaponModelLoadRequest,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    load_weapon_model_from_resource_request, load_weapon_vfx_attachments_from_resource,
    renderer::{
        ModelRenderOptions,
        test_support::{
            WeaponModelSnapshotOptions, WeaponVfxCameraInput,
            render_weapon_model_snapshot_with_options,
        },
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
    let items = std::env::var("VFX_SMOKE_ITEM")
        .map(|value| {
            value
                .split(',')
                .map(|item| {
                    (
                        item.trim().parse::<u32>().expect("VFX_SMOKE_ITEM"),
                        "selected",
                    )
                })
                .collect()
        })
        .unwrap_or_else(|_| {
            vec![
                (15264, "holyshield"),
                (16053, "reikan-lance"),
                (16061, "reikan-book"),
            ]
        });
    for (item_id, label) in items {
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

fn mounted_preview_options(
    name: String,
    vfx: xiv_companion::WeaponVfxAttachments,
    time: f32,
    camera_distance: f32,
    render_options: ModelRenderOptions,
) -> WeaponModelSnapshotOptions {
    let mut options = WeaponModelSnapshotOptions::new(name)
        .with_output_dir("target/weapon-render-snapshots")
        .with_viewport(1024, 1024)
        .with_camera(0.65, 0.35, camera_distance, [0.0, 0.0])
        .with_render_options(render_options)
        .with_weapon_vfx(vfx, time)
        .with_all_unambiguous_auras();
    // Match browser rAF playback, including its initial zero input and its Aura
    // selection. One jump to t=.8 is not the same input as 48 displayed frames.
    options.weapon_vfx_camera_inputs = (0..=(time * 60.0).ceil() as u32)
        .map(|frame| WeaponVfxCameraInput {
            time: (frame as f32 / 60.0).min(time),
            yaw: options.yaw,
            pitch: options.pitch,
            zoom: options.zoom,
            pan: options.pan,
            roll: render_options.camera_roll,
        })
        .collect();
    options
}

fn render_weapon_vfx_item(
    resource: &mut SqPackResource,
    catalog: &xiv_companion::WeaponCatalogPackage,
    item_id: u32,
    label: &str,
    _tag: &str,
) {
    let camera_distance = std::env::var("VFX_SMOKE_DISTANCE")
        .map(|value| value.parse::<f32>().expect("VFX_SMOKE_DISTANCE"))
        .unwrap_or(3.2);
    let msaa_samples = std::env::var("VFX_SMOKE_MSAA")
        .map(|value| value.parse::<u32>().expect("VFX_SMOKE_MSAA"))
        .unwrap_or(1);
    let render_options = ModelRenderOptions {
        msaa_samples,
        ..Default::default()
    };
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
    let mounted_vfx = load_weapon_vfx_attachments_from_resource(resource, &model)
        .unwrap_or_else(|error| panic!("load mounted vfx for {item_id}: {error:#}"))
        .unwrap_or_else(|| panic!("mounted vfx must resolve for {item_id}"));
    eprintln!(
        "mounted effects: {:?}",
        mounted_vfx
            .attachments
            .iter()
            .map(|attachment| (&attachment.model_path, &attachment.data.avfx_path))
            .collect::<Vec<_>>()
    );
    let attachment = mounted_vfx
        .attachments
        .iter()
        .find(|attachment| {
            attachment.data.file.particles.iter().any(|particle| {
                matches!(
                    particle.particle_type,
                    Some(
                        xiv_companion::ParticleType::Decal | xiv_companion::ParticleType::DecalRing
                    )
                )
            })
        })
        .unwrap_or(&mounted_vfx.attachments[0]);
    let vfx = &attachment.data;
    if item_id == 16964 {
        assert!(
            attachment.model_path.contains("/w5682/"),
            "16964 Decal must be mounted by the loaded w5682 accessory: {}",
            attachment.model_path
        );
        assert!(
            !vfx.file
                .warnings
                .iter()
                .any(|warning| warning.contains("quad approximation")),
            "the dedicated Decal path must not retain the old quad fallback diagnostic"
        );
    }
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
        WeaponModelSnapshotOptions::new(format!(
            "installed-vfx-{item_id}-baseline-{msaa_samples}x"
        ))
        .with_output_dir("target/weapon-render-snapshots")
        .with_viewport(1024, 1024)
        .with_camera(0.65, 0.35, camera_distance, [0.0, 0.0])
        .with_render_options(render_options),
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
                draw.vertices.first().map(|v| v.uvs[0]),
                draw.vertices.first().map(|v| v.color)
            );
        }
    }
    // 保持文件序号对齐（实例 model_index 指文件 Modl 序号）。
    // 文件 Tex 序号对齐的贴图输入（解码失败项由渲染端回退）。
    let textures: Vec<Option<xiv_companion::renderer::VfxTextureInput>> = vfx
        .textures
        .iter()
        .map(|texture| {
            texture
                .as_ref()
                .map(xiv_companion::renderer::VfxTextureInput::from)
        })
        .collect();

    // 增亮诊断：同位置粒子放大提亮，验证粒子几何/遮挡（正式强度走上面采样值）。
    let magnified = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new(format!("installed-vfx-{item_id}-ring-{msaa_samples}x"))
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, camera_distance, [0.0, 0.0])
            .with_render_options(render_options)
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
    assert!(
        baseline_bytes != boosted_bytes,
        "boosted vfx particles produced no pixel change"
    );

    // Render the actual sampled values, without the diagnostic boost.
    let snapshot = render_weapon_model_snapshot_with_options(
        mounted_preview_options(
            format!("installed-vfx-{item_id}-vfx-{msaa_samples}x"),
            mounted_vfx.clone(),
            sample_seconds,
            camera_distance,
            render_options,
        ),
        &model,
    )
    .unwrap_or_else(|error| panic!("render weapon with vfx {item_id}: {error:#}"));
    eprintln!("snapshot: {}", snapshot.png_path.display());
    let snapshot_bytes = std::fs::read(&snapshot.png_path).expect("read actual vfx png");
    assert!(
        baseline_bytes != snapshot_bytes,
        "mounted VFX produced no final pixel change"
    );

    // 稳态对照：burst 结束后（t=4s）的常驻特效形态。
    let mut late_quads = Vec::new();
    let mut late_meshes = Vec::new();
    runtime.sample(4.0, &mut late_quads);
    runtime.sample_mesh(4.0, &mut late_meshes);
    if item_id == 16061 {
        let rings: Vec<_> = late_quads.iter().filter_map(|quad| quad.disc).collect();
        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0].counts, [1, 2, 64]);
        assert_eq!(rings[0].vertex_count(), 378);
        for radius in rings[0].radius {
            assert!((radius - 0.26).abs() < 1e-6);
        }
        for width in rings[0].width {
            assert!((width - 0.03).abs() < 1e-6);
        }
        assert!(
            !vfx.file
                .warnings
                .iter()
                .any(|warning| warning.contains("Disc") && warning.contains("quad approximation"))
        );
    }
    let steady = render_weapon_model_snapshot_with_options(
        mounted_preview_options(
            format!("installed-vfx-{item_id}-steady-{msaa_samples}x"),
            mounted_vfx,
            4.0,
            camera_distance,
            render_options,
        ),
        &model,
    )
    .unwrap_or_else(|error| panic!("render steady vfx {item_id}: {error:#}"));
    eprintln!("steady: {}", steady.png_path.display());
}
