pub use xiv_companion_render::{
    BakedColorTableMaps, ColorTableRowColors, PackedModelId, WeaponCatalogCounts,
    WeaponCatalogItem, WeaponCatalogPackage, WeaponMaterialRenderMode, WeaponModelBounds,
    WeaponModelData, WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture,
    WeaponModelTextureKind, WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

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
        let vertices = part
            .vertices
            .iter()
            .map(|vertex| WeaponModelVertex {
                position: vertex.position,
                normal: normalized_or_fallback(vertex.normal),
                uv0: vertex.uv0,
                uv1: vertex.uv1,
                bitangent: vertex.bitangent,
                color: vertex_color_or_fallback(vertex.color),
            })
            .collect::<Vec<_>>();
        let indices = part.indices.iter().map(|index| u32::from(*index)).collect();
        meshes.push(WeaponModelMesh {
            path: path.to_string(),
            part_index: part_index as u32,
            material_index: part.material_index,
            material_slot: part.material_index as usize,
            material_name,
            color,
            vertices,
            indices,
        });
    }

    (!meshes.is_empty())
        .then_some(meshes)
        .ok_or_else(|| anyhow!("model {path} contains no renderable meshes"))
        .with_context(|| format!("failed to extract render meshes from {path}"))
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
fn vertex_color_or_fallback(color: [f32; 4]) -> [f32; 4] {
    if color[..3].iter().any(|value| value.abs() > 0.0001) {
        color
    } else if color[3].abs() > 0.0001 {
        [1.0, 1.0, 1.0, color[3]]
    } else {
        [1.0, 1.0, 1.0, 1.0]
    }
}
