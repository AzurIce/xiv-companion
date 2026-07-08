pub use crate::model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialRenderMode, ModelBounds, ModelData,
    ModelMaterial, ModelMesh, ModelRenderData, ModelTexture, ModelTextureKind, ModelVertex,
    PackedModelId, WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage,
    WeaponMaterialAlphaMode, WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData,
    WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture, WeaponModelTextureKind,
    WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "game-data")]
use std::collections::HashMap;

#[cfg(feature = "game-data")]
const APPLY_ALPHA_TEST: u32 = 0xA9A3_EE25;
#[cfg(feature = "game-data")]
const APPLY_ALPHA_TEST_ON: u32 = 0x72AA_A9AE;
#[cfg(feature = "game-data")]
const G_ALPHA_THRESHOLD: u32 = 0x29AC_0223;
#[cfg(feature = "game-data")]
#[cfg(test)]
const APPLY_ALPHA_TEST_OFF: u32 = 0x5D14_6A23;
#[cfg(feature = "game-data")]
const APPLY_VERTEX_COLOR: u32 = 0x4F4F_0636;
#[cfg(feature = "game-data")]
const APPLY_VERTEX_COLOR_ON: u32 = 0xBD94_649A;

#[cfg(feature = "game-data")]
pub use crate::game_data::normalize_game_dir;

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponModelLoadRequest {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: u64,
    pub model_sub: u64,
}

#[cfg(feature = "game-data")]
impl WeaponModelLoadRequest {
    pub fn primary_model(&self) -> PackedModelId {
        PackedModelId::from_raw(self.model_main)
    }

    pub fn secondary_model(&self) -> Option<PackedModelId> {
        (self.model_sub != 0).then(|| PackedModelId::from_raw(self.model_sub))
    }
}

#[cfg(feature = "game-data")]
impl From<&WeaponCatalogItem> for WeaponModelLoadRequest {
    fn from(item: &WeaponCatalogItem) -> Self {
        Self {
            item_id: item.id,
            item_name: item.name.clone(),
            model_main: item.model_main,
            model_sub: item.model_sub,
        }
    }
}

#[cfg(feature = "game-data")]
pub trait AsyncGameResource {
    type Error: std::fmt::Display;
    type ReadFuture<'a>: std::future::Future<Output = Result<Vec<u8>, Self::Error>> + 'a
    where
        Self: 'a;

    fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a>;
    fn platform(&self) -> physis::Platform;
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDebugInfo {
    pub path: String,
    pub shader_package_name: String,
    pub shader_flags: u32,
    pub shader_flags_hex: String,
    pub texture_paths: Vec<String>,
    pub shader_keys: Vec<MaterialShaderKeyDebug>,
    pub constants_debug: Vec<String>,
    pub samplers: Vec<MaterialSamplerDebug>,
    pub color_table: Option<MaterialColorTableDebug>,
    pub color_dye_table_kind: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialShaderKeyDebug {
    pub category: u32,
    pub category_hex: String,
    pub value: u32,
    pub value_hex: String,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSamplerDebug {
    pub texture_index: usize,
    pub texture_path: Option<String>,
    pub texture_usage: u32,
    pub texture_usage_hex: String,
    pub kind: Option<WeaponModelTextureKind>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialColorTableDebug {
    pub kind: String,
    pub row_count: usize,
    pub rows: Vec<MaterialColorTableRowDebug>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialColorTableRowDebug {
    pub index: usize,
    pub diffuse_color: Option<[f32; 3]>,
    pub specular_color: Option<[f32; 3]>,
    pub emissive_color: Option<[f32; 3]>,
    pub specular_strength: Option<f32>,
    pub gloss_strength: Option<f32>,
    pub roughness: Option<f32>,
    pub metalness: Option<f32>,
    pub alpha: Option<f32>,
    pub tile_set: Option<u16>,
    pub shader_index: Option<u16>,
    pub sphere_index: Option<u16>,
    pub material_repeat: Option<[f32; 2]>,
    pub material_skew: Option<[f32; 2]>,
}

#[cfg(feature = "game-data")]
pub fn load_weapon_model_from_game_dir(
    game_dir: &std::path::Path,
    item: &WeaponCatalogItem,
) -> anyhow::Result<WeaponModelData> {
    use anyhow::{Context, anyhow};

    let game_dir = normalize_game_dir(game_dir)?;
    let game_dir = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = physis::resource::SqPackResource::from_existing(game_dir);
    load_weapon_model_from_resource(&mut resource, item).with_context(|| {
        format!(
            "failed to load weapon model for {} ({})",
            item.name, item.id
        )
    })
}

#[cfg(feature = "game-data")]
pub fn load_weapon_model_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    item: &WeaponCatalogItem,
) -> anyhow::Result<WeaponModelData> {
    load_weapon_model_from_resource_request(resource, &WeaponModelLoadRequest::from(item))
}

#[cfg(feature = "game-data")]
pub fn load_weapon_model_from_resource_request<R: physis::resource::Resource>(
    resource: &mut R,
    request: &WeaponModelLoadRequest,
) -> anyhow::Result<WeaponModelData> {
    use anyhow::anyhow;

    let model_main = request.primary_model();
    let model_sub = request.secondary_model();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();

    load_weapon_model_meshes_from_resource(
        resource,
        model_main,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
    )?;

    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            let _ = load_weapon_model_meshes_from_resource(
                resource,
                model_sub,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
            );
        }
    }

    if meshes.is_empty() {
        return Err(anyhow!(
            "{} has no renderable model meshes",
            request.item_name
        ));
    }

    Ok(WeaponModelData {
        item_id: request.item_id,
        item_name: request.item_name.clone(),
        model_main,
        model_sub,
        loaded_paths,
        bounds: calculate_model_bounds(&meshes),
        materials,
        textures,
        meshes,
    })
}

#[cfg(feature = "game-data")]
pub fn meshes_from_mdl_bytes(path: &str, bytes: &[u8]) -> anyhow::Result<Vec<WeaponModelMesh>> {
    use anyhow::{Context, anyhow};
    use physis::{Platform, ReadableFile};

    let mdl = physis::model::MDL::from_existing(Platform::Win32, bytes)
        .ok_or_else(|| anyhow!("failed to parse model {path}"))?;
    let lod = mdl
        .lods
        .first()
        .ok_or_else(|| anyhow!("model {path} has no LODs"))?;

    let mut meshes = Vec::new();
    for (part_index, part) in lod.parts.iter().enumerate() {
        if part.vertices.is_empty() || part.indices.is_empty() {
            continue;
        }

        let material_name = mdl
            .material_names
            .get(part.material_index as usize)
            .cloned()
            .unwrap_or_else(|| format!("material-{}", part.material_index));
        let color = material_color(part.material_index);
        let part_vertices = part
            .vertices
            .iter()
            .map(|vertex| WeaponModelVertex {
                position: vertex.position,
                normal: normalized_or_fallback(vertex.normal),
                uv0: vertex.uv0,
                uv1: vertex.uv1,
                bitangent: sanitized_bitangent(vertex.bitangent),
                color: vertex_color_or_fallback(vertex.color),
            })
            .collect::<Vec<_>>();
        for range in mesh_index_ranges(part.indices.len(), &part.submeshes) {
            let raw_indices = &part.indices[range.start..range.end];
            let Some((vertices, indices)) = remap_mesh_vertices(&part_vertices, raw_indices) else {
                continue;
            };
            meshes.push(WeaponModelMesh {
                path: mesh_path_with_submesh(path, part_index, range.submesh_index),
                part_index: part_index as u32,
                material_index: part.material_index,
                material_slot: part.material_index as usize,
                material_name: material_name.clone(),
                color,
                vertices,
                indices,
            });
        }
    }

    (!meshes.is_empty())
        .then_some(meshes)
        .ok_or_else(|| anyhow!("model {path} contains no renderable meshes"))
        .with_context(|| format!("failed to extract render meshes from {path}"))
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MeshIndexRange {
    submesh_index: Option<usize>,
    start: usize,
    end: usize,
}

#[cfg(feature = "game-data")]
fn mesh_index_ranges(
    index_count: usize,
    submeshes: &[physis::model::SubMesh],
) -> Vec<MeshIndexRange> {
    normalize_submesh_index_ranges(
        index_count,
        submeshes
            .iter()
            .enumerate()
            .map(|(submesh_index, submesh)| {
                (
                    submesh_index,
                    submesh.index_offset as usize,
                    submesh.index_count as usize,
                )
            }),
    )
}

#[cfg(feature = "game-data")]
fn normalize_submesh_index_ranges<I>(index_count: usize, submeshes: I) -> Vec<MeshIndexRange>
where
    I: IntoIterator<Item = (usize, usize, usize)>,
{
    let raw = submeshes
        .into_iter()
        .filter(|(_, _, count)| *count != 0)
        .collect::<Vec<_>>();
    if raw.is_empty() || index_count == 0 {
        return full_mesh_index_range(index_count);
    }

    let base_index_offset = raw[0].1;
    let mut ranges = Vec::new();
    for (submesh_index, index_offset, count) in raw {
        let relative_start = index_offset.checked_sub(base_index_offset).filter(|start| {
            start
                .checked_add(count)
                .is_some_and(|end| end <= index_count)
        });
        let direct_start = (index_offset
            .checked_add(count)
            .is_some_and(|end| end <= index_count))
        .then_some(index_offset);
        let Some(start) = relative_start.or(direct_start) else {
            continue;
        };
        ranges.push(MeshIndexRange {
            submesh_index: Some(submesh_index),
            start,
            end: start + count,
        });
    }

    if ranges.is_empty() {
        return full_mesh_index_range(index_count);
    }
    if ranges.len() == 1 && ranges[0].start == 0 && ranges[0].end == index_count {
        ranges[0].submesh_index = None;
    }
    ranges
}

#[cfg(feature = "game-data")]
fn full_mesh_index_range(index_count: usize) -> Vec<MeshIndexRange> {
    if index_count == 0 {
        Vec::new()
    } else {
        vec![MeshIndexRange {
            submesh_index: None,
            start: 0,
            end: index_count,
        }]
    }
}

#[cfg(feature = "game-data")]
fn mesh_path_with_submesh(path: &str, part_index: usize, submesh_index: Option<usize>) -> String {
    match submesh_index {
        Some(submesh_index) => format!("{path}#part-{part_index}-submesh-{submesh_index}"),
        None => path.to_string(),
    }
}

#[cfg(feature = "game-data")]
fn remap_mesh_vertices(
    vertices: &[WeaponModelVertex],
    indices: &[u16],
) -> Option<(Vec<WeaponModelVertex>, Vec<u32>)> {
    if indices.len() % 3 != 0 {
        return None;
    }

    let mut remapped_vertices = Vec::new();
    let mut remap = HashMap::<u16, u32>::new();
    let mut remapped_indices = Vec::with_capacity(indices.len());

    for triangle in indices.chunks_exact(3) {
        let a = remap_vertex_index(vertices, &mut remapped_vertices, &mut remap, triangle[0])?;
        let b = remap_vertex_index(vertices, &mut remapped_vertices, &mut remap, triangle[1])?;
        let c = remap_vertex_index(vertices, &mut remapped_vertices, &mut remap, triangle[2])?;

        if should_flip_triangle_winding(
            &remapped_vertices[a as usize],
            &remapped_vertices[b as usize],
            &remapped_vertices[c as usize],
        ) {
            remapped_indices.extend([a, c, b]);
        } else {
            remapped_indices.extend([a, b, c]);
        }
    }

    Some((remapped_vertices, remapped_indices))
}

#[cfg(feature = "game-data")]
fn remap_vertex_index(
    vertices: &[WeaponModelVertex],
    remapped_vertices: &mut Vec<WeaponModelVertex>,
    remap: &mut HashMap<u16, u32>,
    index: u16,
) -> Option<u32> {
    if usize::from(index) >= vertices.len() {
        return None;
    }

    if let Some(remapped_index) = remap.get(&index) {
        Some(*remapped_index)
    } else {
        let remapped_index = remapped_vertices.len() as u32;
        remapped_vertices.push(vertices[usize::from(index)]);
        remap.insert(index, remapped_index);
        Some(remapped_index)
    }
}

#[cfg(feature = "game-data")]
fn should_flip_triangle_winding(
    a: &WeaponModelVertex,
    b: &WeaponModelVertex,
    c: &WeaponModelVertex,
) -> bool {
    let edge_ab = vec3_sub(b.position, a.position);
    let edge_ac = vec3_sub(c.position, a.position);
    let Some(face_normal) = normalize_vec3(vec3_cross(edge_ab, edge_ac)) else {
        return false;
    };

    [a.normal, b.normal, c.normal].into_iter().all(|normal| {
        normalize_vec3(normal).is_some_and(|normal| dot_vec3(face_normal, normal) < -0.25)
    })
}

#[cfg(feature = "game-data")]
fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[cfg(feature = "game-data")]
fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[cfg(feature = "game-data")]
fn dot_vec3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[cfg(feature = "game-data")]
fn normalize_vec3(value: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot_vec3(value, value).sqrt();
    if !length.is_finite() || length <= 0.0001 {
        None
    } else {
        Some([value[0] / length, value[1] / length, value[2] / length])
    }
}

#[cfg(feature = "game-data")]
pub fn material_debug_info_from_mtrl_bytes(
    path: &str,
    bytes: &[u8],
) -> anyhow::Result<MaterialDebugInfo> {
    use anyhow::anyhow;
    use physis::ReadableFile;

    let material = physis::mtrl::Material::from_existing(physis::Platform::Win32, bytes)
        .ok_or_else(|| anyhow!("failed to parse material {path}"))?;
    let sampler_records =
        parse_material_sampler_records(bytes, &ComposedMaterialSemantics::default());
    let shader_flags = parse_material_shader_flags(bytes);

    Ok(MaterialDebugInfo {
        path: path.to_string(),
        shader_package_name: material.shader_package_name.clone(),
        shader_flags,
        shader_flags_hex: hex_u32(shader_flags),
        texture_paths: material.texture_paths.clone(),
        shader_keys: material
            .shader_keys
            .iter()
            .map(|key| MaterialShaderKeyDebug {
                category: key.category,
                category_hex: hex_u32(key.category),
                value: key.value,
                value_hex: hex_u32(key.value),
            })
            .collect(),
        constants_debug: material
            .constants
            .iter()
            .map(|constant| format!("{constant:?}"))
            .collect(),
        samplers: sampler_records
            .into_iter()
            .map(|record| MaterialSamplerDebug {
                texture_index: record.texture_index,
                texture_path: material.texture_paths.get(record.texture_index).cloned(),
                texture_usage: record.texture_usage,
                texture_usage_hex: hex_u32(record.texture_usage),
                kind: record.kind,
            })
            .collect(),
        color_table: material_color_table_debug(material.color_table.as_ref()),
        color_dye_table_kind: material
            .color_dye_table
            .as_ref()
            .map(material_color_dye_table_kind),
    })
}

#[cfg(feature = "game-data")]
fn normalized_or_fallback(normal: [f32; 3]) -> [f32; 3] {
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length > 0.0001 {
        [normal[0] / length, normal[1] / length, normal[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

#[cfg(feature = "game-data")]
fn sanitized_bitangent(bitangent: [f32; 4]) -> [f32; 4] {
    let length =
        (bitangent[0] * bitangent[0] + bitangent[1] * bitangent[1] + bitangent[2] * bitangent[2])
            .sqrt();
    let xyz = if length.is_finite()
        && length > 0.0001
        && bitangent[..3].iter().all(|value| value.is_finite())
    {
        [
            bitangent[0] / length,
            bitangent[1] / length,
            bitangent[2] / length,
        ]
    } else {
        [1.0, 0.0, 0.0]
    };
    let sign = if bitangent[3].is_nan() || bitangent[3] > 0.0 {
        1.0
    } else {
        -1.0
    };
    [xyz[0], xyz[1], xyz[2], sign]
}

#[cfg(feature = "game-data")]
fn vertex_color_or_fallback(color: [f32; 4]) -> [f32; 4] {
    if color[..3].iter().any(|value| value.abs() > 0.0001) {
        color
    } else if color[3].abs() > 0.0001 {
        [1.0, 1.0, 1.0, color[3]]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}

#[cfg(feature = "game-data")]
fn hex_u32(value: u32) -> String {
    format!("0x{value:08x}")
}

#[cfg(feature = "game-data")]
fn material_color_table_debug(
    color_table: Option<&physis::mtrl::ColorTable>,
) -> Option<MaterialColorTableDebug> {
    match color_table? {
        physis::mtrl::ColorTable::LegacyColorTable(table) => Some(MaterialColorTableDebug {
            kind: "Legacy".to_string(),
            row_count: table.rows.len(),
            rows: table
                .rows
                .iter()
                .enumerate()
                .map(|(index, row)| MaterialColorTableRowDebug {
                    index,
                    diffuse_color: Some(row.diffuse_color),
                    specular_color: Some(row.specular_color),
                    emissive_color: Some(row.emissive_color),
                    specular_strength: Some(row.specular_strength),
                    gloss_strength: Some(row.gloss_strength),
                    roughness: None,
                    metalness: None,
                    alpha: None,
                    tile_set: Some(row.tile_set),
                    shader_index: None,
                    sphere_index: None,
                    material_repeat: Some([row.material_repeat_x, row.material_repeat_y]),
                    material_skew: Some(row.material_skew),
                })
                .collect(),
        }),
        physis::mtrl::ColorTable::DawntrailColorTable(table) => Some(MaterialColorTableDebug {
            kind: "Dawntrail".to_string(),
            row_count: table.rows.len(),
            rows: table
                .rows
                .iter()
                .enumerate()
                .map(|(index, row)| MaterialColorTableRowDebug {
                    index,
                    diffuse_color: Some(row.diffuse_color),
                    specular_color: Some(row.specular_color),
                    emissive_color: Some(row.emissive_color),
                    specular_strength: Some(row.unknown2),
                    gloss_strength: Some(row.unknown1),
                    roughness: Some(row.roughness),
                    metalness: Some(row.metalness),
                    alpha: Some(row.tile_alpha),
                    tile_set: Some(row.tile_set),
                    shader_index: Some(row.shader_index),
                    sphere_index: Some(row.sphere_index),
                    material_repeat: Some(row.material_repeat),
                    material_skew: Some(row.material_skew),
                })
                .collect(),
        }),
        physis::mtrl::ColorTable::OpaqueColorTable(_) => Some(MaterialColorTableDebug {
            kind: "Opaque".to_string(),
            row_count: 0,
            rows: Vec::new(),
        }),
    }
}

#[cfg(feature = "game-data")]
fn material_color_dye_table_kind(color_dye_table: &physis::mtrl::ColorDyeTable) -> String {
    match color_dye_table {
        physis::mtrl::ColorDyeTable::LegacyColorDyeTable(_) => "Legacy".to_string(),
        physis::mtrl::ColorDyeTable::DawntrailColorDyeTable(_) => "Dawntrail".to_string(),
        physis::mtrl::ColorDyeTable::OpaqueColorDyeTable(_) => "Opaque".to_string(),
    }
}

#[cfg(feature = "game-data")]
fn load_weapon_model_meshes_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    model: PackedModelId,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<WeaponModelMesh>,
) -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};

    let mut missing = Vec::new();
    for path in weapon_model_candidate_paths(model) {
        let Some(bytes) = resource.read(&path) else {
            missing.push(path);
            continue;
        };

        let mut path_meshes = meshes_from_mdl_bytes(&path, &bytes)
            .with_context(|| format!("failed to load render meshes from {path}"))?;
        push_loaded_path(loaded_paths, path.clone());
        assign_weapon_materials_from_resource(
            resource,
            model,
            &path,
            &mut path_meshes,
            materials,
            textures,
            loaded_paths,
        );
        meshes.append(&mut path_meshes);
        return Ok(());
    }

    Err(anyhow!(
        "unable to read weapon model {} (tried: {})",
        model.model_id,
        missing.join("; ")
    ))
}

#[cfg(feature = "game-data")]
fn assign_weapon_materials_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    model: PackedModelId,
    model_path: &str,
    meshes: &mut [WeaponModelMesh],
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) {
    let mut slots = Vec::<(u16, usize)>::new();
    let mut material_specs = Vec::<(u16, String)>::new();
    for mesh in meshes.iter() {
        if !material_specs
            .iter()
            .any(|(index, _)| *index == mesh.material_index)
        {
            material_specs.push((mesh.material_index, mesh.material_name.clone()));
        }
    }

    for (material_index, material_name) in material_specs {
        let slot = materials.len();
        let material = load_weapon_material_from_resource(
            resource,
            model,
            model_path,
            material_index,
            material_name,
            slot,
            textures,
            loaded_paths,
        );
        materials.push(material);
        slots.push((material_index, slot));
    }

    for mesh in meshes {
        if let Some((_, slot)) = slots
            .iter()
            .find(|(material_index, _)| *material_index == mesh.material_index)
        {
            mesh.material_slot = *slot;
        }
    }
}

#[cfg(feature = "game-data")]
fn load_weapon_material_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    model: PackedModelId,
    model_path: &str,
    material_index: u16,
    material_name: String,
    slot: usize,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponModelMaterial {
    use physis::ReadableFile;

    let fallback = material_color(material_index);
    let candidates = weapon_material_candidate_paths(model, model_path, &material_name);
    for path in candidates {
        let Some(bytes) = resource.read(&path) else {
            continue;
        };
        let Some(material) = physis::mtrl::Material::from_existing(resource.platform(), &bytes)
        else {
            continue;
        };

        push_loaded_path(loaded_paths, path.clone());
        let shader_package_name = material.shader_package_name.clone();
        let summary = summarize_material_colors(material.color_table.as_ref(), fallback);
        let semantics = load_composed_material_semantics_from_resource(
            resource,
            &shader_package_name,
            &material,
            &bytes,
            loaded_paths,
        );
        let sampler_roles = parse_material_sampler_roles(&bytes, &semantics);
        let shader_flags = parse_material_shader_flags(&bytes);
        let alpha_test = semantics.has_material_key(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON);
        let apply_vertex_color =
            semantics.has_material_key(APPLY_VERTEX_COLOR, APPLY_VERTEX_COLOR_ON);
        let material_alpha_threshold = composed_material_alpha_threshold(&semantics);
        let texture_set = load_weapon_material_textures_from_resource(
            resource,
            &path,
            &material,
            &sampler_roles,
            textures,
            loaded_paths,
        );

        let alpha_mode = weapon_material_alpha_mode(
            &shader_package_name,
            shader_flags,
            &texture_set,
            alpha_test,
        );
        let alpha_threshold =
            material_alpha_threshold.unwrap_or_else(|| default_alpha_threshold(alpha_mode));
        let render_mode = weapon_material_render_mode(alpha_mode);
        let opacity = weapon_material_opacity(render_mode);
        let render_backfaces = material_render_backfaces(shader_flags);
        let diffuse_color = if texture_set.base_color.is_some() {
            [1.0, 1.0, 1.0]
        } else {
            summary.diffuse
        };
        let emissive_color = preview_emissive_color(summary.emissive, &texture_set);

        return WeaponModelMaterial {
            slot,
            material_index,
            name: material_name,
            path: Some(path),
            shader_package_name: Some(shader_package_name),
            render_mode,
            alpha_mode,
            alpha_threshold,
            opacity,
            render_backfaces,
            apply_vertex_color,
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            normal_texture: texture_set.normal,
            mask_texture: texture_set.mask,
            specular_texture: texture_set.specular,
            emissive_texture: texture_set.emissive,
            material_properties_texture: texture_set.material_properties,
        };
    }

    fallback_weapon_material(slot, material_index, material_name, fallback)
}

#[cfg(feature = "game-data")]
fn load_weapon_material_textures_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    material_path: &str,
    material: &physis::mtrl::Material,
    sampler_roles: &[MaterialSamplerRole],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in material.texture_paths.iter().enumerate() {
        let kind = classify_weapon_texture(
            raw_texture_path,
            sampler_kind_for_texture(sampler_roles, texture_order),
        );
        let Some(texture_index) = load_weapon_texture_from_resource(
            resource,
            material_path,
            raw_texture_path,
            kind,
            textures,
            loaded_paths,
        ) else {
            continue;
        };
        if !set.indices.contains(&texture_index) {
            set.indices.push(texture_index);
        }
        match textures[texture_index].kind {
            WeaponModelTextureKind::BaseColor => {
                set.base_color.get_or_insert(texture_index);
                if texture_alpha_affects_material_transparency(&textures[texture_index]) {
                    set.has_alpha = true;
                }
            }
            WeaponModelTextureKind::Normal => {
                set.normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Mask => {
                set.mask.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Specular => {
                set.specular.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Emissive => {
                set.emissive.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MaterialProperties => {
                set.material_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Index => {
                set.index.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Other => {}
        }
    }

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        material.color_table.as_ref(),
        set.index,
        set.emissive.is_none(),
        shader_opacity_override(&material.shader_package_name),
        textures,
    ) {
        if let Some(base_color) = set.base_color {
            if let Some(combined) = combine_base_with_colorset_texture(
                material_path,
                base_color,
                baked.base_color,
                textures,
            ) {
                set.base_color = Some(combined);
                add_unique_index(&mut set.indices, combined);
            }
        } else {
            set.base_color = Some(baked.base_color);
            add_unique_index(&mut set.indices, baked.base_color);
        }

        if set.emissive.is_none() {
            if let Some(emissive) = baked.emissive {
                set.emissive = Some(emissive);
                add_unique_index(&mut set.indices, emissive);
            }
        }

        set.specular.get_or_insert(baked.specular);
        add_unique_index(&mut set.indices, baked.specular);
        set.material_properties
            .get_or_insert(baked.material_properties);
        add_unique_index(&mut set.indices, baked.material_properties);
    }

    if set.base_color.is_none() {
        set.base_color = choose_fallback_base_texture(&set.indices, textures);
    }

    set
}

#[cfg(feature = "game-data")]
fn load_weapon_texture_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    material_path: &str,
    raw_texture_path: &str,
    kind: WeaponModelTextureKind,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    use physis::ReadableFile;

    for path in weapon_texture_candidate_paths(material_path, raw_texture_path) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
            textures[index].kind = merge_texture_kind(textures[index].kind, kind);
            return Some(index);
        }

        let Some(bytes) = resource.read(&path) else {
            continue;
        };
        let Some(texture) = physis::tex::Texture::from_existing(resource.platform(), &bytes) else {
            continue;
        };
        let Some(rgba) = crate::texture_decode::decode_texture_rgba(&texture) else {
            continue;
        };
        let index = textures.len();
        textures.push(WeaponModelTexture {
            path: path.clone(),
            kind,
            width: texture.width,
            height: texture.height,
            rgba,
        });
        push_loaded_path(loaded_paths, path);
        return Some(index);
    }

    None
}

#[cfg(feature = "game-data")]
pub async fn load_weapon_model_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &WeaponModelLoadRequest,
) -> anyhow::Result<WeaponModelData> {
    use anyhow::anyhow;

    let model_main = request.primary_model();
    let model_sub = request.secondary_model();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();

    load_weapon_model_meshes_from_async_resource(
        resource,
        model_main,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
    )
    .await?;

    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            let _ = load_weapon_model_meshes_from_async_resource(
                resource,
                model_sub,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
            )
            .await;
        }
    }

    if meshes.is_empty() {
        return Err(anyhow!(
            "{} has no renderable model meshes",
            request.item_name
        ));
    }

    Ok(WeaponModelData {
        item_id: request.item_id,
        item_name: request.item_name.clone(),
        model_main,
        model_sub,
        loaded_paths,
        bounds: calculate_model_bounds(&meshes),
        materials,
        textures,
        meshes,
    })
}

#[cfg(feature = "game-data")]
async fn load_weapon_model_meshes_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    model: PackedModelId,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<WeaponModelMesh>,
) -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};

    let mut missing = Vec::new();
    for path in weapon_model_candidate_paths(model) {
        let bytes = match resource.read(&path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                missing.push(format!("{path}: {error}"));
                continue;
            }
        };

        let mut path_meshes = meshes_from_mdl_bytes(&path, &bytes)
            .with_context(|| format!("failed to load render meshes from {path}"))?;
        push_loaded_path(loaded_paths, path.clone());
        assign_weapon_materials_from_async_resource(
            resource,
            model,
            &path,
            &mut path_meshes,
            materials,
            textures,
            loaded_paths,
        )
        .await;
        meshes.append(&mut path_meshes);
        return Ok(());
    }

    Err(anyhow!(
        "unable to read weapon model {} (tried: {})",
        model.model_id,
        missing.join("; ")
    ))
}

#[cfg(feature = "game-data")]
async fn assign_weapon_materials_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    model: PackedModelId,
    model_path: &str,
    meshes: &mut [WeaponModelMesh],
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) {
    let mut slots = Vec::<(u16, usize)>::new();
    let mut material_specs = Vec::<(u16, String)>::new();
    for mesh in meshes.iter() {
        if !material_specs
            .iter()
            .any(|(index, _)| *index == mesh.material_index)
        {
            material_specs.push((mesh.material_index, mesh.material_name.clone()));
        }
    }

    for (material_index, material_name) in material_specs {
        let slot = materials.len();
        let material = load_weapon_material_from_async_resource(
            resource,
            model,
            model_path,
            material_index,
            material_name,
            slot,
            textures,
            loaded_paths,
        )
        .await;
        materials.push(material);
        slots.push((material_index, slot));
    }

    for mesh in meshes {
        if let Some((_, slot)) = slots
            .iter()
            .find(|(material_index, _)| *material_index == mesh.material_index)
        {
            mesh.material_slot = *slot;
        }
    }
}

#[cfg(feature = "game-data")]
async fn load_weapon_material_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    model: PackedModelId,
    model_path: &str,
    material_index: u16,
    material_name: String,
    slot: usize,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponModelMaterial {
    use physis::ReadableFile;

    let fallback = material_color(material_index);
    let candidates = weapon_material_candidate_paths(model, model_path, &material_name);
    for path in candidates {
        let Ok(bytes) = resource.read(&path).await else {
            continue;
        };
        let Some(material) = physis::mtrl::Material::from_existing(resource.platform(), &bytes)
        else {
            continue;
        };

        push_loaded_path(loaded_paths, path.clone());
        let shader_package_name = material.shader_package_name.clone();
        let summary = summarize_material_colors(material.color_table.as_ref(), fallback);
        let semantics = load_composed_material_semantics_from_async_resource(
            resource,
            &shader_package_name,
            &material,
            &bytes,
            loaded_paths,
        )
        .await;
        let sampler_roles = parse_material_sampler_roles(&bytes, &semantics);
        let shader_flags = parse_material_shader_flags(&bytes);
        let alpha_test = semantics.has_material_key(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON);
        let apply_vertex_color =
            semantics.has_material_key(APPLY_VERTEX_COLOR, APPLY_VERTEX_COLOR_ON);
        let material_alpha_threshold = composed_material_alpha_threshold(&semantics);
        let texture_set = load_weapon_material_textures_from_async_resource(
            resource,
            &path,
            &material,
            &sampler_roles,
            textures,
            loaded_paths,
        )
        .await;

        let alpha_mode = weapon_material_alpha_mode(
            &shader_package_name,
            shader_flags,
            &texture_set,
            alpha_test,
        );
        let alpha_threshold =
            material_alpha_threshold.unwrap_or_else(|| default_alpha_threshold(alpha_mode));
        let render_mode = weapon_material_render_mode(alpha_mode);
        let opacity = weapon_material_opacity(render_mode);
        let render_backfaces = material_render_backfaces(shader_flags);
        let diffuse_color = if texture_set.base_color.is_some() {
            [1.0, 1.0, 1.0]
        } else {
            summary.diffuse
        };
        let emissive_color = preview_emissive_color(summary.emissive, &texture_set);

        return WeaponModelMaterial {
            slot,
            material_index,
            name: material_name,
            path: Some(path),
            shader_package_name: Some(shader_package_name),
            render_mode,
            alpha_mode,
            alpha_threshold,
            opacity,
            render_backfaces,
            apply_vertex_color,
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            normal_texture: texture_set.normal,
            mask_texture: texture_set.mask,
            specular_texture: texture_set.specular,
            emissive_texture: texture_set.emissive,
            material_properties_texture: texture_set.material_properties,
        };
    }

    fallback_weapon_material(slot, material_index, material_name, fallback)
}

#[cfg(feature = "game-data")]
async fn load_weapon_material_textures_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    material_path: &str,
    material: &physis::mtrl::Material,
    sampler_roles: &[MaterialSamplerRole],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in material.texture_paths.iter().enumerate() {
        let kind = classify_weapon_texture(
            raw_texture_path,
            sampler_kind_for_texture(sampler_roles, texture_order),
        );
        let Some(texture_index) = load_weapon_texture_from_async_resource(
            resource,
            material_path,
            raw_texture_path,
            kind,
            textures,
            loaded_paths,
        )
        .await
        else {
            continue;
        };
        if !set.indices.contains(&texture_index) {
            set.indices.push(texture_index);
        }
        match textures[texture_index].kind {
            WeaponModelTextureKind::BaseColor => {
                set.base_color.get_or_insert(texture_index);
                if texture_alpha_affects_material_transparency(&textures[texture_index]) {
                    set.has_alpha = true;
                }
            }
            WeaponModelTextureKind::Normal => {
                set.normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Mask => {
                set.mask.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Specular => {
                set.specular.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Emissive => {
                set.emissive.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MaterialProperties => {
                set.material_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Index => {
                set.index.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Other => {}
        }
    }

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        material.color_table.as_ref(),
        set.index,
        set.emissive.is_none(),
        shader_opacity_override(&material.shader_package_name),
        textures,
    ) {
        if let Some(base_color) = set.base_color {
            if let Some(combined) = combine_base_with_colorset_texture(
                material_path,
                base_color,
                baked.base_color,
                textures,
            ) {
                set.base_color = Some(combined);
                add_unique_index(&mut set.indices, combined);
            }
        } else {
            set.base_color = Some(baked.base_color);
            add_unique_index(&mut set.indices, baked.base_color);
        }

        if set.emissive.is_none() {
            if let Some(emissive) = baked.emissive {
                set.emissive = Some(emissive);
                add_unique_index(&mut set.indices, emissive);
            }
        }

        set.specular.get_or_insert(baked.specular);
        add_unique_index(&mut set.indices, baked.specular);
        set.material_properties
            .get_or_insert(baked.material_properties);
        add_unique_index(&mut set.indices, baked.material_properties);
    }

    if set.base_color.is_none() {
        set.base_color = choose_fallback_base_texture(&set.indices, textures);
    }

    set
}

#[cfg(feature = "game-data")]
async fn load_weapon_texture_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    material_path: &str,
    raw_texture_path: &str,
    kind: WeaponModelTextureKind,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    use physis::ReadableFile;

    for path in weapon_texture_candidate_paths(material_path, raw_texture_path) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
            textures[index].kind = merge_texture_kind(textures[index].kind, kind);
            return Some(index);
        }

        let Ok(bytes) = resource.read(&path).await else {
            continue;
        };
        let Some(texture) = physis::tex::Texture::from_existing(resource.platform(), &bytes) else {
            continue;
        };
        let Some(rgba) = crate::texture_decode::decode_texture_rgba(&texture) else {
            continue;
        };
        let index = textures.len();
        textures.push(WeaponModelTexture {
            path: path.clone(),
            kind,
            width: texture.width,
            height: texture.height,
            rgba,
        });
        push_loaded_path(loaded_paths, path);
        return Some(index);
    }

    None
}

#[cfg(feature = "game-data")]
#[derive(Default)]
struct WeaponTextureSet {
    indices: Vec<usize>,
    base_color: Option<usize>,
    normal: Option<usize>,
    mask: Option<usize>,
    specular: Option<usize>,
    emissive: Option<usize>,
    material_properties: Option<usize>,
    index: Option<usize>,
    has_alpha: bool,
}

#[cfg(feature = "game-data")]
struct BakedWeaponTextureIndices {
    base_color: usize,
    specular: usize,
    material_properties: usize,
    emissive: Option<usize>,
}

#[cfg(feature = "game-data")]
fn texture_has_alpha(texture: &WeaponModelTexture) -> bool {
    texture.rgba.chunks_exact(4).any(|pixel| pixel[3] < 250)
}

#[cfg(feature = "game-data")]
fn texture_alpha_affects_material_transparency(texture: &WeaponModelTexture) -> bool {
    texture.kind == WeaponModelTextureKind::BaseColor && texture_has_alpha(texture)
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct MaterialSamplerRole {
    texture_index: usize,
    kind: WeaponModelTextureKind,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct MaterialSamplerRecord {
    texture_index: usize,
    texture_usage: u32,
    kind: Option<WeaponModelTextureKind>,
}

#[cfg(feature = "game-data")]
#[derive(Default)]
struct ComposedMaterialSemantics {
    material_keys: HashMap<u32, u32>,
    material_constants: HashMap<u32, Vec<f32>>,
    resource_names: HashMap<u32, String>,
}

#[cfg(feature = "game-data")]
impl ComposedMaterialSemantics {
    fn has_material_key(&self, key: u32, value: u32) -> bool {
        self.material_keys.get(&key).copied() == Some(value)
    }

    fn sampler_kind(&self, texture_usage: u32) -> Option<WeaponModelTextureKind> {
        self.resource_names
            .get(&texture_usage)
            .and_then(|name| classify_sampler_name(name))
            .or_else(|| classify_sampler_usage(texture_usage))
    }

    fn material_constant_first_f32(&self, constant_id: u32) -> Option<f32> {
        self.material_constants
            .get(&constant_id)
            .and_then(|values| values.first())
            .copied()
    }

    fn apply_shader_package(&mut self, shader_package: &physis::shpk::ShaderPackage) {
        for key in shader_package
            .material_keys
            .iter()
            .chain(shader_package.system_keys.iter())
            .chain(shader_package.scene_keys.iter())
        {
            self.apply_shader_package_key_default(key.id, key.default_value);
        }

        for parameter in shader_package
            .sampler_parameters
            .iter()
            .chain(shader_package.scalar_parameters.iter())
            .chain(shader_package.texture_parameters.iter())
            .chain(shader_package.uav_parameters.iter())
        {
            self.register_resource_parameter(parameter);
        }
    }

    fn apply_material(&mut self, material: &physis::mtrl::Material) {
        for key in &material.shader_keys {
            self.apply_material_key(key.category, key.value);
        }
    }

    fn apply_shader_package_material_constants(&mut self, bytes: &[u8]) {
        for (id, values) in shader_package_material_defaults(bytes) {
            self.material_constants.entry(id).or_insert(values);
        }
    }

    fn apply_material_constants(&mut self, bytes: &[u8]) {
        for (id, values) in material_constants(bytes) {
            self.material_constants.insert(id, values);
        }
    }

    fn apply_shader_package_key_default(&mut self, key: u32, value: u32) {
        self.material_keys.entry(key).or_insert(value);
    }

    fn apply_material_key(&mut self, key: u32, value: u32) {
        self.material_keys.insert(key, value);
    }

    fn register_resource_parameter(&mut self, parameter: &physis::shpk::ResourceParameter) {
        if parameter.slot == 2 {
            self.register_resource_name(parameter.name.clone());
        }
    }

    fn register_resource_name(&mut self, name: String) {
        let id = physis::shpk::ShaderPackage::crc(&name);
        self.resource_names.insert(id, name);
    }
}

#[cfg(feature = "game-data")]
fn load_composed_material_semantics_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    shader_package_name: &str,
    material: &physis::mtrl::Material,
    material_bytes: &[u8],
    loaded_paths: &mut Vec<String>,
) -> ComposedMaterialSemantics {
    use physis::ReadableFile;

    let mut semantics = ComposedMaterialSemantics::default();
    let path = normalize_game_resource_path(&format!("shader/sm5/shpk/{shader_package_name}"));
    if let Some(bytes) = resource.read(&path) {
        if let Some(shader_package) =
            physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
        {
            semantics.apply_shader_package(&shader_package);
            semantics.apply_shader_package_material_constants(&bytes);
            push_loaded_path(loaded_paths, path);
        }
    }
    semantics.apply_material(material);
    semantics.apply_material_constants(material_bytes);
    semantics
}

#[cfg(feature = "game-data")]
async fn load_composed_material_semantics_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    shader_package_name: &str,
    material: &physis::mtrl::Material,
    material_bytes: &[u8],
    loaded_paths: &mut Vec<String>,
) -> ComposedMaterialSemantics {
    use physis::ReadableFile;

    let mut semantics = ComposedMaterialSemantics::default();
    let path = normalize_game_resource_path(&format!("shader/sm5/shpk/{shader_package_name}"));
    if let Ok(bytes) = resource.read(&path).await {
        if let Some(shader_package) =
            physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
        {
            semantics.apply_shader_package(&shader_package);
            semantics.apply_shader_package_material_constants(&bytes);
            push_loaded_path(loaded_paths, path);
        }
    }
    semantics.apply_material(material);
    semantics.apply_material_constants(material_bytes);
    semantics
}

#[cfg(feature = "game-data")]
struct MaterialColorSummary {
    diffuse: [f32; 3],
    specular: [f32; 3],
    emissive: [f32; 3],
    roughness: f32,
    metalness: f32,
}

#[cfg(feature = "game-data")]
fn add_unique_index(indices: &mut Vec<usize>, index: usize) {
    if !indices.contains(&index) {
        indices.push(index);
    }
}

#[cfg(feature = "game-data")]
fn bake_weapon_color_table_textures(
    material_path: &str,
    color_table: Option<&physis::mtrl::ColorTable>,
    index_texture: Option<usize>,
    bake_emissive: bool,
    opacity_override: Option<f32>,
    textures: &mut Vec<WeaponModelTexture>,
) -> Option<BakedWeaponTextureIndices> {
    let rows = weapon_color_table_rows(color_table?)?;
    let index_texture = textures.get(index_texture?)?;
    let width = index_texture.width;
    let height = index_texture.height;
    let id_rgba = index_texture.rgba.clone();
    let mut baked = bake_color_table_maps(&rows, &id_rgba)?;
    if let Some(opacity) = opacity_override {
        apply_alpha_override(&mut baked.diffuse_rgba, opacity);
    }
    let material_key = normalize_game_resource_path(material_path);

    let base_path = format!("baked://{material_key}#colorset-diffuse");
    let base_color = push_or_replace_baked_texture(
        textures,
        base_path,
        WeaponModelTextureKind::BaseColor,
        width,
        height,
        baked.diffuse_rgba,
    );

    let specular = push_or_replace_baked_texture(
        textures,
        format!("baked://{material_key}#colorset-specular"),
        WeaponModelTextureKind::Specular,
        width,
        height,
        baked.specular_rgba,
    );

    let material_properties = push_or_replace_baked_texture(
        textures,
        format!("baked://{material_key}#colorset-material-properties"),
        WeaponModelTextureKind::MaterialProperties,
        width,
        height,
        baked.material_rgba,
    );

    let emissive = if bake_emissive {
        baked.emissive_rgba.map(|rgba| {
            push_or_replace_baked_texture(
                textures,
                format!("baked://{material_key}#colorset-emissive"),
                WeaponModelTextureKind::Emissive,
                width,
                height,
                rgba,
            )
        })
    } else {
        None
    };

    Some(BakedWeaponTextureIndices {
        base_color,
        specular,
        material_properties,
        emissive,
    })
}

#[cfg(feature = "game-data")]
fn weapon_material_render_mode(alpha_mode: WeaponMaterialAlphaMode) -> WeaponMaterialRenderMode {
    match alpha_mode {
        WeaponMaterialAlphaMode::Opaque | WeaponMaterialAlphaMode::Mask => {
            WeaponMaterialRenderMode::Opaque
        }
        WeaponMaterialAlphaMode::Blend => WeaponMaterialRenderMode::Transparent,
        WeaponMaterialAlphaMode::Glass => WeaponMaterialRenderMode::Glass,
    }
}

#[cfg(feature = "game-data")]
fn weapon_material_alpha_mode(
    shader_package_name: &str,
    shader_flags: u32,
    texture_set: &WeaponTextureSet,
    alpha_test: bool,
) -> WeaponMaterialAlphaMode {
    const ENABLE_TRANSLUCENCY: u32 = 0x10;
    let shader = shader_package_name.to_ascii_lowercase();
    if shader.contains("glass") {
        WeaponMaterialAlphaMode::Glass
    } else if shader_flags & ENABLE_TRANSLUCENCY != 0 {
        WeaponMaterialAlphaMode::Blend
    } else if alpha_test || texture_set.has_alpha {
        WeaponMaterialAlphaMode::Mask
    } else {
        WeaponMaterialAlphaMode::Opaque
    }
}

#[cfg(feature = "game-data")]
fn weapon_material_opacity(mode: WeaponMaterialRenderMode) -> f32 {
    match mode {
        WeaponMaterialRenderMode::Opaque => 1.0,
        WeaponMaterialRenderMode::Transparent => 1.0,
        WeaponMaterialRenderMode::Glass => 0.28,
    }
}

#[cfg(feature = "game-data")]
fn material_render_backfaces(shader_flags: u32) -> bool {
    const HIDE_BACKFACES: u32 = 0x01;
    shader_flags & HIDE_BACKFACES == 0
}

#[cfg(feature = "game-data")]
fn default_alpha_threshold(_mode: WeaponMaterialAlphaMode) -> f32 {
    0.0
}

#[cfg(feature = "game-data")]
fn composed_material_alpha_threshold(semantics: &ComposedMaterialSemantics) -> Option<f32> {
    semantics
        .material_constant_first_f32(G_ALPHA_THRESHOLD)
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
}

#[cfg(feature = "game-data")]
fn shader_opacity_override(shader_package_name: &str) -> Option<f32> {
    let alpha_mode =
        weapon_material_alpha_mode(shader_package_name, 0, &WeaponTextureSet::default(), false);
    let mode = weapon_material_render_mode(alpha_mode);
    (mode == WeaponMaterialRenderMode::Glass).then_some(weapon_material_opacity(mode))
}

#[cfg(feature = "game-data")]
fn apply_alpha_override(rgba: &mut [u8], opacity: f32) {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[3] = pixel[3].min(alpha);
    }
}

#[cfg(feature = "game-data")]
fn combine_base_with_colorset_texture(
    material_path: &str,
    base_index: usize,
    colorset_index: usize,
    textures: &mut Vec<WeaponModelTexture>,
) -> Option<usize> {
    let base = textures.get(base_index)?.clone();
    let colorset = textures.get(colorset_index)?.clone();
    let width = colorset.width.max(1) as usize;
    let height = colorset.height.max(1) as usize;
    let base_width = base.width.max(1) as usize;
    let base_height = base.height.max(1) as usize;

    let mut rgba = Vec::with_capacity(colorset.rgba.len());
    for y in 0..height {
        let base_y = y * base_height / height;
        for x in 0..width {
            let base_x = x * base_width / width;
            let base_offset = (base_y * base_width + base_x) * 4;
            let colorset_offset = (y * width + x) * 4;
            let base = base.rgba.get(base_offset..base_offset + 4)?;
            let colorset = colorset.rgba.get(colorset_offset..colorset_offset + 4)?;
            rgba.push(multiply_srgb_channels(base[0], colorset[0]));
            rgba.push(multiply_srgb_channels(base[1], colorset[1]));
            rgba.push(multiply_srgb_channels(base[2], colorset[2]));
            rgba.push(((u16::from(base[3]) * u16::from(colorset[3])) / 255) as u8);
        }
    }

    Some(push_or_replace_baked_texture(
        textures,
        format!("baked://{material_path}#base-times-colorset"),
        WeaponModelTextureKind::BaseColor,
        colorset.width,
        colorset.height,
        rgba,
    ))
}

#[cfg(feature = "game-data")]
fn multiply_srgb_channels(a: u8, b: u8) -> u8 {
    linear_to_srgb_u8(srgb_u8_to_linear(a) * srgb_u8_to_linear(b))
}

#[cfg(feature = "game-data")]
fn srgb_u8_to_linear(value: u8) -> f32 {
    let srgb = f32::from(value) / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(feature = "game-data")]
fn linear_to_srgb_u8(value: f32) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let srgb = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (srgb.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(feature = "game-data")]
fn push_or_replace_baked_texture(
    textures: &mut Vec<WeaponModelTexture>,
    path: String,
    kind: WeaponModelTextureKind,
    width: u16,
    height: u16,
    rgba: Vec<u8>,
) -> usize {
    if let Some(index) = textures.iter().position(|texture| texture.path == path) {
        textures[index] = WeaponModelTexture {
            path,
            kind,
            width,
            height,
            rgba,
        };
        return index;
    }

    let index = textures.len();
    textures.push(WeaponModelTexture {
        path,
        kind,
        width,
        height,
        rgba,
    });
    index
}

#[cfg(feature = "game-data")]
fn weapon_color_table_rows(
    color_table: &physis::mtrl::ColorTable,
) -> Option<Vec<ColorTableRowColors>> {
    match color_table {
        physis::mtrl::ColorTable::DawntrailColorTable(table) => Some(
            table
                .rows
                .iter()
                .map(|row| ColorTableRowColors {
                    diffuse: row.diffuse_color,
                    specular: row.specular_color,
                    emissive: row.emissive_color,
                    gloss_strength: row.unknown1,
                    specular_strength: row.unknown2,
                    roughness: row.roughness,
                    metalness: row.metalness,
                    tile_alpha: row.tile_alpha,
                })
                .collect(),
        ),
        physis::mtrl::ColorTable::LegacyColorTable(_)
        | physis::mtrl::ColorTable::OpaqueColorTable(_) => None,
    }
}

#[cfg(feature = "game-data")]
fn choose_fallback_base_texture(
    indices: &[usize],
    textures: &[WeaponModelTexture],
) -> Option<usize> {
    indices.iter().copied().find(|index| {
        textures
            .get(*index)
            .is_some_and(|texture| texture.kind == WeaponModelTextureKind::Other)
    })
}

#[cfg(feature = "game-data")]
fn preview_emissive_color(emissive: [f32; 3], texture_set: &WeaponTextureSet) -> [f32; 3] {
    let scale = if texture_set.emissive.is_some() {
        1.0
    } else if texture_set.mask.is_some() {
        0.25
    } else {
        0.0
    };
    [
        emissive[0].clamp(0.0, 4.0) * scale,
        emissive[1].clamp(0.0, 4.0) * scale,
        emissive[2].clamp(0.0, 4.0) * scale,
    ]
}

#[cfg(feature = "game-data")]
fn summarize_material_colors(
    color_table: Option<&physis::mtrl::ColorTable>,
    fallback: [f32; 3],
) -> MaterialColorSummary {
    let mut diffuse = ColorAccumulator::default();
    let mut specular = ColorAccumulator::default();
    let mut emissive = [0.0; 3];
    let mut roughness_total = 0.0;
    let mut metalness_total = 0.0;
    let mut physical_rows = 0_u32;

    match color_table {
        Some(physis::mtrl::ColorTable::LegacyColorTable(table)) => {
            for row in &table.rows {
                diffuse.add_nonzero(row.diffuse_color);
                specular.add_nonzero(row.specular_color);
                emissive = brighter_color(emissive, row.emissive_color);
            }
        }
        Some(physis::mtrl::ColorTable::DawntrailColorTable(table)) => {
            for row in &table.rows {
                diffuse.add_nonzero(row.diffuse_color);
                specular.add_nonzero(row.specular_color);
                emissive = brighter_color(emissive, row.emissive_color);
                if row.roughness.is_finite() && row.metalness.is_finite() {
                    roughness_total += row.roughness.clamp(0.0, 1.0);
                    metalness_total += row.metalness.clamp(0.0, 1.0);
                    physical_rows += 1;
                }
            }
        }
        Some(physis::mtrl::ColorTable::OpaqueColorTable(_)) | None => {}
    }

    MaterialColorSummary {
        diffuse: diffuse.average().unwrap_or(fallback),
        specular: specular.average().unwrap_or([0.45, 0.45, 0.45]),
        emissive,
        roughness: if physical_rows == 0 {
            0.5
        } else {
            roughness_total / physical_rows as f32
        },
        metalness: if physical_rows == 0 {
            0.0
        } else {
            metalness_total / physical_rows as f32
        },
    }
}

#[cfg(feature = "game-data")]
#[derive(Default)]
struct ColorAccumulator {
    total: [f32; 3],
    count: u32,
}

#[cfg(feature = "game-data")]
impl ColorAccumulator {
    fn add_nonzero(&mut self, color: [f32; 3]) {
        if color.iter().any(|value| value.abs() > 0.0001) {
            for (slot, value) in self.total.iter_mut().zip(color) {
                *slot += value;
            }
            self.count += 1;
        }
    }

    fn average(&self) -> Option<[f32; 3]> {
        (self.count != 0).then(|| {
            [
                self.total[0] / self.count as f32,
                self.total[1] / self.count as f32,
                self.total[2] / self.count as f32,
            ]
        })
    }
}

#[cfg(feature = "game-data")]
fn fallback_weapon_material(
    slot: usize,
    material_index: u16,
    name: String,
    fallback: [f32; 3],
) -> WeaponModelMaterial {
    WeaponModelMaterial {
        slot,
        material_index,
        name,
        path: None,
        shader_package_name: None,
        render_mode: WeaponMaterialRenderMode::Opaque,
        alpha_mode: WeaponMaterialAlphaMode::Opaque,
        alpha_threshold: 0.0,
        opacity: 1.0,
        render_backfaces: true,
        apply_vertex_color: false,
        fallback_color: fallback,
        diffuse_color: fallback,
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        normal_texture: None,
        mask_texture: None,
        specular_texture: None,
        emissive_texture: None,
        material_properties_texture: None,
    }
}

#[cfg(feature = "game-data")]
fn brighter_color(current: [f32; 3], candidate: [f32; 3]) -> [f32; 3] {
    let current_luma = current[0] * 0.2126 + current[1] * 0.7152 + current[2] * 0.0722;
    let candidate_luma = candidate[0] * 0.2126 + candidate[1] * 0.7152 + candidate[2] * 0.0722;
    if candidate_luma > current_luma {
        candidate
    } else {
        current
    }
}

#[cfg(feature = "game-data")]
fn parse_material_sampler_roles(
    bytes: &[u8],
    semantics: &ComposedMaterialSemantics,
) -> Vec<MaterialSamplerRole> {
    parse_material_sampler_records(bytes, semantics)
        .into_iter()
        .filter_map(|record| {
            record.kind.map(|kind| MaterialSamplerRole {
                texture_index: record.texture_index,
                kind,
            })
        })
        .collect()
}

#[cfg(feature = "game-data")]
fn parse_material_sampler_records(
    bytes: &[u8],
    semantics: &ComposedMaterialSemantics,
) -> Vec<MaterialSamplerRecord> {
    let Some(layout) = material_shader_table_layout(bytes) else {
        return Vec::new();
    };
    let mut sampler_offset = layout.sampler_offset;

    let mut records = Vec::new();
    for _ in 0..layout.sampler_count {
        let Some(texture_usage) = read_u32_le(bytes, sampler_offset) else {
            return records;
        };
        let Some(texture_index) = bytes.get(sampler_offset + 8).copied().map(usize::from) else {
            return records;
        };
        if texture_index < layout.texture_count {
            records.push(MaterialSamplerRecord {
                texture_index,
                texture_usage,
                kind: semantics.sampler_kind(texture_usage),
            });
        }
        let Some(next) = checked_advance(sampler_offset, 12, bytes.len()) else {
            return records;
        };
        sampler_offset = next;
    }

    records
}

#[cfg(feature = "game-data")]
fn parse_material_shader_flags(bytes: &[u8]) -> u32 {
    material_shader_table_layout(bytes)
        .and_then(|layout| read_u32_le(bytes, layout.table_offset + 8))
        .unwrap_or(0)
}

#[cfg(feature = "game-data")]
fn material_constants(bytes: &[u8]) -> Vec<(u32, Vec<f32>)> {
    let Some(layout) = material_shader_table_layout(bytes) else {
        return Vec::new();
    };
    let mut constant_offset = layout.constant_offset;
    let mut constants = Vec::new();

    for _ in 0..layout.constant_count {
        let Some(id) = read_u32_le(bytes, constant_offset) else {
            return constants;
        };
        let Some(value_offset) = read_u16_le(bytes, constant_offset + 4).map(usize::from) else {
            return constants;
        };
        let Some(value_size) = read_u16_le(bytes, constant_offset + 6).map(usize::from) else {
            return constants;
        };
        if value_size >= 4 {
            let value_start = match layout.shader_values_offset.checked_add(value_offset) {
                Some(value_start) => value_start,
                None => return constants,
            };
            if let Some(values) = read_f32_values(bytes, value_start, value_size / 4) {
                constants.push((id, values));
            }
        }
        let Some(next) = checked_advance(constant_offset, 8, bytes.len()) else {
            return constants;
        };
        constant_offset = next;
    }

    constants
}

#[cfg(feature = "game-data")]
fn shader_package_material_defaults(bytes: &[u8]) -> Vec<(u32, Vec<f32>)> {
    let Some(layout) = shader_package_material_defaults_layout(bytes) else {
        return Vec::new();
    };

    let mut constants = Vec::new();
    let mut parameter_offset = layout.parameter_offset;
    let defaults_offset = layout.defaults_offset;

    for _ in 0..layout.parameter_count {
        let Some(id) = read_u32_le(bytes, parameter_offset) else {
            return constants;
        };
        let Some(byte_offset) = read_u16_le(bytes, parameter_offset + 4).map(usize::from) else {
            return constants;
        };
        let Some(byte_size) = read_u16_le(bytes, parameter_offset + 6).map(usize::from) else {
            return constants;
        };
        if byte_size >= 4 && byte_offset.saturating_add(byte_size) <= layout.defaults_size {
            let value_start = match defaults_offset.checked_add(byte_offset) {
                Some(value_start) => value_start,
                None => return constants,
            };
            if let Some(values) = read_f32_values(bytes, value_start, byte_size / 4) {
                constants.push((id, values));
            }
        }
        let Some(next) = checked_advance(parameter_offset, 8, bytes.len()) else {
            return constants;
        };
        parameter_offset = next;
    }

    constants
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct ShaderPackageMaterialDefaultsLayout {
    parameter_offset: usize,
    defaults_offset: usize,
    defaults_size: usize,
    parameter_count: usize,
}

#[cfg(feature = "game-data")]
fn shader_package_material_defaults_layout(
    bytes: &[u8],
) -> Option<ShaderPackageMaterialDefaultsLayout> {
    if bytes.get(0..4)? != b"ShPk" {
        return None;
    }

    let version = read_u32_le(bytes, 4)?;
    let vertex_shader_count = read_u32_usize(bytes, 24)?;
    let pixel_shader_count = read_u32_usize(bytes, 28)?;
    let defaults_size = read_u32_usize(bytes, 32)?;
    let parameter_count = read_u16_le(bytes, 36).map(usize::from)?;
    let has_defaults = read_u16_le(bytes, 38)? != 0;
    if !has_defaults || defaults_size == 0 || parameter_count == 0 {
        return None;
    }

    let mut offset = 72_usize;
    if version >= 0x0D01 {
        offset = checked_advance(offset, 12, bytes.len())?;
    }
    if version >= 0x0E01 {
        offset = checked_advance(offset, 4, bytes.len())?;
    }

    for _ in 0..vertex_shader_count.saturating_add(pixel_shader_count) {
        offset = shader_package_skip_shader(bytes, offset, version)?;
    }

    let parameter_offset = offset;
    let defaults_offset = checked_advance(
        parameter_offset,
        parameter_count.saturating_mul(8),
        bytes.len(),
    )?;
    checked_advance(defaults_offset, defaults_size, bytes.len())?;

    Some(ShaderPackageMaterialDefaultsLayout {
        parameter_offset,
        defaults_offset,
        defaults_size,
        parameter_count,
    })
}

#[cfg(feature = "game-data")]
fn shader_package_skip_shader(bytes: &[u8], offset: usize, version: u32) -> Option<usize> {
    let scalar_count = read_u16_le(bytes, offset + 8).map(usize::from)?;
    let resource_count = read_u16_le(bytes, offset + 10).map(usize::from)?;
    let uav_count = read_u16_le(bytes, offset + 12).map(usize::from)?;
    let texture_count = read_u16_le(bytes, offset + 14).map(usize::from)?;
    let header_size = if version >= 0x0D01 { 20 } else { 16 };
    let parameter_count = scalar_count
        .saturating_add(resource_count)
        .saturating_add(uav_count)
        .saturating_add(texture_count);
    let offset = checked_advance(offset, header_size, bytes.len())?;
    checked_advance(offset, parameter_count.saturating_mul(16), bytes.len())
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct MaterialShaderTableLayout {
    texture_count: usize,
    table_offset: usize,
    constant_offset: usize,
    sampler_offset: usize,
    shader_values_offset: usize,
    constant_count: usize,
    sampler_count: usize,
}

#[cfg(feature = "game-data")]
fn material_shader_table_layout(bytes: &[u8]) -> Option<MaterialShaderTableLayout> {
    let Some(texture_count) = bytes.get(12).copied().map(usize::from) else {
        return None;
    };
    let Some(uv_set_count) = bytes.get(13).copied().map(usize::from) else {
        return None;
    };
    let Some(color_set_count) = bytes.get(14).copied().map(usize::from) else {
        return None;
    };
    let Some(additional_data_size) = bytes.get(15).copied().map(usize::from) else {
        return None;
    };
    let Some(data_set_size) = read_u16_le(bytes, 6).map(usize::from) else {
        return None;
    };
    let Some(string_table_size) = read_u16_le(bytes, 8).map(usize::from) else {
        return None;
    };

    let mut offset = 16_usize;
    for byte_count in [
        texture_count.saturating_mul(4),
        uv_set_count.saturating_mul(4),
        color_set_count.saturating_mul(4),
        string_table_size,
    ] {
        let Some(next) = checked_advance(offset, byte_count, bytes.len()) else {
            return None;
        };
        offset = next;
    }

    let Some(next) = checked_advance(offset, additional_data_size, bytes.len()) else {
        return None;
    };
    offset = next;

    let Some(next) = checked_advance(offset, data_set_size, bytes.len()) else {
        return None;
    };
    offset = next;

    let shader_value_list_size = read_u16_le(bytes, offset).map(usize::from)?;
    let shader_key_count = read_u16_le(bytes, offset + 2).map(usize::from)?;
    let constant_count = read_u16_le(bytes, offset + 4).map(usize::from)?;
    let sampler_count = read_u16_le(bytes, offset + 6).map(usize::from)?;
    let constant_offset =
        checked_advance(offset, 12 + shader_key_count.saturating_mul(8), bytes.len())?;
    let sampler_offset = checked_advance(
        constant_offset,
        constant_count.saturating_mul(8),
        bytes.len(),
    )?;
    let shader_values_offset = checked_advance(
        sampler_offset,
        sampler_count.saturating_mul(12),
        bytes.len(),
    )?;
    checked_advance(shader_values_offset, shader_value_list_size, bytes.len())?;

    Some(MaterialShaderTableLayout {
        texture_count,
        table_offset: offset,
        constant_offset,
        sampler_offset,
        shader_values_offset,
        constant_count,
        sampler_count,
    })
}

#[cfg(feature = "game-data")]
fn sampler_kind_for_texture(
    sampler_roles: &[MaterialSamplerRole],
    texture_index: usize,
) -> Option<WeaponModelTextureKind> {
    sampler_roles
        .iter()
        .find(|role| role.texture_index == texture_index)
        .map(|role| role.kind)
}

#[cfg(feature = "game-data")]
fn classify_sampler_usage(texture_usage: u32) -> Option<WeaponModelTextureKind> {
    known_sampler_names()
        .iter()
        .find(|(name, _)| physis::shpk::ShaderPackage::crc(name) == texture_usage)
        .map(|(_, kind)| *kind)
}

#[cfg(feature = "game-data")]
fn classify_sampler_name(name: &str) -> Option<WeaponModelTextureKind> {
    known_sampler_names()
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, kind)| *kind)
}

#[cfg(feature = "game-data")]
fn known_sampler_names() -> &'static [(&'static str, WeaponModelTextureKind)] {
    &[
        ("g_SamplerNormal", WeaponModelTextureKind::Normal),
        ("g_NormalSampler", WeaponModelTextureKind::Normal),
        ("g_SamplerNormalMap", WeaponModelTextureKind::Normal),
        ("g_NormalMapSampler", WeaponModelTextureKind::Normal),
        ("g_SamplerNormalMap0", WeaponModelTextureKind::Normal),
        ("g_SamplerNormalMap1", WeaponModelTextureKind::Normal),
        ("g_SamplerEmissive", WeaponModelTextureKind::Emissive),
        ("g_EmissiveSampler", WeaponModelTextureKind::Emissive),
        ("g_SamplerEmission", WeaponModelTextureKind::Emissive),
        ("g_EmissionSampler", WeaponModelTextureKind::Emissive),
        ("g_SamplerLight", WeaponModelTextureKind::Emissive),
        ("g_LightSampler", WeaponModelTextureKind::Emissive),
        ("g_SamplerIndex", WeaponModelTextureKind::Index),
        ("g_IndexSampler", WeaponModelTextureKind::Index),
        ("g_SamplerMask", WeaponModelTextureKind::Mask),
        ("g_MaskSampler", WeaponModelTextureKind::Mask),
        ("g_SamplerMaterial", WeaponModelTextureKind::Mask),
        ("g_MaterialSampler", WeaponModelTextureKind::Mask),
        ("g_SamplerMulti", WeaponModelTextureKind::Mask),
        ("g_MultiSampler", WeaponModelTextureKind::Mask),
        ("g_SamplerSpecular", WeaponModelTextureKind::Specular),
        ("g_SpecularSampler", WeaponModelTextureKind::Specular),
        ("g_SamplerSpecularMap", WeaponModelTextureKind::Specular),
        ("g_SpecularMapSampler", WeaponModelTextureKind::Specular),
        ("g_SamplerSpecularMap0", WeaponModelTextureKind::Specular),
        ("g_SamplerReflect", WeaponModelTextureKind::Specular),
        ("g_ReflectSampler", WeaponModelTextureKind::Specular),
        ("g_SamplerDiffuse", WeaponModelTextureKind::BaseColor),
        ("g_DiffuseSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColor", WeaponModelTextureKind::BaseColor),
        ("g_ColorSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColorMap", WeaponModelTextureKind::BaseColor),
        ("g_ColorMapSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColorMap0", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColorMap1", WeaponModelTextureKind::BaseColor),
        ("g_SamplerAlbedo", WeaponModelTextureKind::BaseColor),
        ("g_AlbedoSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerBaseColor", WeaponModelTextureKind::BaseColor),
        ("g_BaseColorSampler", WeaponModelTextureKind::BaseColor),
        ("g_Sampler0", WeaponModelTextureKind::BaseColor),
        ("g_Sampler1", WeaponModelTextureKind::BaseColor),
        ("g_SamplerEnvMap", WeaponModelTextureKind::Specular),
    ]
}

#[cfg(feature = "game-data")]
fn classify_weapon_texture(
    path: &str,
    sampler_kind: Option<WeaponModelTextureKind>,
) -> WeaponModelTextureKind {
    let path = path.to_ascii_lowercase();
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path.as_str())
        .trim_end_matches(".tex");

    if stem.ends_with("_id") || stem.contains("_id_") || stem.contains("index") {
        return WeaponModelTextureKind::Index;
    }

    if let Some(kind) = sampler_kind {
        return kind;
    }

    if stem.ends_with("_n") || stem.contains("_n_") || stem.contains("normal") {
        WeaponModelTextureKind::Normal
    } else if stem.ends_with("_s") || stem.contains("_s_") || stem.contains("spec") {
        WeaponModelTextureKind::Specular
    } else if stem.ends_with("_m") || stem.contains("_m_") || stem.contains("mask") {
        WeaponModelTextureKind::Mask
    } else if stem.ends_with("_e") || stem.contains("_e_") || stem.contains("emit") {
        WeaponModelTextureKind::Emissive
    } else if stem.ends_with("_a")
        || stem.contains("_a_")
        || stem.ends_with("_d")
        || stem.contains("_d_")
        || stem.contains("albedo")
        || stem.contains("diff")
        || stem.contains("base")
    {
        WeaponModelTextureKind::BaseColor
    } else {
        WeaponModelTextureKind::Other
    }
}

#[cfg(feature = "game-data")]
fn merge_texture_kind(
    existing: WeaponModelTextureKind,
    incoming: WeaponModelTextureKind,
) -> WeaponModelTextureKind {
    match (existing, incoming) {
        (WeaponModelTextureKind::Other, kind) => kind,
        (kind, WeaponModelTextureKind::Other) => kind,
        (WeaponModelTextureKind::Mask, WeaponModelTextureKind::Index) => {
            WeaponModelTextureKind::Index
        }
        (WeaponModelTextureKind::Index, WeaponModelTextureKind::Mask) => {
            WeaponModelTextureKind::Index
        }
        (kind, _) => kind,
    }
}

#[cfg(feature = "game-data")]
fn checked_advance(offset: usize, byte_count: usize, len: usize) -> Option<usize> {
    let next = offset.checked_add(byte_count)?;
    (next <= len).then_some(next)
}

#[cfg(feature = "game-data")]
fn read_u16_le(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(feature = "game-data")]
fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(feature = "game-data")]
fn read_u32_usize(bytes: &[u8], offset: usize) -> Option<usize> {
    usize::try_from(read_u32_le(bytes, offset)?).ok()
}

#[cfg(feature = "game-data")]
fn read_f32_le(bytes: &[u8], offset: usize) -> Option<f32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(f32::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(feature = "game-data")]
fn read_f32_values(bytes: &[u8], offset: usize, count: usize) -> Option<Vec<f32>> {
    let byte_count = count.checked_mul(4)?;
    checked_advance(offset, byte_count, bytes.len())?;
    let mut values = Vec::with_capacity(count);
    for index in 0..count {
        values.push(read_f32_le(bytes, offset + index * 4)?);
    }
    Some(values)
}

#[cfg(feature = "game-data")]
fn weapon_texture_candidate_paths(material_path: &str, texture_path: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let texture_path = normalize_game_resource_path(texture_path);
    if texture_path.is_empty() {
        return candidates;
    }

    push_unique_path(&mut candidates, texture_path.clone());
    if texture_path.starts_with("chara/")
        || texture_path.starts_with("bg/")
        || texture_path.starts_with("ui/")
        || texture_path.starts_with("common/")
    {
        return candidates;
    }

    let material_path = normalize_game_resource_path(material_path);
    let texture_file = texture_path
        .rsplit('/')
        .next()
        .unwrap_or(texture_path.as_str());
    if let Some((object_root, material_tail)) = material_path.split_once("/material/") {
        let texture_root = format!("{object_root}/texture");
        if let Some((version, _)) = material_tail.split_once('/') {
            if version.starts_with('v') {
                push_unique_path(
                    &mut candidates,
                    format!("{texture_root}/{version}/{texture_file}"),
                );
            }
        }
        push_unique_path(&mut candidates, format!("{texture_root}/{texture_file}"));
    }

    if let Some((material_dir, _)) = material_path.rsplit_once('/') {
        push_unique_path(&mut candidates, format!("{material_dir}/{texture_file}"));
    }

    candidates
}

#[cfg(feature = "game-data")]
fn normalize_game_resource_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/").to_ascii_lowercase()
}

#[cfg(feature = "game-data")]
fn push_loaded_path(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(feature = "game-data")]
fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !path.is_empty() && !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

#[cfg(all(test, feature = "game-data"))]
mod weapon_material_tests {
    use super::*;

    #[test]
    fn parse_material_sampler_roles_uses_sampler_texture_index() {
        let mut bytes = vec![0; 16];
        bytes[12] = 2;
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&physis::shpk::ShaderPackage::crc("g_SamplerNormal").to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&[0; 3]);

        let roles = parse_material_sampler_roles(&bytes, &ComposedMaterialSemantics::default());

        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].texture_index, 1);
        assert_eq!(roles[0].kind, WeaponModelTextureKind::Normal);
    }

    #[test]
    fn composed_material_semantics_material_key_overrides_shader_package_default() {
        let mut semantics = ComposedMaterialSemantics::default();

        semantics.apply_shader_package_key_default(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_OFF);
        assert!(!semantics.has_material_key(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON));

        semantics.apply_material_key(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON);
        assert!(semantics.has_material_key(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON));
    }

    #[test]
    fn parse_material_sampler_roles_uses_composed_resource_names() {
        let sampler_name = "G_SAMPLERINDEX";
        let texture_usage = physis::shpk::ShaderPackage::crc(sampler_name);
        assert_eq!(classify_sampler_usage(texture_usage), None);

        let mut semantics = ComposedMaterialSemantics::default();
        semantics.register_resource_name(sampler_name.to_string());

        let mut bytes = vec![0; 16];
        bytes[12] = 1;
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&texture_usage.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0; 3]);

        let roles = parse_material_sampler_roles(&bytes, &semantics);

        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].texture_index, 0);
        assert_eq!(roles[0].kind, WeaponModelTextureKind::Index);
    }

    #[test]
    fn material_constants_read_shader_constant_values() {
        let bytes = test_mtrl_with_constant(G_ALPHA_THRESHOLD, &[0.42], 0);

        assert_eq!(
            material_constants(&bytes),
            vec![(G_ALPHA_THRESHOLD, vec![0.42])]
        );
    }

    #[test]
    fn material_shader_table_layout_uses_header_dataset_size() {
        let bytes = test_mtrl_with_constant(G_ALPHA_THRESHOLD, &[0.25], 8);

        assert_eq!(
            material_constants(&bytes),
            vec![(G_ALPHA_THRESHOLD, vec![0.25])]
        );
    }

    #[test]
    fn shader_package_material_defaults_read_default_constants() {
        let bytes = test_shpk_with_material_defaults(&[(G_ALPHA_THRESHOLD, &[0.35])]);

        assert_eq!(
            shader_package_material_defaults(&bytes),
            vec![(G_ALPHA_THRESHOLD, vec![0.35])]
        );
    }

    #[test]
    fn composed_material_semantics_material_constant_overrides_shader_package_default() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[(G_ALPHA_THRESHOLD, &[0.2])]);
        let material = test_mtrl_with_constant(G_ALPHA_THRESHOLD, &[0.7], 0);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_alpha_threshold(&semantics), Some(0.2));

        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_threshold(&semantics), Some(0.7));
    }

    #[test]
    fn classify_weapon_texture_recognizes_ffxiv_albedo_suffix() {
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w1758/obj/body/b0001/texture/w1758b0001_a.tex",
                None
            ),
            WeaponModelTextureKind::BaseColor
        );
    }

    #[test]
    fn classify_weapon_texture_recognizes_colorset_index_map() {
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w0525/obj/body/b0001/texture/v01_w0525b0001_id.tex",
                None
            ),
            WeaponModelTextureKind::Index
        );
    }

    #[test]
    fn fallback_base_texture_ignores_specialized_maps() {
        let textures = vec![
            test_texture("emissive.tex", WeaponModelTextureKind::Emissive),
            test_texture("normal.tex", WeaponModelTextureKind::Normal),
            test_texture("mask.tex", WeaponModelTextureKind::Mask),
            test_texture("specular.tex", WeaponModelTextureKind::Specular),
            test_texture("id.tex", WeaponModelTextureKind::Index),
        ];
        assert_eq!(choose_fallback_base_texture(&[0, 1, 2, 3], &textures), None);
    }

    #[test]
    fn fallback_base_texture_allows_unknown_maps() {
        let textures = vec![
            test_texture("normal.tex", WeaponModelTextureKind::Normal),
            test_texture("unknown.tex", WeaponModelTextureKind::Other),
        ];
        assert_eq!(choose_fallback_base_texture(&[0, 1], &textures), Some(1));
    }

    #[test]
    fn only_base_texture_alpha_affects_material_transparency() {
        for kind in [
            WeaponModelTextureKind::Normal,
            WeaponModelTextureKind::Mask,
            WeaponModelTextureKind::Specular,
            WeaponModelTextureKind::Emissive,
            WeaponModelTextureKind::Index,
            WeaponModelTextureKind::Other,
        ] {
            let texture = test_texture_with_alpha("non-base-alpha.tex", kind, 0);
            assert!(!texture_alpha_affects_material_transparency(&texture));
        }

        let opaque_base =
            test_texture_with_alpha("base-opaque.tex", WeaponModelTextureKind::BaseColor, 255);
        assert!(!texture_alpha_affects_material_transparency(&opaque_base));

        let alpha_base =
            test_texture_with_alpha("base-alpha.tex", WeaponModelTextureKind::BaseColor, 128);
        assert!(texture_alpha_affects_material_transparency(&alpha_base));
    }

    #[test]
    fn srgb_multiply_uses_linear_space() {
        assert_eq!(multiply_srgb_channels(255, 128), 128);
        assert_ne!(multiply_srgb_channels(128, 128), 64);
    }

    #[test]
    fn submesh_index_ranges_are_made_part_local() {
        let ranges = normalize_submesh_index_ranges(12, [(0, 100, 6), (1, 106, 6), (2, 112, 3)]);

        assert_eq!(
            ranges,
            vec![
                MeshIndexRange {
                    submesh_index: Some(0),
                    start: 0,
                    end: 6,
                },
                MeshIndexRange {
                    submesh_index: Some(1),
                    start: 6,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn submesh_index_ranges_accept_already_local_offsets() {
        let ranges = normalize_submesh_index_ranges(12, [(0, 0, 3), (1, 3, 9)]);

        assert_eq!(
            ranges,
            vec![
                MeshIndexRange {
                    submesh_index: Some(0),
                    start: 0,
                    end: 3,
                },
                MeshIndexRange {
                    submesh_index: Some(1),
                    start: 3,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn remap_mesh_vertices_keeps_submesh_vertex_order() {
        let vertices = vec![
            test_vertex(0.0),
            test_vertex(1.0),
            test_vertex(2.0),
            test_vertex(3.0),
        ];

        let (remapped_vertices, remapped_indices) =
            remap_mesh_vertices(&vertices, &[2, 0, 3, 2, 3, 1]).expect("valid remap");

        assert_eq!(remapped_indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(
            remapped_vertices
                .iter()
                .map(|vertex| vertex.position[0])
                .collect::<Vec<_>>(),
            vec![2.0, 0.0, 3.0, 1.0]
        );
    }

    #[test]
    fn remap_mesh_vertices_keeps_winding_when_normals_match() {
        let vertices = vec![
            test_vertex_at([0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            test_vertex_at([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
            test_vertex_at([0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ];

        let (_, remapped_indices) =
            remap_mesh_vertices(&vertices, &[0, 1, 2]).expect("valid remap");

        assert_eq!(remapped_indices, vec![0, 1, 2]);
    }

    #[test]
    fn remap_mesh_vertices_flips_winding_when_normals_are_opposed() {
        let vertices = vec![
            test_vertex_at([0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            test_vertex_at([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            test_vertex_at([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
        ];

        let (_, remapped_indices) =
            remap_mesh_vertices(&vertices, &[0, 1, 2]).expect("valid remap");

        assert_eq!(remapped_indices, vec![0, 2, 1]);
    }

    #[test]
    fn remap_mesh_vertices_rejects_partial_triangles() {
        let vertices = vec![test_vertex(0.0), test_vertex(1.0), test_vertex(2.0)];

        assert!(remap_mesh_vertices(&vertices, &[0, 1]).is_none());
    }

    #[test]
    fn sanitized_bitangent_normalizes_xyz_and_sign() {
        let bitangent = sanitized_bitangent([0.0, 3.0, 4.0, 0.0]);

        assert_eq!(bitangent, [0.0, 0.6, 0.8, -1.0]);
    }

    #[test]
    fn sanitized_bitangent_falls_back_for_invalid_xyz() {
        let bitangent = sanitized_bitangent([0.0, 0.0, 0.0, f32::NAN]);

        assert_eq!(bitangent, [1.0, 0.0, 0.0, 1.0]);
    }

    fn test_texture(path: &str, kind: WeaponModelTextureKind) -> WeaponModelTexture {
        test_texture_with_alpha(path, kind, 255)
    }

    fn test_texture_with_alpha(
        path: &str,
        kind: WeaponModelTextureKind,
        alpha: u8,
    ) -> WeaponModelTexture {
        WeaponModelTexture {
            path: path.to_string(),
            kind,
            width: 1,
            height: 1,
            rgba: vec![255, 255, 255, alpha],
        }
    }

    fn test_vertex(x: f32) -> WeaponModelVertex {
        WeaponModelVertex {
            position: [x, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0, 0.0],
            uv1: [0.0, 0.0],
            bitangent: [1.0, 0.0, 0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }

    fn test_vertex_at(position: [f32; 3], normal: [f32; 3]) -> WeaponModelVertex {
        let mut vertex = test_vertex(position[0]);
        vertex.position = position;
        vertex.normal = normal;
        vertex
    }

    fn test_mtrl_with_constant(id: u32, values: &[f32], data_set_size: u16) -> Vec<u8> {
        let value_size = (values.len() * 4) as u16;
        let mut bytes = vec![0; 16];
        bytes[6..8].copy_from_slice(&data_set_size.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0xAA, data_set_size as usize));
        bytes.extend_from_slice(&value_size.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&value_size.to_le_bytes());
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn test_shpk_with_material_defaults(parameters: &[(u32, &[f32])]) -> Vec<u8> {
        let defaults_size = parameters
            .iter()
            .map(|(_, values)| values.len() * 4)
            .sum::<usize>() as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"ShPk");
        bytes.extend_from_slice(&0x0C01_u32.to_le_bytes());
        bytes.extend_from_slice(b"DX11");
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&defaults_size.to_le_bytes());
        bytes.extend_from_slice(&(parameters.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());

        let mut byte_offset = 0_u16;
        for (id, values) in parameters {
            let byte_size = (values.len() * 4) as u16;
            bytes.extend_from_slice(&id.to_le_bytes());
            bytes.extend_from_slice(&byte_offset.to_le_bytes());
            bytes.extend_from_slice(&byte_size.to_le_bytes());
            byte_offset += byte_size;
        }

        for (_, values) in parameters {
            for value in *values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }

        bytes
    }
}
