use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponCatalogPackage {
    pub generated_at: String,
    pub game_version: String,
    pub source: String,
    pub counts: WeaponCatalogCounts,
    #[serde(default)]
    pub stains: Vec<WeaponStain>,
    pub items: Vec<WeaponCatalogItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponCatalogCounts {
    pub items: usize,
    #[serde(default)]
    pub stains: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponStain {
    pub id: u8,
    pub name: String,
    pub se_color: u32,
    pub ui_color: [u8; 4],
    pub shade: u8,
    pub sub_order: u8,
    pub metallic: bool,
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
    #[serde(default)]
    pub stain_ids: [u8; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub load_diagnostics: Vec<WeaponModelLoadDiagnostic>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WeaponModelLoadRole {
    Primary,
    Secondary,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelLoadDiagnostic {
    pub role: WeaponModelLoadRole,
    pub model: PackedModelId,
    pub candidates: Vec<WeaponModelLoadCandidateDiagnostic>,
    pub error: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WeaponModelLoadCandidateStatus {
    Missing,
    ReadError,
    ParseError,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeaponModelLoadCandidateDiagnostic {
    pub path: String,
    pub status: WeaponModelLoadCandidateStatus,
    pub error: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submesh: Option<ModelSubmeshInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shape_influences: Vec<ModelShapeInfo>,
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

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSubmeshInfo {
    pub index: usize,
    pub table_index: usize,
    pub attribute_index_mask: u32,
    pub attribute_index_mask_hex: String,
    pub attribute_names: Vec<String>,
    pub bone_start_index: u16,
    pub bone_count: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelShapeInfo {
    pub index: usize,
    pub name: Option<String>,
    pub shape_index_mask: u32,
    pub shape_index_mask_hex: String,
    pub shape_mesh_index: usize,
    pub shape_value_count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelMeshDrawRole {
    #[default]
    Normal,
    Glass,
    LightShaft,
    ShadowOnly,
    Ignored,
    MaterialChange,
    CrestChange,
}

impl ModelMeshDrawRole {
    pub fn renders_in_main_pass(self) -> bool {
        matches!(
            self,
            ModelMeshDrawRole::Normal
                | ModelMeshDrawRole::Glass
                | ModelMeshDrawRole::MaterialChange
                | ModelMeshDrawRole::CrestChange
        )
    }

    pub fn forces_transparent_pass(self) -> bool {
        matches!(self, ModelMeshDrawRole::Glass)
    }
}

pub fn mesh_draw_role_for_category(category: Option<&str>) -> ModelMeshDrawRole {
    let Some(category) = category else {
        return ModelMeshDrawRole::Normal;
    };

    match category.to_ascii_lowercase().as_str() {
        "glass" => ModelMeshDrawRole::Glass,
        "lightshaft" | "light_shaft" => ModelMeshDrawRole::LightShaft,
        "shadow" | "terrainshadow" | "terrain_shadow" => ModelMeshDrawRole::ShadowOnly,
        "verticalfog" | "vertical_fog" => ModelMeshDrawRole::Ignored,
        "materialchange" | "material_change" => ModelMeshDrawRole::MaterialChange,
        "crestchange" | "crest_change" => ModelMeshDrawRole::CrestChange,
        _ => ModelMeshDrawRole::Normal,
    }
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
pub struct ModelLegacyColorDyeTableRow {
    pub template: u16,
    pub diffuse: bool,
    pub specular: bool,
    pub emissive: bool,
    pub gloss: bool,
    pub specular_strength: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDawntrailColorDyeTableRow {
    pub template: u16,
    pub channel: u8,
    pub diffuse: bool,
    pub specular: bool,
    pub emissive: bool,
    pub scalar3: bool,
    pub metalness: bool,
    pub roughness: bool,
    pub sheen_rate: bool,
    pub sheen_tint_rate: bool,
    pub sheen_aperture: bool,
    pub anisotropy: bool,
    pub sphere_map_index: bool,
    pub sphere_map_mask: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", content = "rows", rename_all = "camelCase")]
pub enum ModelColorDyeTable {
    Legacy(Vec<ModelLegacyColorDyeTableRow>),
    Dawntrail(Vec<ModelDawntrailColorDyeTableRow>),
    Opaque,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StainingApplicationReport {
    pub rows_considered: usize,
    pub rows_changed: usize,
    pub rows_skipped_no_stain: usize,
    pub rows_skipped_missing_template: usize,
    pub rows_unavailable: usize,
    pub template_kind_mismatch: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStainingApplication {
    pub stain_ids: [u8; 2],
    pub template_path: String,
    pub report: StainingApplicationReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
    #[serde(default)]
    pub draw_depth_mode: MaterialDrawDepthMode,
    #[serde(default)]
    pub lighting_mode: MaterialLightingMode,
    #[serde(default)]
    pub flow_mode: MaterialFlowMode,
    #[serde(default)]
    pub transparency: f32,
    #[serde(default = "default_material_alpha_aperture")]
    pub alpha_aperture: f32,
    #[serde(default)]
    pub alpha_offset: f32,
    #[serde(default = "default_material_shadow_alpha_threshold")]
    pub shadow_alpha_threshold: f32,
    #[serde(default = "default_material_glass_ior")]
    pub glass_ior: f32,
    #[serde(default = "default_material_glass_thickness_max")]
    pub glass_thickness_max: f32,
    #[serde(default = "default_material_normal_scale")]
    pub normal_scale: f32,
    #[serde(default = "default_material_normal_scale")]
    pub multi_normal_scale: f32,
    #[serde(default = "default_material_normal_scale")]
    pub detail_normal_scale: f32,
    #[serde(default = "default_material_normal_scale")]
    pub multi_detail_normal_scale: f32,
    #[serde(default)]
    pub tile_index: f32,
    #[serde(default = "default_material_tile_alpha")]
    pub tile_alpha: f32,
    #[serde(default = "default_material_tile_scale")]
    pub tile_scale: [f32; 2],
    #[serde(default)]
    pub toon_index: f32,
    #[serde(default = "default_material_toon_light_scale")]
    pub toon_light_scale: f32,
    #[serde(default = "default_material_toon_light_spec_aperture")]
    pub toon_light_spec_aperture: f32,
    #[serde(default = "default_material_toon_reflection_scale")]
    pub toon_reflection_scale: f32,
    #[serde(default = "default_material_toon_spec_index")]
    pub toon_spec_index: f32,
    #[serde(default)]
    pub sheen_rate: f32,
    #[serde(default)]
    pub sheen_tint_rate: f32,
    #[serde(default = "default_material_sheen_aperture")]
    pub sheen_aperture: f32,
    #[serde(default)]
    pub sphere_map_index: f32,
    #[serde(default)]
    pub detail_id: f32,
    #[serde(default)]
    pub multi_detail_id: f32,
    #[serde(default = "default_material_detail_color")]
    pub detail_color: [f32; 4],
    #[serde(default = "default_material_detail_color")]
    pub multi_detail_color: [f32; 4],
    #[serde(default = "default_material_shader_diffuse_color")]
    pub shader_diffuse_color: [f32; 4],
    #[serde(default = "default_material_shader_diffuse_color")]
    pub shader_multi_diffuse_color: [f32; 4],
    #[serde(default = "default_material_shader_emissive_color")]
    pub shader_emissive_color: [f32; 4],
    #[serde(default = "default_material_shader_emissive_color")]
    pub shader_multi_emissive_color: [f32; 4],
    #[serde(default = "default_material_outline_color")]
    pub outline_color: [f32; 4],
    #[serde(default)]
    pub outline_width: f32,
    #[serde(default = "default_material_specular_color_mask")]
    pub specular_color_mask: [f32; 4],
    #[serde(default = "default_material_ssao_mask")]
    pub ssao_mask: f32,
    #[serde(default)]
    pub texture_mip_bias: f32,
    #[serde(default)]
    pub shadow_pos_offset: f32,
    #[serde(default = "default_material_detail_uv_scale")]
    pub detail_color_uv_scale: [f32; 4],
    #[serde(default = "default_material_detail_uv_scale")]
    pub detail_normal_uv_scale: [f32; 4],
    #[serde(default)]
    pub uv_scroll: [f32; 4],
    #[serde(default = "default_material_lightshaft_color")]
    pub lightshaft_color: [f32; 4],
    #[serde(default)]
    pub lightshaft_tex_anim: [f32; 4],
    #[serde(default = "default_material_lightshaft_tex_u")]
    pub lightshaft_tex_u: [f32; 4],
    #[serde(default = "default_material_lightshaft_tex_v")]
    pub lightshaft_tex_v: [f32; 4],
    #[serde(default)]
    pub lightshaft_ray: [f32; 4],
    #[serde(default = "default_material_opacity")]
    pub opacity: f32,
    #[serde(default = "default_render_backfaces")]
    pub render_backfaces: bool,
    #[serde(default)]
    pub apply_vertex_color: bool,
    #[serde(default)]
    pub has_color_dye_table: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_dye_table: Option<ModelColorDyeTable>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staining_application: Option<ModelStainingApplication>,
    #[serde(default)]
    pub texture_arrays: ModelMaterialTextureArrays,
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
    #[serde(default)]
    pub index_texture: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMaterialTextureArrays {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_normal: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_orb: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_diffuse: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_normal: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialDrawDepthMode {
    #[default]
    None,
    Dither,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialLightingMode {
    #[default]
    Default,
    Enabled,
    Disabled,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialFlowMode {
    #[default]
    Standard,
    Flow,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialShaderFamily {
    Character,
    CharacterStockings,
    CharacterGlass,
    CharacterReflection,
    CharacterTransparency,
    CharacterScroll,
    CharacterTattoo,
    CharacterOcclusion,
    Bg,
    BgUvScroll,
    LightShaft,
    Water,
    #[default]
    Unknown,
}

pub fn material_shader_family(shader_package_name: Option<&str>) -> MaterialShaderFamily {
    let Some(shader_package_name) = shader_package_name else {
        return MaterialShaderFamily::Unknown;
    };
    let normalized = shader_package_name.replace('\\', "/");
    let file_name = normalized
        .rsplit('/')
        .next()
        .unwrap_or(normalized.as_str())
        .trim_start_matches("meddle ")
        .trim()
        .to_ascii_lowercase();

    match file_name.as_str() {
        "character.shpk" | "characterlegacy.shpk" | "characterinc.shpk" => {
            MaterialShaderFamily::Character
        }
        "characterstockings.shpk" => MaterialShaderFamily::CharacterStockings,
        "characterglass.shpk" => MaterialShaderFamily::CharacterGlass,
        "characterreflection.shpk" => MaterialShaderFamily::CharacterReflection,
        "charactertransparency.shpk" => MaterialShaderFamily::CharacterTransparency,
        "characterscroll.shpk" => MaterialShaderFamily::CharacterScroll,
        "charactertattoo.shpk" => MaterialShaderFamily::CharacterTattoo,
        "characterocclusion.shpk" => MaterialShaderFamily::CharacterOcclusion,
        "bg.shpk" | "bgcolorchange.shpk" => MaterialShaderFamily::Bg,
        "bguvscroll.shpk" => MaterialShaderFamily::BgUvScroll,
        "lightshaft.shpk" => MaterialShaderFamily::LightShaft,
        "water.shpk" | "river.shpk" => MaterialShaderFamily::Water,
        _ => MaterialShaderFamily::Unknown,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedModel {
    pub meshes: Vec<PreparedMesh>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedModelOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_attribute_mask: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_shape_mask: Option<u32>,
}

impl PreparedModelOptions {
    pub fn with_enabled_attribute_mask(mut self, enabled_attribute_mask: u32) -> Self {
        self.enabled_attribute_mask = Some(enabled_attribute_mask);
        self
    }

    pub fn with_enabled_shape_mask(mut self, enabled_shape_mask: u32) -> Self {
        self.enabled_shape_mask = Some(enabled_shape_mask);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMesh {
    pub mesh_index: usize,
    pub material_slot: usize,
    pub draw_role: ModelMeshDrawRole,
    pub renders_in_main_pass: bool,
    #[serde(default)]
    pub visibility: PreparedMeshVisibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submesh: Option<ModelSubmeshInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shape_influences: Vec<ModelShapeInfo>,
    #[serde(default)]
    pub shape_influence_state: PreparedMeshShapeInfluences,
    pub prepared_material: PreparedMaterial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMeshVisibility {
    pub submesh_attributes_visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_attribute_mask: Option<u32>,
    pub missing_attribute_mask: u32,
}

impl Default for PreparedMeshVisibility {
    fn default() -> Self {
        Self {
            submesh_attributes_visible: true,
            enabled_attribute_mask: None,
            missing_attribute_mask: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMeshShapeInfluences {
    pub available_shape_mask: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_shape_mask: Option<u32>,
    pub active_shape_mask: u32,
    pub inactive_shape_mask: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterial {
    pub render_pass: PreparedRenderPass,
    pub shader_family: MaterialShaderFamily,
    #[serde(default)]
    pub flow_mode: MaterialFlowMode,
    #[serde(default)]
    pub alpha_policy: PreparedMaterialAlphaPolicy,
    pub texture_bindings: PreparedTextureBindings,
    pub texture_sampling: PreparedTextureSamplingSet,
    #[serde(default)]
    pub uv_sources: PreparedMaterialUvSources,
    #[serde(default)]
    pub feature_flags: PreparedMaterialFeatureFlags,
    #[serde(default)]
    pub unsupported_inputs: PreparedMaterialUnsupportedInputs,
    #[serde(default)]
    pub resource_availability: PreparedMaterialResourceAvailability,
    #[serde(default)]
    pub runtime_fallbacks: PreparedMaterialRuntimeFallbacks,
    pub render_backfaces: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterialAlphaPolicy {
    pub source: PreparedAlphaSource,
    pub draw_depth_mode: MaterialDrawDepthMode,
    pub lighting_enabled: bool,
}

impl Default for PreparedMaterialAlphaPolicy {
    fn default() -> Self {
        Self {
            source: PreparedAlphaSource::Opaque,
            draw_depth_mode: MaterialDrawDepthMode::None,
            lighting_enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedAlphaSource {
    #[default]
    Opaque,
    BaseColorAlpha,
    NormalBlue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterialUvSources {
    pub textures: PreparedTextureUvSources,
    #[serde(default)]
    pub scroll: PreparedTextureScrollSet,
    pub uv0_scroll: PreparedUvSource,
    pub uv1_scroll: PreparedUvSource,
}

impl Default for PreparedMaterialUvSources {
    fn default() -> Self {
        Self {
            textures: PreparedTextureUvSources::default(),
            scroll: PreparedTextureScrollSet::default(),
            uv0_scroll: PreparedUvSource::Uv0,
            uv1_scroll: PreparedUvSource::Uv1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PreparedTextureScrollSet {
    pub base_color: bool,
    pub normal: bool,
    pub mask: bool,
    pub material_map: bool,
    pub multi_map: bool,
    pub specular: bool,
    pub emissive: bool,
    pub material_properties: bool,
    pub tile_properties: bool,
    pub sheen_properties: bool,
    pub sphere_properties: bool,
    pub tile_matrix: bool,
    pub index: bool,
    pub other: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedTextureUvSources {
    pub base_color: PreparedUvSource,
    pub normal: PreparedUvSource,
    pub mask: PreparedUvSource,
    pub material_map: PreparedUvSource,
    pub multi_map: PreparedUvSource,
    pub specular: PreparedUvSource,
    pub emissive: PreparedUvSource,
    pub material_properties: PreparedUvSource,
    pub tile_properties: PreparedUvSource,
    pub sheen_properties: PreparedUvSource,
    pub sphere_properties: PreparedUvSource,
    pub tile_matrix: PreparedUvSource,
    pub index: PreparedUvSource,
    pub other: PreparedUvSource,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedUvSource {
    #[default]
    Uv0,
    Uv1,
    Uv2,
    Uv3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterialFeatureFlags {
    pub uses_vertex_color: bool,
    pub uses_color_table: bool,
    pub uses_tile: bool,
    pub uses_detail: bool,
    pub uses_scroll: bool,
    pub uses_flow: bool,
    pub uses_dye: bool,
    pub uses_outline: bool,
    pub uses_toon: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterialUnsupportedInputs {
    pub dye_application: bool,
    pub runtime_color_table: bool,
    pub decal_or_crest: bool,
    pub runtime_material_change: bool,
    pub tile_array: bool,
    pub detail_array: bool,
    pub incomplete_shader_family_logic: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterialResourceAvailability {
    pub tile_array_complete: bool,
    pub detail_array_complete: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedMaterialRuntimeFallbacks {
    pub decal_or_crest: PreparedRuntimeFallback,
    pub material_change: PreparedRuntimeFallback,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedRuntimeFallback {
    #[default]
    NotRequired,
    TransparentTexture,
    BaseMaterial,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedTextureBindings {
    pub base_color: Option<usize>,
    pub normal: Option<usize>,
    pub mask: Option<usize>,
    pub material_map: Option<usize>,
    pub multi_map: Option<usize>,
    pub specular: Option<usize>,
    pub emissive: Option<usize>,
    pub material_properties: Option<usize>,
    pub tile_properties: Option<usize>,
    pub sheen_properties: Option<usize>,
    pub sphere_properties: Option<usize>,
    pub tile_matrix: Option<usize>,
    pub index: Option<usize>,
    pub tile_normal_array: Option<usize>,
    pub tile_orb_array: Option<usize>,
    pub detail_diffuse_array: Option<usize>,
    pub detail_normal_array: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedTextureSamplingSet {
    pub base_color: PreparedTextureSampling,
    pub normal: PreparedTextureSampling,
    pub mask: PreparedTextureSampling,
    pub material_map: PreparedTextureSampling,
    pub multi_map: PreparedTextureSampling,
    pub specular: PreparedTextureSampling,
    pub emissive: PreparedTextureSampling,
    pub material_properties: PreparedTextureSampling,
    pub tile_properties: PreparedTextureSampling,
    pub sheen_properties: PreparedTextureSampling,
    pub sphere_properties: PreparedTextureSampling,
    pub tile_matrix: PreparedTextureSampling,
    pub index: PreparedTextureSampling,
    #[serde(default = "default_texture_array_sampling")]
    pub tile_normal_array: PreparedTextureSampling,
    #[serde(default = "default_texture_array_sampling")]
    pub tile_orb_array: PreparedTextureSampling,
    #[serde(default = "default_texture_array_sampling")]
    pub detail_diffuse_array: PreparedTextureSampling,
    #[serde(default = "default_texture_array_sampling")]
    pub detail_normal_array: PreparedTextureSampling,
    pub other: PreparedTextureSampling,
}

impl Default for PreparedTextureSamplingSet {
    fn default() -> Self {
        Self {
            base_color: prepared_texture_sampling_for_kind(ModelTextureKind::BaseColor),
            normal: prepared_texture_sampling_for_kind(ModelTextureKind::Normal),
            mask: prepared_texture_sampling_for_kind(ModelTextureKind::Mask),
            material_map: prepared_texture_sampling_for_kind(ModelTextureKind::MaterialMap),
            multi_map: prepared_texture_sampling_for_kind(ModelTextureKind::MultiMap),
            specular: prepared_texture_sampling_for_kind(ModelTextureKind::Specular),
            emissive: prepared_texture_sampling_for_kind(ModelTextureKind::Emissive),
            material_properties: prepared_texture_sampling_for_kind(
                ModelTextureKind::MaterialProperties,
            ),
            tile_properties: prepared_texture_sampling_for_kind(ModelTextureKind::TileProperties),
            sheen_properties: prepared_texture_sampling_for_kind(ModelTextureKind::SheenProperties),
            sphere_properties: prepared_texture_sampling_for_kind(
                ModelTextureKind::SphereProperties,
            ),
            tile_matrix: prepared_texture_sampling_for_kind(ModelTextureKind::TileMatrixProperties),
            index: prepared_texture_sampling_for_kind(ModelTextureKind::Index),
            tile_normal_array: prepared_texture_sampling_for_kind(
                ModelTextureKind::TileNormalArray,
            ),
            tile_orb_array: prepared_texture_sampling_for_kind(ModelTextureKind::TileOrbArray),
            detail_diffuse_array: prepared_texture_sampling_for_kind(
                ModelTextureKind::DetailDiffuseArray,
            ),
            detail_normal_array: prepared_texture_sampling_for_kind(
                ModelTextureKind::DetailNormalArray,
            ),
            other: prepared_texture_sampling_for_kind(ModelTextureKind::Other),
        }
    }
}

fn default_texture_array_sampling() -> PreparedTextureSampling {
    prepared_texture_sampling_for_kind(ModelTextureKind::TileNormalArray)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedTextureSampling {
    pub color_space: PreparedTextureColorSpace,
    pub filter: PreparedTextureFilter,
    pub address_mode: PreparedTextureAddressMode,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedTextureColorSpace {
    #[default]
    Srgb,
    NonColor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedTextureFilter {
    #[default]
    Linear,
    Nearest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedTextureAddressMode {
    #[default]
    Repeat,
    ClampToEdge,
    Clip,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PreparedRenderPass {
    #[default]
    Opaque,
    Cutout,
    Transparent,
    Glass,
    AdditiveLightShaft,
}

impl PreparedRenderPass {
    pub fn uses_opaque_pipeline(self) -> bool {
        matches!(self, PreparedRenderPass::Opaque)
    }

    pub fn uses_cutout_pipeline(self) -> bool {
        matches!(self, PreparedRenderPass::Cutout)
    }

    pub fn uses_transparent_pipeline(self) -> bool {
        matches!(self, PreparedRenderPass::Transparent)
    }

    pub fn uses_glass_pipeline(self) -> bool {
        matches!(self, PreparedRenderPass::Glass)
    }

    pub fn uses_additive_pipeline(self) -> bool {
        matches!(self, PreparedRenderPass::AdditiveLightShaft)
    }

    pub fn sorts_back_to_front(self) -> bool {
        self.uses_transparent_pipeline() || self.uses_glass_pipeline()
    }
}

pub fn prepare_model_for_render<M: ModelRenderData + ?Sized>(model: &M) -> PreparedModel {
    prepare_model_for_render_with_options(model, PreparedModelOptions::default())
}

pub fn prepare_model_for_render_with_options<M: ModelRenderData + ?Sized>(
    model: &M,
    options: PreparedModelOptions,
) -> PreparedModel {
    PreparedModel {
        meshes: model
            .meshes()
            .iter()
            .enumerate()
            .map(|(mesh_index, mesh)| {
                let draw_role = mesh_draw_role_for_category(mesh.mesh_category.as_deref());
                let visibility =
                    prepared_mesh_visibility(mesh.submesh.as_ref(), options.enabled_attribute_mask);
                let shape_influence_state = prepared_mesh_shape_influences(
                    &mesh.shape_influences,
                    options.enabled_shape_mask,
                );
                let mut prepared_material = prepare_material_for_draw_role(
                    model.materials().get(mesh.material_slot),
                    draw_role,
                );
                prepared_material.feature_flags.uses_flow =
                    matches!(prepared_material.flow_mode, MaterialFlowMode::Flow)
                        && mesh_has_primary_flow_attribute(mesh);
                PreparedMesh {
                    mesh_index,
                    material_slot: mesh.material_slot,
                    draw_role,
                    renders_in_main_pass: draw_role.renders_in_main_pass()
                        && visibility.submesh_attributes_visible,
                    visibility,
                    submesh: mesh.submesh.clone(),
                    shape_influences: mesh.shape_influences.clone(),
                    shape_influence_state,
                    prepared_material,
                }
            })
            .collect(),
    }
}

fn prepared_mesh_shape_influences(
    shape_influences: &[ModelShapeInfo],
    enabled_shape_mask: Option<u32>,
) -> PreparedMeshShapeInfluences {
    let available_shape_mask = shape_influences
        .iter()
        .fold(0_u32, |mask, shape| mask | shape.shape_index_mask);
    let Some(enabled_shape_mask) = enabled_shape_mask else {
        return PreparedMeshShapeInfluences {
            available_shape_mask,
            ..PreparedMeshShapeInfluences::default()
        };
    };

    PreparedMeshShapeInfluences {
        available_shape_mask,
        enabled_shape_mask: Some(enabled_shape_mask),
        active_shape_mask: available_shape_mask & enabled_shape_mask,
        inactive_shape_mask: available_shape_mask & !enabled_shape_mask,
    }
}

fn prepared_mesh_visibility(
    submesh: Option<&ModelSubmeshInfo>,
    enabled_attribute_mask: Option<u32>,
) -> PreparedMeshVisibility {
    let Some(enabled_attribute_mask) = enabled_attribute_mask else {
        return PreparedMeshVisibility::default();
    };
    let required_attribute_mask = submesh
        .map(|submesh| submesh.attribute_index_mask)
        .unwrap_or(0);
    let missing_attribute_mask = required_attribute_mask & !enabled_attribute_mask;

    PreparedMeshVisibility {
        submesh_attributes_visible: missing_attribute_mask == 0,
        enabled_attribute_mask: Some(enabled_attribute_mask),
        missing_attribute_mask,
    }
}

fn mesh_has_primary_flow_attribute(mesh: &ModelMesh) -> bool {
    mesh.vertices.iter().any(|vertex| vertex.flow0.is_some())
}

pub fn prepare_material_for_draw_role(
    material: Option<&ModelMaterial>,
    draw_role: ModelMeshDrawRole,
) -> PreparedMaterial {
    let shader_family = material_shader_family(
        material.and_then(|material| material.shader_package_name.as_deref()),
    );
    let texture_bindings = prepared_texture_bindings(material);
    let uv_sources = prepared_material_uv_sources(material, shader_family, texture_bindings);

    PreparedMaterial {
        render_pass: prepared_render_pass(material, draw_role, shader_family),
        shader_family,
        flow_mode: material
            .map(|material| material.flow_mode)
            .unwrap_or_default(),
        alpha_policy: prepared_material_alpha_policy(material, shader_family),
        texture_bindings,
        texture_sampling: PreparedTextureSamplingSet::default(),
        uv_sources,
        feature_flags: prepared_material_feature_flags(material, shader_family, texture_bindings),
        unsupported_inputs: prepared_material_unsupported_inputs(
            material,
            draw_role,
            shader_family,
            texture_bindings,
        ),
        resource_availability: prepared_material_resource_availability(material),
        runtime_fallbacks: prepared_material_runtime_fallbacks(draw_role),
        render_backfaces: material
            .map(|material| material.render_backfaces)
            .unwrap_or(true),
    }
}

pub fn prepared_material_alpha_policy(
    material: Option<&ModelMaterial>,
    shader_family: MaterialShaderFamily,
) -> PreparedMaterialAlphaPolicy {
    let Some(material) = material else {
        return PreparedMaterialAlphaPolicy::default();
    };
    let is_character_family = matches!(
        shader_family,
        MaterialShaderFamily::Character
            | MaterialShaderFamily::CharacterGlass
            | MaterialShaderFamily::CharacterReflection
            | MaterialShaderFamily::CharacterTransparency
            | MaterialShaderFamily::CharacterScroll
            | MaterialShaderFamily::CharacterTattoo
            | MaterialShaderFamily::CharacterOcclusion
    );
    let source = if matches!(shader_family, MaterialShaderFamily::CharacterStockings) {
        PreparedAlphaSource::Opaque
    } else if matches!(
        shader_family,
        MaterialShaderFamily::CharacterGlass | MaterialShaderFamily::CharacterTransparency
    ) || (is_character_family
        && !matches!(material.alpha_mode, MaterialAlphaMode::Opaque))
    {
        PreparedAlphaSource::NormalBlue
    } else {
        match material.alpha_mode {
            MaterialAlphaMode::Opaque => PreparedAlphaSource::Opaque,
            MaterialAlphaMode::Mask | MaterialAlphaMode::Blend | MaterialAlphaMode::Glass => {
                PreparedAlphaSource::BaseColorAlpha
            }
        }
    };
    let lighting_enabled = !matches!(
        (shader_family, material.lighting_mode),
        (
            MaterialShaderFamily::CharacterTransparency,
            MaterialLightingMode::Disabled
        )
    );

    PreparedMaterialAlphaPolicy {
        source,
        draw_depth_mode: material.draw_depth_mode,
        lighting_enabled,
    }
}

pub fn prepared_texture_bindings(material: Option<&ModelMaterial>) -> PreparedTextureBindings {
    let Some(material) = material else {
        return PreparedTextureBindings::default();
    };

    PreparedTextureBindings {
        base_color: material.base_color_texture,
        normal: material.normal_texture,
        mask: material.mask_texture,
        material_map: material.material_map_texture,
        multi_map: material.multi_map_texture,
        specular: material.specular_texture,
        emissive: material.emissive_texture,
        material_properties: material.material_properties_texture,
        tile_properties: material.tile_properties_texture,
        sheen_properties: material.sheen_properties_texture,
        sphere_properties: material.sphere_properties_texture,
        tile_matrix: material.tile_matrix_texture,
        index: material.index_texture,
        tile_normal_array: material.texture_arrays.tile_normal,
        tile_orb_array: material.texture_arrays.tile_orb,
        detail_diffuse_array: material.texture_arrays.detail_diffuse,
        detail_normal_array: material.texture_arrays.detail_normal,
    }
}

pub fn prepared_material_uv_sources(
    _material: Option<&ModelMaterial>,
    shader_family: MaterialShaderFamily,
    texture_bindings: PreparedTextureBindings,
) -> PreparedMaterialUvSources {
    let mut sources = PreparedMaterialUvSources::default();
    if matches!(shader_family, MaterialShaderFamily::BgUvScroll) {
        // MeddleTools bguvscroll connects only Color/Normal/Specular Map0 to UV0Scroll.
        sources.scroll.base_color = texture_bindings.base_color.is_some();
        sources.scroll.normal = texture_bindings.normal.is_some();
        sources.scroll.specular = texture_bindings.specular.is_some();
    }
    sources
}

fn prepared_material_uses_scroll(
    material: &ModelMaterial,
    uv_sources: PreparedMaterialUvSources,
) -> bool {
    let textures = uv_sources.textures;
    let scroll = uv_sources.scroll;
    [
        (scroll.base_color, textures.base_color),
        (scroll.normal, textures.normal),
        (scroll.mask, textures.mask),
        (scroll.material_map, textures.material_map),
        (scroll.multi_map, textures.multi_map),
        (scroll.specular, textures.specular),
        (scroll.emissive, textures.emissive),
        (scroll.material_properties, textures.material_properties),
        (scroll.tile_properties, textures.tile_properties),
        (scroll.sheen_properties, textures.sheen_properties),
        (scroll.sphere_properties, textures.sphere_properties),
        (scroll.tile_matrix, textures.tile_matrix),
        (scroll.index, textures.index),
        (scroll.other, textures.other),
    ]
    .into_iter()
    .any(|(enabled, source)| enabled && prepared_uv_source_has_scroll(material.uv_scroll, source))
}

fn prepared_uv_source_has_scroll(uv_scroll: [f32; 4], source: PreparedUvSource) -> bool {
    match source {
        PreparedUvSource::Uv0 => material_vec2_differs([uv_scroll[0], uv_scroll[1]], [0.0; 2]),
        PreparedUvSource::Uv1 => material_vec2_differs([uv_scroll[2], uv_scroll[3]], [0.0; 2]),
        PreparedUvSource::Uv2 | PreparedUvSource::Uv3 => false,
    }
}

pub fn prepared_material_feature_flags(
    material: Option<&ModelMaterial>,
    shader_family: MaterialShaderFamily,
    texture_bindings: PreparedTextureBindings,
) -> PreparedMaterialFeatureFlags {
    let uv_sources = prepared_material_uv_sources(material, shader_family, texture_bindings);
    let mut flags = PreparedMaterialFeatureFlags {
        uses_color_table: texture_bindings.index.is_some()
            || texture_bindings.material_properties.is_some()
            || texture_bindings.tile_properties.is_some()
            || texture_bindings.sheen_properties.is_some()
            || texture_bindings.sphere_properties.is_some()
            || texture_bindings.tile_matrix.is_some(),
        uses_tile: texture_bindings.tile_properties.is_some(),
        uses_detail: texture_bindings.multi_map.is_some(),
        ..PreparedMaterialFeatureFlags::default()
    };

    let Some(material) = material else {
        return flags;
    };

    flags.uses_vertex_color = material.apply_vertex_color;
    flags.uses_dye = material_has_color_dye_table(material);
    flags.uses_tile |= material_scalar_differs(material.tile_index, 0.0)
        || material_scalar_differs(material.tile_alpha, 1.0)
        || material_vec2_differs(material.tile_scale, [16.0, 16.0]);
    flags.uses_detail |= material_scalar_differs(material.multi_normal_scale, 1.0)
        || material_scalar_differs(material.detail_normal_scale, 1.0)
        || material_scalar_differs(material.multi_detail_normal_scale, 1.0)
        || material_scalar_differs(material.detail_id, 0.0)
        || material_scalar_differs(material.multi_detail_id, 0.0)
        || material_vec4_differs(material.detail_color, [0.5, 0.5, 0.5, 1.0])
        || material_vec4_differs(material.multi_detail_color, [0.5, 0.5, 0.5, 1.0])
        || material_vec4_differs(material.detail_color_uv_scale, [4.0; 4])
        || material_vec4_differs(material.detail_normal_uv_scale, [4.0; 4]);
    flags.uses_scroll = prepared_material_uses_scroll(material, uv_sources)
        || material_vec4_differs(material.lightshaft_tex_anim, [0.0; 4]);
    flags.uses_outline = material.outline_width.is_finite()
        && material.outline_width > 0.0
        && matches!(
            shader_family,
            MaterialShaderFamily::Character
                | MaterialShaderFamily::CharacterStockings
                | MaterialShaderFamily::CharacterGlass
                | MaterialShaderFamily::CharacterReflection
                | MaterialShaderFamily::CharacterTransparency
                | MaterialShaderFamily::CharacterScroll
                | MaterialShaderFamily::CharacterTattoo
                | MaterialShaderFamily::CharacterOcclusion
        );
    flags.uses_toon = matches!(
        shader_family,
        MaterialShaderFamily::Character
            | MaterialShaderFamily::CharacterStockings
            | MaterialShaderFamily::CharacterGlass
            | MaterialShaderFamily::CharacterReflection
            | MaterialShaderFamily::CharacterTransparency
            | MaterialShaderFamily::CharacterScroll
            | MaterialShaderFamily::CharacterTattoo
            | MaterialShaderFamily::CharacterOcclusion
    );

    flags
}

pub fn prepared_material_unsupported_inputs(
    material: Option<&ModelMaterial>,
    draw_role: ModelMeshDrawRole,
    shader_family: MaterialShaderFamily,
    texture_bindings: PreparedTextureBindings,
) -> PreparedMaterialUnsupportedInputs {
    let feature_flags = prepared_material_feature_flags(material, shader_family, texture_bindings);
    let resource_availability = prepared_material_resource_availability(material);
    let dye_application = material
        .and_then(|material| material.staining_application.as_ref())
        .is_some_and(staining_application_is_incomplete);

    PreparedMaterialUnsupportedInputs {
        dye_application,
        runtime_color_table: feature_flags.uses_color_table,
        decal_or_crest: matches!(draw_role, ModelMeshDrawRole::CrestChange),
        runtime_material_change: false,
        tile_array: feature_flags.uses_tile && !resource_availability.tile_array_complete,
        detail_array: feature_flags.uses_detail && !resource_availability.detail_array_complete,
        incomplete_shader_family_logic: prepared_shader_family_needs_more_logic(shader_family),
    }
}

pub fn prepared_material_resource_availability(
    material: Option<&ModelMaterial>,
) -> PreparedMaterialResourceAvailability {
    let Some(material) = material else {
        return PreparedMaterialResourceAvailability::default();
    };
    PreparedMaterialResourceAvailability {
        tile_array_complete: material.texture_arrays.tile_normal.is_some()
            && material.texture_arrays.tile_orb.is_some(),
        detail_array_complete: material.texture_arrays.detail_diffuse.is_some()
            && material.texture_arrays.detail_normal.is_some(),
    }
}

pub fn prepared_material_runtime_fallbacks(
    draw_role: ModelMeshDrawRole,
) -> PreparedMaterialRuntimeFallbacks {
    PreparedMaterialRuntimeFallbacks {
        decal_or_crest: if matches!(draw_role, ModelMeshDrawRole::CrestChange) {
            PreparedRuntimeFallback::TransparentTexture
        } else {
            PreparedRuntimeFallback::NotRequired
        },
        material_change: if matches!(draw_role, ModelMeshDrawRole::MaterialChange) {
            PreparedRuntimeFallback::BaseMaterial
        } else {
            PreparedRuntimeFallback::NotRequired
        },
    }
}

fn material_has_color_dye_table(material: &ModelMaterial) -> bool {
    material.has_color_dye_table || material.color_dye_table.is_some()
}

fn staining_application_is_incomplete(application: &ModelStainingApplication) -> bool {
    application.error.is_some()
        || application.report.template_kind_mismatch
        || application.report.rows_skipped_missing_template != 0
        || application.report.rows_unavailable != 0
}

fn prepared_shader_family_needs_more_logic(shader_family: MaterialShaderFamily) -> bool {
    matches!(
        shader_family,
        MaterialShaderFamily::CharacterStockings
            | MaterialShaderFamily::CharacterGlass
            | MaterialShaderFamily::CharacterReflection
            | MaterialShaderFamily::CharacterTransparency
            | MaterialShaderFamily::CharacterScroll
            | MaterialShaderFamily::CharacterTattoo
            | MaterialShaderFamily::CharacterOcclusion
            | MaterialShaderFamily::LightShaft
    )
}

pub fn prepared_texture_sampling_for_kind(kind: ModelTextureKind) -> PreparedTextureSampling {
    match kind {
        ModelTextureKind::BaseColor | ModelTextureKind::Specular | ModelTextureKind::Emissive => {
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::Srgb,
                filter: PreparedTextureFilter::Linear,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        }
        ModelTextureKind::Index
        | ModelTextureKind::TileProperties
        | ModelTextureKind::SheenProperties
        | ModelTextureKind::SphereProperties
        | ModelTextureKind::TileMatrixProperties
        | ModelTextureKind::TileNormalArray
        | ModelTextureKind::TileOrbArray
        | ModelTextureKind::DetailDiffuseArray
        | ModelTextureKind::DetailNormalArray => PreparedTextureSampling {
            color_space: PreparedTextureColorSpace::NonColor,
            filter: PreparedTextureFilter::Nearest,
            address_mode: PreparedTextureAddressMode::Repeat,
        },
        ModelTextureKind::Normal
        | ModelTextureKind::Mask
        | ModelTextureKind::MaterialMap
        | ModelTextureKind::MultiMap
        | ModelTextureKind::MaterialProperties
        | ModelTextureKind::Other => PreparedTextureSampling {
            color_space: PreparedTextureColorSpace::NonColor,
            filter: PreparedTextureFilter::Linear,
            address_mode: PreparedTextureAddressMode::Repeat,
        },
    }
}

fn material_scalar_differs(value: f32, default: f32) -> bool {
    value.is_finite() && (value - default).abs() > 0.000_001
}

fn material_vec2_differs(value: [f32; 2], default: [f32; 2]) -> bool {
    value
        .into_iter()
        .zip(default)
        .any(|(value, default)| material_scalar_differs(value, default))
}

fn material_vec4_differs(value: [f32; 4], default: [f32; 4]) -> bool {
    value
        .into_iter()
        .zip(default)
        .any(|(value, default)| material_scalar_differs(value, default))
}

fn prepared_render_pass(
    material: Option<&ModelMaterial>,
    draw_role: ModelMeshDrawRole,
    shader_family: MaterialShaderFamily,
) -> PreparedRenderPass {
    if matches!(draw_role, ModelMeshDrawRole::LightShaft) {
        return PreparedRenderPass::AdditiveLightShaft;
    }
    if matches!(draw_role, ModelMeshDrawRole::CrestChange) {
        return PreparedRenderPass::Transparent;
    }
    if matches!(draw_role, ModelMeshDrawRole::Glass) {
        return PreparedRenderPass::Glass;
    }
    if matches!(shader_family, MaterialShaderFamily::CharacterGlass) {
        return PreparedRenderPass::Glass;
    }
    if matches!(shader_family, MaterialShaderFamily::CharacterTransparency) {
        return PreparedRenderPass::Transparent;
    }

    let Some(material) = material else {
        return PreparedRenderPass::Opaque;
    };

    match material.alpha_mode {
        MaterialAlphaMode::Glass => PreparedRenderPass::Glass,
        MaterialAlphaMode::Blend => PreparedRenderPass::Transparent,
        MaterialAlphaMode::Mask => PreparedRenderPass::Cutout,
        MaterialAlphaMode::Opaque => match material.render_mode {
            MaterialRenderMode::Glass => PreparedRenderPass::Glass,
            MaterialRenderMode::Transparent => PreparedRenderPass::Transparent,
            MaterialRenderMode::Opaque => PreparedRenderPass::Opaque,
        },
    }
}

fn default_material_opacity() -> f32 {
    1.0
}

fn default_material_normal_scale() -> f32 {
    1.0
}

fn default_material_alpha_aperture() -> f32 {
    2.0
}

fn default_material_shadow_alpha_threshold() -> f32 {
    0.5
}

fn default_material_glass_ior() -> f32 {
    1.0
}

fn default_material_glass_thickness_max() -> f32 {
    0.01
}

fn default_material_tile_alpha() -> f32 {
    1.0
}

fn default_material_tile_scale() -> [f32; 2] {
    [16.0, 16.0]
}

fn default_material_toon_light_scale() -> f32 {
    2.0
}

fn default_material_toon_light_spec_aperture() -> f32 {
    50.0
}

fn default_material_toon_reflection_scale() -> f32 {
    2.5
}

fn default_material_toon_spec_index() -> f32 {
    4.0e-45
}

fn default_material_sheen_aperture() -> f32 {
    1.0
}

fn default_material_detail_uv_scale() -> [f32; 4] {
    [4.0, 4.0, 4.0, 4.0]
}

fn default_material_detail_color() -> [f32; 4] {
    [0.5, 0.5, 0.5, 1.0]
}

fn default_material_shader_diffuse_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_material_shader_emissive_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_material_outline_color() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_material_specular_color_mask() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_material_ssao_mask() -> f32 {
    1.0
}

fn default_material_lightshaft_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_material_lightshaft_tex_u() -> [f32; 4] {
    [1.0, 0.0, 0.0, 0.0]
}

fn default_material_lightshaft_tex_v() -> [f32; 4] {
    [0.0, 1.0, 0.0, 0.0]
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
    #[serde(default = "default_texture_array_size")]
    pub array_size: u16,
    #[serde(default)]
    pub array_layer_height: u16,
    pub rgba: Vec<u8>,
    /// Optional per-pixel float channels for non-unorm semantic data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgba_f32: Option<Vec<[f32; 4]>>,
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
    TileNormalArray,
    TileOrbArray,
    DetailDiffuseArray,
    DetailNormalArray,
    Other,
}

fn default_texture_array_size() -> u16 {
    1
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
    /// Dawntrail ColorTable Scalar11，受 ColorDyeTable 的 Scalar3 flag 控制。
    pub scalar3: f32,
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
            scalar3: 0.0,
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
/// 额外的 ColorTable 语义贴图同样为线性 unorm，用于预览 MeddleTools 中的
/// TileProperties / SheenProperties / SphereProperties / TileMatrixProperties ramp。
/// TileMatrix 同时在 `tile_matrix_rgba_f32` 中保留未 clamp 的 UU / UV / VU / VV。
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
    pub tile_matrix_rgba_f32: Vec<[f32; 4]>,
    /// 所有行 emissive 全黑时为 None
    pub emissive_rgba: Option<Vec<u8>>,
}

/// 按 `_id.tex` 逐像素查 ColorTable 烘焙 diffuse / emissive 贴图。
///
/// MeddleTools 的 ColorTable ramp 语义按偶/奇行拆成 A/B ramp；这里用 R 通道选择行对，
/// 行对 i 对应表中第 2i 与 2i+1 行，G 通道在两行之间线性混合。
/// Dawntrail 通常是 32 行、16 个行对，R 通道以 17 为步长；Legacy 是 16 行、8 个行对。
/// `rows` 为 ColorTable 全部行；`id_rgba` 为索引贴图 RGBA8 数据。
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
    let mut tile_matrix_rgba_f32 = Vec::with_capacity(pixel_count);
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
        tile_matrix_rgba_f32.push(tile_matrix);
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
        tile_matrix_rgba_f32,
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

    #[test]
    fn mesh_draw_role_maps_mdl_categories_to_render_decisions() {
        assert_eq!(
            mesh_draw_role_for_category(Some("normal")),
            ModelMeshDrawRole::Normal
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("glass")),
            ModelMeshDrawRole::Glass
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("lightShaft")),
            ModelMeshDrawRole::LightShaft
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("shadow")),
            ModelMeshDrawRole::ShadowOnly
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("terrainShadow")),
            ModelMeshDrawRole::ShadowOnly
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("verticalFog")),
            ModelMeshDrawRole::Ignored
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("materialChange")),
            ModelMeshDrawRole::MaterialChange
        );
        assert_eq!(
            mesh_draw_role_for_category(Some("crestChange")),
            ModelMeshDrawRole::CrestChange
        );
        assert_eq!(mesh_draw_role_for_category(None), ModelMeshDrawRole::Normal);
    }

    #[test]
    fn prepared_material_maps_alpha_modes_and_draw_roles() {
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Opaque
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Mask,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Cutout
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Blend,
                MaterialRenderMode::Transparent,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Transparent
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Glass,
                MaterialRenderMode::Glass,
                ModelMeshDrawRole::Normal
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::Glass
            ),
            PreparedRenderPass::Glass
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::LightShaft
            ),
            PreparedRenderPass::AdditiveLightShaft
        );
        assert_eq!(
            test_prepared_render_pass(
                MaterialAlphaMode::Opaque,
                MaterialRenderMode::Opaque,
                ModelMeshDrawRole::CrestChange
            ),
            PreparedRenderPass::Transparent
        );
    }

    #[test]
    fn prepared_character_alpha_policy_uses_shader_family_inputs() {
        let mut material = test_material();
        material.shader_package_name = Some("characterglass.shpk".to_string());
        material.draw_depth_mode = MaterialDrawDepthMode::Dither;
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert_eq!(prepared.render_pass, PreparedRenderPass::Glass);
        assert_eq!(
            prepared.alpha_policy.source,
            PreparedAlphaSource::NormalBlue
        );
        assert_eq!(
            prepared.alpha_policy.draw_depth_mode,
            MaterialDrawDepthMode::Dither
        );
        assert!(prepared.alpha_policy.lighting_enabled);

        material.shader_package_name = Some("charactertransparency.shpk".to_string());
        material.lighting_mode = MaterialLightingMode::Disabled;
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert_eq!(prepared.render_pass, PreparedRenderPass::Transparent);
        assert_eq!(
            prepared.alpha_policy.source,
            PreparedAlphaSource::NormalBlue
        );
        assert!(!prepared.alpha_policy.lighting_enabled);

        material.shader_package_name = Some("characterstockings.shpk".to_string());
        material.alpha_mode = MaterialAlphaMode::Blend;
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert_eq!(prepared.alpha_policy.source, PreparedAlphaSource::Opaque);
    }

    #[test]
    fn prepared_material_preserves_culling_and_missing_material_defaults() {
        let mut material = test_material();
        material.render_backfaces = false;
        material.shader_package_name = Some("character.shpk".to_string());

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal),
            PreparedMaterial {
                render_pass: PreparedRenderPass::Opaque,
                shader_family: MaterialShaderFamily::Character,
                flow_mode: MaterialFlowMode::Standard,
                alpha_policy: PreparedMaterialAlphaPolicy::default(),
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                uv_sources: PreparedMaterialUvSources::default(),
                feature_flags: PreparedMaterialFeatureFlags {
                    uses_toon: true,
                    ..PreparedMaterialFeatureFlags::default()
                },
                unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
                resource_availability: PreparedMaterialResourceAvailability::default(),
                runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
                render_backfaces: false,
            }
        );
        assert_eq!(
            prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal),
            PreparedMaterial {
                render_pass: PreparedRenderPass::Opaque,
                shader_family: MaterialShaderFamily::Unknown,
                flow_mode: MaterialFlowMode::Standard,
                alpha_policy: PreparedMaterialAlphaPolicy::default(),
                texture_bindings: PreparedTextureBindings::default(),
                texture_sampling: PreparedTextureSamplingSet::default(),
                uv_sources: PreparedMaterialUvSources::default(),
                feature_flags: PreparedMaterialFeatureFlags::default(),
                unsupported_inputs: PreparedMaterialUnsupportedInputs::default(),
                resource_availability: PreparedMaterialResourceAvailability::default(),
                runtime_fallbacks: PreparedMaterialRuntimeFallbacks::default(),
                render_backfaces: true,
            }
        );
    }

    #[test]
    fn prepared_render_pass_reports_pipeline_class() {
        assert!(PreparedRenderPass::Opaque.uses_opaque_pipeline());
        assert!(!PreparedRenderPass::Cutout.uses_opaque_pipeline());
        assert!(PreparedRenderPass::Cutout.uses_cutout_pipeline());
        assert!(!PreparedRenderPass::Opaque.sorts_back_to_front());
        assert!(!PreparedRenderPass::Cutout.sorts_back_to_front());
        assert!(PreparedRenderPass::Transparent.uses_transparent_pipeline());
        assert!(!PreparedRenderPass::Glass.uses_transparent_pipeline());
        assert!(PreparedRenderPass::Glass.uses_glass_pipeline());
        assert!(PreparedRenderPass::Transparent.sorts_back_to_front());
        assert!(PreparedRenderPass::Glass.sorts_back_to_front());
        assert!(PreparedRenderPass::AdditiveLightShaft.uses_additive_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_opaque_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_cutout_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_transparent_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.uses_glass_pipeline());
        assert!(!PreparedRenderPass::AdditiveLightShaft.sorts_back_to_front());
    }

    #[test]
    fn prepared_material_copies_texture_bindings() {
        let mut material = test_material();
        material.base_color_texture = Some(1);
        material.normal_texture = Some(2);
        material.mask_texture = Some(3);
        material.material_map_texture = Some(4);
        material.multi_map_texture = Some(5);
        material.specular_texture = Some(6);
        material.emissive_texture = Some(7);
        material.material_properties_texture = Some(8);
        material.tile_properties_texture = Some(9);
        material.sheen_properties_texture = Some(10);
        material.sphere_properties_texture = Some(11);
        material.tile_matrix_texture = Some(12);
        material.index_texture = Some(13);
        material.texture_arrays.tile_normal = Some(14);
        material.texture_arrays.tile_orb = Some(15);
        material.texture_arrays.detail_diffuse = Some(16);
        material.texture_arrays.detail_normal = Some(17);

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .texture_bindings,
            PreparedTextureBindings {
                base_color: Some(1),
                normal: Some(2),
                mask: Some(3),
                material_map: Some(4),
                multi_map: Some(5),
                specular: Some(6),
                emissive: Some(7),
                material_properties: Some(8),
                tile_properties: Some(9),
                sheen_properties: Some(10),
                sphere_properties: Some(11),
                tile_matrix: Some(12),
                index: Some(13),
                tile_normal_array: Some(14),
                tile_orb_array: Some(15),
                detail_diffuse_array: Some(16),
                detail_normal_array: Some(17),
            }
        );
        assert_eq!(
            prepared_texture_bindings(None),
            PreparedTextureBindings::default()
        );
    }

    #[test]
    fn prepared_material_reports_material_feature_flags() {
        let mut material = test_material();
        material.apply_vertex_color = true;
        material.has_color_dye_table = true;
        material.material_properties_texture = Some(8);
        material.tile_properties_texture = Some(9);
        material.multi_map_texture = Some(10);
        material.detail_id = 3.0;
        material.detail_color_uv_scale = [8.0, 4.0, 4.0, 4.0];
        material.uv_scroll = [-1.0, 2.0, 0.0, 0.0];
        material.outline_width = 0.01;
        material.shader_package_name = Some("character.shpk".to_string());

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags,
            PreparedMaterialFeatureFlags {
                uses_vertex_color: true,
                uses_color_table: true,
                uses_tile: true,
                uses_detail: true,
                uses_scroll: false,
                uses_flow: false,
                uses_dye: true,
                uses_outline: true,
                uses_toon: true,
            }
        );

        material.shader_package_name = Some("bg.shpk".to_string());
        assert!(
            !prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags
                .uses_outline
        );

        material = test_material();
        material.color_dye_table = Some(ModelColorDyeTable::Opaque);
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert!(prepared.feature_flags.uses_dye);
        assert!(!prepared.unsupported_inputs.dye_application);

        material.staining_application = Some(ModelStainingApplication {
            stain_ids: [1, 0],
            template_path: String::new(),
            report: StainingApplicationReport::default(),
            error: Some("opaque ColorDyeTable cannot be applied".to_string()),
        });
        assert!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .unsupported_inputs
                .dye_application
        );

        material = test_material();
        material.shader_package_name = Some("characterScroll.shpk".to_string());
        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags,
            PreparedMaterialFeatureFlags {
                uses_toon: true,
                ..PreparedMaterialFeatureFlags::default()
            }
        );
        material.uv_scroll = [-1.0, 2.0, 0.0, 0.0];
        assert!(
            !prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags
                .uses_scroll
        );
        material = test_material();
        material.shader_package_name = Some("bguvscroll.shpk".to_string());
        material.base_color_texture = Some(1);
        assert!(
            !prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags
                .uses_scroll
        );
        material.uv_scroll = [-1.0, 2.0, 0.0, 0.0];
        assert!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags
                .uses_scroll
        );
        material = test_material();
        material.lightshaft_tex_anim = [0.5, 0.0, 0.0, 0.0];
        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::LightShaft)
                .feature_flags,
            PreparedMaterialFeatureFlags {
                uses_scroll: true,
                ..PreparedMaterialFeatureFlags::default()
            }
        );
        assert_eq!(
            prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal).feature_flags,
            PreparedMaterialFeatureFlags::default()
        );
    }

    #[test]
    fn prepared_material_reports_unsupported_runtime_inputs() {
        let mut material = test_material();
        material.has_color_dye_table = true;
        material.index_texture = Some(4);
        material.tile_index = 3.0;
        material.multi_map_texture = Some(5);
        material.shader_package_name = Some("characterReflection.shpk".to_string());
        material.staining_application = Some(ModelStainingApplication {
            stain_ids: [1, 0],
            template_path: "chara/base_material/stainingtemplate_gud.stm".to_string(),
            report: StainingApplicationReport {
                rows_skipped_missing_template: 1,
                ..StainingApplicationReport::default()
            },
            error: None,
        });

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::CrestChange)
                .unsupported_inputs,
            PreparedMaterialUnsupportedInputs {
                dye_application: true,
                runtime_color_table: true,
                decal_or_crest: true,
                runtime_material_change: false,
                tile_array: true,
                detail_array: true,
                incomplete_shader_family_logic: true,
            }
        );
        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::MaterialChange)
                .unsupported_inputs,
            PreparedMaterialUnsupportedInputs {
                dye_application: true,
                runtime_color_table: true,
                decal_or_crest: false,
                runtime_material_change: false,
                tile_array: true,
                detail_array: true,
                incomplete_shader_family_logic: true,
            }
        );

        assert_eq!(
            prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal).unsupported_inputs,
            PreparedMaterialUnsupportedInputs::default()
        );
    }

    #[test]
    fn prepared_material_reports_shared_array_availability_and_runtime_fallbacks() {
        let mut material = test_material();
        material.texture_arrays.tile_normal = Some(1);
        material.texture_arrays.tile_orb = Some(2);
        material.texture_arrays.detail_diffuse = Some(3);

        let prepared =
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::CrestChange);
        assert_eq!(
            prepared.resource_availability,
            PreparedMaterialResourceAvailability {
                tile_array_complete: true,
                detail_array_complete: false,
            }
        );
        assert_eq!(
            prepared.runtime_fallbacks,
            PreparedMaterialRuntimeFallbacks {
                decal_or_crest: PreparedRuntimeFallback::TransparentTexture,
                material_change: PreparedRuntimeFallback::NotRequired,
            }
        );

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::MaterialChange)
                .runtime_fallbacks,
            PreparedMaterialRuntimeFallbacks {
                decal_or_crest: PreparedRuntimeFallback::NotRequired,
                material_change: PreparedRuntimeFallback::BaseMaterial,
            }
        );

        material.texture_arrays.detail_normal = Some(4);
        let prepared = prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal);
        assert!(!prepared.unsupported_inputs.tile_array);
        assert!(!prepared.unsupported_inputs.detail_array);
    }

    #[test]
    fn prepared_material_treats_index_texture_as_color_table_input() {
        let mut material = test_material();
        material.index_texture = Some(3);

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal)
                .feature_flags
                .uses_color_table,
            true
        );
    }

    #[test]
    fn prepared_material_reports_texture_and_scroll_uv_sources() {
        let mut material = test_material();
        material.shader_package_name = Some("bguvscroll.shpk".to_string());
        material.base_color_texture = Some(1);
        material.normal_texture = Some(2);
        material.multi_map_texture = Some(3);
        material.material_properties_texture = Some(4);
        material.specular_texture = Some(5);

        assert_eq!(
            prepare_material_for_draw_role(Some(&material), ModelMeshDrawRole::Normal).uv_sources,
            PreparedMaterialUvSources {
                textures: PreparedTextureUvSources {
                    base_color: PreparedUvSource::Uv0,
                    normal: PreparedUvSource::Uv0,
                    mask: PreparedUvSource::Uv0,
                    material_map: PreparedUvSource::Uv0,
                    multi_map: PreparedUvSource::Uv0,
                    specular: PreparedUvSource::Uv0,
                    emissive: PreparedUvSource::Uv0,
                    material_properties: PreparedUvSource::Uv0,
                    tile_properties: PreparedUvSource::Uv0,
                    sheen_properties: PreparedUvSource::Uv0,
                    sphere_properties: PreparedUvSource::Uv0,
                    tile_matrix: PreparedUvSource::Uv0,
                    index: PreparedUvSource::Uv0,
                    other: PreparedUvSource::Uv0,
                },
                scroll: PreparedTextureScrollSet {
                    base_color: true,
                    normal: true,
                    specular: true,
                    ..PreparedTextureScrollSet::default()
                },
                uv0_scroll: PreparedUvSource::Uv0,
                uv1_scroll: PreparedUvSource::Uv1,
            }
        );
        assert_eq!(
            prepare_material_for_draw_role(None, ModelMeshDrawRole::Normal).uv_sources,
            PreparedMaterialUvSources::default()
        );
    }

    #[test]
    fn prepared_model_reports_mesh_level_draw_decisions() {
        let mut glass_material = test_material();
        glass_material.shader_package_name = Some("characterglass.shpk".to_string());
        let mut normal_mesh = test_model_mesh(None, 0);
        normal_mesh.submesh = Some(test_model_submesh_info());
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![test_material(), glass_material],
            textures: Vec::new(),
            meshes: vec![
                normal_mesh,
                test_model_mesh(Some("glass"), 1),
                test_model_mesh(Some("lightShaft"), 99),
            ],
        };

        let prepared = prepare_model_for_render(&model);
        assert_eq!(prepared.meshes.len(), 3);
        assert_eq!(prepared.meshes[0].mesh_index, 0);
        assert_eq!(prepared.meshes[0].material_slot, 0);
        assert_eq!(prepared.meshes[0].draw_role, ModelMeshDrawRole::Normal);
        assert!(prepared.meshes[0].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[0].visibility,
            PreparedMeshVisibility::default()
        );
        assert_eq!(prepared.meshes[0].submesh, Some(test_model_submesh_info()));
        assert_eq!(
            prepared.meshes[0].prepared_material.render_pass,
            PreparedRenderPass::Opaque
        );

        assert_eq!(prepared.meshes[1].mesh_index, 1);
        assert_eq!(prepared.meshes[1].draw_role, ModelMeshDrawRole::Glass);
        assert!(prepared.meshes[1].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[1].prepared_material.shader_family,
            MaterialShaderFamily::CharacterGlass
        );
        assert_eq!(
            prepared.meshes[1].prepared_material.render_pass,
            PreparedRenderPass::Glass
        );

        assert_eq!(prepared.meshes[2].mesh_index, 2);
        assert_eq!(prepared.meshes[2].material_slot, 99);
        assert_eq!(prepared.meshes[2].draw_role, ModelMeshDrawRole::LightShaft);
        assert!(!prepared.meshes[2].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[2].prepared_material.render_pass,
            PreparedRenderPass::AdditiveLightShaft
        );
        assert_eq!(
            prepared.meshes[2].prepared_material.shader_family,
            MaterialShaderFamily::Unknown
        );
    }

    #[test]
    fn prepared_model_applies_explicit_enabled_attribute_mask() {
        let mut visible_mesh = test_model_mesh(None, 0);
        visible_mesh.submesh = Some(test_model_submesh_info());
        let mut hidden_mesh = test_model_mesh(None, 0);
        hidden_mesh.submesh = Some(test_model_submesh_info());
        let mut no_attribute_mesh = test_model_mesh(None, 0);
        no_attribute_mesh.submesh = Some(ModelSubmeshInfo {
            attribute_index_mask: 0,
            attribute_index_mask_hex: "0x00000000".to_string(),
            attribute_names: Vec::new(),
            ..test_model_submesh_info()
        });
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![test_material()],
            textures: Vec::new(),
            meshes: vec![visible_mesh, hidden_mesh, no_attribute_mesh],
        };

        let prepared = prepare_model_for_render_with_options(
            &model,
            PreparedModelOptions::default().with_enabled_attribute_mask(0x0000_0005),
        );
        assert!(prepared.meshes[0].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[0].visibility,
            PreparedMeshVisibility {
                submesh_attributes_visible: true,
                enabled_attribute_mask: Some(0x0000_0005),
                missing_attribute_mask: 0,
            }
        );
        assert!(prepared.meshes[2].renders_in_main_pass);

        let prepared = prepare_model_for_render_with_options(
            &model,
            PreparedModelOptions::default().with_enabled_attribute_mask(0x0000_0001),
        );
        assert!(!prepared.meshes[1].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[1].visibility,
            PreparedMeshVisibility {
                submesh_attributes_visible: false,
                enabled_attribute_mask: Some(0x0000_0001),
                missing_attribute_mask: 0x0000_0004,
            }
        );
        assert!(prepared.meshes[2].renders_in_main_pass);
    }

    #[test]
    fn prepared_model_reports_enabled_shape_influences_without_filtering_meshes() {
        let mut mesh = test_model_mesh(None, 0);
        mesh.shape_influences = vec![test_shape_info(0), test_shape_info(2)];
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![test_material()],
            textures: Vec::new(),
            meshes: vec![mesh],
        };

        let prepared = prepare_model_for_render(&model);

        assert!(prepared.meshes[0].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[0].shape_influences,
            vec![test_shape_info(0), test_shape_info(2)]
        );
        assert_eq!(
            prepared.meshes[0].shape_influence_state,
            PreparedMeshShapeInfluences {
                available_shape_mask: 0x0000_0005,
                enabled_shape_mask: None,
                active_shape_mask: 0,
                inactive_shape_mask: 0,
            }
        );

        let prepared = prepare_model_for_render_with_options(
            &model,
            PreparedModelOptions::default().with_enabled_shape_mask(0x0000_0001),
        );

        assert!(prepared.meshes[0].renders_in_main_pass);
        assert_eq!(
            prepared.meshes[0].shape_influence_state,
            PreparedMeshShapeInfluences {
                available_shape_mask: 0x0000_0005,
                enabled_shape_mask: Some(0x0000_0001),
                active_shape_mask: 0x0000_0001,
                inactive_shape_mask: 0x0000_0004,
            }
        );
    }

    #[test]
    fn prepared_model_reports_mesh_level_flow_feature_flags() {
        let mut plain_mesh = test_model_mesh(None, 0);
        plain_mesh.vertices = vec![test_model_vertex()];
        let mut flow_mesh = test_model_mesh(None, 0);
        let mut flow_vertex = test_model_vertex();
        flow_vertex.flow0 = Some([0.25, 0.5, 0.75, 1.0]);
        flow_mesh.vertices = vec![flow_vertex];
        let mut material = test_material();
        material.flow_mode = MaterialFlowMode::Flow;
        let model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![material],
            textures: Vec::new(),
            meshes: vec![plain_mesh, flow_mesh.clone()],
        };

        let prepared = prepare_model_for_render(&model);

        assert!(!prepared.meshes[0].prepared_material.feature_flags.uses_flow);
        assert!(prepared.meshes[1].prepared_material.feature_flags.uses_flow);

        let mut standard_material = test_material();
        standard_material.flow_mode = MaterialFlowMode::Standard;
        let standard_model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![standard_material],
            textures: Vec::new(),
            meshes: vec![flow_mesh],
        };
        assert!(
            !prepare_model_for_render(&standard_model).meshes[0]
                .prepared_material
                .feature_flags
                .uses_flow
        );

        let mut secondary_only_mesh = test_model_mesh(None, 0);
        let mut secondary_only_vertex = test_model_vertex();
        secondary_only_vertex.flow1 = Some([0.25, 0.5, 0.75, 1.0]);
        secondary_only_mesh.vertices = vec![secondary_only_vertex];
        let mut flow_material = test_material();
        flow_material.flow_mode = MaterialFlowMode::Flow;
        let secondary_only_model = crate::ModelData {
            bounds: crate::ModelBounds::default(),
            materials: vec![flow_material],
            textures: Vec::new(),
            meshes: vec![secondary_only_mesh],
        };
        assert!(
            !prepare_model_for_render(&secondary_only_model).meshes[0]
                .prepared_material
                .feature_flags
                .uses_flow
        );
    }

    #[test]
    fn prepared_texture_sampling_matches_meddletools_role_configs() {
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::BaseColor),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::Srgb,
                filter: PreparedTextureFilter::Linear,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::Specular),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::Srgb,
                filter: PreparedTextureFilter::Linear,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::Normal),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::NonColor,
                filter: PreparedTextureFilter::Linear,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::Mask),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::NonColor,
                filter: PreparedTextureFilter::Linear,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::MaterialProperties),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::NonColor,
                filter: PreparedTextureFilter::Linear,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::Index),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::NonColor,
                filter: PreparedTextureFilter::Nearest,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        assert_eq!(
            prepared_texture_sampling_for_kind(ModelTextureKind::TileProperties),
            PreparedTextureSampling {
                color_space: PreparedTextureColorSpace::NonColor,
                filter: PreparedTextureFilter::Nearest,
                address_mode: PreparedTextureAddressMode::Repeat,
            }
        );
        let array_sampling = PreparedTextureSampling {
            color_space: PreparedTextureColorSpace::NonColor,
            filter: PreparedTextureFilter::Nearest,
            address_mode: PreparedTextureAddressMode::Repeat,
        };
        let sampling_set = PreparedTextureSamplingSet::default();
        assert_eq!(sampling_set.tile_normal_array, array_sampling);
        assert_eq!(sampling_set.tile_orb_array, array_sampling);
        assert_eq!(sampling_set.detail_diffuse_array, array_sampling);
        assert_eq!(sampling_set.detail_normal_array, array_sampling);
    }

    #[test]
    fn material_shader_family_maps_known_character_shader_packages() {
        assert_eq!(
            material_shader_family(Some("character.shpk")),
            MaterialShaderFamily::Character
        );
        assert_eq!(
            material_shader_family(Some("characterlegacy.shpk")),
            MaterialShaderFamily::Character
        );
        assert_eq!(
            material_shader_family(Some("chara/weapon/test/CHARACTERSTOCKINGS.SHPK")),
            MaterialShaderFamily::CharacterStockings
        );
        assert_eq!(
            material_shader_family(Some("characterglass.shpk")),
            MaterialShaderFamily::CharacterGlass
        );
        assert_eq!(
            material_shader_family(Some("characterreflection.shpk")),
            MaterialShaderFamily::CharacterReflection
        );
        assert_eq!(
            material_shader_family(Some("charactertransparency.shpk")),
            MaterialShaderFamily::CharacterTransparency
        );
        assert_eq!(
            material_shader_family(Some("characterscroll.shpk")),
            MaterialShaderFamily::CharacterScroll
        );
        assert_eq!(
            material_shader_family(Some("bg.shpk")),
            MaterialShaderFamily::Bg
        );
        assert_eq!(
            material_shader_family(Some("bguvscroll.shpk")),
            MaterialShaderFamily::BgUvScroll
        );
        assert_eq!(
            material_shader_family(Some("lightshaft.shpk")),
            MaterialShaderFamily::LightShaft
        );
        assert_eq!(
            material_shader_family(Some("river.shpk")),
            MaterialShaderFamily::Water
        );
        assert_eq!(
            material_shader_family(Some("charactertattoo.shpk")),
            MaterialShaderFamily::CharacterTattoo
        );
        assert_eq!(
            material_shader_family(Some("characterocclusion.shpk")),
            MaterialShaderFamily::CharacterOcclusion
        );
        assert_eq!(material_shader_family(None), MaterialShaderFamily::Unknown);
        assert_eq!(
            material_shader_family(Some("unknown.shpk")),
            MaterialShaderFamily::Unknown
        );
    }

    fn test_prepared_render_pass(
        alpha_mode: MaterialAlphaMode,
        render_mode: MaterialRenderMode,
        draw_role: ModelMeshDrawRole,
    ) -> PreparedRenderPass {
        let mut material = test_material();
        material.alpha_mode = alpha_mode;
        material.render_mode = render_mode;
        prepare_material_for_draw_role(Some(&material), draw_role).render_pass
    }

    fn test_material() -> ModelMaterial {
        ModelMaterial {
            slot: 0,
            material_index: 0,
            name: "test".to_string(),
            path: None,
            shader_package_name: None,
            render_mode: MaterialRenderMode::Opaque,
            alpha_mode: MaterialAlphaMode::Opaque,
            alpha_threshold: 0.0,
            draw_depth_mode: MaterialDrawDepthMode::None,
            lighting_mode: MaterialLightingMode::Default,
            flow_mode: MaterialFlowMode::Standard,
            transparency: 0.0,
            alpha_aperture: 2.0,
            alpha_offset: 0.0,
            shadow_alpha_threshold: 0.5,
            glass_ior: 1.0,
            glass_thickness_max: 0.01,
            normal_scale: 1.0,
            multi_normal_scale: 1.0,
            detail_normal_scale: 1.0,
            multi_detail_normal_scale: 1.0,
            tile_index: 0.0,
            tile_alpha: 1.0,
            tile_scale: [16.0, 16.0],
            toon_index: 0.0,
            toon_light_scale: 2.0,
            toon_light_spec_aperture: 50.0,
            toon_reflection_scale: 2.5,
            toon_spec_index: 4.0e-45,
            sheen_rate: 0.0,
            sheen_tint_rate: 0.0,
            sheen_aperture: 1.0,
            sphere_map_index: 0.0,
            detail_id: 0.0,
            multi_detail_id: 0.0,
            detail_color: [0.5, 0.5, 0.5, 1.0],
            multi_detail_color: [0.5, 0.5, 0.5, 1.0],
            shader_diffuse_color: [1.0, 1.0, 1.0, 1.0],
            shader_multi_diffuse_color: [1.0, 1.0, 1.0, 1.0],
            shader_emissive_color: [0.0, 0.0, 0.0, 1.0],
            shader_multi_emissive_color: [0.0, 0.0, 0.0, 1.0],
            outline_color: [0.0, 0.0, 0.0, 1.0],
            outline_width: 0.0,
            specular_color_mask: [1.0, 1.0, 1.0, 1.0],
            ssao_mask: 1.0,
            texture_mip_bias: 0.0,
            shadow_pos_offset: 0.0,
            detail_color_uv_scale: [4.0, 4.0, 4.0, 4.0],
            detail_normal_uv_scale: [4.0, 4.0, 4.0, 4.0],
            uv_scroll: [0.0, 0.0, 0.0, 0.0],
            lightshaft_color: [1.0, 1.0, 1.0, 1.0],
            lightshaft_tex_anim: [0.0, 0.0, 0.0, 0.0],
            lightshaft_tex_u: [1.0, 0.0, 0.0, 0.0],
            lightshaft_tex_v: [0.0, 1.0, 0.0, 0.0],
            lightshaft_ray: [0.0, 0.0, 0.0, 0.0],
            opacity: 1.0,
            render_backfaces: true,
            apply_vertex_color: false,
            has_color_dye_table: false,
            color_dye_table: None,
            staining_application: None,
            texture_arrays: ModelMaterialTextureArrays::default(),
            fallback_color: [1.0, 1.0, 1.0],
            diffuse_color: [1.0, 1.0, 1.0],
            specular_color: [1.0, 1.0, 1.0],
            emissive_color: [0.0, 0.0, 0.0],
            roughness: 0.5,
            metalness: 0.0,
            texture_indices: Vec::new(),
            base_color_texture: None,
            normal_texture: None,
            mask_texture: None,
            material_map_texture: None,
            multi_map_texture: None,
            specular_texture: None,
            emissive_texture: None,
            material_properties_texture: None,
            tile_properties_texture: None,
            sheen_properties_texture: None,
            sphere_properties_texture: None,
            tile_matrix_texture: None,
            index_texture: None,
        }
    }

    fn test_model_mesh(category: Option<&str>, material_slot: usize) -> ModelMesh {
        ModelMesh {
            path: "test.mdl".to_string(),
            part_index: 0,
            mesh_category: category.map(str::to_string),
            submesh: None,
            shape_influences: Vec::new(),
            material_index: material_slot as u16,
            material_slot,
            material_name: "test".to_string(),
            color: [1.0, 1.0, 1.0],
            bone_table: None,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn test_model_vertex() -> ModelVertex {
        ModelVertex {
            position: [0.0, 0.0, 0.0],
            blend_weights: None,
            blend_indices: None,
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0, 0.0],
            uv1: [0.0, 0.0],
            uv2: [0.0, 0.0],
            uv3: [0.0, 0.0],
            bitangent: [1.0, 0.0, 0.0, 1.0],
            normal1: None,
            bitangent1: None,
            color: [1.0, 1.0, 1.0, 1.0],
            color1: None,
            flow0: None,
            flow1: None,
        }
    }

    fn test_model_submesh_info() -> ModelSubmeshInfo {
        ModelSubmeshInfo {
            index: 2,
            table_index: 4,
            attribute_index_mask: 0x0000_0005,
            attribute_index_mask_hex: "0x00000005".to_string(),
            attribute_names: vec!["attr_a".to_string(), "attr_c".to_string()],
            bone_start_index: 6,
            bone_count: 2,
        }
    }

    fn test_shape_info(index: usize) -> ModelShapeInfo {
        let shape_index_mask = 1_u32 << index;
        ModelShapeInfo {
            index,
            name: Some(format!("shape_{index}")),
            shape_index_mask,
            shape_index_mask_hex: format!("0x{shape_index_mask:08X}"),
            shape_mesh_index: index + 10,
            shape_value_count: index as u32 + 1,
        }
    }

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
    fn bake_preserves_tile_matrix_float_values() {
        let rows = vec![
            ColorTableRowColors {
                tile_matrix: [2.0, -0.5, 0.25, 1.5],
                ..Default::default()
            },
            ColorTableRowColors {
                tile_matrix: [0.0, 0.0, 0.0, 0.0],
                ..Default::default()
            },
        ];
        let id_rgba = [0, 0, 0, 255];

        let baked = bake_color_table_maps(&rows, &id_rgba).expect("bake");

        assert_eq!(&baked.tile_matrix_rgba[0..4], &[255, 0, 64, 255]);
        assert_eq!(baked.tile_matrix_rgba_f32, vec![[2.0, -0.5, 0.25, 1.5]]);
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
    fn bake_maps_legacy_pair_steps() {
        // 16 行 Legacy ColorTable = 8 个行对，R=255 选择最后一对的偶数行 14。
        let mut rows = vec![ColorTableRowColors::default(); 16];
        rows[14] = ColorTableRowColors {
            diffuse: [1.0, 1.0, 1.0],
            ..Default::default()
        };
        rows[15] = ColorTableRowColors {
            diffuse: [1.0, 0.0, 0.0],
            ..Default::default()
        };

        let id_rgba = [255, 0, 0, 255, 255, 255, 0, 255];
        let baked = bake_color_table_maps(&rows, &id_rgba).expect("bake");

        assert_eq!(&baked.diffuse_rgba[0..4], &[255, 255, 255, 255]);
        assert_eq!(&baked.diffuse_rgba[4..8], &[255, 0, 0, 255]);
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
