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
