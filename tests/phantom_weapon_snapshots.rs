#![cfg(all(feature = "game-data", feature = "render-test-support"))]

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use physis::resource::{Resource, SqPackResource};
use serde::{Deserialize, Serialize};
use xiv_companion::{
    ModelBounds, ModelMaterial, ModelMesh, ModelTexture, WeaponCatalogItem, WeaponModelData,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    load_weapon_model_from_resource,
    renderer::test_support::{
        WeaponModelSnapshotOptions, render_weapon_model_snapshot_with_options,
    },
};

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
    bounds: ModelBounds,
    loaded_paths: Vec<String>,
    mesh_count: usize,
    material_count: usize,
    texture_count: usize,
    meshes: Vec<MeshSummary>,
    materials: Vec<MaterialSummary>,
    textures: Vec<TextureSummary>,
    raw_files: Vec<RawFileSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshSummary {
    mesh_file: String,
    path: String,
    part_index: u32,
    material_index: u16,
    material_slot: usize,
    material_name: String,
    vertex_count: usize,
    index_count: usize,
    bounds: ModelBounds,
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
    opacity: f32,
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
    emissive_texture: Option<usize>,
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

    manifest.push("# Phantom weapon snapshots".to_string());
    manifest.push(String::new());
    manifest.push(format!("- gameDir: {}", game_dir.display()));
    manifest.push(format!("- outputDir: {}", output_dir.display()));
    manifest.push(String::new());
    manifest.push("| priority | item | focus | snapshot | summary | raw |".to_string());
    manifest.push("| --- | --- | --- | --- | --- | --- |".to_string());

    for case in &fixture.cases {
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
    let texture_summaries = dump_decoded_textures(&model, &case_dir)?;
    let mesh_summaries = dump_meshes(&model, &case_dir)?;
    let material_summaries = model.materials.iter().map(material_summary).collect();
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
        bounds: model.bounds,
        loaded_paths: model.loaded_paths.clone(),
        mesh_count: model.meshes.len(),
        material_count: model.materials.len(),
        texture_count: model.textures.len(),
        meshes: mesh_summaries,
        materials: material_summaries,
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
        });
    }

    Ok(summaries)
}

fn dump_meshes(model: &WeaponModelData, case_dir: &Path) -> Result<Vec<MeshSummary>> {
    let mesh_dir = case_dir.join("meshes");
    fs::create_dir_all(&mesh_dir)
        .with_context(|| format!("failed to create {}", mesh_dir.display()))?;

    let mut summaries = Vec::new();
    for (index, mesh) in model.meshes.iter().enumerate() {
        let mesh_path = mesh_dir.join(format!(
            "{index:03}-m{:04}-p{}.json",
            mesh.material_index, mesh.part_index
        ));
        write_json(&mesh_path, mesh)?;
        summaries.push(mesh_summary(
            mesh,
            path_relative_to_case(&mesh_path, case_dir),
        ));
    }

    Ok(summaries)
}

fn mesh_summary(mesh: &ModelMesh, mesh_file: String) -> MeshSummary {
    MeshSummary {
        mesh_file,
        path: mesh.path.clone(),
        part_index: mesh.part_index,
        material_index: mesh.material_index,
        material_slot: mesh.material_slot,
        material_name: mesh.material_name.clone(),
        vertex_count: mesh.vertices.len(),
        index_count: mesh.indices.len(),
        bounds: xiv_companion::calculate_model_bounds(std::slice::from_ref(mesh)),
    }
}

fn material_summary(material: &ModelMaterial) -> MaterialSummary {
    MaterialSummary {
        slot: material.slot,
        material_index: material.material_index,
        name: material.name.clone(),
        path: material.path.clone(),
        shader_package_name: material.shader_package_name.clone(),
        render_mode: format!("{:?}", material.render_mode),
        opacity: material.opacity,
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
        emissive_texture: material.emissive_texture,
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
