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
            // Weapon Model{Main/Sub} 三段为 (model_id, body_id, variant_id)。
            // 例如 "w2001 b0102 v0001" 的 CSV 为 "2001, 102, 1, 0"。
            body_id: ((raw >> 16) & 0xffff) as u16,
            variant_id: ((raw >> 32) & 0xffff) as u16,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelData {
    pub bounds: ModelBounds,
    #[serde(default)]
    pub materials: Vec<ModelMaterial>,
    #[serde(default)]
    pub textures: Vec<ModelTexture>,
    pub meshes: Vec<ModelMesh>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelData {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: PackedModelId,
    pub model_sub: Option<PackedModelId>,
    pub loaded_paths: Vec<String>,
    pub bounds: ModelBounds,
    #[serde(default)]
    pub materials: Vec<ModelMaterial>,
    #[serde(default)]
    pub textures: Vec<ModelTexture>,
    pub meshes: Vec<ModelMesh>,
}

impl WeaponModelData {
    pub fn to_model_data(&self) -> ModelData {
        ModelData {
            bounds: self.bounds,
            materials: self.materials.clone(),
            textures: self.textures.clone(),
            meshes: self.meshes.clone(),
        }
    }
}

impl From<&WeaponModelData> for ModelData {
    fn from(value: &WeaponModelData) -> Self {
        value.to_model_data()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub center: [f32; 3],
    pub radius: f32,
}

impl Default for ModelBounds {
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
pub struct ModelMesh {
    pub path: String,
    pub part_index: u32,
    #[serde(default)]
    pub mesh_category: Option<String>,
    pub material_index: u16,
    #[serde(default)]
    pub material_slot: usize,
    pub material_name: String,
    pub color: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone_table: Option<ModelBoneTable>,
    pub vertices: Vec<ModelVertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBoneTable {
    pub index: usize,
    pub bone_count: u32,
    pub bone_indices: Vec<u16>,
    pub bone_names: Vec<Option<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVertex {
    pub position: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_weights: Option<ModelBlendWeights>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blend_indices: Option<ModelBlendIndices>,
    pub normal: [f32; 3],
    #[serde(default)]
    pub uv0: [f32; 2],
    #[serde(default)]
    pub uv1: [f32; 2],
    #[serde(default)]
    pub uv2: [f32; 2],
    #[serde(default)]
    pub uv3: [f32; 2],
    #[serde(default)]
    pub bitangent: [f32; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal1: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitangent1: Option<[f32; 4]>,
    #[serde(default)]
    pub color: [f32; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color1: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow0: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow1: Option<[f32; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBlendWeights {
    pub count: u8,
    pub values: [f32; 8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBlendIndices {
    pub count: u8,
    pub values: [u8; 8],
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMaterial {
    pub slot: usize,
    pub material_index: u16,
    pub name: String,
    pub path: Option<String>,
    pub shader_package_name: Option<String>,
    #[serde(default)]
    pub render_mode: MaterialRenderMode,
    #[serde(default)]
    pub alpha_mode: MaterialAlphaMode,
    #[serde(default)]
    pub alpha_threshold: f32,
    #[serde(default = "default_material_opacity")]
    pub opacity: f32,
    #[serde(default = "default_render_backfaces")]
    pub render_backfaces: bool,
    #[serde(default)]
    pub apply_vertex_color: bool,
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
    pub material_map_texture: Option<usize>,
    #[serde(default)]
    pub multi_map_texture: Option<usize>,
    #[serde(default)]
    pub specular_texture: Option<usize>,
    #[serde(default)]
    pub emissive_texture: Option<usize>,
    #[serde(default)]
    pub material_properties_texture: Option<usize>,
    #[serde(default)]
    pub tile_properties_texture: Option<usize>,
    #[serde(default)]
    pub sheen_properties_texture: Option<usize>,
    #[serde(default)]
    pub sphere_properties_texture: Option<usize>,
    #[serde(default)]
    pub tile_matrix_texture: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialRenderMode {
    #[default]
    Opaque,
    Transparent,
    Glass,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialAlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
    Glass,
}

fn default_material_opacity() -> f32 {
    1.0
}

fn default_render_backfaces() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTexture {
    pub path: String,
    pub kind: ModelTextureKind,
    pub width: u16,
    pub height: u16,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelTextureKind {
    BaseColor,
    Normal,
    Mask,
    /// Explicit `g_SamplerMaterial` texture. Its channel semantics are shader-specific.
    MaterialMap,
    /// Explicit `g_SamplerMulti` texture. Its channel semantics are shader-specific.
    MultiMap,
    Specular,
    Emissive,
    /// ColorTable 派生出的物理参数贴图，通道为 metalness / roughness / gloss / specular strength。
    MaterialProperties,
    /// ColorTable 派生出的 tile 参数贴图，通道为 tile index / tile alpha / 1 / 1。
    TileProperties,
    /// ColorTable 派生出的 sheen 参数贴图，通道为 sheen rate / tint / aperture / 1。
    SheenProperties,
    /// ColorTable 派生出的 sphere map 参数贴图，通道为 sphere index / sphere mask / 1 / 1。
    SphereProperties,
    /// ColorTable 派生出的 tile matrix 参数贴图，通道为 UU / UV / VU / VV。
    TileMatrixProperties,
    /// ColorTable 行索引贴图 (`_id.tex`)，本身不是颜色，用于逐像素查调色板
    Index,
    Other,
}

pub type WeaponMaterialRenderMode = MaterialRenderMode;
pub type WeaponMaterialAlphaMode = MaterialAlphaMode;
pub type WeaponModelBounds = ModelBounds;
pub type WeaponModelMaterial = ModelMaterial;
pub type WeaponModelMesh = ModelMesh;
pub type WeaponModelTexture = ModelTexture;
pub type WeaponModelTextureKind = ModelTextureKind;
pub type WeaponModelVertex = ModelVertex;

pub trait ModelRenderData {
    fn bounds(&self) -> &ModelBounds;
    fn materials(&self) -> &[ModelMaterial];
    fn textures(&self) -> &[ModelTexture];
    fn meshes(&self) -> &[ModelMesh];
}

impl ModelRenderData for ModelData {
    fn bounds(&self) -> &ModelBounds {
        &self.bounds
    }

    fn materials(&self) -> &[ModelMaterial] {
        &self.materials
    }

    fn textures(&self) -> &[ModelTexture] {
        &self.textures
    }

    fn meshes(&self) -> &[ModelMesh] {
        &self.meshes
    }
}

impl ModelRenderData for WeaponModelData {
    fn bounds(&self) -> &ModelBounds {
        &self.bounds
    }

    fn materials(&self) -> &[ModelMaterial] {
        &self.materials
    }

    fn textures(&self) -> &[ModelTexture] {
        &self.textures
    }

    fn meshes(&self) -> &[ModelMesh] {
        &self.meshes
    }
}

impl<T: ModelRenderData + ?Sized> ModelRenderData for std::rc::Rc<T> {
    fn bounds(&self) -> &ModelBounds {
        self.as_ref().bounds()
    }

    fn materials(&self) -> &[ModelMaterial] {
        self.as_ref().materials()
    }

    fn textures(&self) -> &[ModelTexture] {
        self.as_ref().textures()
    }

    fn meshes(&self) -> &[ModelMesh] {
        self.as_ref().meshes()
    }
}

impl<T: ModelRenderData + ?Sized> ModelRenderData for std::sync::Arc<T> {
    fn bounds(&self) -> &ModelBounds {
        self.as_ref().bounds()
    }

    fn materials(&self) -> &[ModelMaterial] {
        self.as_ref().materials()
    }

    fn textures(&self) -> &[ModelTexture] {
        self.as_ref().textures()
    }

    fn meshes(&self) -> &[ModelMesh] {
        self.as_ref().meshes()
    }
}

/// ColorTable 单行中参与烘焙的颜色（线性空间）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorTableRowColors {
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub emissive: [f32; 3],
    pub gloss_strength: f32,
    pub specular_strength: f32,
    pub roughness: f32,
    pub metalness: f32,
    pub anisotropy: f32,
    /// ColorTable Tile Alpha，属于 tile 属性，不等同于材质整体透明度。
    pub tile_alpha: f32,
    /// Meddle 的 TileIndex 语义值。Dawntrail 行来自 half(tile_set) * 64。
    pub tile_index: f32,
    pub sheen_rate: f32,
    pub sheen_tint: f32,
    pub sheen_aperture: f32,
    pub sphere_index: f32,
    pub sphere_mask: f32,
    /// Meddle TileMatrix 顺序: UU / UV / VU / VV。
    pub tile_matrix: [f32; 4],
}

impl Default for ColorTableRowColors {
    fn default() -> Self {
        Self {
            diffuse: [0.0; 3],
            specular: [0.0; 3],
            emissive: [0.0; 3],
            gloss_strength: 1.0,
            specular_strength: 1.0,
            roughness: 0.5,
            metalness: 0.0,
            anisotropy: 0.0,
            tile_alpha: 1.0,
            tile_index: 0.0,
            sheen_rate: 0.0,
            sheen_tint: 0.0,
            sheen_aperture: 0.0,
            sphere_index: 0.0,
            sphere_mask: 0.0,
            tile_matrix: [1.0, 0.0, 0.0, 1.0],
        }
    }
}

/// 由 ColorTable + 索引贴图烘焙出的贴图（RGBA8，与索引贴图同尺寸）。
///
/// `diffuse_rgba` / `specular_rgba` / `emissive_rgba` 的 RGB 为 sRGB 编码；
/// `diffuse_rgba` 的 Alpha 固定为不透明；ColorTable TileAlpha 是 tile 属性，不是材质透明度。
/// `specular_rgba` 的 Alpha 来自 ColorTable Anisotropy，与 MeddleTools 的 specular ramp 对齐。
/// `material_rgba` 为线性 unorm，通道顺序对齐 MeddleTools:
/// metalness / roughness / gloss strength / specular strength。
/// 额外的 ColorTable 语义贴图同样为线性 unorm，用于保留 MeddleTools 中的
/// TileProperties / SheenProperties / SphereProperties / TileMatrixProperties ramp。
/// TileIndex 按 0..64 归一化，SphereIndex 按 0..255 归一化。
#[derive(Clone, Debug, PartialEq)]
pub struct BakedColorTableMaps {
    pub diffuse_rgba: Vec<u8>,
    pub specular_rgba: Vec<u8>,
    pub material_rgba: Vec<u8>,
    pub tile_properties_rgba: Vec<u8>,
    pub sheen_properties_rgba: Vec<u8>,
    pub sphere_properties_rgba: Vec<u8>,
    pub tile_matrix_rgba: Vec<u8>,
    /// 所有行 emissive 全黑时为 None
    pub emissive_rgba: Option<Vec<u8>>,
}

/// 按 `_id.tex` 逐像素查 ColorTable 烘焙 diffuse / emissive 贴图。
///
/// Dawntrail 索引贴图编码: R 通道选择行对 (0..=15, 值为 17 的倍数)，
/// 行对 i 对应表中第 2i 与 2i+1 行；G 通道在两行之间线性混合。
/// `rows` 为 ColorTable 全部行（Dawntrail 32 行）；`id_rgba` 为索引贴图 RGBA8 数据。
pub fn bake_color_table_maps(
    rows: &[ColorTableRowColors],
    id_rgba: &[u8],
) -> Option<BakedColorTableMaps> {
    if rows.len() < 2 || rows.len() % 2 != 0 || id_rgba.len() % 4 != 0 {
        return None;
    }

    let pair_count = rows.len() / 2;
    let pixel_count = id_rgba.len() / 4;
    let mut diffuse_rgba = Vec::with_capacity(pixel_count * 4);
    let mut specular_rgba = Vec::with_capacity(pixel_count * 4);
    let mut material_rgba = Vec::with_capacity(pixel_count * 4);
    let mut tile_properties_rgba = Vec::with_capacity(pixel_count * 4);
    let mut sheen_properties_rgba = Vec::with_capacity(pixel_count * 4);
    let mut sphere_properties_rgba = Vec::with_capacity(pixel_count * 4);
    let mut tile_matrix_rgba = Vec::with_capacity(pixel_count * 4);
    let mut emissive_rgba = Vec::with_capacity(pixel_count * 4);
    let mut has_emissive = false;

    for pixel in id_rgba.chunks_exact(4) {
        let pair = ((pixel[0] as f32 / 255.0) * (pair_count - 1) as f32).round() as usize;
        let pair = pair.min(pair_count - 1);
        let blend = pixel[1] as f32 / 255.0;
        let row_a = rows[pair * 2];
        let row_b = rows[pair * 2 + 1];

        let diffuse = lerp_color(row_a.diffuse, row_b.diffuse, blend);
        let specular = lerp_color(row_a.specular, row_b.specular, blend);
        let emissive = lerp_color(row_a.emissive, row_b.emissive, blend);
        let metalness = lerp_value(row_a.metalness, row_b.metalness, blend);
        let roughness = lerp_value(row_a.roughness, row_b.roughness, blend);
        let gloss_strength = lerp_value(row_a.gloss_strength, row_b.gloss_strength, blend);
        let specular_strength = lerp_value(row_a.specular_strength, row_b.specular_strength, blend);
        let anisotropy = lerp_value(row_a.anisotropy, row_b.anisotropy, blend);
        let tile_index = lerp_value(row_a.tile_index, row_b.tile_index, blend);
        let tile_alpha = lerp_value(row_a.tile_alpha, row_b.tile_alpha, blend);
        let sheen_rate = lerp_value(row_a.sheen_rate, row_b.sheen_rate, blend);
        let sheen_tint = lerp_value(row_a.sheen_tint, row_b.sheen_tint, blend);
        let sheen_aperture = lerp_value(row_a.sheen_aperture, row_b.sheen_aperture, blend);
        let sphere_index = lerp_value(row_a.sphere_index, row_b.sphere_index, blend);
        let sphere_mask = lerp_value(row_a.sphere_mask, row_b.sphere_mask, blend);
        let tile_matrix = lerp_color4(row_a.tile_matrix, row_b.tile_matrix, blend);
        if emissive.iter().any(|value| *value > 0.001) {
            has_emissive = true;
        }

        push_srgb_pixel(&mut diffuse_rgba, diffuse, 1.0);
        push_srgb_pixel(&mut specular_rgba, specular, anisotropy);
        push_unorm_pixel(
            &mut material_rgba,
            [metalness, roughness, gloss_strength, specular_strength],
        );
        push_unorm_pixel(
            &mut tile_properties_rgba,
            [tile_index / 64.0, tile_alpha, 1.0, 1.0],
        );
        push_unorm_pixel(
            &mut sheen_properties_rgba,
            [sheen_rate, sheen_tint, sheen_aperture, 1.0],
        );
        push_unorm_pixel(
            &mut sphere_properties_rgba,
            [sphere_index / 255.0, sphere_mask, 1.0, 1.0],
        );
        push_unorm_pixel(&mut tile_matrix_rgba, tile_matrix);
        push_srgb_pixel(&mut emissive_rgba, emissive, 1.0);
    }

    Some(BakedColorTableMaps {
        diffuse_rgba,
        specular_rgba,
        material_rgba,
        tile_properties_rgba,
        sheen_properties_rgba,
        sphere_properties_rgba,
        tile_matrix_rgba,
        emissive_rgba: has_emissive.then_some(emissive_rgba),
    })
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn lerp_value(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    a + (b - a) * t
}

fn lerp_color4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn push_srgb_pixel(rgba: &mut Vec<u8>, linear: [f32; 3], alpha: f32) {
    for value in linear {
        rgba.push((linear_to_srgb(value).clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    rgba.push((alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
}

fn push_unorm_pixel(rgba: &mut Vec<u8>, linear: [f32; 4]) {
    for value in linear {
        rgba.push((value.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
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
    let mut material_roots = vec![material_root];
    if let Some((material_model_id, material_body_id)) =
        weapon_ids_from_material_file(material_file)
    {
        push_unique_path(
            &mut material_roots,
            format!(
                "chara/weapon/w{material_model_id:04}/obj/body/b{material_body_id:04}/material"
            ),
        );
    }

    let mut versions = Vec::new();
    for version in [model.variant_id, model.body_id, 1, 101, 201] {
        if version != 0 && !versions.contains(&version) {
            versions.push(version);
        }
    }

    for material_root in material_roots {
        for version in &versions {
            push_unique_path(
                &mut candidates,
                format!("{material_root}/v{version:04}/{material_file}"),
            );
        }
        push_unique_path(&mut candidates, format!("{material_root}/{material_file}"));
    }
    candidates
}

fn weapon_ids_from_material_file(material_file: &str) -> Option<(u16, u16)> {
    let tail = material_file.strip_prefix("mt_w")?;
    let (model_id, tail) = tail.split_at_checked(4)?;
    let tail = tail.strip_prefix('b')?;
    let (body_id, _) = tail.split_at_checked(4)?;
    Some((model_id.parse().ok()?, body_id.parse().ok()?))
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

pub fn calculate_model_bounds(meshes: &[ModelMesh]) -> ModelBounds {
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
        return ModelBounds::default();
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

    ModelBounds {
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

#[cfg(test)]
mod color_table_bake_tests {
    use super::*;

    fn rows_with_two_pairs() -> Vec<ColorTableRowColors> {
        // 行对 0: 纯红 <-> 纯绿；行对 1: 纯蓝 <-> 纯蓝（带 emissive）
        vec![
            ColorTableRowColors {
                diffuse: [1.0, 0.0, 0.0],
                emissive: [0.0, 0.0, 0.0],
                tile_alpha: 1.0,
                ..Default::default()
            },
            ColorTableRowColors {
                diffuse: [0.0, 1.0, 0.0],
                emissive: [0.0, 0.0, 0.0],
                tile_alpha: 0.5,
                ..Default::default()
            },
            ColorTableRowColors {
                diffuse: [0.0, 0.0, 1.0],
                emissive: [1.0, 0.0, 0.0],
                tile_alpha: 1.0,
                ..Default::default()
            },
            ColorTableRowColors {
                diffuse: [0.0, 0.0, 1.0],
                emissive: [1.0, 0.0, 0.0],
                tile_alpha: 1.0,
                ..Default::default()
            },
        ]
    }

    #[test]
    fn bake_selects_row_pair_from_red_channel() {
        // 两个像素: R=0 → 行对 0, R=255 → 行对 1；G=0 → 不混合
        let id_rgba = [0, 0, 0, 255, 255, 0, 0, 255];
        let baked = bake_color_table_maps(&rows_with_two_pairs(), &id_rgba).expect("bake");

        // 像素 0: 纯红 (sRGB 255,0,0)
        assert_eq!(&baked.diffuse_rgba[0..4], &[255, 0, 0, 255]);
        // 像素 1: 纯蓝
        assert_eq!(&baked.diffuse_rgba[4..8], &[0, 0, 255, 255]);
        // 行对 1 有 emissive → 生成 emissive 贴图
        let emissive = baked.emissive_rgba.expect("emissive map");
        assert_eq!(&emissive[4..8], &[255, 0, 0, 255]);
    }

    #[test]
    fn bake_blends_rows_with_green_channel() {
        // R=0 → 行对 0；G=255 → 完全取第二行 (纯绿)
        let id_rgba = [0, 255, 0, 255];
        let baked = bake_color_table_maps(&rows_with_two_pairs(), &id_rgba).expect("bake");
        assert_eq!(&baked.diffuse_rgba[0..4], &[0, 255, 0, 255]);
        // 全部像素 emissive 为 0 → 无 emissive 贴图
        assert!(baked.emissive_rgba.is_none());
    }

    #[test]
    fn bake_keeps_diffuse_alpha_opaque_when_tile_alpha_varies() {
        // TileAlpha 属于 tile 属性，不是 diffuse/material alpha。
        let id_rgba = [0, 128, 0, 255];
        let baked = bake_color_table_maps(&rows_with_two_pairs(), &id_rgba).expect("bake");
        assert_eq!(baked.diffuse_rgba[3], 255);
    }

    #[test]
    fn bake_uses_anisotropy_for_specular_alpha() {
        let rows = vec![
            ColorTableRowColors {
                specular: [1.0, 1.0, 1.0],
                anisotropy: 0.1,
                specular_strength: 0.2,
                ..Default::default()
            },
            ColorTableRowColors {
                specular: [1.0, 1.0, 1.0],
                anisotropy: 0.25,
                specular_strength: 0.75,
                ..Default::default()
            },
        ];
        let id_rgba = [0, 255, 0, 255];
        let baked = bake_color_table_maps(&rows, &id_rgba).expect("bake");

        assert_eq!(baked.specular_rgba[3], 64);
        assert_eq!(baked.material_rgba[3], 191);
    }

    #[test]
    fn bake_preserves_meddletools_extra_color_table_ramps() {
        let rows = vec![
            ColorTableRowColors {
                tile_index: 8.0,
                tile_alpha: 0.25,
                sheen_rate: 0.1,
                sheen_tint: 0.2,
                sheen_aperture: 0.3,
                sphere_index: 32.0,
                sphere_mask: 0.4,
                tile_matrix: [0.1, 0.2, 0.3, 0.4],
                ..Default::default()
            },
            ColorTableRowColors {
                tile_index: 16.0,
                tile_alpha: 0.75,
                sheen_rate: 0.25,
                sheen_tint: 0.5,
                sheen_aperture: 0.75,
                sphere_index: 128.0,
                sphere_mask: 0.25,
                tile_matrix: [1.0, 0.75, 0.5, 0.25],
                ..Default::default()
            },
        ];
        let id_rgba = [0, 255, 0, 255];
        let baked = bake_color_table_maps(&rows, &id_rgba).expect("bake");

        assert_eq!(&baked.tile_properties_rgba[0..4], &[64, 191, 255, 255]);
        assert_eq!(&baked.sheen_properties_rgba[0..4], &[64, 128, 191, 255]);
        assert_eq!(&baked.sphere_properties_rgba[0..4], &[128, 64, 255, 255]);
        assert_eq!(&baked.tile_matrix_rgba[0..4], &[255, 191, 128, 64]);
    }

    #[test]
    fn bake_maps_dawntrail_pair_steps() {
        // 32 行 = 16 个行对，R 通道以 17 为步长；第 8 对应取行 16
        let mut rows = vec![ColorTableRowColors::default(); 32];
        rows[16] = ColorTableRowColors {
            diffuse: [1.0, 1.0, 1.0],
            emissive: [0.0, 0.0, 0.0],
            tile_alpha: 1.0,
            ..Default::default()
        };
        rows[17] = rows[16];
        let id_rgba = [8 * 17, 0, 0, 255];
        let baked = bake_color_table_maps(&rows, &id_rgba).expect("bake");
        assert_eq!(&baked.diffuse_rgba[0..4], &[255, 255, 255, 255]);
    }

    #[test]
    fn bake_accepts_noisy_dawntrail_pair_steps() {
        let mut rows = vec![ColorTableRowColors::default(); 32];
        rows[4] = ColorTableRowColors {
            diffuse: [1.0, 1.0, 1.0],
            ..Default::default()
        };
        rows[5] = rows[4];

        let id_rgba = [30, 0, 0, 255, 34, 0, 0, 255];
        let baked = bake_color_table_maps(&rows, &id_rgba).expect("bake");

        assert_eq!(&baked.diffuse_rgba[0..4], &[255, 255, 255, 255]);
        assert_eq!(&baked.diffuse_rgba[4..8], &[255, 255, 255, 255]);
    }

    #[test]
    fn bake_rejects_invalid_input() {
        assert!(bake_color_table_maps(&[], &[0, 0, 0, 255]).is_none());
        let rows = rows_with_two_pairs();
        assert!(bake_color_table_maps(&rows, &[0, 0, 0]).is_none());
    }

    #[test]
    fn packed_weapon_model_uses_body_then_variant_order() {
        let model = PackedModelId::from_raw(4_301_653_969);
        assert_eq!(model.model_id, 2001);
        assert_eq!(model.body_id, 102);
        assert_eq!(model.variant_id, 1);
        assert_eq!(
            weapon_model_candidate_paths(model)
                .first()
                .map(String::as_str),
            Some("chara/weapon/w2001/obj/body/b0102/model/w2001b0102.mdl")
        );
    }

    #[test]
    fn weapon_material_candidates_include_material_name_weapon_root() {
        // 有些双手武器的副手 MDL 放在 w0387，但材质名仍引用主手 w0337 的文件。
        // 不能只在副手自身 material 目录里查，否则会 fallback 成米色。
        let model = PackedModelId {
            raw: 0,
            model_id: 387,
            variant_id: 1,
            body_id: 1,
        };
        let candidates = weapon_material_candidate_paths(
            model,
            "chara/weapon/w0387/obj/body/b0001/model/w0387b0001.mdl",
            "/mt_w0337b0001_a.mtrl",
        );

        assert!(candidates.contains(
            &"chara/weapon/w0387/obj/body/b0001/material/v0001/mt_w0337b0001_a.mtrl".to_string()
        ));
        assert!(candidates.contains(
            &"chara/weapon/w0337/obj/body/b0001/material/v0001/mt_w0337b0001_a.mtrl".to_string()
        ));
    }
}
