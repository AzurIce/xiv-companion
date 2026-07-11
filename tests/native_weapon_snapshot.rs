#![cfg(feature = "render-test-support")]

use xiv_companion_render::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion_render::{
    ModelTextureKind, PackedModelId, WeaponModelBounds, WeaponModelData, WeaponModelMaterial,
    WeaponModelMesh, WeaponModelTexture, WeaponModelVertex,
};

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
#[ignore = "writes synthetic bguvscroll Map1 snapshots with native wgpu"]
fn render_mock_secondary_scroll_map_snapshot() {
    let primary = mock_secondary_scroll_model(0.0);
    let secondary = mock_secondary_scroll_model(1.0);
    let render = |name, model| {
        render_weapon_model_snapshot_with_options(
            WeaponModelSnapshotOptions::new(name).with_viewport(512, 512),
            model,
        )
        .expect("render synthetic secondary scroll map snapshot")
    };

    let primary_snapshot = render("native-bguvscroll-primary", &primary);
    let secondary_snapshot = render("native-bguvscroll-secondary", &secondary);
    let pixels = |path| {
        image::open(path)
            .expect("decode synthetic bguvscroll PNG")
            .to_rgba8()
            .into_raw()
    };

    let primary_pixels = pixels(primary_snapshot.png_path);
    let secondary_pixels = pixels(secondary_snapshot.png_path);
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
}

fn mock_secondary_scroll_model(vertex_alpha: f32) -> WeaponModelData {
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
