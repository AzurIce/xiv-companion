#![cfg(feature = "render-test-support")]

use xiv_companion_render::test_support::{
    WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
};
use xiv_companion_render::{
    PackedModelId, WeaponModelBounds, WeaponModelData, WeaponModelMesh, WeaponModelVertex,
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
