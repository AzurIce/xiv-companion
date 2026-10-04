#![cfg(all(feature = "game-data", feature = "render-test-support"))]

//! 诊断探针：按粒子定义隔离真实特效，以及固定网格粒子的贴图采样/合成。
//! `render_isolated_particle_definitions` 接受 VFX_PROBE_ITEM、VFX_PROBE_SECONDS
//! 和逗号分隔的 VFX_PROBE_PARTICLES，默认逐项绘制物品 25053 在 4 秒时的粒子。
//! 运行：XIV_GAME_DIR=... cargo test --features game-data,render-test-support
//!   --test vfx_mesh_probe -- --ignored --nocapture

use physis::resource::SqPackResource;
use xiv_companion::renderer::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion::{WeaponModelLoadRequest, load_weapon_model_from_resource_request};

#[test]
#[ignore = "renders each selected particle definition from a mounted real VFX"]
fn render_isolated_particle_definitions() {
    let raw_dir = std::path::PathBuf::from(std::env::var("XIV_GAME_DIR").expect("XIV_GAME_DIR"));
    let game_dir = xiv_companion::game_data::normalize_game_dir(&raw_dir).expect("game dir");
    let game_dir_text = game_dir.to_str().expect("UTF-8 game dir");
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let catalog = xiv_companion::game_data::export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir_text),
        game_dir.display().to_string(),
        xiv_companion::game_data::game_version(&game_dir),
        "vfx-particle-probe".to_string(),
    )
    .expect("weapon catalog");
    let item_id = std::env::var("VFX_PROBE_ITEM")
        .map(|value| value.parse::<u32>().expect("VFX_PROBE_ITEM"))
        .unwrap_or(25053);
    let time = std::env::var("VFX_PROBE_SECONDS")
        .map(|value| value.parse::<f32>().expect("VFX_PROBE_SECONDS"))
        .unwrap_or(4.0);
    let item = catalog
        .items
        .iter()
        .find(|item| item.id == item_id)
        .expect("catalog item");
    let model =
        load_weapon_model_from_resource_request(&mut resource, &WeaponModelLoadRequest::from(item))
            .expect("weapon model");
    let vfx = xiv_companion::load_weapon_vfx_from_resource(&mut resource, &model)
        .expect("load VFX")
        .expect("mounted VFX");
    let indexes: Vec<usize> = std::env::var("VFX_PROBE_PARTICLES")
        .map(|value| {
            value
                .split(',')
                .map(|index| index.trim().parse().expect("particle index"))
                .collect()
        })
        .unwrap_or_else(|_| (0..vfx.file.particles.len()).collect());
    let textures: Vec<_> = vfx
        .textures
        .iter()
        .map(|texture| {
            texture
                .as_ref()
                .map(xiv_companion::renderer::VfxTextureInput::from)
        })
        .collect();
    let meshes: Vec<_> = vfx
        .file
        .models
        .iter()
        .map(|model| model.draw.clone().unwrap_or_default())
        .collect();
    for index in indexes {
        let particle = vfx
            .file
            .particles
            .get(index)
            .expect("particle index in range");
        let mut isolated = vfx.file.clone();
        // Keep indexes and seeds stable. Disabling other definitions can relax
        // the emitter child cap, so this is attribution, not a full-scene oracle.
        for emitter in &mut isolated.emitters {
            for item in &mut emitter.particle_items {
                item.enabled &= item.target_index == index as i32;
            }
        }
        let runtime = xiv_companion::VfxRuntime::with_bind_points(&isolated, &vfx.bind_points);
        let mut quads = Vec::new();
        let mut instances = Vec::new();
        runtime.sample(time, &mut quads);
        runtime.sample_mesh(time, &mut instances);
        eprintln!(
            "particle {index} {:?}: {} quads, {} meshes, RMT={}, RBDT={}",
            particle.particle_type,
            quads.len(),
            instances.len(),
            particle.draw_mode,
            particle.rotation_direction_base
        );
        let shot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(format!("vfx-particle-{item_id}-{index}-{time}s"))
                .with_output_dir("target/weapon-render-snapshots")
                .with_viewport(512, 512)
                .with_camera(0.65, 0.35, 3.2, [0.0, 0.0])
                .with_vfx_quads(quads)
                .with_vfx_mesh_instances(instances)
                .with_vfx_meshes(meshes.clone())
                .with_vfx_textures(textures.clone()),
            &model,
        )
        .expect("isolated particle snapshot");
        eprintln!("{}", shot.png_path.display());
    }
}

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
    let vfx = xiv_companion::load_weapon_vfx_from_resource(&mut resource, &model)
        .expect("load mounted vfx")
        .expect("mounted vfx");

    let textures: Vec<Option<xiv_companion::renderer::VfxTextureInput>> = vfx
        .textures
        .iter()
        .map(|texture| {
            texture
                .as_ref()
                .map(xiv_companion::renderer::VfxTextureInput::from)
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
        parent_basis: xiv_companion::VFX_IDENTITY_BASIS,
        movement_direction: [0.0; 3],
        facing_parent_basis: xiv_companion::VFX_IDENTITY_BASIS,
        rotation_direction_base: xiv_companion_data::avfx::rotation_direction_base::NONE,
        scale: [1.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0, 1.0],
        fresnel: None,
        draw_layer: 0,
        soft_key_offset: 0.0,
        draw_priority: 0,
        draw_order: None,
        uv_origins: [[0.0, 0.0], [0.2, 0.0], [0.0, 0.0], [0.0, 0.0]],
        uv_scales: [[2.0, 0.4], [1.0, 1.0], [1.0, 1.0], [1.0, 1.0]],
        uv_rotations: [0.0; 4],
        uv_by_pixel_position: [false; 4],
        texture_indexes: [5, 8, -1, -1],
        texture_uv_sets: [0, 1, 0, 0],
        combine_mode_tc1: [1, 1],
        combine_modes: [[0, 0]; 3],
        color_to_alpha: [false; 4],
        texture_borders: [[0; 2]; 4],
        texture_filters: [1; 4],
        texture_normal_index: -1,
        normal_uv_set: 0,
        normal_uv_origin: [0.0; 2],
        normal_uv_scale: [1.0; 2],
        normal_uv_rotation: 0.0,
        normal_uv_by_pixel_position: false,
        normal_texture_borders: [0; 2],
        normal_texture_filter: 1,
        normal_power: 0.0,
        reflection_enabled: false,
        reflection_use_screen_copy: false,
        reflection_texture_index: -1,
        reflection_texture_filter: 1,
        reflection_calculate_color: 0,
        reflection_rate: 0.0,
        reflection_power: 0.0,
        texture1_is_shape_mask: true,
        texture1_enabled: true,
        texture1_use_screen_copy: false,
        draw_mode: xiv_companion_data::avfx::DRAW_MODE_ADD,
        depth_test: true,
        depth_write: false,
        texture_distortion_index: -1,
        distortion_power: 0.0,
        distortion_targets: 0,
        uvd_origin: [0.0, 0.0],
        uvd_scale: [1.0, 1.0],
        uvd_rotation: 0.0,
        uvd_by_pixel_position: false,
        distortion_uv_set: 2,
        distortion_borders: [0; 2],
        distortion_filter: 1,
        texture_palette_index: -1,
        palette_offset: 0.0,
        palette_border: 0,
        palette_filter: 1,
        cull_mode: 0,
        model_index: 3,
        soft_particle: false,
        soft_particle_fade_range: 1.0,
        depth_offset_type: 0,
        depth_offset: 0.0,
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
    for (tex1, tex2, comb, tag) in [(-1, 8, 0, "tex8only"), (8, 5, 1, "tex8-add-tex5")] {
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
        particle_type: Some(xiv_companion_data::avfx::ParticleType::Quad),
        particle_index: 0,
        soft_particle: false,
        soft_particle_fade_range: 1.0,
        depth_offset_type: 0,
        depth_offset: 0.0,
        powder_single: false,
        windmill_uv_type: 0,
        disc: None,
        polygon: None,
        laser: None,
        line: None,
        polyline: None,
        decal: None,
        position: [0.0, 1.0, 0.0],
        size: [0.1, 0.1],
        orientation: [0.0, 0.0, 0.0, 1.0],
        parent_basis: xiv_companion::VFX_IDENTITY_BASIS,
        movement_direction: [0.0; 3],
        facing_parent_basis: xiv_companion::VFX_IDENTITY_BASIS,
        rotation_direction_base:
            xiv_companion_data::avfx::rotation_direction_base::CAMERA_BILLBOARD,
        color: [0.0, 0.7, 2.0, 1.0],
        draw_layer: 0,
        soft_key_offset: 0.0,
        draw_priority: 0,
        draw_order: None,
        pivot: [0.0, 0.0],
        texture_indexes: [3, -1, -1, -1],
        texture_uv_sets: [0; 4],
        combine_mode_tc1: [1, 1],
        combine_modes: [[0; 2]; 3],
        color_to_alpha: [false; 4],
        uv_origins: [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]],
        uv_scales: [[0.5, 0.5], [1.0, 1.0], [1.0, 1.0], [1.0, 1.0]],
        uv_rotations: [0.0; 4],
        uv_by_pixel_position: [false; 4],
        texture_borders: [[0; 2]; 4],
        texture_filters: [1; 4],
        texture1_is_shape_mask: true,
        texture1_enabled: true,
        texture1_use_screen_copy: false,
        draw_mode: xiv_companion_data::avfx::DRAW_MODE_ADD,
        depth_test: true,
        depth_write: false,
        cull_mode: 0,
        texture_distortion_index: -1,
        distortion_power: 0.0,
        distortion_targets: 0,
        uvd_origin: [0.0, 0.0],
        uvd_scale: [1.0, 1.0],
        uvd_rotation: 0.0,
        uvd_by_pixel_position: false,
        distortion_uv_set: 0,
        distortion_borders: [0; 2],
        distortion_filter: 1,
        texture_palette_index: -1,
        palette_offset: 0.0,
        palette_border: 0,
        palette_filter: 1,
    };
    let spark = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("vfx-probe-sparkle")
            .with_output_dir("target/weapon-render-snapshots")
            .with_viewport(512, 512)
            .with_camera(0.0, 0.0, 1.2, [0.0, 1.0])
            .with_vfx_quads([xiv_companion::VfxQuad {
                size: [0.3, 0.3],
                ..sparkle
            }])
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
