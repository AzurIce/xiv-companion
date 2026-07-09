#![cfg(all(feature = "game-data", feature = "render-test-support"))]

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use physis::resource::{Resource, SqPackResource};
use serde::{Deserialize, Serialize};
use xiv_companion::{
    MaterialSemanticSummaryDebug, ModelBounds, ModelMaterial, ModelMesh, ModelShapeInfo,
    ModelSubmeshInfo, ModelTexture, PreparedMaterial, PreparedMesh, PreparedMeshShapeInfluences,
    WeaponCatalogItem, WeaponModelData,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    load_weapon_model_from_resource, material_debug_info_from_mtrl_bytes,
    material_debug_info_from_resource, mdl_metadata_from_mdl_bytes, prepare_model_for_render,
    renderer::test_support::{
        WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
    },
};
use xiv_companion_data::{MdlMeshMetadata, MdlMetadata, ModelBoneTable};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    cases: Vec<PhantomWeaponCase>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhantomWeaponCase {
    case_id: String,
    priority: String,
    item_id: u32,
    name: String,
    #[serde(default)]
    focus: Vec<String>,
}

#[derive(Debug)]
struct CaseArtifacts {
    case_dir: PathBuf,
    snapshot_path: PathBuf,
    summary_path: PathBuf,
    raw_manifest_path: PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelDebugSummary {
    case_id: String,
    priority: String,
    item_id: u32,
    name: String,
    focus: Vec<String>,
    snapshot: String,
    model_main: xiv_companion::PackedModelId,
    model_sub: Option<xiv_companion::PackedModelId>,
    load_diagnostics: Vec<xiv_companion::WeaponModelLoadDiagnostic>,
    bounds: ModelBounds,
    loaded_paths: Vec<String>,
    mesh_count: usize,
    material_count: usize,
    texture_count: usize,
    meshes: Vec<MeshSummary>,
    model_debug_files: Vec<ModelDebugFileSummary>,
    materials: Vec<MaterialSummary>,
    material_debug_files: Vec<MaterialDebugFileSummary>,
    textures: Vec<TextureSummary>,
    raw_files: Vec<RawFileSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshSummary {
    mesh_file: String,
    path: String,
    part_index: u32,
    mesh_category: Option<String>,
    draw_role: xiv_companion::ModelMeshDrawRole,
    rendered_in_main_pass: bool,
    prepared_material: Option<PreparedMaterial>,
    prepared_submesh: Option<ModelSubmeshInfo>,
    prepared_shape_influences: Vec<ModelShapeInfo>,
    prepared_shape_influence_state: PreparedMeshShapeInfluences,
    metadata_file: Option<String>,
    submesh_index: Option<usize>,
    submeshes: Vec<MeshSubmeshSummary>,
    bone_table: Option<ModelBoneTable>,
    shapes: Vec<MeshShapeSummary>,
    material_index: u16,
    material_slot: usize,
    material_name: String,
    vertex_count: usize,
    index_count: usize,
    bounds: ModelBounds,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshSubmeshSummary {
    table_index: usize,
    index_offset: u32,
    relative_index_offset: Option<i64>,
    index_count: u32,
    attribute_index_mask_hex: String,
    attribute_names: Vec<String>,
    bone_start_index: u16,
    bone_count: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshShapeSummary {
    shape_index: usize,
    name: Option<String>,
    shape_mesh_index: usize,
    shape_value_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialSummary {
    slot: usize,
    material_index: u16,
    name: String,
    path: Option<String>,
    shader_package_name: Option<String>,
    render_mode: String,
    alpha_mode: String,
    alpha_threshold: f32,
    transparency: f32,
    alpha_aperture: f32,
    alpha_offset: f32,
    shadow_alpha_threshold: f32,
    glass_ior: f32,
    glass_thickness_max: f32,
    normal_scale: f32,
    multi_normal_scale: f32,
    detail_normal_scale: f32,
    multi_detail_normal_scale: f32,
    tile_index: f32,
    tile_alpha: f32,
    tile_scale: [f32; 2],
    toon_index: f32,
    toon_light_scale: f32,
    sheen_rate: f32,
    sheen_tint_rate: f32,
    sheen_aperture: f32,
    sphere_map_index: f32,
    detail_id: f32,
    multi_detail_id: f32,
    detail_color: [f32; 4],
    multi_detail_color: [f32; 4],
    shader_diffuse_color: [f32; 4],
    shader_multi_diffuse_color: [f32; 4],
    shader_emissive_color: [f32; 4],
    shader_multi_emissive_color: [f32; 4],
    outline_color: [f32; 4],
    outline_width: f32,
    specular_color_mask: [f32; 4],
    ssao_mask: f32,
    texture_mip_bias: f32,
    shadow_pos_offset: f32,
    detail_color_uv_scale: [f32; 4],
    detail_normal_uv_scale: [f32; 4],
    uv_scroll: [f32; 4],
    lightshaft_color: [f32; 4],
    lightshaft_tex_anim: [f32; 4],
    lightshaft_tex_u: [f32; 4],
    lightshaft_tex_v: [f32; 4],
    lightshaft_ray: [f32; 4],
    opacity: f32,
    render_backfaces: bool,
    apply_vertex_color: bool,
    fallback_color: [f32; 3],
    diffuse_color: [f32; 3],
    specular_color: [f32; 3],
    emissive_color: [f32; 3],
    roughness: f32,
    metalness: f32,
    texture_indices: Vec<usize>,
    base_color_texture: Option<usize>,
    normal_texture: Option<usize>,
    mask_texture: Option<usize>,
    material_map_texture: Option<usize>,
    multi_map_texture: Option<usize>,
    specular_texture: Option<usize>,
    emissive_texture: Option<usize>,
    material_properties_texture: Option<usize>,
    tile_properties_texture: Option<usize>,
    sheen_properties_texture: Option<usize>,
    sphere_properties_texture: Option<usize>,
    tile_matrix_texture: Option<usize>,
    debug_file: Option<String>,
    semantic_summary: Option<MaterialSemanticSummaryDebug>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialDebugFileSummary {
    slot: usize,
    material_index: u16,
    material_name: String,
    resource_path: Option<String>,
    debug_file: Option<String>,
    semantic_summary: Option<MaterialSemanticSummaryDebug>,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelDebugFileSummary {
    resource_path: String,
    debug_file: Option<String>,
    mesh_count: Option<usize>,
    submesh_count: Option<usize>,
    material_count: Option<usize>,
    attribute_count: Option<usize>,
    bone_table_count: Option<usize>,
    shape_count: Option<usize>,
    has_extra_lods: Option<bool>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextureSummary {
    index: usize,
    path: String,
    kind: String,
    width: u16,
    height: u16,
    rgba_bytes: usize,
    decoded_png: Option<String>,
    pixel_stats: Option<TexturePixelStats>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TexturePixelStats {
    pixel_count: usize,
    alpha_min: u8,
    alpha_max: u8,
    alpha_average: f32,
    transparent_pixels: usize,
    translucent_pixels: usize,
    opaque_pixels: usize,
    average_rgb: [f32; 3],
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawFileSummary {
    resource_path: String,
    dump_path: Option<String>,
    bytes: Option<usize>,
    kind: String,
    error: Option<String>,
}

#[test]
#[ignore = "renders all xx之幻梦 weapons from a local game SqPack into target/phantom-weapon-snapshots"]
fn render_phantom_weapon_snapshots() -> Result<()> {
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/phantom_weapons.json"))
        .context("failed to parse phantom weapon fixture")?;
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let output_dir = PathBuf::from("target").join("phantom-weapon-snapshots");
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;

    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir_text),
        game_dir.display().to_string(),
        game_version(&game_dir),
        "local-render-test".to_string(),
    )
    .context("failed to export weapon catalog from local SqPack")?;
    let catalog_by_id = catalog
        .items
        .into_iter()
        .map(|item| (item.id, item))
        .collect::<HashMap<_, _>>();

    let mut resource = SqPackResource::from_existing(game_dir_text);
    let mut manifest = Vec::new();
    let mut failures = Vec::new();
    let case_filter = phantom_case_filter();

    manifest.push("# Phantom weapon snapshots".to_string());
    manifest.push(String::new());
    manifest.push(format!("- gameDir: {}", game_dir.display()));
    manifest.push(format!("- outputDir: {}", output_dir.display()));
    if let Some(filter) = &case_filter {
        manifest.push(format!(
            "- filter: {}",
            filter.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    manifest.push(String::new());
    manifest.push("| priority | item | focus | snapshot | summary | raw |".to_string());
    manifest.push("| --- | --- | --- | --- | --- | --- |".to_string());

    for case in &fixture.cases {
        if !phantom_case_matches_filter(case, case_filter.as_ref()) {
            continue;
        }

        let Some(item) = catalog_by_id.get(&case.item_id) else {
            failures.push(format!(
                "{} {}: item not found in catalog",
                case.item_id, case.name
            ));
            continue;
        };

        match render_case(case, item, &mut resource, &output_dir) {
            Ok(artifacts) => {
                eprintln!(
                    "rendered {} {} -> {}",
                    case.item_id,
                    case.name,
                    artifacts.snapshot_path.display()
                );
                let case_dir = artifacts
                    .case_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(".");
                manifest.push(format!(
                    "| {} | {} {} | {} | [png]({}) | [json]({}) | [raw]({}) |",
                    case.priority,
                    case.item_id,
                    case.name,
                    case.focus.join(", "),
                    markdown_path(case_dir, &artifacts.snapshot_path),
                    markdown_path(case_dir, &artifacts.summary_path),
                    markdown_path(case_dir, &artifacts.raw_manifest_path)
                ));
            }
            Err(error) => {
                let message = format!("{} {}: {error:#}", case.item_id, case.name);
                eprintln!("failed {message}");
                failures.push(message);
            }
        }
    }

    let manifest_path = output_dir.join("index.md");
    fs::write(&manifest_path, manifest.join("\n"))
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;
    eprintln!("index: {}", manifest_path.display());

    if !failures.is_empty() {
        anyhow::bail!(
            "failed to render {} phantom weapon snapshots:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    Ok(())
}

fn render_case(
    case: &PhantomWeaponCase,
    item: &WeaponCatalogItem,
    resource: &mut SqPackResource,
    output_dir: &Path,
) -> Result<CaseArtifacts> {
    let case_dir = output_dir.join(snapshot_name(case));
    fs::create_dir_all(&case_dir)
        .with_context(|| format!("failed to create {}", case_dir.display()))?;

    let model = load_weapon_model_from_resource(resource, item)
        .with_context(|| format!("failed to load model for {}", case.case_id))?;
    let snapshot = render_weapon_model_snapshot_with_options(
        WeaponModelSnapshotOptions::new("snapshot")
            .with_output_dir(&case_dir)
            .with_viewport(1024, 1024)
            .with_camera(0.65, 0.35, 3.2, [0.0, 0.0]),
        &model,
    )
    .with_context(|| format!("failed to render snapshot for {}", case.case_id))?;

    let raw_files = dump_raw_files(resource, &model, &case_dir)?;
    let (model_debug_files, model_metadata_by_path) =
        dump_model_debug(resource, &model, &case_dir)?;
    let material_debug_files = dump_material_debug(resource, &model, &case_dir)?;
    let material_debug_by_slot = material_debug_files
        .iter()
        .cloned()
        .map(|file| (file.slot, file))
        .collect::<HashMap<_, _>>();
    let texture_summaries = dump_decoded_textures(&model, &case_dir)?;
    let mesh_summaries = dump_meshes(&model, &case_dir, &model_metadata_by_path)?;
    let material_summaries = model
        .materials
        .iter()
        .map(|material| material_summary(material, &material_debug_by_slot))
        .collect();
    let summary_path = case_dir.join("model-summary.json");
    let raw_manifest_path = case_dir.join("raw-manifest.json");

    let summary = ModelDebugSummary {
        case_id: case.case_id.clone(),
        priority: case.priority.clone(),
        item_id: case.item_id,
        name: case.name.clone(),
        focus: case.focus.clone(),
        snapshot: path_relative_to_case(&snapshot.png_path, &case_dir),
        model_main: model.model_main,
        model_sub: model.model_sub,
        load_diagnostics: model.load_diagnostics.clone(),
        bounds: model.bounds,
        loaded_paths: model.loaded_paths.clone(),
        mesh_count: model.meshes.len(),
        material_count: model.materials.len(),
        texture_count: model.textures.len(),
        meshes: mesh_summaries,
        model_debug_files,
        materials: material_summaries,
        material_debug_files,
        textures: texture_summaries,
        raw_files: raw_files.clone(),
    };

    write_json(&summary_path, &summary)?;
    write_json(&raw_manifest_path, &raw_files)?;

    Ok(CaseArtifacts {
        case_dir,
        snapshot_path: snapshot.png_path,
        summary_path,
        raw_manifest_path,
    })
}

fn snapshot_name(case: &PhantomWeaponCase) -> String {
    format!("{}-{}-{}", case.priority, case.item_id, case.case_id)
}

fn game_dir() -> PathBuf {
    std::env::var_os("XIV_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\_ff14\game"))
}

fn phantom_case_filter() -> Option<HashSet<String>> {
    let raw = std::env::var("XIV_PHANTOM_CASES").ok()?;
    let values = raw
        .split([',', ';', ' ', '\n', '\t'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    (!values.is_empty()).then_some(values)
}

fn phantom_case_matches_filter(case: &PhantomWeaponCase, filter: Option<&HashSet<String>>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    filter.contains(&case.case_id)
        || filter.contains(&case.item_id.to_string())
        || filter.contains(&case.name)
}

fn dump_raw_files(
    resource: &mut SqPackResource,
    model: &WeaponModelData,
    case_dir: &Path,
) -> Result<Vec<RawFileSummary>> {
    let raw_dir = case_dir.join("raw");
    fs::create_dir_all(&raw_dir)
        .with_context(|| format!("failed to create {}", raw_dir.display()))?;

    let mut files = Vec::new();
    for resource_path in &model.loaded_paths {
        let kind = resource_kind(resource_path);
        let dump_path = raw_dump_path(&raw_dir, resource_path);
        let Some(bytes) = resource.read(resource_path) else {
            files.push(RawFileSummary {
                resource_path: resource_path.clone(),
                dump_path: None,
                bytes: None,
                kind,
                error: Some("resource could not be read again from SqPack".to_string()),
            });
            continue;
        };

        if let Some(parent) = dump_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&dump_path, &bytes)
            .with_context(|| format!("failed to write {}", dump_path.display()))?;
        files.push(RawFileSummary {
            resource_path: resource_path.clone(),
            dump_path: Some(path_relative_to_case(&dump_path, case_dir)),
            bytes: Some(bytes.len()),
            kind,
            error: None,
        });
    }

    Ok(files)
}

fn dump_model_debug(
    resource: &mut SqPackResource,
    model: &WeaponModelData,
    case_dir: &Path,
) -> Result<(Vec<ModelDebugFileSummary>, HashMap<String, MdlMetadata>)> {
    let model_dir = case_dir.join("models");
    fs::create_dir_all(&model_dir)
        .with_context(|| format!("failed to create {}", model_dir.display()))?;

    let mut summaries = Vec::new();
    let mut metadata_by_path = HashMap::new();
    for resource_path in model
        .loaded_paths
        .iter()
        .filter(|path| path.ends_with(".mdl"))
    {
        let Some(bytes) = resource.read(resource_path) else {
            summaries.push(ModelDebugFileSummary {
                resource_path: resource_path.clone(),
                debug_file: None,
                mesh_count: None,
                submesh_count: None,
                material_count: None,
                attribute_count: None,
                bone_table_count: None,
                shape_count: None,
                has_extra_lods: None,
                error: Some("model could not be read again from SqPack".to_string()),
            });
            continue;
        };

        match mdl_metadata_from_mdl_bytes(resource_path, &bytes) {
            Ok(metadata) => {
                let debug_path =
                    model_dir.join(format!("{}.json", safe_resource_file(resource_path)));
                let summary = ModelDebugFileSummary {
                    resource_path: resource_path.clone(),
                    debug_file: Some(path_relative_to_case(&debug_path, case_dir)),
                    mesh_count: Some(metadata.meshes.len()),
                    submesh_count: Some(metadata.submeshes.len()),
                    material_count: Some(metadata.materials.len()),
                    attribute_count: Some(metadata.attributes.len()),
                    bone_table_count: Some(metadata.bone_tables.len()),
                    shape_count: Some(metadata.shapes.len()),
                    has_extra_lods: Some(metadata.model_header.has_extra_lods),
                    error: None,
                };
                write_json(&debug_path, &metadata)?;
                metadata_by_path.insert(resource_path.clone(), metadata);
                summaries.push(summary);
            }
            Err(error) => summaries.push(ModelDebugFileSummary {
                resource_path: resource_path.clone(),
                debug_file: None,
                mesh_count: None,
                submesh_count: None,
                material_count: None,
                attribute_count: None,
                bone_table_count: None,
                shape_count: None,
                has_extra_lods: None,
                error: Some(format!("{error:#}")),
            }),
        }
    }

    Ok((summaries, metadata_by_path))
}

fn dump_material_debug(
    resource: &mut SqPackResource,
    model: &WeaponModelData,
    case_dir: &Path,
) -> Result<Vec<MaterialDebugFileSummary>> {
    let material_dir = case_dir.join("materials");
    fs::create_dir_all(&material_dir)
        .with_context(|| format!("failed to create {}", material_dir.display()))?;

    let mut summaries = Vec::new();
    for material in &model.materials {
        let Some(resource_path) = material.path.as_ref() else {
            summaries.push(MaterialDebugFileSummary {
                slot: material.slot,
                material_index: material.material_index,
                material_name: material.name.clone(),
                resource_path: None,
                debug_file: None,
                semantic_summary: None,
                error: Some("fallback material has no .mtrl path".to_string()),
            });
            continue;
        };

        let Some(bytes) = resource.read(resource_path) else {
            summaries.push(MaterialDebugFileSummary {
                slot: material.slot,
                material_index: material.material_index,
                material_name: material.name.clone(),
                resource_path: Some(resource_path.clone()),
                debug_file: None,
                semantic_summary: None,
                error: Some("material could not be read again from SqPack".to_string()),
            });
            continue;
        };

        match material_debug_info_from_resource(resource, resource_path)
            .or_else(|_| material_debug_info_from_mtrl_bytes(resource_path, &bytes))
        {
            Ok(debug) => {
                let debug_path = material_dir.join(format!(
                    "{:03}-m{:04}-{}.json",
                    material.slot,
                    material.material_index,
                    safe_stem(&material.name)
                ));
                write_json(&debug_path, &debug)?;
                summaries.push(MaterialDebugFileSummary {
                    slot: material.slot,
                    material_index: material.material_index,
                    material_name: material.name.clone(),
                    resource_path: Some(resource_path.clone()),
                    debug_file: Some(path_relative_to_case(&debug_path, case_dir)),
                    semantic_summary: Some(debug.summary.clone()),
                    error: None,
                });
            }
            Err(error) => summaries.push(MaterialDebugFileSummary {
                slot: material.slot,
                material_index: material.material_index,
                material_name: material.name.clone(),
                resource_path: Some(resource_path.clone()),
                debug_file: None,
                semantic_summary: None,
                error: Some(format!("{error:#}")),
            }),
        }
    }

    Ok(summaries)
}

fn dump_decoded_textures(model: &WeaponModelData, case_dir: &Path) -> Result<Vec<TextureSummary>> {
    let texture_dir = case_dir.join("textures");
    fs::create_dir_all(&texture_dir)
        .with_context(|| format!("failed to create {}", texture_dir.display()))?;

    let mut summaries = Vec::new();
    for (index, texture) in model.textures.iter().enumerate() {
        let png_path = decoded_texture_path(&texture_dir, index, texture);
        let decoded_png = if texture.width != 0
            && texture.height != 0
            && texture.rgba.len() == texture.width as usize * texture.height as usize * 4
        {
            image::save_buffer_with_format(
                &png_path,
                &texture.rgba,
                texture.width.into(),
                texture.height.into(),
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .with_context(|| format!("failed to write {}", png_path.display()))?;
            Some(path_relative_to_case(&png_path, case_dir))
        } else {
            None
        };

        summaries.push(TextureSummary {
            index,
            path: texture.path.clone(),
            kind: format!("{:?}", texture.kind),
            width: texture.width,
            height: texture.height,
            rgba_bytes: texture.rgba.len(),
            decoded_png,
            pixel_stats: texture_pixel_stats(texture),
        });
    }

    Ok(summaries)
}

fn texture_pixel_stats(texture: &ModelTexture) -> Option<TexturePixelStats> {
    if texture.rgba.len() != texture.width as usize * texture.height as usize * 4 {
        return None;
    }

    let pixel_count = texture.rgba.len() / 4;
    if pixel_count == 0 {
        return None;
    }

    let mut alpha_min = u8::MAX;
    let mut alpha_max = u8::MIN;
    let mut alpha_total = 0_u64;
    let mut rgb_total = [0_u64; 3];
    let mut transparent_pixels = 0_usize;
    let mut translucent_pixels = 0_usize;
    let mut opaque_pixels = 0_usize;

    for pixel in texture.rgba.chunks_exact(4) {
        let alpha = pixel[3];
        alpha_min = alpha_min.min(alpha);
        alpha_max = alpha_max.max(alpha);
        alpha_total += u64::from(alpha);
        for channel in 0..3 {
            rgb_total[channel] += u64::from(pixel[channel]);
        }
        match alpha {
            0 => transparent_pixels += 1,
            255 => opaque_pixels += 1,
            _ => translucent_pixels += 1,
        }
    }

    Some(TexturePixelStats {
        pixel_count,
        alpha_min,
        alpha_max,
        alpha_average: alpha_total as f32 / pixel_count as f32 / 255.0,
        transparent_pixels,
        translucent_pixels,
        opaque_pixels,
        average_rgb: [
            rgb_total[0] as f32 / pixel_count as f32 / 255.0,
            rgb_total[1] as f32 / pixel_count as f32 / 255.0,
            rgb_total[2] as f32 / pixel_count as f32 / 255.0,
        ],
    })
}

fn dump_meshes(
    model: &WeaponModelData,
    case_dir: &Path,
    metadata_by_path: &HashMap<String, MdlMetadata>,
) -> Result<Vec<MeshSummary>> {
    let mesh_dir = case_dir.join("meshes");
    fs::create_dir_all(&mesh_dir)
        .with_context(|| format!("failed to create {}", mesh_dir.display()))?;

    let mut summaries = Vec::new();
    let prepared_model = prepare_model_for_render(model);
    for (index, mesh) in model.meshes.iter().enumerate() {
        let prepared_mesh = prepared_model
            .meshes
            .get(index)
            .with_context(|| format!("missing prepared mesh for mesh {index}"))?;
        let mesh_path = mesh_dir.join(format!(
            "{index:03}-m{:04}-p{}.json",
            mesh.material_index, mesh.part_index
        ));
        write_json(&mesh_path, mesh)?;
        summaries.push(mesh_summary(
            mesh,
            path_relative_to_case(&mesh_path, case_dir),
            prepared_mesh,
            metadata_by_path,
        ));
    }

    Ok(summaries)
}

fn mesh_summary(
    mesh: &ModelMesh,
    mesh_file: String,
    prepared_mesh: &PreparedMesh,
    metadata_by_path: &HashMap<String, MdlMetadata>,
) -> MeshSummary {
    let resource_path = mesh_resource_path(&mesh.path);
    let metadata = metadata_by_path.get(resource_path);
    let submesh_index = mesh_submesh_index(&mesh.path);
    let metadata_file =
        metadata.map(|metadata| format!("models/{}.json", safe_resource_file(&metadata.path)));
    let raw_mesh = metadata.and_then(|metadata| metadata.meshes.get(mesh.part_index as usize));
    let draw_role = prepared_mesh.draw_role;
    let rendered_in_main_pass = prepared_mesh.renders_in_main_pass;

    MeshSummary {
        mesh_file,
        path: mesh.path.clone(),
        part_index: mesh.part_index,
        mesh_category: mesh.mesh_category.clone(),
        draw_role,
        rendered_in_main_pass,
        prepared_material: rendered_in_main_pass.then_some(prepared_mesh.prepared_material),
        prepared_submesh: prepared_mesh.submesh.clone(),
        prepared_shape_influences: prepared_mesh.shape_influences.clone(),
        prepared_shape_influence_state: prepared_mesh.shape_influence_state,
        metadata_file,
        submesh_index,
        submeshes: raw_mesh
            .map(|raw_mesh| mesh_submesh_summaries(raw_mesh, submesh_index))
            .unwrap_or_default(),
        bone_table: mesh.bone_table.clone(),
        shapes: metadata
            .map(|metadata| mesh_shape_summaries(metadata, mesh.part_index as usize))
            .unwrap_or_default(),
        material_index: mesh.material_index,
        material_slot: mesh.material_slot,
        material_name: mesh.material_name.clone(),
        vertex_count: mesh.vertices.len(),
        index_count: mesh.indices.len(),
        bounds: xiv_companion::calculate_model_bounds(std::slice::from_ref(mesh)),
    }
}

fn mesh_resource_path(mesh_path: &str) -> &str {
    mesh_path
        .split_once('#')
        .map_or(mesh_path, |(path, _)| path)
}

fn mesh_submesh_index(mesh_path: &str) -> Option<usize> {
    mesh_path
        .split_once("#part-")
        .and_then(|(_, fragment)| fragment.rsplit_once("-submesh-"))
        .and_then(|(_, submesh)| submesh.parse().ok())
}

fn mesh_submesh_summaries(
    mesh: &MdlMeshMetadata,
    selected_submesh_index: Option<usize>,
) -> Vec<MeshSubmeshSummary> {
    let selected = selected_submesh_index
        .and_then(|index| mesh.submeshes.get(index).map(std::slice::from_ref));

    selected
        .unwrap_or(mesh.submeshes.as_slice())
        .iter()
        .map(|submesh| MeshSubmeshSummary {
            table_index: submesh.table_index,
            index_offset: submesh.index_offset,
            relative_index_offset: submesh.relative_index_offset,
            index_count: submesh.index_count,
            attribute_index_mask_hex: submesh.attribute_index_mask_hex.clone(),
            attribute_names: submesh.attribute_names.clone(),
            bone_start_index: submesh.bone_start_index,
            bone_count: submesh.bone_count,
        })
        .collect()
}

fn mesh_shape_summaries(metadata: &MdlMetadata, mesh_index: usize) -> Vec<MeshShapeSummary> {
    let Some(mesh) = metadata.meshes.get(mesh_index) else {
        return Vec::new();
    };

    let mut summaries = Vec::new();
    for shape in &metadata.shapes {
        let start = usize::from(shape.shape_mesh_start_indices[0]);
        let count = usize::from(shape.shape_mesh_counts[0]);
        for shape_mesh_index in start..start.saturating_add(count) {
            let Some(shape_mesh) = metadata.shape_meshes.get(shape_mesh_index) else {
                continue;
            };
            if shape_mesh.mesh_index_offset != mesh.start_index {
                continue;
            }
            summaries.push(MeshShapeSummary {
                shape_index: shape.index,
                name: shape.name.clone(),
                shape_mesh_index,
                shape_value_count: shape_mesh.shape_value_count,
            });
        }
    }

    summaries
}

fn material_summary(
    material: &ModelMaterial,
    debug_by_slot: &HashMap<usize, MaterialDebugFileSummary>,
) -> MaterialSummary {
    let debug = debug_by_slot.get(&material.slot);
    MaterialSummary {
        slot: material.slot,
        material_index: material.material_index,
        name: material.name.clone(),
        path: material.path.clone(),
        shader_package_name: material.shader_package_name.clone(),
        render_mode: format!("{:?}", material.render_mode),
        alpha_mode: format!("{:?}", material.alpha_mode),
        alpha_threshold: material.alpha_threshold,
        transparency: material.transparency,
        alpha_aperture: material.alpha_aperture,
        alpha_offset: material.alpha_offset,
        shadow_alpha_threshold: material.shadow_alpha_threshold,
        glass_ior: material.glass_ior,
        glass_thickness_max: material.glass_thickness_max,
        normal_scale: material.normal_scale,
        multi_normal_scale: material.multi_normal_scale,
        detail_normal_scale: material.detail_normal_scale,
        multi_detail_normal_scale: material.multi_detail_normal_scale,
        tile_index: material.tile_index,
        tile_alpha: material.tile_alpha,
        tile_scale: material.tile_scale,
        toon_index: material.toon_index,
        toon_light_scale: material.toon_light_scale,
        sheen_rate: material.sheen_rate,
        sheen_tint_rate: material.sheen_tint_rate,
        sheen_aperture: material.sheen_aperture,
        sphere_map_index: material.sphere_map_index,
        detail_id: material.detail_id,
        multi_detail_id: material.multi_detail_id,
        detail_color: material.detail_color,
        multi_detail_color: material.multi_detail_color,
        shader_diffuse_color: material.shader_diffuse_color,
        shader_multi_diffuse_color: material.shader_multi_diffuse_color,
        shader_emissive_color: material.shader_emissive_color,
        shader_multi_emissive_color: material.shader_multi_emissive_color,
        outline_color: material.outline_color,
        outline_width: material.outline_width,
        specular_color_mask: material.specular_color_mask,
        ssao_mask: material.ssao_mask,
        texture_mip_bias: material.texture_mip_bias,
        shadow_pos_offset: material.shadow_pos_offset,
        detail_color_uv_scale: material.detail_color_uv_scale,
        detail_normal_uv_scale: material.detail_normal_uv_scale,
        uv_scroll: material.uv_scroll,
        lightshaft_color: material.lightshaft_color,
        lightshaft_tex_anim: material.lightshaft_tex_anim,
        lightshaft_tex_u: material.lightshaft_tex_u,
        lightshaft_tex_v: material.lightshaft_tex_v,
        lightshaft_ray: material.lightshaft_ray,
        opacity: material.opacity,
        render_backfaces: material.render_backfaces,
        apply_vertex_color: material.apply_vertex_color,
        fallback_color: material.fallback_color,
        diffuse_color: material.diffuse_color,
        specular_color: material.specular_color,
        emissive_color: material.emissive_color,
        roughness: material.roughness,
        metalness: material.metalness,
        texture_indices: material.texture_indices.clone(),
        base_color_texture: material.base_color_texture,
        normal_texture: material.normal_texture,
        mask_texture: material.mask_texture,
        material_map_texture: material.material_map_texture,
        multi_map_texture: material.multi_map_texture,
        specular_texture: material.specular_texture,
        emissive_texture: material.emissive_texture,
        material_properties_texture: material.material_properties_texture,
        tile_properties_texture: material.tile_properties_texture,
        sheen_properties_texture: material.sheen_properties_texture,
        sphere_properties_texture: material.sphere_properties_texture,
        tile_matrix_texture: material.tile_matrix_texture,
        debug_file: debug.and_then(|debug| debug.debug_file.clone()),
        semantic_summary: debug.and_then(|debug| debug.semantic_summary.clone()),
    }
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context("failed to encode JSON")?;
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

fn raw_dump_path(raw_dir: &Path, resource_path: &str) -> PathBuf {
    let mut path = raw_dir.to_path_buf();
    for part in resource_path.replace('\\', "/").split('/') {
        if part.is_empty() || matches!(part, "." | "..") {
            continue;
        }
        path.push(part);
    }
    path
}

fn decoded_texture_path(texture_dir: &Path, index: usize, texture: &ModelTexture) -> PathBuf {
    let kind = format!("{:?}", texture.kind).to_ascii_lowercase();
    let stem = texture
        .path
        .rsplit('/')
        .next()
        .unwrap_or(texture.path.as_str())
        .trim_end_matches(".tex");
    texture_dir.join(format!("{index:03}-{kind}-{}.png", safe_stem(stem)))
}

fn resource_kind(resource_path: &str) -> String {
    resource_path
        .rsplit('.')
        .next()
        .filter(|extension| *extension != resource_path)
        .unwrap_or("resource")
        .to_ascii_lowercase()
}

fn markdown_path(case_dir: &str, path: &Path) -> String {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    format!("{case_dir}/{file_name}")
}

fn path_relative_to_case(path: &Path, case_dir: &Path) -> String {
    path.strip_prefix(case_dir)
        .unwrap_or(path)
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_stem(value: &str) -> String {
    let mut stem = String::with_capacity(value.len().max(1));
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            stem.push(ch);
        } else if !stem.ends_with('-') {
            stem.push('-');
        }
    }

    let stem = stem.trim_matches('-');
    if stem.is_empty() {
        "resource".to_string()
    } else {
        stem.to_string()
    }
}

fn safe_resource_file(resource_path: &str) -> String {
    safe_stem(resource_path.trim_end_matches(".mdl"))
}
