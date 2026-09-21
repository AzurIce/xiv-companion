#![cfg(all(feature = "game-data", feature = "render-test-support"))]

//! 诊断探针：隔离渲染单个网格粒子（火舌），验证贴图采样/合成路径。
//! 运行：XIV_GAME_DIR=... cargo test --features game-data,render-test-support
//!   --test vfx_mesh_probe -- --ignored --nocapture

use physis::resource::{Resource, SqPackResource};
use xiv_companion::renderer::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion::{WeaponModelLoadRequest, load_weapon_model_from_resource_request};

#[test]
#[ignore = "isolated mesh-particle probe for the lance flame"]
fn render_isolated_flame_mesh() {
    let raw_dir = std::path::PathBuf::from(
        std::env::var("XIV_GAME_DIR").expect("set XIV_GAME_DIR to the local game install"),
    );
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("normalize");
    let mut resource = SqPackResource::from_existing(game_dir.to_str().expect("utf8"));

    let catalog = xiv_companion::game_data::export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir.to_str().expect("utf8")),
        game_dir.display().to_string(),
        xiv_companion::game_data::game_version(&game_dir),
        "vfx-mesh-probe".to_string(),
    )
    .expect("export weapon catalog");
    let item = catalog
        .items
        .iter()
        .find(|item| item.id == 16053)
        .expect("16053 in catalog");
    let request = WeaponModelLoadRequest::from(item);
    let model = load_weapon_model_from_resource_request(&mut resource, &request)
        .expect("load weapon model");
    let vfx = xiv_companion::load_weapon_vfx_from_resource(&mut resource, &request)
        .expect("mounted vfx");

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
    let meshes: Vec<xiv_companion::VfxDrawModel> = vfx
        .file
        .models
        .iter()
        .map(|model| model.draw.clone().unwrap_or_default())
        .collect();

    // 单个火舌实例（model 3，tex1=5 tex2=8，固定 UV 变换/颜色）。
    let flame = xiv_companion::VfxMeshInstance {
        position: [0.0, 1.0, -0.02],
        orientation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        draw_priority: 0,
        uv_origins: [[0.0, 0.0], [0.2, 0.0], [0.0, 0.0], [0.0, 0.0]],
        uv_scales: [[2.0, 0.4], [1.0, 1.0], [1.0, 1.0], [1.0, 1.0]],
        texture_indexes: [5, 8, -1, -1],
        combine_modes: [[0, 0]; 3],
        color_to_alpha: [false; 4],
        texture_borders: [[0; 2]; 4],
        texture1_is_shape_mask: true,
        blend_add: true,
        texture_distortion_index: -1,
        distortion_power: 0.0,
        distortion_targets: 0,
        uvd_origin: [0.0, 0.0],
        uvd_scale: [1.0, 1.0],
        distortion_borders: [0; 2],
        cull_mode: 0,
        model_index: 3,
    };
    let flame_only = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-flame-textured")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_mesh_instances([flame])
            .with_vfx_meshes(meshes.clone())
            .with_vfx_textures(textures.clone()),
        &model,
    )
    .expect("render flame textured");
    eprintln!("flame textured: {}", flame_only.png_path.display());

    // 对照：无贴图（应落回退光点×实例色）。
    let mut untextured = flame;
    untextured.texture_indexes = [-1; 4];
    let flame_plain = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-flame-plain")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_mesh_instances([untextured])
            .with_vfx_meshes(meshes.clone()),
        &model,
    )
    .expect("render flame plain");
    eprintln!("flame plain: {}", flame_plain.png_path.display());

    // 全采样拆分：仅网格（带贴图）/ 仅四边形，看白焰到底来自哪条路径。
    let runtime = vfx.runtime();
    let mut quads = Vec::new();
    let mut mesh_instances = Vec::new();
    runtime.sample(0.8, &mut quads);
    runtime.sample_mesh(0.8, &mut mesh_instances);
    let mesh_only = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-mesh-only")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_mesh_instances(mesh_instances)
            .with_vfx_meshes(meshes.clone())
            .with_vfx_textures(textures.clone()),
        &model,
    )
    .expect("render mesh only");
    eprintln!("mesh only: {}", mesh_only.png_path.display());
    let quad_only = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-quad-only")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_vfx_quads(quads)
            .with_vfx_textures(textures.clone()),
        &model,
    )
    .expect("render quad only");
    eprintln!("quad only: {}", quad_only.png_path.display());

    // 单模型隔离：仅壳（model 0）/ 仅火舌（model 3）。
    let runtime2 = vfx.runtime();
    let mut mesh_instances2 = Vec::new();
    runtime2.sample_mesh(0.8, &mut mesh_instances2);
    for (model_index, tag) in [(0usize, "shell"), (3usize, "tongue")] {
        let subset: Vec<_> = mesh_instances2
            .iter()
            .copied()
            .filter(|instance| instance.model_index == model_index)
            .collect();
        let shot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(format!("vfx-probe-model{model_index}-{tag}"))
                .with_output_dir("target/weapon-render-snapshots")
                .with_viewport(1024, 1024)
                .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
                .with_vfx_mesh_instances(subset)
                .with_vfx_meshes(meshes.clone())
                .with_vfx_textures(textures.clone()),
            &model,
        )
        .expect("render single model");
        eprintln!("model{model_index} {tag}: {}", shot.png_path.display());
    }

    // 朝向对照：火舌网格 4 种 (翻转 × 原点) 变体并排（固定缩放 1、无旋转），
    // 对照游戏内火舌轮廓判定 avfx 模型空间朝向。
    let flip_mesh = |mesh: &xiv_companion::VfxDrawModel, flip: bool| {
        let mut mesh = mesh.clone();
        if flip {
            for vertex in &mut mesh.vertices {
                vertex.position[1] = -vertex.position[1];
            }
        }
        mesh
    };
    // 全特效对照：保持全部粒子，只改火舌（model 3）的朝向/位置。
    let runtime3 = vfx.runtime();
    let mut full_quads = Vec::new();
    runtime3.sample(4.0, &mut full_quads);
    let mut full_meshes = Vec::new();
    runtime3.sample_mesh(4.0, &mut full_meshes);
    for (flip, origin_y, tag) in [
        (false, 0.0_f32, "noflip-keep"),
        (true, 0.0, "flip-keep"),
        (false, 0.5, "noflip-up05"),
        (true, -0.5, "flip-down05"),
    ] {
        let mut variant_meshes: Vec<xiv_companion::VfxDrawModel> = meshes.clone();
        variant_meshes[3] = flip_mesh(&meshes[3], flip);
        let instances: Vec<_> = full_meshes
            .iter()
            .map(|instance| {
                let mut instance = *instance;
                if instance.model_index == 3 {
                    instance.position[1] += origin_y;
                }
                instance
            })
            .collect();
        let shot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(format!("vfx-probe-full-{tag}"))
                .with_output_dir("target/weapon-render-snapshots")
                .with_viewport(1024, 1024)
                .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
                .with_vfx_quads(full_quads.clone())
                .with_vfx_mesh_instances(instances)
                .with_vfx_meshes(variant_meshes)
                .with_vfx_textures(textures.clone()),
            &model,
        )
        .expect("render full variant");
        eprintln!("full {tag}: {}", shot.png_path.display());
    }

    // 曝光实验：bloom 拉满 + HDR 场景读回（量化焰心亮度）。
    let runtime4 = vfx.runtime();
    let mut q4 = Vec::new();
    runtime4.sample(4.0, &mut q4);
    let mut m4 = Vec::new();
    runtime4.sample_mesh(4.0, &mut m4);
    let mut options = xiv_companion::renderer::ModelRenderOptions::default();
    options.bloom_strength = 2.0;
    let bright = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-full-bloom2")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
            .with_render_options(options)
            .with_vfx_quads(q4)
            .with_vfx_mesh_instances(m4)
            .with_vfx_meshes(meshes.clone())
            .with_vfx_textures(textures.clone())
            .with_hdr_scene_capture(),
        &model,
    )
    .expect("render bloom2");
    eprintln!("bloom2: {}", bright.png_path.display());
    // HDR 场景里刃头区域（画面中部偏上）的最大亮度
    if let Some(hdr) = &bright.hdr_scene_rgba {
        let (w, h) = (bright.width as usize, bright.height as usize);
        let mut max_v = 0.0_f32;
        for y in (h / 4)..(h / 2) {
            for x in (w / 4)..(w * 3 / 4) {
                let px = hdr[y * w + x];
                max_v = max_v.max(px[0]).max(px[1]).max(px[2]);
            }
        }
        eprintln!("hdr max in blade region: {max_v}");
    }

    // 贴图角色对照：火舌 A) 只用 tex8 渐变（不吃 tex5 暗纹） B) tex8+tex5 相加。
    let runtime5 = vfx.runtime();
    let mut m5 = Vec::new();
    runtime5.sample_mesh(4.0, &mut m5);
    for (tex1, tex2, comb, tag) in [
        (-1, 8, 0, "tex8only"),
        (8, 5, 1, "tex8-add-tex5"),
    ] {
        let instances: Vec<_> = m5
            .iter()
            .map(|instance| {
                let mut instance = *instance;
                if instance.model_index == 3 {
                    instance.texture_indexes = [tex1, tex2, -1, -1];
                    instance.combine_modes = [[comb, 0], [0, 0], [0, 0]];
                    instance.texture1_is_shape_mask = tex1 == 5;
                }
                instance
            })
            .collect();
        let shot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(format!("vfx-probe-tongue-{tag}"))
                .with_output_dir("target/weapon-render-snapshots")
                .with_viewport(1024, 1024)
                .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
                .with_vfx_mesh_instances(instances)
                .with_vfx_meshes(meshes.clone())
                .with_vfx_textures(textures.clone()),
            &model,
        )
        .expect("render tongue variant");
        eprintln!("tongue {tag}: {}", shot.png_path.display());
    }

    // 孤立星点 quad：tex3 遮罩 + flipbook cell (0,0)，看形状。
    let sparkle = xiv_companion::VfxQuad {
        position: [0.0, 1.0, 0.0],
        size: [0.1, 0.1],
        rotation: 0.0,
        orientation: [0.0, 0.0, 0.0, 1.0],
        billboard: true,
        color: [0.0, 0.7, 2.0, 1.0],
        draw_priority: 0,
        pivot: [0.0, 0.0],
        texture_indexes: [3, -1, -1, -1],
        combine_modes: [[0; 2]; 3],
        color_to_alpha: [false; 4],
        uv_origins: [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]],
        uv_scales: [[0.5, 0.5], [1.0, 1.0], [1.0, 1.0], [1.0, 1.0]],
        texture_borders: [[0; 2]; 4],
        texture1_is_shape_mask: true,
        blend_add: true,
        texture_distortion_index: -1,
        distortion_power: 0.0,
        distortion_targets: 0,
        uvd_origin: [0.0, 0.0],
        uvd_scale: [1.0, 1.0],
        distortion_borders: [0; 2],
    };
    let spark = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-sparkle")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(512, 512)
            .with_camera(0.0, 0.0, 1.2, [0.0, 1.0])
            .with_vfx_quads([xiv_companion::VfxQuad { size: [0.3, 0.3], ..sparkle }])
            .with_vfx_textures(textures.clone()),
        &model,
    )
    .expect("render sparkle");
    eprintln!("sparkle: {}", spark.png_path.display());
    // 对照：无贴图（回退光点）与同贴图但关遮罩。
    for (tex, mask, tag) in [(-1, false, "fallback"), (3, false, "nomask")] {
        let mut q = sparkle;
        q.texture_indexes = [tex, -1, -1, -1];
        q.texture1_is_shape_mask = mask;
        let shot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(format!("vfx-probe-sparkle-{tag}"))
                .with_output_dir("target/weapon-render-snapshots")
                .with_viewport(512, 512)
                .with_camera(0.0, 0.0, 1.2, [0.0, 1.0])
                .with_vfx_quads([q])
                .with_vfx_textures(textures.clone()),
            &model,
        )
        .expect("render sparkle variant");
        eprintln!("sparkle {tag}: {}", shot.png_path.display());
    }
}
