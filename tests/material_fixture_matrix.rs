use std::collections::BTreeSet;

use serde::Deserialize;
use xiv_companion_data::{
    ColorTableRowColors, MaterialShaderFamily, ModelBounds, ModelData, ModelMaterial, ModelMesh,
    PreparedAlphaSource, PreparedRenderPass, bake_color_table_maps, prepare_model_for_render,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMatrix {
    version: u32,
    required_coverage: Vec<String>,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureCase {
    id: String,
    coverage: Vec<String>,
    row_count: usize,
    pair_index: usize,
    shader_package_name: String,
    alpha_mode: String,
    value_mode: String,
    transparency: f32,
    render_backfaces: bool,
    stain_ids: [u8; 2],
    row: ColorTableRowColors,
    expected: ExpectedFixture,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedFixture {
    diffuse_rgba: [u8; 4],
    specular_rgba: [u8; 4],
    material_rgba: [u8; 4],
    tile_rgba: [u8; 4],
    emissive_rgba: Option<[u8; 4]>,
    render_pass: String,
    alpha_source: String,
    shader_family: String,
    render_backfaces: bool,
}

#[test]
fn checked_in_material_fixture_matrix_matches_bake_and_prepared_semantics() {
    let matrix: FixtureMatrix =
        serde_json::from_str(include_str!("fixtures/material_fixture_matrix.json"))
            .expect("parse checked-in material fixture matrix");
    assert_eq!(matrix.version, 1);
    assert!(!matrix.cases.is_empty());

    let actual_coverage = matrix
        .cases
        .iter()
        .flat_map(|case| case.coverage.iter().cloned())
        .collect::<BTreeSet<_>>();
    for required in &matrix.required_coverage {
        assert!(
            actual_coverage.contains(required),
            "material fixture matrix is missing required coverage: {required}"
        );
    }

    let mut ids = BTreeSet::new();
    for case in &matrix.cases {
        assert!(
            ids.insert(case.id.as_str()),
            "duplicate fixture id: {}",
            case.id
        );
        assert!(
            matches!(case.row_count, 16 | 32),
            "{} has invalid ColorTable row count",
            case.id
        );
        assert!(
            case.pair_index < case.row_count / 2,
            "{} has invalid row-pair index",
            case.id
        );

        let mut rows = vec![ColorTableRowColors::default(); case.row_count];
        rows[case.pair_index * 2] = case.row;
        rows[case.pair_index * 2 + 1] = case.row;
        let pair_count = case.row_count / 2;
        let id_r = ((case.pair_index as f32 / (pair_count - 1) as f32) * 255.0).round() as u8;
        let baked = bake_color_table_maps(&rows, &[id_r, 0, 0, 255])
            .unwrap_or_else(|| panic!("{} must produce ColorTable maps", case.id));

        assert_eq!(
            baked.diffuse_rgba, case.expected.diffuse_rgba,
            "{} diffuse",
            case.id
        );
        assert_eq!(
            baked.specular_rgba, case.expected.specular_rgba,
            "{} specular",
            case.id
        );
        assert_eq!(
            baked.material_rgba, case.expected.material_rgba,
            "{} material",
            case.id
        );
        assert_eq!(
            baked.tile_properties_rgba, case.expected.tile_rgba,
            "{} tile",
            case.id
        );
        assert_eq!(
            baked.emissive_rgba.as_deref(),
            case.expected
                .emissive_rgba
                .as_ref()
                .map(|rgba| rgba.as_slice()),
            "{} emissive",
            case.id
        );
        assert_eq!(baked.sheen_properties_rgba_f32[0][0], case.row.sheen_rate);
        assert_eq!(baked.sheen_properties_rgba_f32[0][1], case.row.sheen_tint);
        assert_eq!(
            baked.sheen_properties_rgba_f32[0][2],
            case.row.sheen_aperture
        );
        assert_eq!(
            baked.sphere_properties_rgba_f32[0][0],
            case.row.sphere_index / 255.0
        );
        assert_eq!(baked.sphere_properties_rgba_f32[0][1], case.row.sphere_mask);
        assert_eq!(baked.tile_matrix_rgba_f32[0], case.row.tile_matrix);

        if case.coverage.iter().any(|tag| tag == "dyed") {
            assert_ne!(
                case.stain_ids,
                [0, 0],
                "{} dyed fixture needs a stain",
                case.id
            );
        }
        if case.coverage.iter().any(|tag| tag == "metallic") {
            assert!(
                case.row.metalness >= 0.9,
                "{} metallic probe is too weak",
                case.id
            );
        }
        if case.coverage.iter().any(|tag| tag == "emissive") {
            assert!(case.row.emissive.iter().any(|value| *value > 0.0));
        }

        let material: ModelMaterial = serde_json::from_value(serde_json::json!({
            "slot": 0,
            "materialIndex": 0,
            "name": case.id,
            "path": null,
            "shaderPackageName": case.shader_package_name,
            "alphaMode": case.alpha_mode,
            "valueMode": case.value_mode,
            "transparency": case.transparency,
            "renderBackfaces": case.render_backfaces,
            "fallbackColor": case.row.diffuse,
            "diffuseColor": case.row.diffuse,
            "specularColor": case.row.specular,
            "emissiveColor": case.row.emissive,
            "roughness": case.row.roughness,
            "metalness": case.row.metalness
        }))
        .unwrap_or_else(|error| panic!("{} material must deserialize: {error}", case.id));
        let prepared = prepare_model_for_render(&ModelData {
            bounds: ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: vec![ModelMesh {
                path: format!("fixture://{}.mdl", case.id),
                part_index: 0,
                mesh_category: Some("normal".to_string()),
                submesh: None,
                shape_influences: Vec::new(),
                shape_targets: Vec::new(),
                material_index: 0,
                material_slot: 0,
                material_name: case.id.clone(),
                color: [1.0; 3],
                bone_table: None,
                vertices: Vec::new(),
                indices: Vec::new(),
            }],
        })
        .meshes[0]
            .prepared_material;

        assert_eq!(
            prepared.render_pass,
            expected_render_pass(&case.expected.render_pass),
            "{} pass",
            case.id
        );
        assert_eq!(
            prepared.alpha_policy.source,
            expected_alpha_source(&case.expected.alpha_source),
            "{} alpha",
            case.id
        );
        assert_eq!(
            prepared.shader_family,
            expected_shader_family(&case.expected.shader_family),
            "{} family",
            case.id
        );
        assert_eq!(
            prepared.render_backfaces, case.expected.render_backfaces,
            "{} backfaces",
            case.id
        );
    }
}

fn expected_render_pass(value: &str) -> PreparedRenderPass {
    match value {
        "opaque" => PreparedRenderPass::Opaque,
        "cutout" => PreparedRenderPass::Cutout,
        "transparent" => PreparedRenderPass::Transparent,
        "glass" => PreparedRenderPass::Glass,
        "additiveLightShaft" => PreparedRenderPass::AdditiveLightShaft,
        other => panic!("unknown expected render pass: {other}"),
    }
}

fn expected_alpha_source(value: &str) -> PreparedAlphaSource {
    match value {
        "opaque" => PreparedAlphaSource::Opaque,
        "baseColorAlpha" => PreparedAlphaSource::BaseColorAlpha,
        "normalBlue" => PreparedAlphaSource::NormalBlue,
        "materialTransparency" => PreparedAlphaSource::MaterialTransparency,
        "normalAlpha" => PreparedAlphaSource::NormalAlpha,
        other => panic!("unknown expected alpha source: {other}"),
    }
}

fn expected_shader_family(value: &str) -> MaterialShaderFamily {
    match value {
        "character" => MaterialShaderFamily::Character,
        "skin" => MaterialShaderFamily::Skin,
        "characterGlass" => MaterialShaderFamily::CharacterGlass,
        "water" => MaterialShaderFamily::Water,
        other => panic!("unknown expected shader family: {other}"),
    }
}
