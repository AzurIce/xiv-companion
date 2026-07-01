use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponCatalogPackage {
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub counts: WeaponCatalogCounts,
    pub items: Vec<WeaponCatalogItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponCatalogCounts {
    pub items: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponCatalogItem {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub icon: u32,
    pub item_ui_category: u32,
    pub item_search_category: u32,
    pub equip_slot_category: u32,
    pub price_mid: u32,
    pub price_low: u32,
    pub model_main: u64,
    pub model_sub: u64,
}

impl WeaponCatalogItem {
    pub fn primary_model(&self) -> PackedModelId {
        PackedModelId::from_raw(self.model_main)
    }

    pub fn secondary_model(&self) -> Option<PackedModelId> {
        (self.model_sub != 0).then(|| PackedModelId::from_raw(self.model_sub))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackedModelId {
    pub raw: u64,
    pub model_id: u16,
    pub variant_id: u16,
    pub body_id: u16,
}

impl PackedModelId {
    pub fn from_raw(raw: u64) -> Self {
        Self {
            raw,
            model_id: (raw & 0xffff) as u16,
            variant_id: ((raw >> 16) & 0xffff) as u16,
            body_id: ((raw >> 32) & 0xffff) as u16,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelData {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: PackedModelId,
    pub model_sub: Option<PackedModelId>,
    pub loaded_paths: Vec<String>,
    pub bounds: WeaponModelBounds,
    pub meshes: Vec<WeaponModelMesh>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub center: [f32; 3],
    pub radius: f32,
}

impl Default for WeaponModelBounds {
    fn default() -> Self {
        Self {
            min: [0.0; 3],
            max: [0.0; 3],
            center: [0.0; 3],
            radius: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelMesh {
    pub path: String,
    pub part_index: u32,
    pub material_index: u16,
    pub material_name: String,
    pub color: [f32; 3],
    pub vertices: Vec<WeaponModelVertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub fn is_weapon_equip_slot_category(category: u32) -> bool {
    // 1/2 are main/off hand. 13 and 14 cover two-handed and dual-wield
    // main-hand categories in current game data.
    matches!(category, 1 | 2 | 13 | 14)
}

pub fn weapon_model_candidate_paths(model: PackedModelId) -> Vec<String> {
    if model.model_id == 0 {
        return Vec::new();
    }

    let mut body_ids = Vec::new();
    for body_id in [model.body_id, model.variant_id, 1, 101, 201] {
        if body_id != 0 && !body_ids.contains(&body_id) {
            body_ids.push(body_id);
        }
    }

    body_ids
        .into_iter()
        .map(|body_id| {
            format!(
                "chara/weapon/w{model_id:04}/obj/body/b{body_id:04}/model/w{model_id:04}b{body_id:04}.mdl",
                model_id = model.model_id,
            )
        })
        .collect()
}

pub fn weapon_slot_label(category: u32) -> &'static str {
    match category {
        1 => "主手",
        2 => "副手",
        13 => "双手主手",
        14 => "双持主手",
        _ => "武器",
    }
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
        let vertices = part
            .vertices
            .iter()
            .map(|vertex| WeaponModelVertex {
                position: vertex.position,
                normal: normalized_or_fallback(vertex.normal),
            })
            .collect::<Vec<_>>();
        let indices = part.indices.iter().map(|index| u32::from(*index)).collect();
        meshes.push(WeaponModelMesh {
            path: path.to_string(),
            part_index: part_index as u32,
            material_index: part.material_index,
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

pub fn calculate_model_bounds(meshes: &[WeaponModelMesh]) -> WeaponModelBounds {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut has_vertex = false;

    for mesh in meshes {
        for vertex in &mesh.vertices {
            has_vertex = true;
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex.position[axis]);
                max[axis] = max[axis].max(vertex.position[axis]);
            }
        }
    }

    if !has_vertex {
        return WeaponModelBounds::default();
    }

    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let mut radius = 0.0_f32;
    for mesh in meshes {
        for vertex in &mesh.vertices {
            let dx = vertex.position[0] - center[0];
            let dy = vertex.position[1] - center[1];
            let dz = vertex.position[2] - center[2];
            radius = radius.max((dx * dx + dy * dy + dz * dz).sqrt());
        }
    }

    WeaponModelBounds {
        min,
        max,
        center,
        radius: radius.max(0.1),
    }
}

fn normalized_or_fallback(normal: [f32; 3]) -> [f32; 3] {
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length > 0.0001 {
        [normal[0] / length, normal[1] / length, normal[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn material_color(material_index: u16) -> [f32; 3] {
    const COLORS: [[f32; 3]; 8] = [
        [0.78, 0.72, 0.64],
        [0.56, 0.66, 0.78],
        [0.72, 0.58, 0.50],
        [0.62, 0.70, 0.54],
        [0.72, 0.60, 0.78],
        [0.50, 0.68, 0.70],
        [0.82, 0.68, 0.48],
        [0.66, 0.66, 0.66],
    ];
    COLORS[material_index as usize % COLORS.len()]
}
