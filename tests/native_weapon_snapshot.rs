#![cfg(feature = "render-test-support")]

#[cfg(feature = "game-data")]
use physis::resource::SqPackResource;
#[cfg(feature = "game-data")]
use xiv_companion::{WeaponModelLoadRequest, load_weapon_model_from_resource_request};
use xiv_companion_render::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion_render::{
    ModelDebugMode, ModelRenderOptions, ModelTextureKind, PackedModelId, WeaponModelBounds,
    WeaponModelData, WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture, WeaponModelVertex,
};

#[test]
#[cfg(feature = "game-data")]
#[ignore = "renders installed equipment-style fist model to target/weapon-render-snapshots"]
fn render_installed_equipment_style_fist_snapshot() {
    let game_dir = std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
    let request = WeaponModelLoadRequest {
        item_id: 49_100,
        item_name: "幻境指虎·半影（复制品）".to_string(),
        model_main: 0x0000_0000_0001_2276,
        model_sub: 0,
        stain_ids: [0, 0],
    };
    let mut resource = SqPackResource::from_existing(&game_dir);
    let model = load_weapon_model_from_resource_request(&mut resource, &request)
        .expect("load equipment-style fist");
    let snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("installed-equipment-style-fist-49100")
            .with_viewport(640, 640),
        &model,
    )
    .expect("render equipment-style fist snapshot");

    eprintln!("png: {}", snapshot.png_path.display());
    eprintln!(
        "adapter: {} ({:?})",
        snapshot.adapter_name, snapshot.adapter_backend
    );
}

#[test]
#[cfg(feature = "game-data")]
#[ignore = "renders installed shape-morphed fist snapshots to target/weapon-render-snapshots"]
fn render_installed_equipment_fist_shape_snapshot() {
    let game_dir = std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
    let request = WeaponModelLoadRequest {
        item_id: 42_697,
        item_name: "新生王国指虎".to_string(),
        model_main: 74_357,
        model_sub: 0,
        stain_ids: [0, 0],
    };
    let mut resource = SqPackResource::from_existing(&game_dir);
    let model = load_weapon_model_from_resource_request(&mut resource, &request)
        .expect("load shape-bearing equipment fist");
    assert!(
        model
            .meshes
            .iter()
            .any(|mesh| !mesh.shape_targets.is_empty())
    );

    let render = |name, shape_mask| {
        let mut options = WeaponModelSnapshotOptions::new(name).with_viewport(640, 640);
        if let Some(shape_mask) = shape_mask {
            options = options.with_enabled_shape_mask(shape_mask);
        }
        let snapshot = render_weapon_model_snapshot_with_options(options, &model)
            .expect("render equipment fist shape snapshot");
        image::open(&snapshot.png_path)
            .expect("decode equipment fist shape PNG")
            .to_rgba8()
            .into_raw()
    };
    let base = render("installed-equipment-fist-42697-shape-base", None);
    let shaped = render("installed-equipment-fist-42697-shape-bit0", Some(1));
    let rgb_difference: u64 = base
        .chunks_exact(4)
        .zip(shaped.chunks_exact(4))
        .map(|(base, shaped)| {
            (0..3)
                .map(|channel| base[channel].abs_diff(shaped[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    eprintln!("shape RGB difference: {rgb_difference}");
    assert!(
        rgb_difference > 5_000,
        "enabled shp_arm must produce a stable visible GPU difference"
    );
}

#[test]
#[ignore = "writes target/weapon-render-snapshots/native-demo-triangle.png with native wgpu"]
fn render_mock_weapon_model_snapshot() {
    let snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-demo-triangle").with_viewport(640, 480),
        &mock_weapon_model(),
    )
    .expect("render native weapon snapshot");

    eprintln!("png: {}", snapshot.png_path.display());
    eprintln!(
        "adapter: {} ({:?})",
        snapshot.adapter_name, snapshot.adapter_backend
    );
}

#[test]
#[ignore = "writes synthetic secondary vertex channel debug snapshots with native wgpu"]
fn render_mock_secondary_vertex_channel_debug_snapshots() {
    let mut model = mock_weapon_model();
    for vertex in &mut model.meshes[0].vertices {
        vertex.color1 = Some([1.0, 0.0, 0.0, 1.0]);
        vertex.normal1 = Some([0.0, 1.0, 0.0]);
        vertex.flow0 = Some([-1.0, 0.0, 1.0, 0.0]);
        vertex.flow1 = Some([0.0, 1.0, -1.0, 0.0]);
    }
    let render = |name, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(320, 320)
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            &model,
        )
        .expect("render secondary vertex channel debug snapshot");
        image::open(snapshot.png_path)
            .expect("decode secondary vertex channel debug PNG")
            .to_rgba8()
            .into_raw()
    };
    let images = [
        render("native-debug-vertex-color1", ModelDebugMode::VertexColor1),
        render("native-debug-normal1", ModelDebugMode::SecondaryNormal),
        render("native-debug-flow0", ModelDebugMode::Flow0),
        render("native-debug-flow1", ModelDebugMode::Flow1),
    ];

    for pair in images.windows(2) {
        let rgb_difference: u64 = pair[0]
            .chunks_exact(4)
            .zip(pair[1].chunks_exact(4))
            .map(|(left, right)| {
                (0..3)
                    .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                    .sum::<u64>()
            })
            .sum();
        assert!(rgb_difference > 10_000);
    }
}

fn mock_weapon_model() -> WeaponModelData {
    WeaponModelData {
        item_id: 1,
        item_name: "Mock Weapon".to_string(),
        model_main: PackedModelId::from_raw(1),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["mock/native-demo-triangle.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.6, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.1, 0.0],
            radius: 1.0,
        },
        materials: Vec::new(),
        textures: Vec::new(),
        meshes: vec![WeaponModelMesh {
            path: "mock/native-demo-triangle.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "mock".to_string(),
            color: [0.8, 0.72, 0.62],
            bone_table: None,
            vertices: vec![
                vertex([-0.8, -0.6, 0.0], [0.95, 0.82, 0.68, 1.0]),
                vertex([0.8, -0.6, 0.0], [0.74, 0.83, 0.95, 1.0]),
                vertex([0.0, 0.8, 0.0], [0.78, 0.95, 0.74, 1.0]),
            ],
            indices: vec![0, 1, 2],
        }],
    }
}

fn vertex(position: [f32; 3], color: [f32; 4]) -> WeaponModelVertex {
    WeaponModelVertex {
        position,
        blend_weights: None,
        blend_indices: None,
        normal: [0.0, 0.0, 1.0],
        uv0: [0.0, 0.0],
        uv1: [0.0, 0.0],
        uv2: [0.0, 0.0],
        uv3: [0.0, 0.0],
        bitangent: [0.0, 1.0, 0.0, 1.0],
        normal1: None,
        bitangent1: None,
        color,
        color1: None,
        flow0: None,
        flow1: None,
    }
}

#[test]
#[ignore = "writes synthetic water snapshots with native wgpu"]
fn render_mock_water_material_snapshot() {
    let transparent = mock_water_model(0.35, true);
    let opaque = mock_water_model(1.0, true);
    let flat = mock_water_model(0.35, false);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic water snapshot")
    };

    let transparent_snapshot = render("native-water-transparent", &transparent);
    let opaque_snapshot = render("native-water-opaque", &opaque);
    let flat_snapshot = render("native-water-flat", &flat);
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic water PNG")
            .to_rgba8()
            .into_raw()
    };
    let transparent_png = pixels(transparent_snapshot.png_path);
    let opaque_png = pixels(opaque_snapshot.png_path);
    let flat_png = pixels(flat_snapshot.png_path);

    assert_ne!(
        transparent_png, opaque_png,
        "water alpha must affect output"
    );
    assert_ne!(
        transparent_png, flat_png,
        "water wave normal must affect output"
    );
}

#[test]
#[ignore = "writes synthetic tattoo alpha snapshots with native wgpu"]
fn render_mock_tattoo_normal_alpha_snapshot() {
    let low_alpha = mock_tattoo_model(32);
    let high_alpha = mock_tattoo_model(224);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic tattoo snapshot")
    };

    let low_snapshot = render("native-tattoo-normal-alpha-low", &low_alpha);
    let high_snapshot = render("native-tattoo-normal-alpha-high", &high_alpha);
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic tattoo PNG")
            .to_rgba8()
            .into_raw()
    };
    let low_pixels = pixels(low_snapshot.png_path);
    let high_pixels = pixels(high_snapshot.png_path);
    let rgb_difference: u64 = low_pixels
        .chunks_exact(4)
        .zip(high_pixels.chunks_exact(4))
        .map(|(low, high)| {
            (0..3)
                .map(|channel| low[channel].abs_diff(high[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    assert!(
        rgb_difference > 100_000,
        "tattoo output must respond to normal alpha when normal blue is unchanged"
    );
}

#[test]
#[ignore = "writes synthetic float tile matrix snapshots with native wgpu"]
fn render_mock_float_tile_matrix_snapshot() {
    let identity = mock_tile_matrix_model(1.0);
    let repeated = mock_tile_matrix_model(2.0);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic tile matrix snapshot")
    };

    let identity_snapshot = render("native-tile-matrix-identity", &identity);
    let repeated_snapshot = render("native-tile-matrix-repeat-2", &repeated);
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic tile matrix PNG")
            .to_rgba8()
            .into_raw()
    };
    let identity_pixels = pixels(identity_snapshot.png_path);
    let repeated_pixels = pixels(repeated_snapshot.png_path);
    let rgb_difference: u64 = identity_pixels
        .chunks_exact(4)
        .zip(repeated_pixels.chunks_exact(4))
        .map(|(identity, repeated)| {
            (0..3)
                .map(|channel| identity[channel].abs_diff(repeated[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    assert!(
        rgb_difference > 100_000,
        "tile rendering must respond to rgba_f32 values that differ beyond the shared RGBA8 clamp"
    );
}

#[test]
#[ignore = "writes synthetic character tile channel snapshots with native wgpu"]
fn render_mock_character_tile_channel_snapshot() {
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic character tile channel snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic character tile channel PNG")
            .to_rgba8()
            .into_raw()
    };
    let rgb_difference = |left: &[u8], right: &[u8]| -> u64 {
        left.chunks_exact(4)
            .zip(right.chunks_exact(4))
            .map(|(left, right)| {
                (0..3)
                    .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                    .sum::<u64>()
            })
            .sum()
    };

    let low_rg = mock_tile_channel_model([0, 0, 160, 0], [224, 128, 255, 0]);
    let high_rg = mock_tile_channel_model([255, 255, 160, 255], [224, 128, 255, 0]);
    let low_rg_pixels = render("native-character-tile-low-rg", &low_rg);
    let high_rg_pixels = render("native-character-tile-high-rg", &high_rg);
    assert_eq!(
        low_rg_pixels, high_rg_pixels,
        "ORB red, green, and alpha must not affect character tile shading"
    );

    let low_blue = mock_tile_channel_model([128, 128, 32, 255], [224, 128, 255, 0]);
    let high_blue = mock_tile_channel_model([128, 128, 224, 255], [224, 128, 255, 0]);
    let low_blue_pixels = render("native-character-tile-low-blue", &low_blue);
    let high_blue_pixels = render("native-character-tile-high-blue", &high_blue);
    assert!(
        rgb_difference(&low_blue_pixels, &high_blue_pixels) > 100_000,
        "ORB blue must directly darken character base color"
    );

    let low_normal_alpha = mock_tile_channel_model([128, 128, 255, 255], [224, 128, 255, 0]);
    let high_normal_alpha = mock_tile_channel_model([128, 128, 255, 255], [224, 128, 255, 255]);
    let low_normal_pixels = render("native-character-tile-normal-alpha-low", &low_normal_alpha);
    let high_normal_pixels = render(
        "native-character-tile-normal-alpha-high",
        &high_normal_alpha,
    );
    assert!(
        rgb_difference(&low_normal_pixels, &high_normal_pixels) > 100_000,
        "tile normal alpha multiplied by TileAlpha must control normal contribution"
    );

    let identity_matrix = mock_uniform_tile_matrix_model(1.0);
    let repeated_matrix = mock_uniform_tile_matrix_model(2.0);
    let identity_pixels = render("native-character-uniform-tile-matrix-one", &identity_matrix);
    let repeated_pixels = render("native-character-uniform-tile-matrix-two", &repeated_matrix);
    assert_eq!(
        identity_pixels, repeated_pixels,
        "TileMatrix must only transform tile UV and cannot directly change lighting"
    );
}

#[test]
#[ignore = "writes synthetic character texture mip bias snapshots with native wgpu"]
fn render_mock_character_texture_mip_bias_snapshot() {
    let unbiased = mock_texture_mip_bias_model(-8.0);
    let blurred = mock_texture_mip_bias_model(4.0);
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic character texture mip bias snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic character texture mip bias PNG")
            .to_rgba8()
            .into_raw()
    };
    let unbiased_pixels = render("native-character-mip-bias-negative-eight", &unbiased);
    let blurred_pixels = render("native-character-mip-bias-four", &blurred);
    let rgb_difference: u64 = unbiased_pixels
        .chunks_exact(4)
        .zip(blurred_pixels.chunks_exact(4))
        .map(|(unbiased, blurred)| {
            (0..3)
                .map(|channel| unbiased[channel].abs_diff(blurred[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    assert!(
        rgb_difference > 100_000,
        "g_TextureMipBias must select different levels from the uploaded mip chain"
    );
}

#[test]
#[ignore = "writes synthetic ColorTable extra ramp snapshots with native wgpu"]
fn render_mock_color_table_extra_ramp_snapshot() {
    let sheen_normal = [-0.231, 0.405, 0.884];
    let sphere_normal = [0.8, 0.0, 0.6];
    let neutral_sheen = mock_extra_ramp_model([0, 0, 0, 255], [0, 0, 0, 255], sheen_normal);
    let sheen = mock_extra_ramp_model([255, 255, 0, 255], [0, 0, 0, 255], sheen_normal);
    let neutral_sphere = mock_extra_ramp_model([0, 0, 0, 255], [0, 0, 0, 255], sphere_normal);
    let sphere = mock_extra_ramp_model([0, 0, 0, 255], [255, 255, 0, 255], sphere_normal);
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic ColorTable extra ramp snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic ColorTable extra ramp PNG")
            .to_rgba8()
            .into_raw()
    };
    let rgb_difference = |left: &[u8], right: &[u8]| -> u64 {
        left.chunks_exact(4)
            .zip(right.chunks_exact(4))
            .map(|(left, right)| {
                (0..3)
                    .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                    .sum::<u64>()
            })
            .sum()
    };

    let neutral_sheen_pixels = render("native-extra-ramp-neutral-sheen", &neutral_sheen);
    let sheen_pixels = render("native-extra-ramp-sheen", &sheen);
    let neutral_sphere_pixels = render("native-extra-ramp-neutral-sphere", &neutral_sphere);
    let sphere_pixels = render("native-extra-ramp-sphere", &sphere);
    let sheen_difference = rgb_difference(&neutral_sheen_pixels, &sheen_pixels);
    let sphere_difference = rgb_difference(&neutral_sphere_pixels, &sphere_pixels);
    assert!(
        sheen_difference > 100_000,
        "ColorTable sheen properties must affect final shading"
    );
    assert!(
        sphere_difference > 100_000,
        "ColorTable sphere properties must affect final shading"
    );
}

#[test]
#[ignore = "writes synthetic detail multi blend snapshots with native wgpu"]
fn render_mock_detail_multi_blend_snapshot() {
    let primary = mock_detail_blend_model(0.0);
    let secondary = mock_detail_blend_model(1.0);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic detail blend snapshot")
    };

    let primary_snapshot = render("native-detail-blend-primary", &primary);
    let secondary_snapshot = render("native-detail-blend-secondary", &secondary);
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic detail blend PNG")
            .to_rgba8()
            .into_raw()
    };
    let primary_pixels = pixels(primary_snapshot.png_path);
    let secondary_pixels = pixels(secondary_snapshot.png_path);
    let rgb_difference: u64 = primary_pixels
        .chunks_exact(4)
        .zip(secondary_pixels.chunks_exact(4))
        .map(|(primary, secondary)| {
            (0..3)
                .map(|channel| primary[channel].abs_diff(secondary[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    assert!(
        rgb_difference > 100_000,
        "GetMultiValues detail output must select primary/multi layers with vertex alpha"
    );
}

#[test]
#[ignore = "writes synthetic bguvscroll Map1 snapshots with native wgpu"]
fn render_mock_secondary_scroll_map_snapshot() {
    let primary = mock_secondary_scroll_model(0.0, false);
    let secondary = mock_secondary_scroll_model(1.0, false);
    let secondary_frame = mock_secondary_scroll_model(1.0, true);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic secondary scroll map snapshot")
    };

    let primary_snapshot = render("native-bguvscroll-primary", &primary);
    let secondary_snapshot = render("native-bguvscroll-secondary", &secondary);
    let secondary_frame_snapshot = render("native-bguvscroll-secondary-frame", &secondary_frame);
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic bguvscroll PNG")
            .to_rgba8()
            .into_raw()
    };

    let primary_pixels = pixels(primary_snapshot.png_path);
    let secondary_pixels = pixels(secondary_snapshot.png_path);
    let secondary_frame_pixels = pixels(secondary_frame_snapshot.png_path);
    assert!(
        primary_pixels
            .chunks_exact(4)
            .any(|pixel| pixel[0] > 160 && pixel[0] > pixel[2].saturating_add(80)),
        "vertex alpha zero must keep the opaque primary map visible"
    );
    assert!(
        secondary_pixels
            .chunks_exact(4)
            .any(|pixel| pixel[2] > 160 && pixel[2] > pixel[0].saturating_add(80)),
        "vertex alpha one must select the secondary map"
    );
    assert_ne!(
        primary_pixels, secondary_pixels,
        "GetMultiValues vertex alpha must blend Map0 and Map1"
    );
    let frame_rgb_difference: u64 = secondary_pixels
        .chunks_exact(4)
        .zip(secondary_frame_pixels.chunks_exact(4))
        .map(|(primary_frame, secondary_frame)| {
            (0..3)
                .map(|channel| primary_frame[channel].abs_diff(secondary_frame[channel]) as u64)
                .sum::<u64>()
        })
        .sum();
    assert!(
        frame_rgb_difference > 10_000,
        "secondary normal map must use normal1/bitangent1 instead of the primary tangent frame"
    );
}

fn mock_secondary_scroll_model(vertex_alpha: f32, use_secondary_frame: bool) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic bguvscroll",
        "path": null,
        "shaderPackageName": "bguvscroll.shpk",
        "alphaMode": "mask",
        "alphaThreshold": 0.5,
        "valueMode": "multi",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "shaderMultiDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [1.0, 1.0, 1.0],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.2, 0.2, 0.2],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.5,
        "metalness": 0.0,
        "textureIndices": [0, 1, 2, 3, 4, 5],
        "baseColorTexture": 0,
        "secondaryBaseColorTexture": 1,
        "normalTexture": 2,
        "secondaryNormalTexture": 3,
        "specularTexture": 4,
        "secondarySpecularTexture": 5,
    }))
    .expect("deserialize synthetic bguvscroll material");
    let texture = |path: &str, kind, rgba| WeaponModelTexture {
        path: path.to_string(),
        kind,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba,
        rgba_f32: None,
    };
    let textures = vec![
        texture(
            "synthetic/color0.tex",
            ModelTextureKind::BaseColor,
            vec![255, 24, 24, 255],
        ),
        texture(
            "synthetic/color1.tex",
            ModelTextureKind::SecondaryBaseColor,
            vec![24, 48, 255, 255],
        ),
        texture(
            "synthetic/normal0.tex",
            ModelTextureKind::Normal,
            vec![128, 128, 255, 255],
        ),
        texture(
            "synthetic/normal1.tex",
            ModelTextureKind::SecondaryNormal,
            vec![200, 128, 238, 255],
        ),
        texture(
            "synthetic/specular0.tex",
            ModelTextureKind::Specular,
            vec![32, 32, 32, 255],
        ),
        texture(
            "synthetic/specular1.tex",
            ModelTextureKind::SecondarySpecular,
            vec![220, 220, 220, 255],
        ),
    ];
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let vertices = positions
        .into_iter()
        .zip(uvs)
        .map(|(position, uv)| {
            let mut vertex = vertex(position, [1.0, 1.0, 1.0, vertex_alpha]);
            vertex.uv0 = uv;
            vertex.uv1 = uv;
            if use_secondary_frame {
                vertex.normal1 = Some([0.0, 1.0, 0.0]);
                vertex.bitangent1 = Some([0.0, 0.0, 1.0, 1.0]);
            }
            vertex
        })
        .collect();

    WeaponModelData {
        item_id: 3,
        item_name: "Synthetic BgUvScroll".to_string(),
        model_main: PackedModelId::from_raw(3),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/bguvscroll.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures,
        meshes: vec![WeaponModelMesh {
            path: "synthetic/bguvscroll.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic bguvscroll".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_tattoo_model(normal_alpha: u8) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic tattoo",
        "path": null,
        "shaderPackageName": "charactertattoo.shpk",
        "alphaMode": "blend",
        "drawDepthMode": "dither",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [0.95, 0.25, 0.2],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.0, 0.0, 0.0],
        "emissiveColor": [0.7, 0.08, 0.04],
        "roughness": 1.0,
        "metalness": 0.0,
        "textureIndices": [0],
        "normalTexture": 0,
    }))
    .expect("deserialize synthetic tattoo material");
    let texture = WeaponModelTexture {
        path: "synthetic/tattoo_normal.tex".to_string(),
        kind: ModelTextureKind::Normal,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![128, 128, 255, normal_alpha],
        rgba_f32: None,
    };
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    let vertices = positions
        .into_iter()
        .map(|position| vertex(position, [1.0; 4]))
        .collect();

    WeaponModelData {
        item_id: 4,
        item_name: "Synthetic Tattoo".to_string(),
        model_main: PackedModelId::from_raw(4),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/tattoo.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: vec![texture],
        meshes: vec![WeaponModelMesh {
            path: "synthetic/tattoo.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic tattoo".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_tile_matrix_model(repeat: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic tile matrix",
        "path": null,
        "shaderPackageName": "character.shpk",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [0.7, 0.75, 0.82],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.35,
        "metalness": 0.0,
        "tileScale": [1.0, 1.0],
        "textureIndices": [0, 1, 2, 3],
        "tilePropertiesTexture": 0,
        "tileMatrixTexture": 1,
        "textureArrays": {
            "tileNormal": 2,
            "tileOrb": 3
        }
    }))
    .expect("deserialize synthetic tile matrix material");
    let texture = |path: &str, kind, width, height, rgba| WeaponModelTexture {
        path: path.to_string(),
        kind,
        width,
        height,
        array_size: 1,
        array_layer_height: 0,
        rgba,
        rgba_f32: None,
    };
    let mut tile_matrix = texture(
        "synthetic/tile_matrix.tex",
        ModelTextureKind::TileMatrixProperties,
        1,
        1,
        vec![255, 0, 0, 255],
    );
    tile_matrix.rgba_f32 = Some(vec![[repeat, 0.0, 0.0, repeat]]);
    let tile_array = |path: &str, kind, layer0: Vec<u8>, layer1: Vec<u8>| {
        let mut rgba = layer0;
        rgba.extend(layer1);
        WeaponModelTexture {
            path: path.to_string(),
            kind,
            width: 2,
            height: 4,
            array_size: 2,
            array_layer_height: 2,
            rgba,
            rgba_f32: None,
        }
    };
    let textures = vec![
        texture(
            "synthetic/tile_properties.tex",
            ModelTextureKind::TileProperties,
            1,
            1,
            vec![0, 255, 255, 255],
        ),
        tile_matrix,
        tile_array(
            "synthetic/tile_normal_array.tex",
            ModelTextureKind::TileNormalArray,
            vec![
                32, 128, 255, 255, 224, 128, 255, 255, 128, 32, 255, 255, 128, 224, 255, 255,
            ],
            [128, 128, 255, 255].repeat(4),
        ),
        tile_array(
            "synthetic/tile_orb_array.tex",
            ModelTextureKind::TileOrbArray,
            vec![
                255, 32, 255, 255, 80, 224, 32, 255, 160, 80, 224, 255, 32, 160, 80, 255,
            ],
            [255, 128, 255, 255].repeat(4),
        ),
    ];
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let vertices = positions
        .into_iter()
        .zip(uvs)
        .map(|(position, uv)| {
            let mut vertex = vertex(position, [1.0; 4]);
            vertex.uv0 = uv;
            vertex
        })
        .collect();

    WeaponModelData {
        item_id: 5,
        item_name: "Synthetic Tile Matrix".to_string(),
        model_main: PackedModelId::from_raw(5),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/tile_matrix.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures,
        meshes: vec![WeaponModelMesh {
            path: "synthetic/tile_matrix.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic tile matrix".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_tile_channel_model(orb: [u8; 4], normal: [u8; 4]) -> WeaponModelData {
    let mut model = mock_tile_matrix_model(1.0);
    for texture in &mut model.textures {
        let pixel = match texture.kind {
            ModelTextureKind::TileNormalArray => normal,
            ModelTextureKind::TileOrbArray => orb,
            _ => continue,
        };
        texture.rgba = pixel.repeat(texture.rgba.len() / 4);
    }
    model
}

fn mock_uniform_tile_matrix_model(repeat: f32) -> WeaponModelData {
    let mut model = mock_tile_channel_model([128, 128, 255, 255], [224, 128, 255, 255]);
    let tile_matrix = model
        .textures
        .iter_mut()
        .find(|texture| texture.kind == ModelTextureKind::TileMatrixProperties)
        .expect("synthetic model has a TileMatrix texture");
    tile_matrix.rgba_f32 = Some(vec![[repeat, 0.0, 0.0, repeat]]);
    model
}

fn mock_texture_mip_bias_model(texture_mip_bias: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic character mip bias",
        "path": null,
        "shaderPackageName": "character.shpk",
        "fallbackColor": [1.0, 1.0, 1.0],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.0, 0.0, 0.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 1.0,
        "metalness": 0.0,
        "textureMipBias": texture_mip_bias,
        "textureIndices": [0],
        "baseColorTexture": 0
    }))
    .expect("deserialize synthetic character mip bias material");
    let width = 256u16;
    let height = 256u16;
    let mut rgba = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
    for y in 0..height {
        for x in 0..width {
            let bright = ((x / 8) + (y / 8)) % 2 == 0;
            rgba.extend_from_slice(if bright {
                &[255, 255, 255, 255]
            } else {
                &[24, 48, 96, 255]
            });
        }
    }
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    let uvs = [[0.0, 0.0], [16.0, 0.0], [16.0, 16.0], [0.0, 16.0]];
    let vertices = positions
        .into_iter()
        .zip(uvs)
        .map(|(position, uv)| {
            let mut vertex = vertex(position, [1.0; 4]);
            vertex.uv0 = uv;
            vertex
        })
        .collect();

    WeaponModelData {
        item_id: 6,
        item_name: "Synthetic Character Mip Bias".to_string(),
        model_main: PackedModelId::from_raw(6),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/character_mip_bias.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: vec![WeaponModelTexture {
            path: "synthetic/character_mip_bias.tex".to_string(),
            kind: ModelTextureKind::BaseColor,
            width,
            height,
            array_size: 1,
            array_layer_height: 0,
            rgba,
            rgba_f32: None,
        }],
        meshes: vec![WeaponModelMesh {
            path: "synthetic/character_mip_bias.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic character mip bias".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_extra_ramp_model(
    sheen: [u8; 4],
    sphere: [u8; 4],
    surface_normal: [f32; 3],
) -> WeaponModelData {
    let mut model = mock_texture_mip_bias_model(0.0);
    model.item_id = 7;
    model.item_name = "Synthetic ColorTable Extra Ramp".to_string();
    model.model_main = PackedModelId::from_raw(7);
    model.loaded_paths = vec!["synthetic/extra_ramp.mdl".to_string()];
    model.materials = vec![
        serde_json::from_value(serde_json::json!({
            "slot": 0,
            "materialIndex": 0,
            "name": "synthetic extra ramp",
            "path": null,
            "shaderPackageName": "character.shpk",
            "fallbackColor": [0.38, 0.46, 0.58],
            "diffuseColor": [1.0, 1.0, 1.0],
            "specularColor": [0.9, 0.95, 1.0],
            "emissiveColor": [0.0, 0.0, 0.0],
            "roughness": 0.3,
            "metalness": 0.0,
            "textureIndices": [0, 1],
            "sheenPropertiesTexture": 0,
            "spherePropertiesTexture": 1
        }))
        .expect("deserialize synthetic ColorTable extra ramp material"),
    ];
    model.textures = vec![
        WeaponModelTexture {
            path: "synthetic/sheen_properties.tex".to_string(),
            kind: ModelTextureKind::SheenProperties,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 0,
            rgba: sheen.to_vec(),
            rgba_f32: None,
        },
        WeaponModelTexture {
            path: "synthetic/sphere_properties.tex".to_string(),
            kind: ModelTextureKind::SphereProperties,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 0,
            rgba: sphere.to_vec(),
            rgba_f32: None,
        },
    ];
    for vertex in &mut model.meshes[0].vertices {
        vertex.normal = surface_normal;
    }
    model.meshes[0].material_name = "synthetic extra ramp".to_string();
    model
}

fn mock_detail_blend_model(vertex_alpha: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic detail blend",
        "path": null,
        "shaderPackageName": "bg.shpk",
        "valueMode": "multi",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "shaderMultiDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [0.72, 0.72, 0.72],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.2, 0.2, 0.2],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.55,
        "metalness": 0.0,
        "detailId": 0.0,
        "multiDetailId": 1.0,
        "detailColor": [1.0, 0.35, 0.35, 1.0],
        "multiDetailColor": [0.35, 0.35, 1.0, 1.0],
        "detailColorUvScale": [1.0, 1.0, 1.0, 1.0],
        "detailNormalUvScale": [1.0, 1.0, 1.0, 1.0],
        "textureIndices": [0, 1, 2],
        "multiMapTexture": 0,
        "textureArrays": {
            "detailDiffuse": 1,
            "detailNormal": 2
        }
    }))
    .expect("deserialize synthetic detail blend material");
    let textures = vec![
        WeaponModelTexture {
            path: "synthetic/multi_map.tex".to_string(),
            kind: ModelTextureKind::MultiMap,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 0,
            rgba: vec![0, 0, 0, 255],
            rgba_f32: None,
        },
        WeaponModelTexture {
            path: "synthetic/detail_d_array.tex".to_string(),
            kind: ModelTextureKind::DetailDiffuseArray,
            width: 1,
            height: 2,
            array_size: 2,
            array_layer_height: 1,
            rgba: vec![255, 32, 32, 255, 32, 32, 255, 255],
            rgba_f32: None,
        },
        WeaponModelTexture {
            path: "synthetic/detail_n_array.tex".to_string(),
            kind: ModelTextureKind::DetailNormalArray,
            width: 1,
            height: 2,
            array_size: 2,
            array_layer_height: 1,
            rgba: vec![224, 128, 255, 255, 128, 224, 255, 255],
            rgba_f32: None,
        },
    ];
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let vertices = positions
        .into_iter()
        .zip(uvs)
        .map(|(position, uv)| {
            let mut vertex = vertex(position, [1.0, 1.0, 1.0, vertex_alpha]);
            vertex.uv0 = uv;
            vertex
        })
        .collect();

    WeaponModelData {
        item_id: 6,
        item_name: "Synthetic Detail Blend".to_string(),
        model_main: PackedModelId::from_raw(6),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/detail_blend.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures,
        meshes: vec![WeaponModelMesh {
            path: "synthetic/detail_blend.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic detail blend".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_water_model(transparency: f32, with_wave: bool) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic water",
        "path": null,
        "shaderPackageName": "water.shpk",
        "transparency": transparency,
        "waterDeepColor": [0.08, 0.34, 0.52, 1.0],
        "waterRefractionColor": [0.2, 0.5, 0.7, 1.0],
        "waterWhitecapColor": [0.8, 0.9, 1.0, 0.3],
        "fallbackColor": [1.0, 1.0, 1.0],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.18,
        "metalness": 0.0,
        "textureIndices": if with_wave { vec![0] } else { Vec::<usize>::new() },
        "waterWaveTexture": if with_wave { Some(0) } else { None },
    }))
    .expect("deserialize synthetic water material");
    let textures = if with_wave {
        vec![WeaponModelTexture {
            path: "synthetic/water_wave.tex".to_string(),
            kind: ModelTextureKind::WaterWave,
            width: 2,
            height: 2,
            array_size: 1,
            array_layer_height: 0,
            rgba: vec![
                230, 128, 255, 255, 128, 230, 255, 255, 26, 128, 255, 255, 128, 26, 255, 255,
            ],
            rgba_f32: None,
        }]
    } else {
        Vec::new()
    };
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    let uvs = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
    let vertices = positions
        .into_iter()
        .zip(uvs)
        .map(|(position, uv)| {
            let mut vertex = vertex(position, [1.0; 4]);
            vertex.uv0 = uv;
            vertex
        })
        .collect();

    WeaponModelData {
        item_id: 2,
        item_name: "Synthetic Water".to_string(),
        model_main: PackedModelId::from_raw(2),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: Vec::new(),
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures,
        meshes: vec![WeaponModelMesh {
            path: "synthetic/water.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("water".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic water".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}
