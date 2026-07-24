#![cfg(feature = "render-test-support")]

#[cfg(feature = "game-data")]
use physis::resource::SqPackResource;
#[cfg(feature = "game-data")]
use xiv_companion::{WeaponModelLoadRequest, load_weapon_model_from_resource_request};
#[cfg(feature = "game-data")]
use xiv_companion_data::MaterialSpecularType;
use xiv_companion_render::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion_render::{
    ColorTableRowColors, MaterialAlphaMode, ModelDebugMode, ModelRenderOptions, ModelTextureKind,
    ModelTextureTexelLayout, PackedModelId, WeaponModelBounds, WeaponModelData,
    WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture, WeaponModelVertex,
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
#[cfg(feature = "game-data")]
#[ignore = "renders installed legacy specular-type snapshots to target/weapon-render-snapshots"]
fn render_installed_legacy_specular_mask_snapshot() {
    let game_dir = std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
    let request = WeaponModelLoadRequest {
        item_id: 30_520,
        item_name: "改良型伊修加德新型天星盘".to_string(),
        model_main: 8_593_934_389,
        model_sub: 0,
        stain_ids: [0, 0],
    };
    let mut resource = SqPackResource::from_existing(&game_dir);
    let mut model = load_weapon_model_from_resource_request(&mut resource, &request)
        .expect("load legacy specular-mask weapon");
    let material_index = model
        .materials
        .iter()
        .position(|material| material.specular_type == MaterialSpecularType::Mask)
        .expect("legacy specular-mask material");

    let render = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(640, 640),
            model,
        )
        .expect("render legacy specular snapshot");
        image::open(snapshot.png_path)
            .expect("decode legacy specular PNG")
            .to_rgba8()
            .into_raw()
    };
    let masked = render("installed-legacy-specular-mask-30520", &model);
    model.materials[material_index].specular_type = MaterialSpecularType::Default;
    model.materials[material_index].specular_type_raw = Some(0x198D_11CD);
    let default = render("installed-legacy-specular-default-30520", &model);
    let rgb_difference: u64 = masked
        .chunks_exact(4)
        .zip(default.chunks_exact(4))
        .map(|(masked, default)| {
            (0..3)
                .map(|channel| masked[channel].abs_diff(default[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    eprintln!("legacy specular Mask/Default RGB difference: {rgb_difference}");
    assert!(
        rgb_difference > 5_000,
        "SpecularType Mask must produce a stable visible GPU difference"
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

    let deep_color_source = 12.0;
    let mut hdr = mock_water_model(1.0, false);
    hdr.materials[0].water_deep_color = [deep_color_source, 0.0, 0.0, 1.0];
    let hdr_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-water-deep-color-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::BaseColor,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &hdr,
    )
    .expect("capture linear water deep color scene");
    let hdr_scene = hdr_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let center = hdr_scene[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (center[0] - deep_color_source).abs() <= 0.01,
        "g_WaterDeepColor must remain linear HDR instead of clipping at the old preview limit: source {deep_color_source}, sampled {center:?}"
    );
    assert_eq!(center[1], 0.0);
    assert_eq!(center[2], 0.0);
}

#[test]
#[ignore = "writes synthetic lightshaft snapshots with native wgpu"]
fn render_mock_lightshaft_sampler_blend_snapshot() {
    let primary_only = mock_lightshaft_model(0.0);
    let multiplied = mock_lightshaft_model(1.0);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic lightshaft snapshot")
    };
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic lightshaft PNG")
            .to_rgba8()
            .into_raw()
    };
    let primary_pixels = pixels(render("native-lightshaft-primary", &primary_only).png_path);
    let multiplied_pixels = pixels(render("native-lightshaft-multiplied", &multiplied).png_path);
    let rgb_difference: u64 = primary_pixels
        .chunks_exact(4)
        .zip(multiplied_pixels.chunks_exact(4))
        .map(|(primary, multiplied)| {
            (0..3)
                .map(|channel| primary[channel].abs_diff(multiplied[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    assert!(
        rgb_difference > 100_000,
        "lightshaft output must use vertex blue to blend Sampler0 with Sampler0*Sampler1"
    );
}

#[test]
#[ignore = "writes synthetic lightshaft alpha-test snapshots with native wgpu"]
fn render_mock_lightshaft_alpha_test_snapshot() {
    let visible = mock_lightshaft_alpha_test_model(0.0);
    let clipped = mock_lightshaft_alpha_test_model(0.9);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic lightshaft alpha-test snapshot")
    };
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic lightshaft alpha-test PNG")
            .to_rgba8()
            .into_raw()
    };
    let visible_pixels = pixels(render("native-lightshaft-alpha-visible", &visible).png_path);
    let clipped_pixels = pixels(render("native-lightshaft-alpha-clipped", &clipped).png_path);
    let rgb_difference: u64 = visible_pixels
        .chunks_exact(4)
        .zip(clipped_pixels.chunks_exact(4))
        .map(|(visible, clipped)| {
            (0..3)
                .map(|channel| visible[channel].abs_diff(clipped[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    assert!(
        rgb_difference > 100_000,
        "lightshaft ApplyAlphaTest must discard emission below g_AlphaThreshold"
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
#[ignore = "writes synthetic character normal-channel alpha snapshots with native wgpu"]
fn render_mock_character_normal_channel_alpha_remaps_vertex_alpha_snapshot() {
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic character normal-channel alpha snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic character normal-channel alpha PNG")
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

    let character_zero = mock_character_normal_blue_model(224, 0.0);
    let character_one = mock_character_normal_blue_model(224, 1.0);
    assert_eq!(
        render("native-character-normal-blue-vertex-zero", &character_zero),
        render("native-character-normal-blue-vertex-one", &character_one),
        "0xAD94E254=1 must remap vertex alpha to one before NormalBlue opacity"
    );

    let character_low = mock_character_normal_blue_model(32, 0.0);
    let low_pixels = render("native-character-normal-blue-low", &character_low);
    let high_pixels = render("native-character-normal-blue-high", &character_zero);
    assert!(
        rgb_difference(&low_pixels, &high_pixels) > 100_000,
        "normal Blue must continue to control character transparency when vertex alpha is zero"
    );

    let mut default_zero = mock_character_normal_blue_model(224, 0.0);
    let mut default_one = mock_character_normal_blue_model(224, 1.0);
    default_zero.materials[0].vertex_alpha_to_one = 0.0;
    default_one.materials[0].vertex_alpha_to_one = 0.0;
    assert!(
        rgb_difference(
            &render(
                "native-character-normal-blue-default-vertex-zero",
                &default_zero
            ),
            &render(
                "native-character-normal-blue-default-vertex-one",
                &default_one
            ),
        ) > 100_000,
        "0xAD94E254=0 must preserve vertex alpha in NormalBlue opacity"
    );

    let tattoo_zero = mock_tattoo_vertex_alpha_model(224, 0.0);
    let tattoo_one = mock_tattoo_vertex_alpha_model(224, 1.0);
    assert_eq!(
        render("native-tattoo-normal-alpha-vertex-zero", &tattoo_zero),
        render("native-tattoo-normal-alpha-vertex-one", &tattoo_one),
        "tattoo normal-Alpha transparency must not be multiplied by vertex alpha"
    );
}

#[test]
#[ignore = "writes synthetic alpha-shaping evidence-boundary snapshots with native wgpu"]
fn render_mock_alpha_shaping_boundary_snapshot() {
    let default_model = mock_character_normal_blue_model(128, 1.0);
    let mut override_model = default_model.clone();
    override_model.materials[0].alpha_aperture = 1.0;
    override_model.materials[0].alpha_offset = 0.05;
    let render = |name: &str, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic alpha-shaping boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic alpha-shaping boundary PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render(
            "native-alpha-shaping-default-final",
            &default_model,
            ModelDebugMode::Final,
        ),
        render(
            "native-alpha-shaping-override-final",
            &override_model,
            ModelDebugMode::Final,
        ),
        "unverified aperture/offset values must not silently reshape Final alpha"
    );

    let default_debug = render(
        "native-alpha-shaping-default-diagnostic",
        &default_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let override_debug = render(
        "native-alpha-shaping-override-diagnostic",
        &override_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let expected_default = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_override = compose_expected_bytes([0.94, 0.44, 0.78]);
    for channel in 0..3 {
        assert!(default_debug[center + channel].abs_diff(expected_default[channel]) <= 3);
        assert!(override_debug[center + channel].abs_diff(expected_override[channel]) <= 3);
    }
}

#[test]
#[ignore = "writes synthetic vertex-color evidence-boundary snapshots with native wgpu"]
fn render_mock_vertex_color_composition_boundary_snapshot() {
    let default_model = mock_character_normal_blue_model(255, 1.0);
    let mut override_model = default_model.clone();
    override_model.materials[0].apply_vertex_color = true;
    for vertex in &mut override_model.meshes[0].vertices {
        vertex.color[0] = 0.2;
        vertex.color[1] = 0.7;
        vertex.color[2] = 0.4;
    }
    let render = |name: &str, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic vertex-color boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic vertex-color boundary PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render(
            "native-vertex-color-composition-default-final",
            &default_model,
            ModelDebugMode::Final,
        ),
        render(
            "native-vertex-color-composition-override-final",
            &override_model,
            ModelDebugMode::Final,
        ),
        "unverified ApplyVertexColor RGB composition must not silently alter Final"
    );

    let default_debug = render(
        "native-vertex-color-composition-default-diagnostic",
        &default_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let override_debug = render(
        "native-vertex-color-composition-override-diagnostic",
        &override_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let expected_default = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_override = compose_expected_bytes([0.18, 0.76, 0.54]);
    for channel in 0..3 {
        assert!(default_debug[center + channel].abs_diff(expected_default[channel]) <= 3);
        assert!(override_debug[center + channel].abs_diff(expected_override[channel]) <= 3);
    }

    let default_vertex_debug = render(
        "native-vertex-color-composition-default-debug",
        &default_model,
        ModelDebugMode::VertexColor,
    );
    let override_vertex_debug = render(
        "native-vertex-color-composition-override-debug",
        &override_model,
        ModelDebugMode::VertexColor,
    );
    let vertex_debug_difference: u64 = default_vertex_debug
        .chunks_exact(4)
        .zip(override_vertex_debug.chunks_exact(4))
        .map(|(left, right)| {
            (0..3)
                .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                .sum::<u64>()
        })
        .sum();
    assert!(
        vertex_debug_difference > 100_000,
        "direct VertexColor debug must continue to expose the differing RGB payload"
    );
}

#[test]
#[ignore = "writes synthetic specular-color-mask evidence-boundary snapshots with native wgpu"]
fn render_mock_specular_color_mask_composition_boundary_snapshot() {
    let default_model = mock_metallic_fixture_model(1.0, 0.12);
    let mut override_model = default_model.clone();
    override_model.materials[0].specular_color_mask = [0.0, 0.0, 0.0, 0.0];
    let render = |name: &str, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    bloom: false,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic specular-color-mask boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic specular-color-mask boundary PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render(
            "native-specular-color-mask-default-final",
            &default_model,
            ModelDebugMode::Final,
        ),
        render(
            "native-specular-color-mask-override-final",
            &override_model,
            ModelDebugMode::Final,
        ),
        "unverified g_SpecularColorMask values must not silently alter Final specular"
    );

    let default_debug = render(
        "native-specular-color-mask-default-diagnostic",
        &default_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let override_debug = render(
        "native-specular-color-mask-override-diagnostic",
        &override_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let expected_default = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_override = compose_expected_bytes([0.84, 0.28, 0.62]);
    for channel in 0..3 {
        assert!(default_debug[center + channel].abs_diff(expected_default[channel]) <= 3);
        assert!(override_debug[center + channel].abs_diff(expected_override[channel]) <= 3);
    }
}

#[test]
#[ignore = "writes synthetic outline evidence-boundary snapshots with native wgpu"]
fn render_mock_outline_composition_boundary_snapshot() {
    let default_model = mock_metallic_fixture_model(0.0, 0.65);
    let mut override_model = default_model.clone();
    override_model.materials[0].outline_width = 0.08;
    override_model.materials[0].outline_color = [0.05, 0.9, 0.95, 1.0];
    let render = |name: &str, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    bloom: false,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic outline boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic outline boundary PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render(
            "native-outline-composition-default-final",
            &default_model,
            ModelDebugMode::Final,
        ),
        render(
            "native-outline-composition-override-final",
            &override_model,
            ModelDebugMode::Final,
        ),
        "unverified outline width/color must not silently add a geometry pass to Final"
    );

    let default_debug = render(
        "native-outline-composition-default-diagnostic",
        &default_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let override_debug = render(
        "native-outline-composition-override-diagnostic",
        &override_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let expected_default = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_override = compose_expected_bytes([0.46, 0.82, 0.94]);
    for channel in 0..3 {
        assert!(default_debug[center + channel].abs_diff(expected_default[channel]) <= 3);
        assert!(override_debug[center + channel].abs_diff(expected_override[channel]) <= 3);
    }
}

#[test]
#[ignore = "writes synthetic transparent triangle sorting snapshots with native wgpu"]
fn render_mock_transparent_triangle_sorting_snapshot() {
    let model = mock_transparent_triangle_sorting_model();
    let render = |name, yaw| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_camera(yaw, 0.0, 2.5, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    bloom: false,
                    ..ModelRenderOptions::default()
                }),
            &model,
        )
        .expect("render synthetic transparent triangle sorting snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic transparent triangle sorting PNG")
            .to_rgba8()
            .get_pixel(256, 256)
            .0
    };

    let front = render("native-transparent-triangle-sort-front", 0.0);
    let back = render(
        "native-transparent-triangle-sort-back",
        std::f32::consts::PI,
    );

    assert!(
        front[2] > front[0].saturating_add(12),
        "front view must blend the nearer blue layer last: {front:?}"
    );
    assert!(
        back[0] > back[2].saturating_add(12),
        "back view must re-sort and blend the nearer red layer last: {back:?}"
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
#[ignore = "writes synthetic ColorTable A/B tile ramp snapshots with native wgpu"]
fn render_mock_character_tile_ab_ramp_snapshot() {
    let render = |name, model, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic character tile A/B ramp snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic character tile A/B ramp PNG")
            .to_rgba8()
            .get_pixel(256, 256)
            .0
    };

    let ramp_a = mock_tile_ab_ramp_model(0, 255, 255);
    let ramp_mid = mock_tile_ab_ramp_model(128, 255, 255);
    let ramp_b = mock_tile_ab_ramp_model(255, 255, 255);

    let orb_a = render(
        "native-character-tile-ab-orb-a",
        &ramp_a,
        ModelDebugMode::TileOrbArray,
    );
    let orb_mid = render(
        "native-character-tile-ab-orb-mid",
        &ramp_mid,
        ModelDebugMode::TileOrbArray,
    );
    let orb_b = render(
        "native-character-tile-ab-orb-b",
        &ramp_b,
        ModelDebugMode::TileOrbArray,
    );
    assert!(
        orb_a[0] > orb_mid[0]
            && orb_mid[0] > orb_b[0]
            && orb_a[2] < orb_mid[2]
            && orb_mid[2] < orb_b[2],
        "ColorTable G must blend separately sampled Tile ORB layers: {orb_a:?} {orb_mid:?} {orb_b:?}"
    );

    let normal_a = render(
        "native-character-tile-ab-normal-a",
        &ramp_a,
        ModelDebugMode::TileNormalArray,
    );
    let normal_mid = render(
        "native-character-tile-ab-normal-mid",
        &ramp_mid,
        ModelDebugMode::TileNormalArray,
    );
    let normal_b = render(
        "native-character-tile-ab-normal-b",
        &ramp_b,
        ModelDebugMode::TileNormalArray,
    );
    assert!(
        normal_a[0] > normal_a[1].saturating_add(20)
            && normal_b[1] > normal_b[0].saturating_add(20)
            && normal_mid[0] > normal_a[1]
            && normal_mid[1] > normal_a[1],
        "A/B TileMatrix axes must rotate and then blend tile normals: {normal_a:?} {normal_mid:?} {normal_b:?}"
    );

    let neutral_a = mock_tile_ab_ramp_model(0, 0, 255);
    let neutral_orb = render(
        "native-character-tile-ab-orb-neutral-alpha",
        &neutral_a,
        ModelDebugMode::TileOrbArray,
    );
    assert!(
        neutral_orb[2] > orb_a[2].saturating_add(30),
        "TileAlpha zero must move ORB A toward its neutral value: {orb_a:?} -> {neutral_orb:?}"
    );
    assert!(
        (140..=205).contains(&neutral_orb[1]),
        "TileAlpha zero must restore the ORB neutral green channel: {neutral_orb:?}"
    );
    assert!(
        neutral_orb[0] >= 220 && neutral_orb[2] >= 220,
        "TileAlpha zero must restore the ORB neutral red/blue channels: {neutral_orb:?}"
    );

    let final_active = render(
        "native-character-tile-ab-final-active-alpha",
        &ramp_a,
        ModelDebugMode::Final,
    );
    let final_neutral = render(
        "native-character-tile-ab-final-neutral-alpha",
        &neutral_a,
        ModelDebugMode::Final,
    );
    let active_luma =
        u16::from(final_active[0]) + u16::from(final_active[1]) + u16::from(final_active[2]);
    let neutral_luma =
        u16::from(final_neutral[0]) + u16::from(final_neutral[1]) + u16::from(final_neutral[2]);
    assert!(
        neutral_luma > active_luma + 30,
        "TileAlpha-neutral ORB blue must stop darkening Final: {final_active:?} -> {final_neutral:?}"
    );
}

#[test]
#[ignore = "writes synthetic modern ColorTable shaping snapshots with native wgpu"]
fn render_mock_modern_colortable_shaping_snapshot() {
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_camera(0.0, 0.75, 2.5, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode: ModelDebugMode::TileOrbArray,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic modern ColorTable shaping snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic modern ColorTable shaping PNG")
            .to_rgba8()
            .get_pixel(256, 256)
            .0
    };

    let modern_high_anisotropy_model =
        mock_modern_colortable_shaping_model(0.85, 0.85, "character.shpk");
    let modern_low_anisotropy_model =
        mock_modern_colortable_shaping_model(0.15, 0.15, "character.shpk");
    let modern_a_high_model = mock_modern_colortable_shaping_model(0.85, 0.15, "character.shpk");
    let modern_a_low_model = mock_modern_colortable_shaping_model(0.15, 0.85, "character.shpk");
    let modern_unorm_ceiling_model =
        mock_modern_colortable_shaping_model(1.0, 1.0, "character.shpk");
    let modern_hdr_anisotropy_model =
        mock_modern_colortable_shaping_model(7.0, 7.0, "character.shpk");
    let legacy_high_anisotropy_model =
        mock_modern_colortable_shaping_model(0.85, 0.85, "characterlegacy.shpk");
    let legacy_low_anisotropy_model =
        mock_modern_colortable_shaping_model(0.15, 0.15, "characterlegacy.shpk");
    let modern_high_anisotropy = render(
        "native-modern-colortable-shaping-high-anisotropy",
        &modern_high_anisotropy_model,
    );
    let modern_low_anisotropy = render(
        "native-modern-colortable-shaping-low-anisotropy",
        &modern_low_anisotropy_model,
    );
    let modern_a_high = render(
        "native-modern-colortable-shaping-a-high-b-low",
        &modern_a_high_model,
    );
    let modern_a_low = render(
        "native-modern-colortable-shaping-a-low-b-high",
        &modern_a_low_model,
    );
    let modern_unorm_ceiling = render(
        "native-modern-colortable-shaping-unorm-ceiling",
        &modern_unorm_ceiling_model,
    );
    let modern_hdr_anisotropy = render(
        "native-modern-colortable-shaping-hdr-anisotropy",
        &modern_hdr_anisotropy_model,
    );
    let legacy_high_anisotropy = render(
        "native-legacy-colortable-shaping-high-anisotropy",
        &legacy_high_anisotropy_model,
    );
    let legacy_low_anisotropy = render(
        "native-legacy-colortable-shaping-low-anisotropy",
        &legacy_low_anisotropy_model,
    );

    assert!(
        modern_high_anisotropy[0] > modern_low_anisotropy[0].saturating_add(8)
            && modern_high_anisotropy[2] < modern_low_anisotropy[2].saturating_sub(8),
        "modern anisotropy must shape the grazing ColorTable weight: {modern_high_anisotropy:?} -> {modern_low_anisotropy:?}"
    );
    assert_eq!(
        modern_a_high, modern_high_anisotropy,
        "modern shaping must read anisotropy from A, ignoring B: {modern_a_high:?} vs {modern_high_anisotropy:?}"
    );
    assert_eq!(
        modern_a_low, modern_low_anisotropy,
        "modern shaping must read anisotropy from A, ignoring B: {modern_a_low:?} vs {modern_low_anisotropy:?}"
    );
    let hdr_difference: u16 = (0..3)
        .map(|channel| {
            u16::from(modern_hdr_anisotropy[channel].abs_diff(modern_unorm_ceiling[channel]))
        })
        .sum();
    assert!(
        hdr_difference > 8,
        "modern shaping must preserve installed anisotropy above the UNORM ceiling: {modern_hdr_anisotropy:?} vs {modern_unorm_ceiling:?}"
    );
    assert_eq!(
        legacy_high_anisotropy, legacy_low_anisotropy,
        "legacy ColorTable blend must remain unshaped: {legacy_high_anisotropy:?} vs {legacy_low_anisotropy:?}"
    );
}

#[test]
#[ignore = "writes synthetic character texture mip bias snapshots with native wgpu"]
fn render_mock_character_texture_mip_bias_snapshot() {
    let unbiased = mock_texture_mip_bias_model(-8.0);
    let blurred = mock_texture_mip_bias_model(4.0);
    let render = |name, model, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic character texture mip bias snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic character texture mip bias PNG")
            .to_rgba8()
            .into_raw()
    };
    let unbiased_pixels = render(
        "native-character-mip-bias-negative-eight",
        &unbiased,
        ModelDebugMode::Final,
    );
    let blurred_pixels = render(
        "native-character-mip-bias-four",
        &blurred,
        ModelDebugMode::Final,
    );
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
        "g_TextureMipBias must select different levels for the verified primary diffuse sampler"
    );

    for (label, negative_name, positive_name, debug_mode) in [
        (
            "specular",
            "native-character-mip-bias-specular-negative-eight",
            "native-character-mip-bias-specular-four",
            ModelDebugMode::Specular,
        ),
        (
            "emissive",
            "native-character-mip-bias-emissive-negative-eight",
            "native-character-mip-bias-emissive-four",
            ModelDebugMode::Emissive,
        ),
    ] {
        let unbiased_debug = render(negative_name, &unbiased, debug_mode);
        let blurred_debug = render(positive_name, &blurred, debug_mode);
        assert_eq!(
            unbiased_debug, blurred_debug,
            "g_TextureMipBias must not alter the unverified {label} sampler role"
        );
    }
}

#[test]
#[ignore = "writes synthetic character tile mip bias snapshots with native wgpu"]
fn render_mock_character_tile_mip_bias_snapshot() {
    let sharp = mock_tile_mip_bias_model(-1.0);
    let blurred = mock_tile_mip_bias_model(1.0);
    let render = |name: String, model, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic character tile mip bias snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic character tile mip bias PNG")
            .to_rgba8()
            .into_raw()
    };

    for (label, debug_mode) in [
        ("normal", ModelDebugMode::TileNormalArray),
        ("orb", ModelDebugMode::TileOrbArray),
    ] {
        let sharp_pixels = render(
            format!("native-character-tile-mip-bias-{label}-negative-one"),
            &sharp,
            debug_mode,
        );
        let blurred_pixels = render(
            format!("native-character-tile-mip-bias-{label}-positive-one"),
            &blurred,
            debug_mode,
        );
        let rgb_difference: u64 = sharp_pixels
            .chunks_exact(4)
            .zip(blurred_pixels.chunks_exact(4))
            .map(|(sharp, blurred)| {
                (0..3)
                    .map(|channel| sharp[channel].abs_diff(blurred[channel]) as u64)
                    .sum::<u64>()
            })
            .sum();
        assert!(
            rgb_difference > 100_000,
            "g_TileMipBiasOffset must select different mip levels for Tile {label}: {rgb_difference}"
        );
    }
}

#[test]
#[ignore = "writes synthetic tile-array minification snapshots with native wgpu"]
fn render_mock_tile_array_minification_snapshot() {
    let checker = mock_tile_array_minification_model(false);
    let averaged = mock_tile_array_minification_model(true);
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_render_options(ModelRenderOptions {
                    debug_mode: ModelDebugMode::TileNormalArray,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic tile-array minification snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic tile-array minification PNG")
            .to_rgba8()
            .into_raw()
    };
    let checker_pixels = render("native-tile-array-minification-checker", &checker);
    let averaged_pixels = render("native-tile-array-minification-average", &averaged);
    assert_eq!(
        checker_pixels, averaged_pixels,
        "minified checker must select the independently generated per-layer average"
    );
}

#[test]
#[ignore = "writes synthetic tile-array floor-selection snapshots with native wgpu"]
fn render_mock_tile_array_floor_selection_snapshot() {
    let ramp_fractional = mock_tile_array_floor_selection_model(Some(86), 0.0);
    let ramp_layer_21 = mock_tile_array_floor_selection_model(Some(84), 0.0);
    let fallback_fractional = mock_tile_array_floor_selection_model(None, 21.75);
    let fallback_layer_21 = mock_tile_array_floor_selection_model(None, 21.0);
    let render = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(320, 320)
                .with_render_options(ModelRenderOptions {
                    debug_mode: ModelDebugMode::TileNormalArray,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic tile-array floor-selection snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic tile-array floor-selection PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render("native-tile-floor-ramp-fractional", &ramp_fractional),
        render("native-tile-floor-ramp-layer-21", &ramp_layer_21),
        "RGBA8 TileProperties value 86 decodes to 21.58 and must floor to layer 21"
    );
    assert_eq!(
        render(
            "native-tile-floor-fallback-fractional",
            &fallback_fractional,
        ),
        render("native-tile-floor-fallback-layer-21", &fallback_layer_21),
        "fractional g_TileIndex fallback must floor to layer 21"
    );
}

#[test]
#[ignore = "writes synthetic ColorTable extra ramp snapshots with native wgpu"]
fn render_mock_color_table_extra_ramp_snapshot() {
    let sheen_normal = [-0.231, 0.405, 0.884];
    let sphere_normal = [0.9, 0.0, -0.4359];
    let neutral_sheen = mock_extra_ramp_model([0, 0, 0, 255], [0, 0, 0, 255], sheen_normal);
    let sheen = mock_extra_ramp_model([255, 255, 0, 255], [0, 0, 0, 255], sheen_normal);
    let neutral_sphere = mock_extra_ramp_model([0, 0, 0, 255], [0, 0, 0, 255], sphere_normal);
    let sphere = mock_extra_ramp_model([0, 0, 0, 255], [255, 255, 0, 255], sphere_normal);
    let render_final = |name, model| {
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
    let render_unsupported = |name, model| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_render_options(ModelRenderOptions {
                    debug_mode: ModelDebugMode::UnsupportedInputs,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic ColorTable extra ramp unsupported snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic ColorTable extra ramp unsupported PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render_final("native-extra-ramp-neutral-sheen", &neutral_sheen),
        render_final("native-extra-ramp-sheen", &sheen),
        "MeddleTools does not prove a final Sheen composition, so the ramp must not silently alter Final shading"
    );
    assert_eq!(
        render_final("native-extra-ramp-neutral-sphere", &neutral_sphere),
        render_final("native-extra-ramp-sphere", &sphere),
        "MeddleTools does not prove a final Sphere composition, so the ramp must not silently alter Final shading"
    );

    let sheen_diagnostic = render_unsupported("native-extra-ramp-sheen-unsupported", &sheen);
    let sphere_diagnostic = render_unsupported("native-extra-ramp-sphere-unsupported", &sphere);
    let center = ((512 / 2) * 512 + (512 / 2)) * 4;
    let sheen_rgb = &sheen_diagnostic[center..center + 3];
    let sphere_rgb = &sphere_diagnostic[center..center + 3];
    let expected_sheen = compose_expected_bytes([0.95, 0.48, 0.62]);
    let expected_sphere = compose_expected_bytes([0.34, 0.7, 0.92]);
    for channel in 0..3 {
        assert!(
            sheen_rgb[channel].abs_diff(expected_sheen[channel]) <= 3,
            "nonzero Sheen must show its unsupported diagnostic hue"
        );
        assert!(
            sphere_rgb[channel].abs_diff(expected_sphere[channel]) <= 3,
            "nonzero Sphere must show its unsupported diagnostic hue"
        );
    }
    assert_ne!(
        sheen_rgb, sphere_rgb,
        "Sheen and Sphere diagnostics must remain distinguishable"
    );
}

#[test]
#[ignore = "writes synthetic detail multi blend snapshots with native wgpu"]
fn render_mock_detail_multi_blend_snapshot() {
    let primary = mock_detail_blend_model(0.0);
    let secondary = mock_detail_blend_model(1.0);
    let render = |name, model, debug_mode| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(512, 512)
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic detail blend snapshot")
    };

    let primary_snapshot = render(
        "native-detail-blend-primary",
        &primary,
        ModelDebugMode::Final,
    );
    let secondary_snapshot = render(
        "native-detail-blend-secondary",
        &secondary,
        ModelDebugMode::Final,
    );
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

    assert_eq!(
        rgb_difference, 0,
        "unverified detail influence must not alter Final when only vertex alpha changes"
    );

    let debug_pixels = |name, model, debug_mode| {
        let snapshot = render(name, model, debug_mode);
        pixels(snapshot.png_path)
    };
    let primary_diffuse = debug_pixels(
        "native-detail-blend-primary-diffuse",
        &primary,
        ModelDebugMode::DetailDiffuseArray,
    );
    let secondary_diffuse = debug_pixels(
        "native-detail-blend-secondary-diffuse",
        &secondary,
        ModelDebugMode::DetailDiffuseArray,
    );
    let primary_normal = debug_pixels(
        "native-detail-blend-primary-normal",
        &primary,
        ModelDebugMode::DetailNormalArray,
    );
    let secondary_normal = debug_pixels(
        "native-detail-blend-secondary-normal",
        &secondary,
        ModelDebugMode::DetailNormalArray,
    );
    let unsupported = debug_pixels(
        "native-detail-blend-unsupported",
        &primary,
        ModelDebugMode::UnsupportedInputs,
    );
    let debug_difference = |left: &[u8], right: &[u8]| -> u64 {
        left.chunks_exact(4)
            .zip(right.chunks_exact(4))
            .map(|(left, right)| {
                (0..3)
                    .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                    .sum::<u64>()
            })
            .sum()
    };
    assert!(
        debug_difference(&primary_diffuse, &secondary_diffuse) > 100_000,
        "detail diffuse debug must retain primary/multi layer selection"
    );
    assert!(
        debug_difference(&primary_normal, &secondary_normal) > 10_000,
        "detail normal debug must retain primary/multi layer selection"
    );
    let center = ((512 / 2) * 512 + (512 / 2)) * 4;
    let expected_unsupported = compose_expected_bytes([0.72, 0.52, 0.96]);
    for channel in 0..3 {
        assert!(
            unsupported[center + channel].abs_diff(expected_unsupported[channel]) <= 3,
            "detail composition must be visible as a structured unsupported diagnostic"
        );
    }
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
            .any(|pixel| pixel[2] > 60 && pixel[2] > pixel[0].saturating_add(40)),
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
        texel_layout: ModelTextureTexelLayout::Standard,
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
            // Keep the secondary material-properties sample neutral here. This
            // fixture verifies Map1 colour selection and the secondary tangent
            // frame; a bright BG specular sample drives the material into a
            // dark metallic response after tone mapping and makes both signals
            // unobservable in the final snapshot.
            vec![32, 32, 32, 255],
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

fn mock_lightshaft_model(vertex_blue: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic lightshaft",
        "path": null,
        "shaderPackageName": "lightshaft.shpk",
        "alphaMode": "blend",
        "fallbackColor": [1.0, 1.0, 1.0],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.0, 0.0, 0.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 1.0,
        "metalness": 0.0,
        "lightshaftColor": [1.0, 1.0, 1.0, 1.0],
        "textureIndices": [0, 1],
        "baseColorTexture": 0,
        "secondaryBaseColorTexture": 1,
    }))
    .expect("deserialize synthetic lightshaft material");
    let texture = |path: &str, kind, rgba| WeaponModelTexture {
        path: path.to_string(),
        kind,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba,
        rgba_f32: None,
    };
    let mut model = mock_weapon_model();
    model.item_id = 4;
    model.item_name = "Synthetic LightShaft".to_string();
    model.materials = vec![material];
    model.textures = vec![
        texture(
            "synthetic/lightshaft0.tex",
            ModelTextureKind::BaseColor,
            vec![255, 160, 80, 255],
        ),
        texture(
            "synthetic/lightshaft1.tex",
            ModelTextureKind::SecondaryBaseColor,
            vec![48, 224, 128, 255],
        ),
    ];
    model.meshes[0].mesh_category = Some("lightShaft".to_string());
    model.meshes[0].material_name = "synthetic lightshaft".to_string();
    for vertex in &mut model.meshes[0].vertices {
        vertex.color = [1.0, 1.0, vertex_blue, 1.0];
    }
    model
}

fn mock_lightshaft_alpha_test_model(alpha_threshold: f32) -> WeaponModelData {
    let mut model = mock_lightshaft_model(0.0);
    model.materials[0].alpha_mode = MaterialAlphaMode::Mask;
    model.materials[0].alpha_threshold = alpha_threshold;
    model
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
        texel_layout: ModelTextureTexelLayout::Standard,
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

fn mock_tattoo_vertex_alpha_model(normal_alpha: u8, vertex_alpha: f32) -> WeaponModelData {
    let mut model = mock_tattoo_model(normal_alpha);
    for vertex in &mut model.meshes[0].vertices {
        vertex.color[3] = vertex_alpha;
    }
    model
}

fn mock_character_normal_blue_model(normal_blue: u8, vertex_alpha: f32) -> WeaponModelData {
    let mut model = mock_tattoo_vertex_alpha_model(255, vertex_alpha);
    model.item_name = "Synthetic Character Normal Blue".to_string();
    model.materials[0].name = "synthetic character normal blue".to_string();
    model.materials[0].shader_package_name = Some("character.shpk".to_string());
    model.materials[0].vertex_alpha_to_one = 1.0;
    model.materials[0].draw_depth_mode = xiv_companion_render::MaterialDrawDepthMode::None;
    model.textures[0].path = "synthetic/character_normal_blue.tex".to_string();
    model.textures[0].rgba = vec![128, 128, normal_blue, 255];
    model.meshes[0].material_name = "synthetic character normal blue".to_string();
    model
}

fn mock_transparent_triangle_sorting_model() -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic transparent triangle sorting",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "blend",
        "drawDepthMode": "none",
        "lightingMode": "disabled",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [1.0, 1.0, 1.0],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.0, 0.0, 0.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 1.0,
        "metalness": 0.0,
        "opacity": 1.0,
        "renderBackfaces": true,
        "textureIndices": [0, 1],
        "baseColorTexture": 0,
        "normalTexture": 1
    }))
    .expect("deserialize synthetic transparent triangle sorting material");
    let base_color = WeaponModelTexture {
        path: "synthetic/transparent_triangle_sorting_base.tex".to_string(),
        kind: ModelTextureKind::BaseColor,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![255, 13, 13, 255, 13, 13, 255, 255],
        rgba_f32: None,
    };
    let normal = WeaponModelTexture {
        path: "synthetic/transparent_triangle_sorting_normal.tex".to_string(),
        kind: ModelTextureKind::Normal,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![128, 128, 128, 255],
        rgba_f32: None,
    };
    let quad = |z, uv_x| {
        let mut vertices = [
            vertex([-0.8, -0.8, z], [1.0; 4]),
            vertex([0.8, -0.8, z], [1.0; 4]),
            vertex([0.8, 0.8, z], [1.0; 4]),
            vertex([-0.8, 0.8, z], [1.0; 4]),
        ];
        for vertex in &mut vertices {
            vertex.uv0 = [uv_x, 0.5];
        }
        vertices
    };
    let vertices = quad(-0.15, 0.25)
        .into_iter()
        .chain(quad(0.15, 0.75))
        .collect();

    WeaponModelData {
        item_id: 5,
        item_name: "Synthetic Transparent Triangle Sorting".to_string(),
        model_main: PackedModelId::from_raw(5),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/transparent_triangle_sorting.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, -0.15],
            max: [0.8, 0.8, 0.15],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: vec![base_color, normal],
        meshes: vec![WeaponModelMesh {
            path: "synthetic/transparent_triangle_sorting.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic transparent triangle sorting".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
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
        texel_layout: ModelTextureTexelLayout::Standard,
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
            texel_layout: ModelTextureTexelLayout::Standard,
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

fn mock_tile_array_minification_model(uniform_average: bool) -> WeaponModelData {
    let mut model = mock_uniform_tile_matrix_model(512.0);
    let normal = model
        .textures
        .iter_mut()
        .find(|texture| texture.kind == ModelTextureKind::TileNormalArray)
        .expect("synthetic model has a tile normal array");
    let layer = if uniform_average {
        [218, 218, 128, 128].repeat(4)
    } else {
        vec![
            255, 128, 0, 0, 128, 255, 64, 64, 255, 128, 192, 192, 128, 255, 255, 255,
        ]
    };
    normal.rgba = layer.repeat(2);
    model
}

fn mock_tile_mip_bias_model(tile_mip_bias_offset: f32) -> WeaponModelData {
    let mut model = mock_tile_matrix_model(128.0);
    model.materials[0].tile_mip_bias_offset = tile_mip_bias_offset;
    model
}

fn mock_tile_ab_ramp_model(blend: u8, alpha_a: u8, alpha_b: u8) -> WeaponModelData {
    let mut model = mock_tile_matrix_model(1.0);
    let tile_properties = model
        .textures
        .iter_mut()
        .find(|texture| texture.kind == ModelTextureKind::TileProperties)
        .expect("synthetic model has TileProperties");
    tile_properties.texel_layout = ModelTextureTexelLayout::ColorTableTileRampAb;
    tile_properties.width = 2;
    let source_blend = 255u8.saturating_sub(blend);
    tile_properties.rgba = vec![0, alpha_a, source_blend, 255, 4, alpha_b, source_blend, 255];

    let tile_matrix = model
        .textures
        .iter_mut()
        .find(|texture| texture.kind == ModelTextureKind::TileMatrixProperties)
        .expect("synthetic model has TileMatrixProperties");
    tile_matrix.texel_layout = ModelTextureTexelLayout::ColorTableTileRampAb;
    tile_matrix.width = 2;
    tile_matrix.rgba = vec![255, 0, 0, 255, 0, 0, 255, 0];
    tile_matrix.rgba_f32 = Some(vec![[1.0, 0.0, 0.0, 1.0], [0.0, -1.0, 1.0, 0.0]]);

    for texture in &mut model.textures {
        match texture.kind {
            ModelTextureKind::TileNormalArray => {
                texture.rgba = [224, 128, 255, 255].repeat(8);
            }
            ModelTextureKind::TileOrbArray => {
                texture.rgba = [255, 32, 64, 255]
                    .repeat(4)
                    .into_iter()
                    .chain([64, 224, 255, 255].repeat(4))
                    .collect();
            }
            _ => {}
        }
    }
    model
}

fn mock_modern_colortable_shaping_model(
    anisotropy_a: f32,
    anisotropy_b: f32,
    shader_package_name: &str,
) -> WeaponModelData {
    let mut model = mock_tile_ab_ramp_model(128, 255, 255);
    model.materials[0].shader_package_name = Some(shader_package_name.to_string());

    let append_ramp = |model: &mut WeaponModelData, kind, rgba: Vec<u8>| {
        let index = model.textures.len();
        model.textures.push(WeaponModelTexture {
            path: format!("synthetic/colortable-{kind:?}.tex"),
            kind,
            texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
            width: 2,
            height: 1,
            array_size: 1,
            array_layer_height: 0,
            rgba,
            rgba_f32: None,
        });
        index
    };
    let base_color = append_ramp(
        &mut model,
        ModelTextureKind::BaseColor,
        [255, 255, 255, 255].repeat(2),
    );
    let anisotropy_a_byte = (anisotropy_a.clamp(0.0, 1.0) * 255.0).round() as u8;
    let anisotropy_b_byte = (anisotropy_b.clamp(0.0, 1.0) * 255.0).round() as u8;
    let specular = append_ramp(
        &mut model,
        ModelTextureKind::Specular,
        [
            255,
            255,
            255,
            anisotropy_a_byte,
            255,
            255,
            255,
            anisotropy_b_byte,
        ]
        .to_vec(),
    );
    model.textures[specular].rgba_f32 = Some(vec![
        [1.0, 1.0, 1.0, anisotropy_a],
        [1.0, 1.0, 1.0, anisotropy_b],
    ]);
    let material_properties = append_ramp(
        &mut model,
        ModelTextureKind::MaterialProperties,
        [0, 128, 0, 255].repeat(2),
    );
    let sheen_properties = append_ramp(
        &mut model,
        ModelTextureKind::SheenProperties,
        [0, 0, 0, 255].repeat(2),
    );
    let sphere_properties = append_ramp(
        &mut model,
        ModelTextureKind::SphereProperties,
        [0, 0, 255, 255].repeat(2),
    );
    let material = &mut model.materials[0];
    material.base_color_texture = Some(base_color);
    material.specular_texture = Some(specular);
    material.material_properties_texture = Some(material_properties);
    material.sheen_properties_texture = Some(sheen_properties);
    material.sphere_properties_texture = Some(sphere_properties);
    model
}

fn mock_tile_array_floor_selection_model(
    tile_properties_red: Option<u8>,
    tile_index: f32,
) -> WeaponModelData {
    let mut model = mock_uniform_tile_matrix_model(1.0);
    model.materials[0].tile_index = tile_index;
    if let Some(red) = tile_properties_red {
        let tile_properties = model
            .textures
            .iter_mut()
            .find(|texture| texture.kind == ModelTextureKind::TileProperties)
            .expect("synthetic model has TileProperties");
        tile_properties.rgba = vec![red, 255, 255, 255];
    } else {
        model.materials[0].tile_properties_texture = None;
    }

    for texture in &mut model.textures {
        let pixel = match texture.kind {
            ModelTextureKind::TileNormalArray => [128, 128, 255, 255],
            ModelTextureKind::TileOrbArray => [255, 128, 255, 255],
            _ => continue,
        };
        texture.width = 1;
        texture.height = 64;
        texture.array_size = 64;
        texture.array_layer_height = 1;
        texture.rgba = pixel.repeat(64);
        if texture.kind == ModelTextureKind::TileNormalArray {
            texture.rgba[22 * 4..23 * 4].copy_from_slice(&[255, 128, 255, 255]);
        }
    }
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
        "textureIndices": [0, 1, 2],
        "baseColorTexture": 0,
        "specularTexture": 1,
        "emissiveTexture": 2
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
        textures: [
            (
                "synthetic/character_mip_bias_base.tex",
                ModelTextureKind::BaseColor,
            ),
            (
                "synthetic/character_mip_bias_specular.tex",
                ModelTextureKind::Specular,
            ),
            (
                "synthetic/character_mip_bias_emissive.tex",
                ModelTextureKind::Emissive,
            ),
        ]
        .into_iter()
        .map(|(path, kind)| WeaponModelTexture {
            path: path.to_string(),
            kind,
            texel_layout: ModelTextureTexelLayout::Standard,
            width,
            height,
            array_size: 1,
            array_layer_height: 0,
            rgba: rgba.clone(),
            rgba_f32: None,
        })
        .collect(),
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
            texel_layout: ModelTextureTexelLayout::Standard,
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

#[test]
#[ignore = "writes synthetic two-sided normal orientation snapshots with native wgpu"]
fn render_mock_two_sided_normal_orientation_snapshots() {
    let reference = mock_two_sided_orientation_model(true);
    let two_sided = mock_two_sided_orientation_model(false);
    let render = |name: &str, yaw: f32, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(yaw, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render two-sided normal orientation snapshot");
        image::open(snapshot.png_path)
            .expect("decode two-sided normal orientation PNG")
            .to_rgba8()
            .into_raw()
    };
    let reference_pixels = render("native-two-sided-orientation-reference", 0.0, &reference);
    let front_pixels = render("native-two-sided-orientation-front", 0.0, &two_sided);
    let back_pixels = render(
        "native-two-sided-orientation-back",
        std::f32::consts::PI,
        &two_sided,
    );

    // Probe the center crop, which covers the quad from every camera used here.
    let center_luminance = |pixels: &[u8]| -> (f64, f64) {
        let (width, height) = (256usize, 256usize);
        let mut luminances = Vec::new();
        for y in height / 4..height * 3 / 4 {
            for x in width / 4..width * 3 / 4 {
                let offset = (y * width + x) * 4;
                let red = pixels[offset] as f64;
                let green = pixels[offset + 1] as f64;
                let blue = pixels[offset + 2] as f64;
                luminances.push(0.2126 * red + 0.7152 * green + 0.0722 * blue);
            }
        }
        let mean = luminances.iter().sum::<f64>() / luminances.len() as f64;
        luminances.sort_by(f64::total_cmp);
        let p95 = luminances[luminances.len() * 95 / 100];
        (mean, p95)
    };
    let (reference_mean, reference_p95) = center_luminance(&reference_pixels);
    let (front_mean, front_p95) = center_luminance(&front_pixels);
    let (back_mean, back_p95) = center_luminance(&back_pixels);
    eprintln!(
        "two-sided orientation luminance: reference mean {reference_mean:.1} p95 {reference_p95:.1}, \
         front mean {front_mean:.1} p95 {front_p95:.1}, back mean {back_mean:.1} p95 {back_p95:.1}"
    );

    assert!(
        reference_mean > 100.0,
        "reference front-lit surface must be clearly lit (mean luminance {reference_mean:.1})"
    );
    for (side, mean, p95) in [
        ("front", front_mean, front_p95),
        ("back", back_mean, back_p95),
    ] {
        let mean_ratio = mean / reference_mean;
        assert!(
            (0.8..=1.25).contains(&mean_ratio),
            "{side} side of a two-sided surface must match the reference luminance \
             (ratio {mean_ratio:.2}, {side} {mean:.1} vs reference {reference_mean:.1})"
        );
        let p95_ratio = p95 / reference_p95.max(1.0);
        assert!(
            (0.8..=1.25).contains(&p95_ratio),
            "{side} side of a two-sided surface must match the reference specular response \
             (ratio {p95_ratio:.2}, {side} p95 {p95:.1} vs reference p95 {reference_p95:.1})"
        );
    }
    let symmetry_ratio = back_mean / front_mean.max(1.0);
    assert!(
        (0.9..=1.12).contains(&symmetry_ratio),
        "both directions of the same two-sided surface must shade symmetrically \
         (ratio {symmetry_ratio:.2}, front {front_mean:.1} vs back {back_mean:.1})"
    );
}

fn mock_two_sided_orientation_model(normals_agree_with_winding: bool) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic two-sided orientation",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [0.7, 0.72, 0.78],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.6,
        "metalness": 0.0,
        "renderBackfaces": !normals_agree_with_winding,
        "textureIndices": [0],
        "normalTexture": 0,
    }))
    .expect("deserialize synthetic two-sided orientation material");
    let normal = WeaponModelTexture {
        path: "synthetic/two_sided_orientation_normal.tex".to_string(),
        kind: ModelTextureKind::Normal,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![128, 128, 255, 255],
        rgba_f32: None,
    };
    let positions = [
        [-0.8, -0.8, 0.0],
        [0.8, -0.8, 0.0],
        [0.8, 0.8, 0.0],
        [-0.8, 0.8, 0.0],
    ];
    // The quad winds counter-clockwise when seen from +Z. Disagreeing normals
    // point to -Z, mimicking imports where vertex normals fight the winding.
    let geometric_normal = if normals_agree_with_winding {
        [0.0, 0.0, 1.0]
    } else {
        [0.0, 0.0, -1.0]
    };
    let vertices = positions
        .into_iter()
        .map(|position| {
            let mut vertex = vertex(position, [1.0; 4]);
            vertex.normal = geometric_normal;
            vertex
        })
        .collect();

    WeaponModelData {
        item_id: 6,
        item_name: "Synthetic Two-Sided Orientation".to_string(),
        model_main: PackedModelId::from_raw(6),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/two_sided_orientation.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: vec![normal],
        meshes: vec![WeaponModelMesh {
            path: "synthetic/two_sided_orientation.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic two-sided orientation".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

#[test]
#[ignore = "writes synthetic baked specular color space snapshots with native wgpu"]
fn render_mock_baked_specular_color_space_snapshot() {
    // The baked ColorTable specular ramp stores sRGB-encoded RGB. Sampled with
    // sRGB GPU decoding, the shader sees the original linear ColorTable value.
    // The observable PNG passes the scene value through the documented compose
    // transform (Khronos PBR Neutral tone map + sRGB encode), so the expected
    // bytes are computed with the same operators. A Non-Color upload would
    // sample the raw bytes as linear and miss the expectation by a wide
    // margin (for example 181 -> 219 on the green channel).
    let source_specular_bytes = [137u8, 188, 225];
    let model = mock_baked_specular_color_space_model(source_specular_bytes);
    let snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-baked-specular-color-space")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::Specular,
                ..ModelRenderOptions::default()
            }),
        &model,
    )
    .expect("render baked specular color space snapshot");
    let pixels = image::open(snapshot.png_path)
        .expect("decode baked specular color space PNG")
        .to_rgba8()
        .into_raw();

    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let sampled = &pixels[center..center + 3];
    let source_linear = source_specular_bytes.map(|byte| {
        let value = f32::from(byte) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    });
    let expected = tonemap_pbr_neutral_rgb(source_linear).map(srgb_encode_u8);
    eprintln!(
        "baked specular color space: source {source_specular_bytes:?}, \
         expected after compose {expected:?}, sampled {sampled:?}"
    );
    for channel in 0..3 {
        let difference = sampled[channel].abs_diff(expected[channel]);
        assert!(
            difference <= 2,
            "sampled linear GPU value must match the source ColorTable sRGB encoding \
             (channel {channel}: sampled {} vs expected {})",
            sampled[channel],
            expected[channel]
        );
    }
}

/// Mirrors `tonemap_pbr_neutral` in `postprocess.wgsl` (Khronos PBR Neutral).
fn tonemap_pbr_neutral_rgb(color: [f32; 3]) -> [f32; 3] {
    let min_channel = color[0].min(color[1]).min(color[2]);
    let offset = if min_channel < 0.08 {
        min_channel - 6.25 * min_channel * min_channel
    } else {
        0.04
    };
    let mut adjusted = color.map(|channel| channel - offset);
    let peak = adjusted[0].max(adjusted[1]).max(adjusted[2]);
    if peak < 0.76 {
        return adjusted;
    }
    let d = 1.0 - 0.76;
    let new_peak = 1.0 - d * d / (peak + d - 0.76);
    for channel in &mut adjusted {
        *channel *= new_peak / peak;
    }
    let desaturation = 1.0 - 1.0 / (0.15 * (peak - new_peak) + 1.0);
    adjusted.map(|channel| channel + (new_peak - channel) * desaturation)
}

/// Mirrors `linear_to_srgb_channel` in `postprocess.wgsl`.
fn srgb_encode_u8(value: f32) -> u8 {
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Applies the compose-pass transform (tone map + sRGB encode) to a linear
/// scene color so expected debug-view bytes match the GPU output.
fn compose_expected_bytes(linear_color: [f32; 3]) -> [u8; 3] {
    tonemap_pbr_neutral_rgb(linear_color).map(srgb_encode_u8)
}

#[test]
#[ignore = "writes synthetic unsupported-inputs diagnostic snapshots with native wgpu"]
fn render_mock_unsupported_inputs_debug_snapshots() {
    let supported = mock_unsupported_inputs_model("character.shpk", "opaque", false);
    let mut ssao = supported.clone();
    ssao.materials[0].ssao_mask = 0.9;
    let mut legacy_gloss = supported.clone();
    legacy_gloss.materials[0].shader_package_name = Some("characterlegacy.shpk".to_string());
    legacy_gloss.materials[0].color_table_rows = Some(vec![ColorTableRowColors::default()]);
    let glass = mock_unsupported_inputs_model("characterglass.shpk", "glass", false);
    let crystal_environment = mock_unsupported_inputs_model("crystal.shpk", "opaque", true);
    let render = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode: ModelDebugMode::UnsupportedInputs,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render unsupported-inputs diagnostic snapshot");
        image::open(snapshot.png_path)
            .expect("decode unsupported-inputs diagnostic PNG")
            .to_rgba8()
            .into_raw()
    };
    let supported_pixels = render("native-unsupported-inputs-supported", &supported);
    let ssao_pixels = render("native-unsupported-inputs-ssao", &ssao);
    let legacy_gloss_pixels = render("native-unsupported-inputs-legacy-gloss", &legacy_gloss);
    let glass_pixels = render("native-unsupported-inputs-glass", &glass);
    let crystal_pixels = render(
        "native-unsupported-inputs-crystal-environment",
        &crystal_environment,
    );

    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let supported_rgb = &supported_pixels[center..center + 3];
    let ssao_rgb = &ssao_pixels[center..center + 3];
    let legacy_gloss_rgb = &legacy_gloss_pixels[center..center + 3];
    let glass_rgb = &glass_pixels[center..center + 3];
    let crystal_rgb = &crystal_pixels[center..center + 3];
    let expected_supported = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_ssao = compose_expected_bytes([0.58, 0.72, 0.16]);
    let expected_legacy_gloss = compose_expected_bytes([0.76, 0.3, 0.9]);
    let expected_glass = compose_expected_bytes([0.15, 0.85, 0.9]);
    let expected_crystal = compose_expected_bytes([0.25, 0.5, 1.0]);
    eprintln!(
        "unsupported-inputs diagnostic: supported {supported_rgb:?} (expected {expected_supported:?}), \
         ssao {ssao_rgb:?} (expected {expected_ssao:?}), \
         legacy Gloss {legacy_gloss_rgb:?} (expected {expected_legacy_gloss:?}), \
         glass {glass_rgb:?} (expected {expected_glass:?}), \
         crystal environment {crystal_rgb:?} (expected {expected_crystal:?})"
    );

    for channel in 0..3 {
        assert!(
            supported_rgb[channel].abs_diff(expected_supported[channel]) <= 3,
            "supported material must show the fully-supported hue \
             (channel {channel}: {} vs expected {})",
            supported_rgb[channel],
            expected_supported[channel]
        );
        assert!(
            ssao_rgb[channel].abs_diff(expected_ssao[channel]) <= 3,
            "non-default g_SSAOMask must show the explicit unsupported AO hue \
             (channel {channel}: {} vs expected {})",
            ssao_rgb[channel],
            expected_ssao[channel]
        );
        assert!(
            legacy_gloss_rgb[channel].abs_diff(expected_legacy_gloss[channel]) <= 3,
            "characterlegacy ColorTable must show the explicit unsupported Gloss composition hue \
             (channel {channel}: {} vs expected {})",
            legacy_gloss_rgb[channel],
            expected_legacy_gloss[channel]
        );
        assert!(
            glass_rgb[channel].abs_diff(expected_glass[channel]) <= 3,
            "characterglass must show its known-incomplete family hue \
             (channel {channel}: {} vs expected {})",
            glass_rgb[channel],
            expected_glass[channel]
        );
        assert!(
            crystal_rgb[channel].abs_diff(expected_crystal[channel]) <= 3,
            "crystal EnvMap must show the explicit unsupported environment hue \
             (channel {channel}: {} vs expected {})",
            crystal_rgb[channel],
            expected_crystal[channel]
        );
    }
    let hue_difference: u32 = (0..3)
        .map(|channel| supported_rgb[channel].abs_diff(glass_rgb[channel]) as u32)
        .sum();
    assert!(
        hue_difference > 150,
        "known-incomplete families must be visibly distinct from supported materials \
         (supported {supported_rgb:?} vs glass {glass_rgb:?})"
    );

    let render_final = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render SSAO Final snapshot");
        image::open(snapshot.png_path)
            .expect("decode SSAO Final PNG")
            .to_rgba8()
            .into_raw()
    };
    assert_eq!(
        render_final("native-ssao-default-final", &supported),
        render_final("native-ssao-nondefault-final", &ssao),
        "g_SSAOMask has no verified runtime SSAO composition and must not silently darken Final shading"
    );
}

fn mock_unsupported_inputs_model(
    shader_package_name: &str,
    alpha_mode: &str,
    with_environment: bool,
) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic unsupported inputs",
        "path": null,
        "shaderPackageName": shader_package_name,
        "alphaMode": alpha_mode,
        "fallbackColor": [0.7, 0.72, 0.78],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.2, 0.2, 0.2],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.5,
        "metalness": 0.0,
        "renderBackfaces": false,
        "textureIndices": if with_environment { vec![0] } else { Vec::<usize>::new() },
        "environmentTexture": if with_environment { Some(0) } else { None },
    }))
    .expect("deserialize synthetic unsupported-inputs material");
    let textures = if with_environment {
        vec![WeaponModelTexture {
            path: "synthetic/crystal_environment.tex".to_string(),
            kind: ModelTextureKind::Environment,
            texel_layout: ModelTextureTexelLayout::Standard,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 0,
            rgba: vec![255, 255, 255, 255],
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
    let vertices = positions
        .into_iter()
        .map(|position| vertex(position, [1.0; 4]))
        .collect();

    WeaponModelData {
        item_id: 9,
        item_name: "Synthetic Unsupported Inputs".to_string(),
        model_main: PackedModelId::from_raw(9),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/unsupported_inputs.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures,
        meshes: vec![WeaponModelMesh {
            path: "synthetic/unsupported_inputs.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic unsupported inputs".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_baked_specular_color_space_model(specular_bytes: [u8; 3]) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic baked specular color space",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "fallbackColor": [0.7, 0.72, 0.78],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.5,
        "metalness": 0.0,
        "renderBackfaces": false,
        "textureIndices": [0],
        "specularTexture": 0,
    }))
    .expect("deserialize synthetic baked specular material");
    let specular = WeaponModelTexture {
        path: "baked://synthetic.mtrl#colorset-specular".to_string(),
        kind: ModelTextureKind::Specular,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![specular_bytes[0], specular_bytes[1], specular_bytes[2], 128],
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
        item_id: 7,
        item_name: "Synthetic Baked Specular Color Space".to_string(),
        model_main: PackedModelId::from_raw(7),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/baked_specular_color_space.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: vec![specular],
        meshes: vec![WeaponModelMesh {
            path: "synthetic/baked_specular_color_space.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic baked specular color space".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

#[test]
#[ignore = "writes synthetic HDR emissive snapshots with native wgpu"]
fn render_mock_hdr_emissive_detail_snapshots() {
    // Both emissive levels are above 1.0. An LDR intermediate would clip them
    // to the same value; the HDR pipeline must keep them distinct after tone
    // mapping. The Khronos PBR Neutral operator desaturates compressed
    // highlights, so the green channel grows with the peak even though both
    // reds stay near white.
    let low = mock_hdr_emissive_model(2.0);
    let high = mock_hdr_emissive_model(8.0);
    let baked_low = mock_hdr_baked_emissive_model(2.0);
    let baked_high = mock_hdr_baked_emissive_model(8.0);
    let render = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render HDR emissive snapshot");
        image::open(snapshot.png_path)
            .expect("decode HDR emissive PNG")
            .to_rgba8()
            .into_raw()
    };
    let low_pixels = render("native-hdr-emissive-low", &low);
    let high_pixels = render("native-hdr-emissive-high", &high);
    let baked_low_pixels = render("native-hdr-baked-emissive-low", &baked_low);
    let baked_high_pixels = render("native-hdr-baked-emissive-high", &baked_high);

    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let low_rgb = &low_pixels[center..center + 3];
    let high_rgb = &high_pixels[center..center + 3];
    eprintln!("HDR emissive: level 2.0 -> {low_rgb:?}, level 8.0 -> {high_rgb:?}");

    assert!(
        low_rgb[0] > 220,
        "moderate HDR highlight must stay near white (red {})",
        low_rgb[0]
    );
    assert!(
        high_rgb[0] >= low_rgb[0],
        "stronger HDR highlight must not invert the tone-mapped red ({} vs {})",
        high_rgb[0],
        low_rgb[0]
    );
    assert!(
        high_rgb[1] > low_rgb[1].saturating_add(30),
        "HDR highlight detail must survive composition: green {} (8.0) vs {} (2.0) \
         would both be 0 when clipped to LDR",
        high_rgb[1],
        low_rgb[1]
    );

    let baked_low_rgb = &baked_low_pixels[center..center + 3];
    let baked_high_rgb = &baked_high_pixels[center..center + 3];
    assert!(
        baked_high_rgb[1] > baked_low_rgb[1].saturating_add(30),
        "baked ColorTable emissive float payload must preserve HDR detail: {baked_low_rgb:?} vs {baked_high_rgb:?}"
    );

    let source_emissive = 61.46875;
    let sampled = mock_hdr_baked_emissive_model(source_emissive);
    let sampled_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-baked-emissive-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::Emissive,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &sampled,
    )
    .expect("capture linear HDR emissive scene");
    let sampled_hdr = sampled_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let sampled_center = sampled_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (sampled_center[0] - source_emissive).abs() <= 0.04,
        "GPU emissive sample must match the source ColorTable value within Rgba16Float precision: source {source_emissive}, sampled {sampled_center:?}"
    );
    assert_eq!(sampled_center[1], 0.0);
    assert_eq!(sampled_center[2], 0.0);

    let shader_emissive_source = 16.0;
    let shader_emissive = mock_hdr_emissive_model(shader_emissive_source);
    let shader_emissive_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-shader-emissive-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::Emissive,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &shader_emissive,
    )
    .expect("capture linear shader emissive scene");
    let shader_emissive_hdr = shader_emissive_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let shader_emissive_center = shader_emissive_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (shader_emissive_center[0] - shader_emissive_source).abs() <= 0.01,
        "g_EmissiveColor must remain linear HDR instead of clipping at the old preview limit: source {shader_emissive_source}, sampled {shader_emissive_center:?}"
    );
    assert_eq!(shader_emissive_center[1], 0.0);
    assert_eq!(shader_emissive_center[2], 0.0);

    let shader_diffuse_source = 9.0;
    let mut shader_diffuse = mock_hdr_emissive_model(0.0);
    shader_diffuse.materials[0].diffuse_color = [1.0; 3];
    shader_diffuse.materials[0].shader_diffuse_color = [shader_diffuse_source, 0.0, 0.0, 1.0];
    let shader_diffuse_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-shader-diffuse-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::BaseColor,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &shader_diffuse,
    )
    .expect("capture linear shader diffuse scene");
    let shader_diffuse_hdr = shader_diffuse_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let shader_diffuse_center = shader_diffuse_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (shader_diffuse_center[0] - shader_diffuse_source).abs() <= 0.01,
        "g_DiffuseColor must remain linear HDR instead of clipping at the old preview limit: source {shader_diffuse_source}, sampled {shader_diffuse_center:?}"
    );
    assert_eq!(shader_diffuse_center[1], 0.0);
    assert_eq!(shader_diffuse_center[2], 0.0);

    let diffuse_source = 6.7929688;
    let diffuse = mock_hdr_baked_diffuse_model(diffuse_source);
    let diffuse_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-baked-diffuse-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::BaseColor,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &diffuse,
    )
    .expect("capture linear HDR diffuse scene");
    let diffuse_hdr = diffuse_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let diffuse_center = diffuse_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (diffuse_center[0] - diffuse_source).abs() <= 0.01,
        "GPU diffuse sample must match the source ColorTable value within Rgba16Float precision: source {diffuse_source}, sampled {diffuse_center:?}"
    );
    assert_eq!(diffuse_center[1], 0.0);
    assert_eq!(diffuse_center[2], 0.0);

    let specular_source = 4900.0;
    let specular = mock_hdr_baked_specular_model(specular_source);
    let specular_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-baked-specular-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::Specular,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &specular,
    )
    .expect("capture linear HDR specular scene");
    let specular_hdr = specular_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let specular_center = specular_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (specular_center[0] - specular_source).abs() <= 2.0,
        "GPU specular sample must match the source ColorTable value within Rgba16Float precision: source {specular_source}, sampled {specular_center:?}"
    );
    assert_eq!(specular_center[1], 0.0);
    assert_eq!(specular_center[2], 0.0);

    let material_properties_source = [1.0, 0.5, 193.375, 100.0];
    let material_properties = mock_hdr_baked_material_properties_model(material_properties_source);
    let material_properties_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-baked-material-properties-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::MaterialProperties,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &material_properties,
    )
    .expect("capture linear HDR material properties scene");
    let material_properties_hdr = material_properties_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let material_properties_center = material_properties_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!((material_properties_center[0] - 1.0).abs() <= 0.001);
    assert!((material_properties_center[1] - 0.5).abs() <= 0.001);
    assert!(
        (material_properties_center[2] - material_properties_source[3]).abs() <= 0.04,
        "GPU material-properties sample must preserve installed SpecularStrength above UNORM: source {material_properties_source:?}, sampled {material_properties_center:?}"
    );

    let material_strength_model = |strength| {
        let mut model = mock_hdr_baked_material_properties_model([0.0, 0.35, 1.0, strength]);
        model.materials[0].diffuse_color = [0.05; 3];
        model.materials[0].specular_color = [0.02; 3];
        model
    };
    let capture_strength = |name, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    bloom: false,
                    ..ModelRenderOptions::default()
                })
                .with_hdr_scene_capture(),
            model,
        )
        .expect("capture SpecularStrength final scene");
        snapshot
            .hdr_scene_rgba
            .expect("HDR scene capture was requested")[(256 / 2) * 256 + (256 / 2)]
    };
    let strength_one = material_strength_model(1.0);
    let strength_hundred = material_strength_model(100.0);
    let strength_one_center = capture_strength("native-specular-strength-one", &strength_one);
    let strength_hundred_center =
        capture_strength("native-specular-strength-hundred", &strength_hundred);
    let luminance = |color: [f32; 4]| color[0] * 0.2126 + color[1] * 0.7152 + color[2] * 0.0722;
    assert!(
        luminance(strength_hundred_center) > luminance(strength_one_center) + 0.02,
        "raw installed SpecularStrength must affect Final before the physical F0 bound: strength 1 {strength_one_center:?}, strength 100 {strength_hundred_center:?}"
    );

    let sheen_source = 52.09375;
    let mut sheen = mock_extra_ramp_model([0, 0, 255, 255], [0, 0, 255, 255], [0.0, 0.0, 1.0]);
    sheen.textures[0].rgba_f32 = Some(vec![[0.0, 0.0, sheen_source, 1.0]]);
    let sheen_snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-hdr-baked-sheen-linear-sample")
            .with_viewport(256, 256)
            .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::SheenProperties,
                bloom: false,
                ..ModelRenderOptions::default()
            })
            .with_hdr_scene_capture(),
        &sheen,
    )
    .expect("capture linear HDR sheen scene");
    let sheen_hdr = sheen_snapshot
        .hdr_scene_rgba
        .expect("HDR scene capture was requested");
    let sheen_center = sheen_hdr[(256 / 2) * 256 + (256 / 2)];
    assert!(
        (sheen_center[2] - sheen_source).abs() <= 0.02,
        "GPU sheen sample must match the source ColorTable aptitude within Rgba16Float precision: source {sheen_source}, sampled {sheen_center:?}"
    );
    assert_eq!(sheen_center[0], 0.0);
    assert_eq!(sheen_center[1], 0.0);
}

#[test]
#[ignore = "validates Character ColorTable emissive luminance scaling with native wgpu"]
fn render_mock_character_colortable_emissive_luminance_scale() {
    let exact = mock_character_colortable_emissive_luminance_model("character.shpk");
    let control = mock_character_colortable_emissive_luminance_model("characterscroll.shpk");
    let capture =
        |name: &str, model: &WeaponModelData, debug_mode, dynamic_emissive_color: [f32; 3]| {
            render_weapon_model_snapshot_with_options(
                WeaponModelSnapshotOptions::new(name)
                    .with_viewport(256, 256)
                    .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                    .with_render_options(ModelRenderOptions {
                        debug_mode,
                        bloom: false,
                        dynamic_emissive_color,
                        ..ModelRenderOptions::default()
                    })
                    .with_hdr_scene_capture(),
                model,
            )
            .expect("capture ColorTable emissive luminance scene")
            .hdr_scene_rgba
            .expect("HDR scene capture was requested")[(256 / 2) * 256 + (256 / 2)]
        };

    let exact_final = capture(
        "native-character-colortable-emissive-luminance-exact",
        &exact,
        ModelDebugMode::Final,
        [1.0; 3],
    );
    let control_final = capture(
        "native-character-colortable-emissive-luminance-control",
        &control,
        ModelDebugMode::Final,
        [1.0; 3],
    );
    assert!(
        exact_final[0] > control_final[0] + 0.25,
        "exact Character Final must scale ColorTable emissive by lit luminance: exact {exact_final:?}, control {control_final:?}"
    );

    let exact_dynamic = capture(
        "native-character-colortable-emissive-dynamic-exact",
        &exact,
        ModelDebugMode::Final,
        [2.0, 1.0, 1.0],
    );
    let control_dynamic = capture(
        "native-character-colortable-emissive-dynamic-control",
        &control,
        ModelDebugMode::Final,
        [2.0, 1.0, 1.0],
    );
    assert!(
        exact_dynamic[0] > exact_final[0] + 0.25,
        "dynamic emissive must multiply exact Character ColorTable emissive: base {exact_final:?}, dynamic {exact_dynamic:?}"
    );
    for lane in 0..3 {
        assert!(
            (control_dynamic[lane] - control_final[lane]).abs() <= 0.001,
            "dynamic emissive must not affect non-exact Final controls: base {control_final:?}, dynamic {control_dynamic:?}"
        );
    }

    let exact_debug = capture(
        "native-character-colortable-emissive-debug-exact",
        &exact,
        ModelDebugMode::Emissive,
        [2.0, 1.0, 1.0],
    );
    let control_debug = capture(
        "native-character-colortable-emissive-debug-control",
        &control,
        ModelDebugMode::Emissive,
        [2.0, 1.0, 1.0],
    );
    for lane in 0..3 {
        assert!(
            (exact_debug[lane] - control_debug[lane]).abs() <= 0.001,
            "raw ColorTable emissive debug must remain unscaled: exact {exact_debug:?}, control {control_debug:?}"
        );
    }
}

fn mock_hdr_emissive_model(emissive_strength: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic hdr emissive",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "fallbackColor": [0.0, 0.0, 0.0],
        "diffuseColor": [0.0, 0.0, 0.0],
        "specularColor": [0.0, 0.0, 0.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "shaderEmissiveColor": [emissive_strength, 0.0, 0.0, 1.0],
        "roughness": 1.0,
        "metalness": 0.0,
        "renderBackfaces": false,
    }))
    .expect("deserialize synthetic HDR emissive material");
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
        item_id: 8,
        item_name: "Synthetic HDR Emissive".to_string(),
        model_main: PackedModelId::from_raw(8),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/hdr_emissive.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![WeaponModelMesh {
            path: "synthetic/hdr_emissive.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic hdr emissive".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_hdr_baked_emissive_model(emissive_strength: f32) -> WeaponModelData {
    let mut model = mock_hdr_emissive_model(0.0);
    model.textures.push(WeaponModelTexture {
        path: "baked://synthetic.mtrl#colorset-emissive".to_string(),
        kind: ModelTextureKind::Emissive,
        texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: [255, 0, 0, 255].repeat(2),
        rgba_f32: Some(vec![[emissive_strength, 0.0, 0.0, 1.0]; 2]),
    });
    model.materials[0].emissive_texture = Some(0);
    model.materials[0].texture_indices = vec![0];
    model
}

fn mock_hdr_baked_diffuse_model(diffuse_strength: f32) -> WeaponModelData {
    let mut model = mock_hdr_emissive_model(0.0);
    model.materials[0].diffuse_color = [1.0; 3];
    model.textures.push(WeaponModelTexture {
        path: "baked://synthetic.mtrl#colorset-diffuse".to_string(),
        kind: ModelTextureKind::BaseColor,
        texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: [255, 0, 0, 255].repeat(2),
        rgba_f32: Some(vec![[diffuse_strength, 0.0, 0.0, 1.0]; 2]),
    });
    model.materials[0].base_color_texture = Some(0);
    model.materials[0].texture_indices = vec![0];
    model
}

fn mock_hdr_baked_specular_model(specular_strength: f32) -> WeaponModelData {
    let mut model = mock_hdr_emissive_model(0.0);
    model.textures.push(WeaponModelTexture {
        path: "baked://synthetic.mtrl#colorset-specular".to_string(),
        kind: ModelTextureKind::Specular,
        texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: [255, 0, 0, 0].repeat(2),
        rgba_f32: Some(vec![[specular_strength, 0.0, 0.0, 0.0]; 2]),
    });
    model.materials[0].specular_texture = Some(0);
    model.materials[0].texture_indices = vec![0];
    model
}

fn mock_hdr_baked_material_properties_model(properties: [f32; 4]) -> WeaponModelData {
    let mut model = mock_hdr_emissive_model(0.0);
    model.textures.push(WeaponModelTexture {
        path: "baked://synthetic.mtrl#colorset-material-properties".to_string(),
        kind: ModelTextureKind::MaterialProperties,
        texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: [255, 128, 255, 255].repeat(2),
        rgba_f32: Some(vec![properties; 2]),
    });
    model.materials[0].material_properties_texture = Some(0);
    model.materials[0].texture_indices = vec![0];
    model
}

fn mock_character_colortable_emissive_luminance_model(
    shader_package_name: &str,
) -> WeaponModelData {
    let mut model = mock_hdr_baked_emissive_model(1.0);
    model.materials[0].shader_package_name = Some(shader_package_name.to_string());
    model.materials[0].color_table_rows = Some(vec![ColorTableRowColors::default()]);
    model.materials[0].diffuse_color = [8.0; 3];
    let properties_index = model.textures.len();
    model.textures.push(WeaponModelTexture {
        path: "baked://synthetic.mtrl#colorset-material-properties".to_string(),
        kind: ModelTextureKind::MaterialProperties,
        texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: [0, 255, 255, 255].repeat(2),
        rgba_f32: Some(vec![[0.0, 1.0, 1.0, 1.0]; 2]),
    });
    model.materials[0].material_properties_texture = Some(properties_index);
    model.materials[0].texture_indices.push(properties_index);
    model
}

fn mock_emissive_source_boundary_model(
    fallback: [f32; 3],
    texture_pixel: Option<[u8; 4]>,
) -> WeaponModelData {
    let mut model = mock_hdr_emissive_model(0.0);
    model.item_name = "Synthetic Emissive Source Boundary".to_string();
    model.materials[0].emissive_color = fallback;
    if let Some(texture_pixel) = texture_pixel {
        model.textures.push(WeaponModelTexture {
            path: "synthetic/emissive_source.tex".to_string(),
            kind: ModelTextureKind::Emissive,
            texel_layout: ModelTextureTexelLayout::Standard,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 0,
            rgba: texture_pixel.to_vec(),
            rgba_f32: None,
        });
        model.materials[0].emissive_texture = Some(0);
        model.materials[0].texture_indices = vec![0];
    }
    model
}

#[test]
#[ignore = "writes synthetic emissive source-boundary snapshots with native wgpu"]
fn render_mock_emissive_source_boundary_snapshots() {
    let texture = Some([192, 80, 24, 255]);
    let textured_neutral = mock_emissive_source_boundary_model([0.0; 3], texture);
    let textured_fallback = mock_emissive_source_boundary_model([4.0, 4.0, 4.0], texture);
    let fallback_off = mock_emissive_source_boundary_model([0.0; 3], None);
    let fallback_on = mock_emissive_source_boundary_model([2.0, 0.1, 0.0], None);
    let render = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render emissive source-boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode emissive source-boundary PNG")
            .to_rgba8()
            .into_raw()
    };
    let textured_neutral = render("native-emissive-texture-neutral", &textured_neutral);
    let textured_fallback = render("native-emissive-texture-fallback", &textured_fallback);
    assert_eq!(
        textured_neutral, textured_fallback,
        "a bound emissive texture must replace the material fallback instead of adding its row summary"
    );

    let fallback_off = render("native-emissive-fallback-off", &fallback_off);
    let fallback_on = render("native-emissive-fallback-on", &fallback_on);
    let rgb_difference: u64 = fallback_off
        .chunks_exact(4)
        .zip(fallback_on.chunks_exact(4))
        .map(|(off, on)| {
            (0..3)
                .map(|channel| off[channel].abs_diff(on[channel]) as u64)
                .sum::<u64>()
        })
        .sum();
    assert!(
        rgb_difference > 100_000,
        "material emissive fallback must remain visible when no emissive texture is bound"
    );
}

#[test]
#[ignore = "writes synthetic bloom threshold fixture snapshots with native wgpu"]
fn render_mock_bloom_threshold_fixtures() {
    let emissive = mock_bloom_fixture_model(0.0, 1.0, 4.0);
    let knee_emissive = mock_bloom_fixture_model(0.0, 1.0, 0.85);
    let dielectric = mock_bloom_fixture_model(0.0, 0.9, 0.0);
    let metallic = mock_bloom_fixture_model(1.0, 0.35, 0.0);
    let render = |name: &str, model: &WeaponModelData, bloom: bool| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    bloom,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render bloom threshold fixture snapshot");
        image::open(snapshot.png_path)
            .expect("decode bloom threshold fixture PNG")
            .to_rgba8()
            .into_raw()
    };
    let emissive_on = render("native-bloom-emissive-on", &emissive, true);
    let emissive_off = render("native-bloom-emissive-off", &emissive, false);
    let knee_on = render("native-bloom-knee-emissive-on", &knee_emissive, true);
    let dielectric_on = render("native-bloom-dielectric-on", &dielectric, true);
    let metallic_on = render("native-bloom-metallic-on", &metallic, true);

    let luma_at = |pixels: &[u8], x: usize, y: usize| {
        let offset = (y * 256 + x) * 4;
        0.2126 * pixels[offset] as f64
            + 0.7152 * pixels[offset + 1] as f64
            + 0.0722 * pixels[offset + 2] as f64
    };
    // Locate the geometric silhouette on the middle row using the bloom-off
    // render, then probe the strip just outside it: only bloom bleed can
    // light those pixels, since the quad does not cover them.
    let edge_x = (8..128)
        .find(|&x| luma_at(&emissive_off, x, 128) > 80.0)
        .expect("bloom-off render must contain the emissive quad");
    let bleed_max = |pixels: &[u8]| {
        let mut max_luma = 0.0f64;
        for y in 96..160 {
            for x in edge_x.saturating_sub(9)..edge_x.saturating_sub(2) {
                max_luma = max_luma.max(luma_at(pixels, x, y));
            }
        }
        max_luma
    };
    let emissive_bleed = bleed_max(&emissive_on);
    let emissive_off_bleed = bleed_max(&emissive_off);
    let knee_bleed = bleed_max(&knee_on);
    let dielectric_bleed = bleed_max(&dielectric_on);
    let metallic_bleed = bleed_max(&metallic_on);
    eprintln!(
        "bloom fixtures: edge x {edge_x}, emissive {emissive_bleed:.1}, \
         emissive off {emissive_off_bleed:.1}, knee {knee_bleed:.1}, \
         dielectric {dielectric_bleed:.1}, metallic {metallic_bleed:.1}"
    );

    assert!(
        emissive_bleed > dielectric_bleed + 20.0,
        "emissive above the scene-linear threshold must bloom past the silhouette \
         (emissive {emissive_bleed:.1} vs dielectric {dielectric_bleed:.1})"
    );
    assert!(
        emissive_bleed > emissive_off_bleed + 20.0,
        "the bleed must come from the bloom pass, not from geometry coverage \
         (on {emissive_bleed:.1} vs off {emissive_off_bleed:.1})"
    );
    assert!(
        knee_bleed < dielectric_bleed + 2.0,
        "emissive below the scene-linear threshold must not bloom \
         (knee {knee_bleed:.1} vs dielectric {dielectric_bleed:.1})"
    );
    assert!(
        (metallic_bleed - dielectric_bleed).abs() <= 10.0,
        "dielectric and metallic surfaces below the threshold must not bloom \
         (dielectric {dielectric_bleed:.1} vs metallic {metallic_bleed:.1})"
    );
}

fn mock_bloom_fixture_model(metalness: f32, roughness: f32, emissive: f32) -> WeaponModelData {
    let diffuse = if emissive > 0.0 { 0.0 } else { 1.0 };
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic bloom fixture",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "fallbackColor": [diffuse, diffuse, diffuse],
        "diffuseColor": [diffuse, diffuse, diffuse],
        "specularColor": [0.2, 0.2, 0.2],
        "emissiveColor": [0.0, 0.0, 0.0],
        "shaderEmissiveColor": [emissive, emissive, emissive, 1.0],
        "roughness": roughness,
        "metalness": metalness,
        "renderBackfaces": false,
    }))
    .expect("deserialize synthetic bloom fixture material");
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
        item_id: 10,
        item_name: "Synthetic Bloom Fixture".to_string(),
        model_main: PackedModelId::from_raw(10),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/bloom_fixture.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![WeaponModelMesh {
            path: "synthetic/bloom_fixture.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic bloom fixture".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

#[test]
#[ignore = "writes synthetic baked diffuse emissive color space snapshots with native wgpu"]
fn render_mock_baked_diffuse_emissive_color_space_snapshots() {
    // Same contract as the baked specular color-space test: baked ColorTable
    // diffuse/emissive ramps store sRGB-encoded RGB; the GPU must sample them
    // back to the source linear values. The debug BaseColor/Emissive views
    // expose the sampled value through the documented compose transform.
    let source_diffuse_bytes = [96u8, 176, 240];
    let source_emissive_bytes = [220u8, 128, 48];
    let model = mock_baked_diffuse_emissive_model(source_diffuse_bytes, source_emissive_bytes);
    let render = |name: &str, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            &model,
        )
        .expect("render baked diffuse/emissive color space snapshot");
        image::open(snapshot.png_path)
            .expect("decode baked diffuse/emissive color space PNG")
            .to_rgba8()
            .into_raw()
    };
    let diffuse_pixels = render(
        "native-baked-diffuse-color-space",
        ModelDebugMode::BaseColor,
    );
    let emissive_pixels = render(
        "native-baked-emissive-color-space",
        ModelDebugMode::Emissive,
    );

    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let srgb_decode = |byte: u8| {
        let value = f32::from(byte) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    for (label, pixels, source_bytes) in [
        ("diffuse", diffuse_pixels, source_diffuse_bytes),
        ("emissive", emissive_pixels, source_emissive_bytes),
    ] {
        let sampled = &pixels[center..center + 3];
        let source_linear = source_bytes.map(srgb_decode);
        let expected = tonemap_pbr_neutral_rgb(source_linear).map(srgb_encode_u8);
        eprintln!(
            "baked {label} color space: source {source_bytes:?}, expected {expected:?}, sampled {sampled:?}"
        );
        for channel in 0..3 {
            let difference = sampled[channel].abs_diff(expected[channel]);
            assert!(
                difference <= 2,
                "{label} ramp must sample back to the source ColorTable value \
                 (channel {channel}: sampled {} vs expected {})",
                sampled[channel],
                expected[channel]
            );
        }
    }
}

fn mock_baked_diffuse_emissive_model(
    diffuse_bytes: [u8; 3],
    emissive_bytes: [u8; 3],
) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic baked diffuse emissive",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "shaderDiffuseColor": [1.0, 1.0, 1.0, 1.0],
        "fallbackColor": [0.7, 0.72, 0.78],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [0.2, 0.2, 0.2],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.5,
        "metalness": 0.0,
        "renderBackfaces": false,
        "textureIndices": [0, 1],
        "baseColorTexture": 0,
        "emissiveTexture": 1,
    }))
    .expect("deserialize synthetic baked diffuse/emissive material");
    let texture = |path: &str, kind, bytes: [u8; 3]| WeaponModelTexture {
        path: path.to_string(),
        kind,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![bytes[0], bytes[1], bytes[2], 255],
        rgba_f32: None,
    };
    let textures = vec![
        texture(
            "baked://synthetic.mtrl#colorset-diffuse",
            ModelTextureKind::BaseColor,
            diffuse_bytes,
        ),
        texture(
            "baked://synthetic.mtrl#colorset-emissive",
            ModelTextureKind::Emissive,
            emissive_bytes,
        ),
    ];
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
        item_id: 11,
        item_name: "Synthetic Baked Diffuse Emissive".to_string(),
        model_main: PackedModelId::from_raw(11),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/baked_diffuse_emissive.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures,
        meshes: vec![WeaponModelMesh {
            path: "synthetic/baked_diffuse_emissive.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic baked diffuse emissive".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

#[test]
#[ignore = "writes synthetic metallic roughness fresnel snapshots with native wgpu"]
fn render_mock_metallic_roughness_fresnel_snapshots() {
    let glossy_metal = mock_metallic_fixture_model(1.0, 0.15);
    let dielectric = mock_metallic_fixture_model(0.0, 0.15);
    let render = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render metallic fixture snapshot");
        image::open(snapshot.png_path)
            .expect("decode metallic fixture PNG")
            .to_rgba8()
            .into_raw()
    };
    let glossy = render("native-metallic-glossy", &glossy_metal);
    let dielectric_pixels = render("native-dielectric-glossy", &dielectric);

    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let glossy_rgb = &glossy[center..center + 3];
    let dielectric_rgb = &dielectric_pixels[center..center + 3];
    eprintln!("metallic fixtures: glossy metal {glossy_rgb:?}, dielectric {dielectric_rgb:?}");

    assert!(
        glossy_rgb[0] > glossy_rgb[1].saturating_mul(2),
        "metallic Fresnel must tint reflections with the base color \
         (red {} vs green {})",
        glossy_rgb[0],
        glossy_rgb[1]
    );
    assert!(
        dielectric_rgb[1] > glossy_rgb[1].saturating_add(12),
        "dielectric specular/environment response must stay white, not base-tinted \
         (dielectric green {} vs metallic green {})",
        dielectric_rgb[1],
        glossy_rgb[1]
    );
}

#[test]
#[ignore = "writes synthetic roughness highlight sweep snapshots with native wgpu"]
fn render_mock_roughness_highlight_sweep_snapshots() {
    // A curved strip whose interpolated normals sweep through the half
    // direction, so the GGX lobe width becomes visible spatially: glossy
    // metal spikes at the N==H spot while rough metal spreads the highlight.
    let glossy_metal = mock_roughness_sweep_model(1.0, 0.15);
    let rough_metal = mock_roughness_sweep_model(1.0, 0.85);
    let render = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render roughness sweep snapshot");
        image::open(snapshot.png_path)
            .expect("decode roughness sweep PNG")
            .to_rgba8()
            .into_raw()
    };
    let glossy = render("native-roughness-sweep-glossy", &glossy_metal);
    let rough = render("native-roughness-sweep-rough", &rough_metal);

    let red_histogram = |pixels: &[u8]| {
        let mut above_spike = 0u32;
        let mut above_spread = 0u32;
        for y in 0..256usize {
            for x in 0..256usize {
                let red = pixels[(y * 256 + x) * 4];
                if red > 240 {
                    above_spike += 1;
                }
                if red > 150 {
                    above_spread += 1;
                }
            }
        }
        (above_spike, above_spread)
    };
    let (glossy_spike, glossy_spread) = red_histogram(&glossy);
    let (rough_spike, rough_spread) = red_histogram(&rough);
    eprintln!(
        "roughness sweep: glossy spike/spread {glossy_spike}/{glossy_spread}, \
         rough spike/spread {rough_spike}/{rough_spread}"
    );

    assert!(
        glossy_spike > 200,
        "glossy metal must spike near the half direction ({glossy_spike} spike pixels)"
    );
    assert!(
        rough_spike == 0,
        "rough metal must spread the highlight below the spike threshold ({rough_spike} spike pixels)"
    );
    assert!(
        rough_spread > glossy_spread * 5 / 4,
        "rough metal must spread reflection energy over more surface \
         (rough {rough_spread} vs glossy {glossy_spread} pixels above mid threshold)"
    );
}

#[test]
#[ignore = "writes synthetic Legacy Gloss roughness-parameterization snapshots with native wgpu"]
fn render_mock_legacy_gloss_roughness_parameterization_snapshots() {
    let modern = mock_legacy_gloss_roughness_model("character.shpk", 100.0);
    let legacy = mock_legacy_gloss_roughness_model("characterlegacy.shpk", 100.0);
    let legacy_low_gloss = mock_legacy_gloss_roughness_model("characterlegacy.shpk", 1.0);
    let mut legacy_high_specular_strength = legacy.clone();
    for pixel in legacy_high_specular_strength.textures[0]
        .rgba_f32
        .as_mut()
        .expect("synthetic material-properties float payload")
    {
        pixel[3] = 100.0;
    }
    let render = |name, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    bloom: false,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render Legacy Gloss roughness parameterization snapshot");
        image::open(snapshot.png_path)
            .expect("decode Legacy Gloss roughness parameterization PNG")
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

    let modern_final = render(
        "native-modern-roughness-control",
        &modern,
        ModelDebugMode::Final,
    );
    let legacy_final = render("native-legacy-gloss-100", &legacy, ModelDebugMode::Final);
    let legacy_low_final = render(
        "native-legacy-gloss-1",
        &legacy_low_gloss,
        ModelDebugMode::Final,
    );
    let legacy_high_specular_strength_final = render(
        "native-legacy-specular-strength-100-boundary",
        &legacy_high_specular_strength,
        ModelDebugMode::Final,
    );
    let modern_legacy_difference = rgb_difference(&modern_final, &legacy_final);
    let legacy_gloss_difference = rgb_difference(&legacy_final, &legacy_low_final);
    eprintln!(
        "Legacy Gloss direct-lobe differences: modern/legacy={modern_legacy_difference}, gloss100/gloss1={legacy_gloss_difference}"
    );
    assert!(
        modern_legacy_difference > 100_000,
        "exact Legacy package must parameterize preview roughness from GlossStrength"
    );
    assert!(
        legacy_gloss_difference > 100_000,
        "Legacy preview roughness must respond to the sampled raw GlossStrength"
    );
    assert_eq!(
        legacy_final, legacy_high_specular_strength_final,
        "raw Legacy SpecularStrength belongs to the unsupported wetness/environment composite, not preview GGX F0"
    );

    let modern_debug = render(
        "native-modern-material-properties-control",
        &modern,
        ModelDebugMode::MaterialProperties,
    );
    let legacy_debug = render(
        "native-legacy-material-properties-raw",
        &legacy,
        ModelDebugMode::MaterialProperties,
    );
    let legacy_high_specular_strength_debug = render(
        "native-legacy-specular-strength-100-raw",
        &legacy_high_specular_strength,
        ModelDebugMode::MaterialProperties,
    );
    assert_eq!(
        modern_debug, legacy_debug,
        "Legacy parameterization must preserve the raw material-properties debug payload"
    );
    assert_ne!(
        legacy_debug, legacy_high_specular_strength_debug,
        "Legacy raw/debug payload must still expose SpecularStrength even though Final does not mis-map it to F0"
    );
}

#[test]
#[ignore = "writes synthetic normal-angle lighting snapshots with native wgpu"]
fn render_mock_normal_angle_lighting_response_snapshots() {
    // At yaw=0 the stable preview key points along this normalized vector.
    // The paired normals preserve NdotV, normal.y, and |normal.x| while only
    // changing their azimuth relative to the key. That keeps the view-fill and
    // vertical ambient inputs equal, making the direct NdotL response visible.
    let light = [-0.4386_f32, 0.6339, 0.6339];
    let away_from_light = [-light[0], light[1], light[2]];
    let view = [0.0_f32, 0.0, 1.0];
    let mut half = [0.0_f32; 3];
    for axis in 0..3 {
        half[axis] = light[axis] + view[axis];
    }
    let half_len = (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt();
    for value in &mut half {
        *value /= half_len;
    }
    let away_from_half = [-half[0], half[1], half[2]];

    let fixture = |metalness: f32, roughness: f32, normal: [f32; 3]| {
        let mut model = mock_metallic_fixture_model(metalness, roughness);
        let material = &mut model.materials[0];
        // Use BG to keep this fixture focused on the shared direct PBR path.
        material.shader_package_name = Some("bg.shpk".to_string());
        material.fallback_color = [0.72; 3];
        material.diffuse_color = [1.0; 3];
        material.specular_color = [1.0; 3];
        for vertex in &mut model.meshes[0].vertices {
            vertex.normal = normal;
        }
        model
    };
    let matte_key = fixture(0.0, 1.0, light);
    let matte_away = fixture(0.0, 1.0, away_from_light);
    let glossy_key = fixture(1.0, 0.12, half);
    let glossy_away = fixture(1.0, 0.12, away_from_half);

    let render_luminance = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render normal-angle lighting snapshot");
        let image = image::open(snapshot.png_path)
            .expect("decode normal-angle lighting PNG")
            .to_rgba8();
        let pixel = image.get_pixel(128, 128).0;
        0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2])
    };
    let matte_key_luma = render_luminance("native-normal-angle-matte-key", &matte_key);
    let matte_away_luma = render_luminance("native-normal-angle-matte-away", &matte_away);
    let glossy_key_luma = render_luminance("native-normal-angle-glossy-key", &glossy_key);
    let glossy_away_luma = render_luminance("native-normal-angle-glossy-away", &glossy_away);
    eprintln!(
        "normal-angle response: matte key/away {matte_key_luma:.1}/{matte_away_luma:.1}, \
         glossy key/away {glossy_key_luma:.1}/{glossy_away_luma:.1}"
    );

    assert!(
        matte_key_luma > matte_away_luma + 12.0,
        "a matte normal aligned with the key must be brighter than its equal-ambient pair \
         ({matte_key_luma:.1} vs {matte_away_luma:.1})"
    );
    assert!(
        glossy_key_luma > glossy_away_luma + 24.0,
        "a glossy normal aligned with the half vector must retain a distinct GGX peak \
         ({glossy_key_luma:.1} vs {glossy_away_luma:.1})"
    );
}

#[test]
#[ignore = "writes synthetic Toon boundary snapshots with native wgpu"]
fn render_mock_toon_override_boundary_snapshot() {
    let default_model = mock_metallic_fixture_model(0.0, 0.45);
    let mut override_model = default_model.clone();
    {
        let material = &mut override_model.materials[0];
        material.toon_index = 3.0;
        material.toon_light_scale = 3.0;
        material.toon_light_spec_aperture = 12.0;
        material.toon_reflection_scale = 5.0;
        material.toon_spec_index = 4.0;
    }
    let render = |name: &str, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic Toon boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic Toon boundary PNG")
            .to_rgba8()
            .into_raw()
    };

    let default_final = render(
        "native-toon-default-final",
        &default_model,
        ModelDebugMode::Final,
    );
    let override_final = render(
        "native-toon-override-final",
        &override_model,
        ModelDebugMode::Final,
    );
    assert_eq!(
        default_final, override_final,
        "non-default Toon inputs must not silently alter Final without a verified MeddleTools formula"
    );

    let default_debug = render(
        "native-toon-default-diagnostic",
        &default_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let override_debug = render(
        "native-toon-override-diagnostic",
        &override_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let default_rgb = &default_debug[center..center + 3];
    let override_rgb = &override_debug[center..center + 3];
    let expected_default = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_override = compose_expected_bytes([0.88, 0.64, 0.22]);
    for channel in 0..3 {
        assert!(default_rgb[channel].abs_diff(expected_default[channel]) <= 3);
        assert!(override_rgb[channel].abs_diff(expected_override[channel]) <= 3);
    }
}

#[test]
#[ignore = "writes synthetic multi-color evidence-boundary snapshots with native wgpu"]
fn render_mock_multi_color_boundary_snapshot() {
    let default_model = mock_metallic_fixture_model(0.0, 0.45);
    let mut override_model = default_model.clone();
    override_model.materials[0].shader_multi_diffuse_color = [0.1, 4.0, 0.2, 1.0];
    override_model.materials[0].shader_multi_emissive_color = [3.0, 0.2, 2.0, 1.0];
    let render = |name: &str, model: &WeaponModelData, debug_mode| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0])
                .with_render_options(ModelRenderOptions {
                    debug_mode,
                    ..ModelRenderOptions::default()
                }),
            model,
        )
        .expect("render synthetic multi-color boundary snapshot");
        image::open(snapshot.png_path)
            .expect("decode synthetic multi-color boundary PNG")
            .to_rgba8()
            .into_raw()
    };

    assert_eq!(
        render(
            "native-multi-color-default-final",
            &default_model,
            ModelDebugMode::Final,
        ),
        render(
            "native-multi-color-override-final",
            &override_model,
            ModelDebugMode::Final,
        ),
        "unverified generic multi colors must not silently alter Final"
    );

    let default_debug = render(
        "native-multi-color-default-diagnostic",
        &default_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let override_debug = render(
        "native-multi-color-override-diagnostic",
        &override_model,
        ModelDebugMode::UnsupportedInputs,
    );
    let center = ((256 / 2) * 256 + (256 / 2)) * 4;
    let expected_default = compose_expected_bytes([0.05, 0.22, 0.1]);
    let expected_override = compose_expected_bytes([0.92, 0.36, 0.16]);
    for channel in 0..3 {
        assert!(default_debug[center + channel].abs_diff(expected_default[channel]) <= 3);
        assert!(override_debug[center + channel].abs_diff(expected_override[channel]) <= 3);
    }
}

#[test]
#[ignore = "writes synthetic anisotropic GGX snapshots with native wgpu"]
fn render_mock_anisotropic_specular_direction_snapshots() {
    let render_luminance = |name: &str, model: &WeaponModelData| {
        let snapshot = render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name)
                .with_viewport(256, 256)
                .with_camera(0.0, 0.0, 3.2, [0.0, 0.0]),
            model,
        )
        .expect("render anisotropic GGX snapshot");
        let image = image::open(snapshot.png_path)
            .expect("decode anisotropic GGX PNG")
            .to_rgba8();
        let pixel = image.get_pixel(128, 128).0;
        0.2126 * f64::from(pixel[0]) + 0.7152 * f64::from(pixel[1]) + 0.0722 * f64::from(pixel[2])
    };

    // At the center fragment H projects to approximately (-0.57, 0.82) in
    // the XY tangent plane. These source bitangents make the derived tangent
    // align with that projection, then rotate it by 90 degrees.
    let along_half_projection = [-0.821, -0.570, 0.0, 1.0];
    let across_half_projection = [0.570, -0.821, 0.0, 1.0];
    let isotropic_tangent_x = mock_anisotropy_model(0, along_half_projection);
    let isotropic_tangent_y = mock_anisotropy_model(0, across_half_projection);
    let anisotropic_tangent_x = mock_anisotropy_model(255, along_half_projection);
    let anisotropic_tangent_y = mock_anisotropy_model(255, across_half_projection);

    let isotropic_x = render_luminance("native-anisotropy-zero-tangent-x", &isotropic_tangent_x);
    let isotropic_y = render_luminance("native-anisotropy-zero-tangent-y", &isotropic_tangent_y);
    let anisotropic_x = render_luminance("native-anisotropy-one-tangent-x", &anisotropic_tangent_x);
    let anisotropic_y = render_luminance("native-anisotropy-one-tangent-y", &anisotropic_tangent_y);
    eprintln!(
        "anisotropic response: isotropic tangent x/y {isotropic_x:.1}/{isotropic_y:.1}, \
         anisotropic tangent x/y {anisotropic_x:.1}/{anisotropic_y:.1}"
    );

    assert!(
        (isotropic_x - isotropic_y).abs() <= 2.0,
        "zero anisotropy must be invariant under tangent rotation \
         ({isotropic_x:.1} vs {isotropic_y:.1})"
    );
    assert!(
        (anisotropic_x - anisotropic_y).abs() >= 12.0,
        "nonzero ColorTable anisotropy must produce a directional GGX response \
         ({anisotropic_x:.1} vs {anisotropic_y:.1})"
    );
}

fn mock_anisotropy_model(alpha: u8, bitangent: [f32; 4]) -> WeaponModelData {
    let mut model = mock_metallic_fixture_model(1.0, 0.28);
    let material = &mut model.materials[0];
    material.shader_package_name = Some("character.shpk".to_string());
    material.fallback_color = [0.72; 3];
    material.diffuse_color = [1.0; 3];
    material.specular_color = [1.0; 3];
    material.texture_indices = vec![0];
    material.specular_texture = Some(0);
    model.textures = vec![WeaponModelTexture {
        path: "baked://synthetic-anisotropy.mtrl#colorset-specular".to_string(),
        kind: ModelTextureKind::Specular,
        texel_layout: ModelTextureTexelLayout::Standard,
        width: 1,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: vec![255, 255, 255, alpha],
        rgba_f32: None,
    }];
    for vertex in &mut model.meshes[0].vertices {
        vertex.bitangent = bitangent;
    }
    model
}

#[test]
#[ignore = "writes a synthetic perspective view-vector snapshot with native wgpu"]
fn render_mock_perspective_view_vector_snapshot() {
    let model = mock_perspective_view_vector_model();
    let snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("native-perspective-view-vector")
            .with_viewport(512, 512)
            .with_camera(0.0, 0.0, 2.2, [0.0, 0.0])
            .with_render_options(ModelRenderOptions {
                debug_mode: ModelDebugMode::ViewDirection,
                ..ModelRenderOptions::default()
            }),
        &model,
    )
    .expect("render perspective view-vector snapshot");
    let pixels = image::open(snapshot.png_path)
        .expect("decode perspective view-vector PNG")
        .to_rgba8()
        .into_raw();

    let rgb_at = |x: usize, y: usize| -> [u8; 3] {
        let offset = (y * 512 + x) * 4;
        [pixels[offset], pixels[offset + 1], pixels[offset + 2]]
    };
    let distance = |left: [u8; 3], right: [u8; 3]| -> u32 {
        (0..3)
            .map(|channel| left[channel].abs_diff(right[channel]) as u32)
            .sum()
    };
    let background = rgb_at(0, 256);
    let covered = (0..512)
        .filter(|x| distance(rgb_at(*x, 256), background) > 24)
        .collect::<Vec<_>>();
    let left = *covered.first().expect("perspective fixture left edge");
    let right = *covered.last().expect("perspective fixture right edge");
    assert!(
        right - left > 200,
        "perspective fixture must cover the center row"
    );

    let left_rgb = rgb_at(left + 16, 256);
    let right_rgb = rgb_at(right - 16, 256);
    let center_rgb = rgb_at((left + right) / 2, 256);
    eprintln!(
        "perspective view vector: covered {left}..{right}, left {left_rgb:?}, \
         center {center_rgb:?}, right {right_rgb:?}"
    );
    assert!(
        left_rgb[0] > right_rgb[0].saturating_add(45),
        "left/right fragments must encode opposite horizontal directions to the camera \
         ({left_rgb:?} vs {right_rgb:?})"
    );
    assert!(
        center_rgb[2] > 220 && center_rgb[0].abs_diff(center_rgb[1]) < 4,
        "the center fragment must point almost directly toward the +Z camera ({center_rgb:?})"
    );
}

fn mock_perspective_view_vector_model() -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic perspective view vector",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "fallbackColor": [0.8, 0.08, 0.03],
        "diffuseColor": [1.0, 1.0, 1.0],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": 0.12,
        "metalness": 1.0,
        "renderBackfaces": false
    }))
    .expect("deserialize perspective view-vector material");
    let positions = [
        [-1.0, -0.65, 0.0],
        [1.0, -0.65, 0.0],
        [1.0, 0.65, 0.0],
        [-1.0, 0.65, 0.0],
    ];
    let vertices = positions
        .into_iter()
        .map(|position| vertex(position, [1.0; 4]))
        .collect();

    WeaponModelData {
        item_id: 12,
        item_name: "Synthetic Perspective View Vector".to_string(),
        model_main: PackedModelId::from_raw(12),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/perspective_view_vector.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-1.0, -0.65, 0.0],
            max: [1.0, 0.65, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![WeaponModelMesh {
            path: "synthetic/perspective_view_vector.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic perspective view vector".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}

fn mock_roughness_sweep_model(metalness: f32, roughness: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic roughness sweep",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "fallbackColor": [1.0, 0.05, 0.05],
        "diffuseColor": [1.0, 0.05, 0.05],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": roughness,
        "metalness": metalness,
        "renderBackfaces": true,
    }))
    .expect("deserialize synthetic roughness sweep material");

    // Matches the preview rig at yaw=0: light = (-right*0.45 + up*0.65 +
    // view*0.65) with view=(0,0,1), so the half direction is normalize(L+V).
    let light = [-0.4386_f32, 0.6339, 0.6339];
    let view = [0.0_f32, 0.0, 1.0];
    let mut half = [0.0_f32; 3];
    for axis in 0..3 {
        half[axis] = light[axis] + view[axis];
    }
    let half_len = (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt();
    for axis in 0..3 {
        half[axis] /= half_len;
    }

    // Strip normals sweep the great circle from view through the half vector.
    let segments = 12usize;
    let radius = 0.55f32;
    let strip_axis = {
        let cross = [
            view[1] * half[2] - view[2] * half[1],
            view[2] * half[0] - view[0] * half[2],
            view[0] * half[1] - view[1] * half[0],
        ];
        let len = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
        [cross[0] / len, cross[1] / len, cross[2] / len]
    };
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for segment in 0..=segments {
        let t = -0.3 + 1.6 * segment as f32 / segments as f32;
        let mut normal = [0.0_f32; 3];
        for axis in 0..3 {
            normal[axis] = view[axis] + t * (half[axis] - view[axis]);
        }
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        for axis in 0..3 {
            normal[axis] /= len;
        }
        for side in [-1.0_f32, 1.0_f32] {
            let mut position = [0.0_f32; 3];
            for axis in 0..3 {
                position[axis] = normal[axis] * radius + strip_axis[axis] * side * 0.8;
            }
            let mut vertex = vertex(position, [1.0; 4]);
            vertex.normal = normal;
            vertices.push(vertex);
        }
        if segment > 0 {
            let base = (segment * 2) as u32;
            indices.extend_from_slice(&[base - 2, base - 1, base, base - 1, base + 1, base]);
        }
    }

    WeaponModelData {
        item_id: 13,
        item_name: "Synthetic Roughness Sweep".to_string(),
        model_main: PackedModelId::from_raw(13),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/roughness_sweep.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-1.0, -1.0, -0.2],
            max: [1.0, 1.0, 1.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![WeaponModelMesh {
            path: "synthetic/roughness_sweep.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic roughness sweep".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices,
        }],
    }
}

fn mock_legacy_gloss_roughness_model(
    shader_package_name: &str,
    gloss_strength: f32,
) -> WeaponModelData {
    let mut model = mock_roughness_sweep_model(1.0, 0.85);
    model.item_name = format!("Synthetic {shader_package_name} Gloss {gloss_strength}");
    model.materials[0].shader_package_name = Some(shader_package_name.to_string());
    model.textures.push(WeaponModelTexture {
        path: "baked://synthetic-legacy.mtrl#colorset-material-properties".to_string(),
        kind: ModelTextureKind::MaterialProperties,
        texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
        width: 2,
        height: 1,
        array_size: 1,
        array_layer_height: 0,
        rgba: [255, 217, 255, 255].repeat(2),
        rgba_f32: Some(vec![[1.0, 0.85, gloss_strength, 1.0]; 2]),
    });
    model.materials[0].material_properties_texture = Some(0);
    model.materials[0].texture_indices = vec![0];
    model
}

fn mock_metallic_fixture_model(metalness: f32, roughness: f32) -> WeaponModelData {
    let material: WeaponModelMaterial = serde_json::from_value(serde_json::json!({
        "slot": 0,
        "materialIndex": 0,
        "name": "synthetic metallic fixture",
        "path": null,
        "shaderPackageName": "character.shpk",
        "alphaMode": "opaque",
        "fallbackColor": [1.0, 0.05, 0.05],
        "diffuseColor": [1.0, 0.05, 0.05],
        "specularColor": [1.0, 1.0, 1.0],
        "emissiveColor": [0.0, 0.0, 0.0],
        "roughness": roughness,
        "metalness": metalness,
        "renderBackfaces": false,
    }))
    .expect("deserialize synthetic metallic fixture material");
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
        item_id: 12,
        item_name: "Synthetic Metallic Fixture".to_string(),
        model_main: PackedModelId::from_raw(12),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics: Vec::new(),
        loaded_paths: vec!["synthetic/metallic_fixture.mdl".to_string()],
        bounds: WeaponModelBounds {
            min: [-0.8, -0.8, 0.0],
            max: [0.8, 0.8, 0.0],
            center: [0.0, 0.0, 0.0],
            radius: 1.2,
        },
        materials: vec![material],
        textures: Vec::new(),
        meshes: vec![WeaponModelMesh {
            path: "synthetic/metallic_fixture.mdl".to_string(),
            part_index: 0,
            mesh_category: Some("normal".to_string()),
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: "synthetic metallic fixture".to_string(),
            color: [1.0; 3],
            bone_table: None,
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
    }
}
