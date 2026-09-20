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
        uv_origin: [0.0, 0.0],
        uv_scale: [2.0, 0.4],
        uv2_origin: [0.2, 0.0],
        uv2_scale: [1.0, 1.0],
        texture_index: 5,
        texture2_index: 8,
        combine_color: 0,
        combine_alpha: 0,
        color_to_alpha: false,
        color_to_alpha2: false,
        blend_add: true,
        texture_distortion_index: -1,
        distortion_power: 0.0,
        distortion_targets: 0,
        uvd_origin: [0.0, 0.0],
        uvd_scale: [1.0, 1.0],
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
    untextured.texture_index = -1;
    untextured.texture2_index = -1;
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
}
