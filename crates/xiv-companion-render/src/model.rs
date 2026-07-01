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
    #[serde(default)]
    pub materials: Vec<WeaponModelMaterial>,
    #[serde(default)]
    pub textures: Vec<WeaponModelTexture>,
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
    #[serde(default)]
    pub material_slot: usize,
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
    #[serde(default)]
    pub uv0: [f32; 2],
    #[serde(default)]
    pub uv1: [f32; 2],
    #[serde(default)]
    pub bitangent: [f32; 4],
    #[serde(default)]
    pub color: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelMaterial {
    pub slot: usize,
    pub material_index: u16,
    pub name: String,
    pub path: Option<String>,
    pub shader_package_name: Option<String>,
    pub fallback_color: [f32; 3],
    pub diffuse_color: [f32; 3],
    pub specular_color: [f32; 3],
    pub emissive_color: [f32; 3],
    pub roughness: f32,
    pub metalness: f32,
    #[serde(default)]
    pub texture_indices: Vec<usize>,
    #[serde(default)]
    pub base_color_texture: Option<usize>,
    #[serde(default)]
    pub normal_texture: Option<usize>,
    #[serde(default)]
    pub mask_texture: Option<usize>,
    #[serde(default)]
    pub emissive_texture: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelTexture {
    pub path: String,
    pub kind: WeaponModelTextureKind,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WeaponModelTextureKind {
    BaseColor,
    Normal,
    Mask,
    Specular,
    Emissive,
    Other,
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

pub fn weapon_material_candidate_paths(
    model: PackedModelId,
    model_path: &str,
    material_name: &str,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let normalized_name = normalize_resource_path(material_name);
    if normalized_name.is_empty() {
        return candidates;
    }

    push_unique_path(&mut candidates, normalized_name.clone());
    if normalized_name.starts_with("chara/") {
        return candidates;
    }

    let normalized_model_path = normalize_resource_path(model_path);
    let Some((object_root, _)) = normalized_model_path.split_once("/model/") else {
        return candidates;
    };
    let material_root = format!("{object_root}/material");

    if normalized_name.starts_with("v") {
        push_unique_path(
            &mut candidates,
            format!("{material_root}/{normalized_name}"),
        );
    }

    let material_file = normalized_name
        .rsplit('/')
        .next()
        .unwrap_or(normalized_name.as_str());
    let mut versions = Vec::new();
    for version in [model.variant_id, model.body_id, 1, 101, 201] {
        if version != 0 && !versions.contains(&version) {
            versions.push(version);
        }
    }

    for version in versions {
        push_unique_path(
            &mut candidates,
            format!("{material_root}/v{version:04}/{material_file}"),
        );
    }
    push_unique_path(&mut candidates, format!("{material_root}/{material_file}"));
    candidates
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

pub fn material_color(material_index: u16) -> [f32; 3] {
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

fn normalize_resource_path(path: &str) -> String {
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

fn push_unique_path(paths: &mut Vec<String>, path: String) {
    if !path.is_empty() && !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}
