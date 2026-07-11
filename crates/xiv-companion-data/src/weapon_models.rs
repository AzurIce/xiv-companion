pub use crate::model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialDrawDepthMode, MaterialFlowMode,
    MaterialLightingMode, MaterialRenderMode, MaterialSubColorMode, MaterialValueMode, ModelBounds,
    ModelColorDyeTable, ModelData, ModelDawntrailColorDyeTableRow, ModelLegacyColorDyeTableRow,
    ModelMaterial, ModelMaterialTextureArrays, ModelMesh, ModelMeshDrawRole, ModelRenderData,
    ModelStainingApplication, ModelSubmeshInfo, ModelTexture, ModelTextureKind, ModelVertex,
    PackedModelId, PreparedMeshVisibility, PreparedModelOptions, StainingApplicationReport,
    WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage, WeaponMaterialAlphaMode,
    WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData,
    WeaponModelLoadCandidateDiagnostic, WeaponModelLoadCandidateStatus, WeaponModelLoadDiagnostic,
    WeaponModelLoadRole, WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture,
    WeaponModelTextureKind, WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, mesh_draw_role_for_category,
    weapon_material_candidate_paths, weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "game-data")]
use std::collections::HashMap;

#[cfg(feature = "game-data")]
use crate::staining::{
    DAWNTRAIL_STAINING_TEMPLATE_PATH, LEGACY_STAINING_TEMPLATE_PATH, MAX_STAIN_ID,
    StainingTemplate, apply_staining_template_to_rows,
};

pub const CHARACTER_TILE_NORMAL_ARRAY_PATH: &str = "chara/common/texture/tile_norm_array.tex";
pub const CHARACTER_TILE_ORB_ARRAY_PATH: &str = "chara/common/texture/tile_orb_array.tex";
pub const BG_DETAIL_DIFFUSE_ARRAY_PATH: &str = "bgcommon/nature/detail/texture/detail_d_array.tex";
pub const BG_DETAIL_NORMAL_ARRAY_PATH: &str = "bgcommon/nature/detail/texture/detail_n_array.tex";

#[cfg(feature = "game-data")]
const APPLY_ALPHA_TEST: u32 = 0xA9A3_EE25;
#[cfg(feature = "game-data")]
const APPLY_ALPHA_TEST_ON: u32 = 0x72AA_A9AE;
#[cfg(feature = "game-data")]
const G_ALPHA_THRESHOLD: u32 = 0x29AC_0223;
#[cfg(feature = "game-data")]
const G_ALPHA_APERTURE: u32 = 0xD62B_F368;
#[cfg(feature = "game-data")]
const G_ALPHA_OFFSET: u32 = 0xD07A_6A65;
#[cfg(feature = "game-data")]
const G_SHADOW_ALPHA_THRESHOLD: u32 = 0xD925_FF32;
#[cfg(feature = "game-data")]
const G_TRANSPARENCY: u32 = 0x53E8_417B;
#[cfg(feature = "game-data")]
const G_WATER_DEEP_COLOR: u32 = 0xD315_E728;
#[cfg(feature = "game-data")]
const G_WATER_REFRACTION_COLOR: u32 = 0xBA16_3700;
#[cfg(feature = "game-data")]
const G_WATER_WHITECAP_COLOR: u32 = 0x29FA_2AC1;
#[cfg(feature = "game-data")]
const DRAW_DEPTH_MODE: u32 = 0xE8DA_5B62;
#[cfg(feature = "game-data")]
const DRAW_DEPTH_MODE_DITHER: u32 = 0x7B80_4D6E;
#[cfg(feature = "game-data")]
const ENABLE_LIGHTING: u32 = 0x0033_C8B5;
#[cfg(feature = "game-data")]
const ENABLE_LIGHTING_OFF: u32 = 0x93D6_C21A;
#[cfg(feature = "game-data")]
const ENABLE_LIGHTING_ON: u32 = 0xD1E6_0FD9;
#[cfg(feature = "game-data")]
const CATEGORY_FLOW_MAP_TYPE: u32 = 0x40D1_481E;
#[cfg(feature = "game-data")]
const FLOW_MAP_STANDARD: u32 = 0x337C_6BC4;
#[cfg(feature = "game-data")]
const FLOW_MAP_FLOW: u32 = 0x71AD_A939;
#[cfg(feature = "game-data")]
const GET_VALUES: u32 = 0xB616_DC5A;
#[cfg(feature = "game-data")]
const GET_VALUES_MULTI: u32 = 0x1DF2_985C;
#[cfg(feature = "game-data")]
const GET_VALUES_MULTI_MATERIAL: u32 = 0x5CC6_05B5;
#[cfg(feature = "game-data")]
const GET_VALUES_COMPATIBILITY: u32 = 0x600E_F9DF;
#[cfg(feature = "game-data")]
const GET_VALUES_SINGLE: u32 = 0x669A_451B;
#[cfg(feature = "game-data")]
const GET_ALPHA_MULTI_VALUES: u32 = 0x9418_20BE;
#[cfg(feature = "game-data")]
const GET_ALPHA_MULTI_VALUES2: u32 = 0xE49A_D72B;
#[cfg(feature = "game-data")]
const GET_ALPHA_MULTI_VALUES3: u32 = 0x939D_E7BD;
#[cfg(feature = "game-data")]
const GET_SUB_COLOR: u32 = 0x2482_6489;
#[cfg(feature = "game-data")]
const GET_SUB_COLOR_FACE: u32 = 0x6E5B_8F10;
#[cfg(feature = "game-data")]
const GET_SUB_COLOR_HAIR: u32 = 0xF7B8_956E;
#[cfg(feature = "game-data")]
const G_GLASS_IOR: u32 = 0x7801_E004;
#[cfg(feature = "game-data")]
const G_GLASS_THICKNESS_MAX: u32 = 0xC464_7F37;
#[cfg(feature = "game-data")]
const G_NORMAL_SCALE: u32 = 0xB554_5FBB;
#[cfg(feature = "game-data")]
const G_MULTI_NORMAL_SCALE: u32 = 0x793A_C5A3;
#[cfg(feature = "game-data")]
const G_DETAIL_NORMAL_SCALE: u32 = 0x9F42_EDA2;
#[cfg(feature = "game-data")]
const G_MULTI_DETAIL_NORMAL_SCALE: u32 = 0xA83D_BDF1;
#[cfg(feature = "game-data")]
const G_TILE_ALPHA: u32 = 0x12C6_AC9F;
#[cfg(feature = "game-data")]
const G_TILE_INDEX: u32 = 0x4255_F2F4;
#[cfg(feature = "game-data")]
const G_TILE_SCALE: u32 = 0x2E60_B071;
#[cfg(feature = "game-data")]
const G_TOON_INDEX: u32 = 0xDF15_112D;
#[cfg(feature = "game-data")]
const G_TOON_LIGHT_SCALE: u32 = 0x3CCE_9E4C;
#[cfg(feature = "game-data")]
const G_TOON_LIGHT_SPEC_APERTURE: u32 = 0x7590_36EE;
#[cfg(feature = "game-data")]
const G_TOON_REFLECTION_SCALE: u32 = 0xD96F_AF7A;
#[cfg(feature = "game-data")]
const G_TOON_SPEC_INDEX: u32 = 0x00A6_80BC;
#[cfg(feature = "game-data")]
const G_SHEEN_APERTURE: u32 = 0xF490_F76E;
#[cfg(feature = "game-data")]
const G_SHEEN_RATE: u32 = 0x800E_E35F;
#[cfg(feature = "game-data")]
const G_SHEEN_TINT_RATE: u32 = 0x1F26_4897;
#[cfg(feature = "game-data")]
const G_SPHERE_MAP_INDEX: u32 = 0x0749_53E9;
#[cfg(feature = "game-data")]
const G_DETAIL_COLOR_UV_SCALE: u32 = 0xC63D_9716;
#[cfg(feature = "game-data")]
const G_DETAIL_ID: u32 = 0x8981_D4D9;
#[cfg(feature = "game-data")]
const G_DETAIL_NORMAL_UV_SCALE: u32 = 0x025A_9BEE;
#[cfg(feature = "game-data")]
const G_MULTI_DETAIL_ID: u32 = 0xAC15_6136;
#[cfg(feature = "game-data")]
const G_DETAIL_COLOR: u32 = 0xDD93_D839;
#[cfg(feature = "game-data")]
const G_MULTI_DETAIL_COLOR: u32 = 0x11FD_4221;
#[cfg(feature = "game-data")]
const G_DIFFUSE_COLOR: u32 = 0x2C2A_34DD;
#[cfg(feature = "game-data")]
const G_MULTI_DIFFUSE_COLOR: u32 = 0x3F8A_C211;
#[cfg(feature = "game-data")]
const G_EMISSIVE_COLOR: u32 = 0x38A6_4362;
#[cfg(feature = "game-data")]
const G_MULTI_EMISSIVE_COLOR: u32 = 0xAA67_6D0F;
#[cfg(feature = "game-data")]
const G_OUTLINE_COLOR: u32 = 0x623C_C4FE;
#[cfg(feature = "game-data")]
const G_OUTLINE_WIDTH: u32 = 0x8870_C938;
#[cfg(feature = "game-data")]
const G_SPECULAR_COLOR_MASK: u32 = 0xCB03_38DC;
#[cfg(feature = "game-data")]
const G_SSAO_MASK: u32 = 0xB7FA_33E2;
#[cfg(feature = "game-data")]
const G_TEXTURE_MIP_BIAS: u32 = 0x3955_1220;
#[cfg(feature = "game-data")]
const G_SHADOW_POS_OFFSET: u32 = 0x5351_646E;
#[cfg(feature = "game-data")]
const G_UV_SCROLL_TIME: u32 = 0x9A69_6A17;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_TEX_ANIM: u32 = 0x14D8_E13D;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_TEX_U: u32 = 0x5926_A043;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_TEX_V: u32 = 0xC02F_F1F9;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_RAY: u32 = 0x827B_DD09;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_COLOR: u32 = 0xD27C_58B9;
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
    pub stain_ids: [u8; 2],
}

#[cfg(feature = "game-data")]
impl WeaponModelLoadRequest {
    pub fn primary_model(&self) -> PackedModelId {
        PackedModelId::from_raw(self.model_main)
    }

    pub fn secondary_model(&self) -> Option<PackedModelId> {
        (self.model_sub != 0).then(|| PackedModelId::from_raw(self.model_sub))
    }

    pub fn with_stain_ids(mut self, stain_ids: [u8; 2]) -> Self {
        self.stain_ids = stain_ids;
        self
    }

    fn normalized_stain_ids(&self) -> [u8; 2] {
        self.stain_ids
            .map(|stain_id| (stain_id <= MAX_STAIN_ID).then_some(stain_id).unwrap_or(0))
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
            stain_ids: [0, 0],
        }
    }
}

#[cfg(feature = "game-data")]
#[derive(Default)]
struct WeaponStainingTemplateLoad {
    template: Option<StainingTemplate>,
    error: Option<String>,
}

#[cfg(feature = "game-data")]
struct WeaponStainingTemplates {
    stain_ids: [u8; 2],
    legacy: WeaponStainingTemplateLoad,
    dawntrail: WeaponStainingTemplateLoad,
}

#[cfg(feature = "game-data")]
impl WeaponStainingTemplates {
    fn disabled(stain_ids: [u8; 2]) -> Self {
        Self {
            stain_ids,
            legacy: WeaponStainingTemplateLoad::default(),
            dawntrail: WeaponStainingTemplateLoad::default(),
        }
    }

    fn requested(&self) -> bool {
        self.stain_ids.iter().any(|stain_id| *stain_id != 0)
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
    pub summary: MaterialSemanticSummaryDebug,
    pub file_header: Option<MaterialFileHeaderDebug>,
    pub shader_package_name: String,
    pub shader_header: Option<MaterialShaderHeaderDebug>,
    pub shader_flags: u32,
    pub shader_flags_hex: String,
    pub texture_paths: Vec<String>,
    pub texture_offsets: Vec<MaterialTextureOffsetDebug>,
    pub uv_color_sets: Vec<MaterialNamedSetDebug>,
    pub color_sets: Vec<MaterialNamedSetDebug>,
    pub additional_data: Vec<u8>,
    pub data_set_size: usize,
    pub shader_keys: Vec<MaterialShaderKeyDebug>,
    pub shader_value_list_size: usize,
    pub shader_value_count: usize,
    pub constants: Vec<MaterialConstantDebug>,
    pub constants_debug: Vec<String>,
    pub samplers: Vec<MaterialSamplerDebug>,
    pub color_table: Option<MaterialColorTableDebug>,
    pub color_dye_table_kind: Option<String>,
    pub color_dye_table: Option<MaterialColorDyeTableDebug>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSemanticSummaryDebug {
    pub shader_flags: u32,
    pub shader_flags_hex: String,
    pub shader_key_count: usize,
    pub resolved_shader_key_count: usize,
    pub resolved_constant_count: usize,
    pub texture_flag_count: usize,
    pub sampler_flag_count: usize,
    pub shader_keys: Vec<MaterialResolvedShaderKeyDebug>,
    pub constants: Vec<MaterialResolvedConstantDebug>,
    pub texture_flags: Vec<MaterialTextureFlagSummaryDebug>,
    pub sampler_flags: Vec<MaterialSamplerFlagSummaryDebug>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialResolvedShaderKeyDebug {
    pub category: u32,
    pub category_hex: String,
    pub category_name: Option<String>,
    pub value: u32,
    pub value_hex: String,
    pub value_name: Option<String>,
    pub source: String,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialResolvedConstantDebug {
    pub id: u32,
    pub id_hex: String,
    pub name: Option<String>,
    pub value_count: usize,
    pub values: Vec<f32>,
    pub source: String,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTextureFlagSummaryDebug {
    pub index: usize,
    pub flags: u16,
    pub flags_hex: String,
    pub path: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSamplerFlagSummaryDebug {
    pub texture_index: usize,
    pub texture_path: Option<String>,
    pub texture_usage: u32,
    pub texture_usage_hex: String,
    pub texture_usage_name: Option<String>,
    pub flags: u32,
    pub flags_hex: String,
    pub kind: Option<WeaponModelTextureKind>,
    pub kind_source: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialFileHeaderDebug {
    pub version: u32,
    pub version_hex: String,
    pub file_size: u16,
    pub data_set_size: u16,
    pub string_table_size: u16,
    pub shader_package_name_offset: u16,
    pub texture_count: u8,
    pub uv_set_count: u8,
    pub color_set_count: u8,
    pub additional_data_size: u8,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialShaderHeaderDebug {
    pub shader_value_list_size: u16,
    pub shader_key_count: u16,
    pub constant_count: u16,
    pub sampler_count: u16,
    pub flags: u32,
    pub flags_hex: String,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialTextureOffsetDebug {
    pub index: usize,
    pub offset: u16,
    pub flags: u16,
    pub flags_hex: String,
    pub path: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialNamedSetDebug {
    pub index: usize,
    pub name_offset: u16,
    pub set_index: u8,
    pub unknown1: u8,
    pub name: Option<String>,
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
pub struct MaterialConstantDebug {
    pub id: u32,
    pub id_hex: String,
    pub value_offset: u16,
    pub value_size: u16,
    pub value_count: usize,
    pub raw_values: Vec<u32>,
    pub raw_values_hex: Vec<String>,
    pub values: Vec<f32>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialSamplerDebug {
    pub texture_index: usize,
    pub texture_path: Option<String>,
    pub texture_usage: u32,
    pub texture_usage_hex: String,
    pub texture_usage_name: Option<String>,
    pub flags: u32,
    pub flags_hex: String,
    pub kind: Option<WeaponModelTextureKind>,
    pub kind_source: Option<String>,
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
    pub anisotropy: Option<f32>,
    pub tile_alpha: Option<f32>,
    pub tile_index: Option<f32>,
    pub sheen_rate: Option<f32>,
    pub sheen_tint: Option<f32>,
    pub sheen_aperture: Option<f32>,
    pub sphere_mask: Option<f32>,
    pub tile_set: Option<u16>,
    pub shader_index: Option<u16>,
    pub sphere_index: Option<u16>,
    pub tile_matrix: Option<[f32; 4]>,
    pub material_repeat: Option<[f32; 2]>,
    pub material_skew: Option<[f32; 2]>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialColorDyeTableDebug {
    pub kind: String,
    pub row_count: usize,
    pub rows: Vec<MaterialColorDyeTableRowDebug>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialColorDyeTableRowDebug {
    pub index: usize,
    pub template: u16,
    pub channel: Option<u8>,
    pub diffuse: bool,
    pub specular: bool,
    pub emissive: bool,
    pub gloss: Option<bool>,
    pub specular_strength: Option<bool>,
    pub scalar3: Option<bool>,
    pub metalness: Option<bool>,
    pub roughness: Option<bool>,
    pub sheen_rate: Option<bool>,
    pub sheen_tint_rate: Option<bool>,
    pub sheen_aperture: Option<bool>,
    pub anisotropy: Option<bool>,
    pub sphere_map_index: Option<bool>,
    pub sphere_map_mask: Option<bool>,
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
#[derive(Clone, Debug, PartialEq, Eq)]
struct WeaponModelMeshLoadFailure {
    model: PackedModelId,
    candidates: Vec<WeaponModelLoadCandidateDiagnostic>,
}

#[cfg(feature = "game-data")]
impl WeaponModelMeshLoadFailure {
    fn new(model: PackedModelId, candidates: Vec<WeaponModelLoadCandidateDiagnostic>) -> Self {
        Self { model, candidates }
    }

    fn message(&self) -> String {
        let tried = self
            .candidates
            .iter()
            .map(|candidate| format!("{}: {}", candidate.path, candidate.error))
            .collect::<Vec<_>>()
            .join("; ");
        format!(
            "unable to read weapon model {} (tried: {})",
            self.model.model_id, tried
        )
    }

    fn into_error(self) -> anyhow::Error {
        anyhow::anyhow!(self.message())
    }

    fn into_diagnostic(self, role: WeaponModelLoadRole) -> WeaponModelLoadDiagnostic {
        WeaponModelLoadDiagnostic {
            role,
            model: self.model,
            error: self.message(),
            candidates: self.candidates,
        }
    }
}

#[cfg(feature = "game-data")]
fn model_load_candidate(
    path: String,
    status: WeaponModelLoadCandidateStatus,
    error: impl Into<String>,
) -> WeaponModelLoadCandidateDiagnostic {
    WeaponModelLoadCandidateDiagnostic {
        path,
        status,
        error: error.into(),
    }
}

#[cfg(feature = "game-data")]
fn load_weapon_staining_templates_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    stain_ids: [u8; 2],
    loaded_paths: &mut Vec<String>,
) -> WeaponStainingTemplates {
    if !stain_ids.iter().any(|stain_id| *stain_id != 0) {
        return WeaponStainingTemplates::disabled(stain_ids);
    }

    WeaponStainingTemplates {
        stain_ids,
        legacy: load_staining_template_from_resource(
            resource,
            LEGACY_STAINING_TEMPLATE_PATH,
            loaded_paths,
        ),
        dawntrail: load_staining_template_from_resource(
            resource,
            DAWNTRAIL_STAINING_TEMPLATE_PATH,
            loaded_paths,
        ),
    }
}

#[cfg(feature = "game-data")]
fn load_staining_template_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    path: &str,
    loaded_paths: &mut Vec<String>,
) -> WeaponStainingTemplateLoad {
    let Some(bytes) = resource.read(path) else {
        return WeaponStainingTemplateLoad {
            template: None,
            error: Some(format!("failed to read {path}")),
        };
    };
    match StainingTemplate::from_bytes(&bytes) {
        Ok(template) => {
            push_loaded_path(loaded_paths, path.to_string());
            WeaponStainingTemplateLoad {
                template: Some(template),
                error: None,
            }
        }
        Err(error) => WeaponStainingTemplateLoad {
            template: None,
            error: Some(format!("failed to parse {path}: {error:#}")),
        },
    }
}

#[cfg(feature = "game-data")]
async fn load_weapon_staining_templates_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    stain_ids: [u8; 2],
    loaded_paths: &mut Vec<String>,
) -> WeaponStainingTemplates {
    if !stain_ids.iter().any(|stain_id| *stain_id != 0) {
        return WeaponStainingTemplates::disabled(stain_ids);
    }

    let legacy = load_staining_template_from_async_resource(
        resource,
        LEGACY_STAINING_TEMPLATE_PATH,
        loaded_paths,
    )
    .await;
    let dawntrail = load_staining_template_from_async_resource(
        resource,
        DAWNTRAIL_STAINING_TEMPLATE_PATH,
        loaded_paths,
    )
    .await;
    WeaponStainingTemplates {
        stain_ids,
        legacy,
        dawntrail,
    }
}

#[cfg(feature = "game-data")]
async fn load_staining_template_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    path: &str,
    loaded_paths: &mut Vec<String>,
) -> WeaponStainingTemplateLoad {
    let bytes = match resource.read(path).await {
        Ok(bytes) => bytes,
        Err(error) => {
            return WeaponStainingTemplateLoad {
                template: None,
                error: Some(format!("failed to read {path}: {error}")),
            };
        }
    };
    match StainingTemplate::from_bytes(&bytes) {
        Ok(template) => {
            push_loaded_path(loaded_paths, path.to_string());
            WeaponStainingTemplateLoad {
                template: Some(template),
                error: None,
            }
        }
        Err(error) => WeaponStainingTemplateLoad {
            template: None,
            error: Some(format!("failed to parse {path}: {error:#}")),
        },
    }
}

#[cfg(feature = "game-data")]
fn apply_weapon_staining(
    rows: Option<&mut [ColorTableRowColors]>,
    dye_table: Option<&ModelColorDyeTable>,
    staining: &WeaponStainingTemplates,
) -> Option<ModelStainingApplication> {
    if !staining.requested() {
        return None;
    }
    let dye_table = dye_table?;

    let (template_path, template, load_error) = match dye_table {
        ModelColorDyeTable::Legacy(_) => (
            LEGACY_STAINING_TEMPLATE_PATH,
            staining.legacy.template.as_ref(),
            staining.legacy.error.clone(),
        ),
        ModelColorDyeTable::Dawntrail(_) => {
            if let Some(template) = staining.dawntrail.template.as_ref() {
                (DAWNTRAIL_STAINING_TEMPLATE_PATH, Some(template), None)
            } else if let Some(template) = staining.legacy.template.as_ref() {
                (LEGACY_STAINING_TEMPLATE_PATH, Some(template), None)
            } else {
                let error = [
                    staining.dawntrail.error.as_deref(),
                    staining.legacy.error.as_deref(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("; ");
                (
                    DAWNTRAIL_STAINING_TEMPLATE_PATH,
                    None,
                    (!error.is_empty()).then_some(error),
                )
            }
        }
        ModelColorDyeTable::Opaque => {
            return Some(ModelStainingApplication {
                stain_ids: staining.stain_ids,
                template_path: String::new(),
                report: StainingApplicationReport::default(),
                error: Some("opaque ColorDyeTable cannot be applied".to_string()),
            });
        }
    };

    let Some(rows) = rows else {
        return Some(ModelStainingApplication {
            stain_ids: staining.stain_ids,
            template_path: template_path.to_string(),
            report: StainingApplicationReport::default(),
            error: Some("material has no supported ColorTable rows".to_string()),
        });
    };
    let Some(template) = template else {
        return Some(ModelStainingApplication {
            stain_ids: staining.stain_ids,
            template_path: template_path.to_string(),
            report: StainingApplicationReport::default(),
            error: load_error.or_else(|| Some(format!("{template_path} is unavailable"))),
        });
    };

    Some(ModelStainingApplication {
        stain_ids: staining.stain_ids,
        template_path: template_path.to_string(),
        report: apply_staining_template_to_rows(rows, dye_table, &staining.stain_ids, template),
        error: None,
    })
}

#[cfg(feature = "game-data")]
fn material_needs_tile_arrays(material: &ModelMaterial) -> bool {
    let shader_family =
        crate::model::material_shader_family(material.shader_package_name.as_deref());
    if !matches!(
        shader_family,
        crate::model::MaterialShaderFamily::Character
            | crate::model::MaterialShaderFamily::CharacterStockings
            | crate::model::MaterialShaderFamily::CharacterGlass
            | crate::model::MaterialShaderFamily::CharacterReflection
            | crate::model::MaterialShaderFamily::CharacterTransparency
            | crate::model::MaterialShaderFamily::CharacterScroll
            | crate::model::MaterialShaderFamily::CharacterTattoo
            | crate::model::MaterialShaderFamily::CharacterOcclusion
    ) {
        return false;
    }
    let bindings = crate::model::prepared_texture_bindings(Some(material));
    crate::model::prepared_material_feature_flags(Some(material), shader_family, bindings).uses_tile
}

#[cfg(feature = "game-data")]
fn material_needs_detail_arrays(material: &ModelMaterial) -> bool {
    let shader_family =
        crate::model::material_shader_family(material.shader_package_name.as_deref());
    if !matches!(
        shader_family,
        crate::model::MaterialShaderFamily::Bg | crate::model::MaterialShaderFamily::BgUvScroll
    ) {
        return false;
    }
    let bindings = crate::model::prepared_texture_bindings(Some(material));
    crate::model::prepared_material_feature_flags(Some(material), shader_family, bindings)
        .uses_detail
}

#[cfg(feature = "game-data")]
fn attach_shared_material_arrays_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    materials: &mut [ModelMaterial],
    textures: &mut Vec<ModelTexture>,
    loaded_paths: &mut Vec<String>,
) {
    let needs_tile = materials.iter().any(material_needs_tile_arrays);
    let needs_detail = materials.iter().any(material_needs_detail_arrays);
    let tile_normal = needs_tile.then(|| {
        load_shared_texture_array_from_resource(
            resource,
            CHARACTER_TILE_NORMAL_ARRAY_PATH,
            ModelTextureKind::TileNormalArray,
            textures,
            loaded_paths,
        )
    });
    let tile_orb = needs_tile.then(|| {
        load_shared_texture_array_from_resource(
            resource,
            CHARACTER_TILE_ORB_ARRAY_PATH,
            ModelTextureKind::TileOrbArray,
            textures,
            loaded_paths,
        )
    });
    let detail_diffuse = needs_detail.then(|| {
        load_shared_texture_array_from_resource(
            resource,
            BG_DETAIL_DIFFUSE_ARRAY_PATH,
            ModelTextureKind::DetailDiffuseArray,
            textures,
            loaded_paths,
        )
    });
    let detail_normal = needs_detail.then(|| {
        load_shared_texture_array_from_resource(
            resource,
            BG_DETAIL_NORMAL_ARRAY_PATH,
            ModelTextureKind::DetailNormalArray,
            textures,
            loaded_paths,
        )
    });

    for material in materials {
        if material_needs_tile_arrays(material) {
            apply_shared_array_result(material, &tile_normal, SharedArraySlot::TileNormal);
            apply_shared_array_result(material, &tile_orb, SharedArraySlot::TileOrb);
        }
        if material_needs_detail_arrays(material) {
            apply_shared_array_result(material, &detail_diffuse, SharedArraySlot::DetailDiffuse);
            apply_shared_array_result(material, &detail_normal, SharedArraySlot::DetailNormal);
        }
    }
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy)]
enum SharedArraySlot {
    TileNormal,
    TileOrb,
    DetailDiffuse,
    DetailNormal,
}

#[cfg(feature = "game-data")]
fn apply_shared_array_result(
    material: &mut ModelMaterial,
    result: &Option<Result<usize, String>>,
    slot: SharedArraySlot,
) {
    let Some(result) = result else {
        return;
    };
    match result {
        Ok(index) => {
            let target = match slot {
                SharedArraySlot::TileNormal => &mut material.texture_arrays.tile_normal,
                SharedArraySlot::TileOrb => &mut material.texture_arrays.tile_orb,
                SharedArraySlot::DetailDiffuse => &mut material.texture_arrays.detail_diffuse,
                SharedArraySlot::DetailNormal => &mut material.texture_arrays.detail_normal,
            };
            *target = Some(*index);
            add_unique_index(&mut material.texture_indices, *index);
        }
        Err(error) => {
            if !material.texture_arrays.errors.contains(error) {
                material.texture_arrays.errors.push(error.clone());
            }
        }
    }
}

#[cfg(feature = "game-data")]
fn load_shared_texture_array_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    path: &str,
    kind: ModelTextureKind,
    textures: &mut Vec<ModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Result<usize, String> {
    if let Some(index) = textures.iter().position(|texture| texture.path == path) {
        return Ok(index);
    }
    let bytes = resource
        .read(path)
        .ok_or_else(|| format!("failed to read shared texture array {path}"))?;
    decode_and_push_shared_texture_array(
        resource.platform(),
        path,
        kind,
        &bytes,
        textures,
        loaded_paths,
    )
}

#[cfg(feature = "game-data")]
fn decode_and_push_shared_texture_array(
    platform: physis::Platform,
    path: &str,
    kind: ModelTextureKind,
    bytes: &[u8],
    textures: &mut Vec<ModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Result<usize, String> {
    use physis::ReadableFile;

    let mut texture = physis::tex::Texture::from_existing(platform, bytes)
        .ok_or_else(|| format!("failed to parse shared texture array {path}"))?;
    let decoded = crate::texture_decode::decode_texture_rgba_with_layout(&mut texture, bytes)
        .ok_or_else(|| format!("failed to decode shared texture array {path}"))?;
    if decoded.array_size <= 1 {
        return Err(format!("shared texture {path} is not a 2D array"));
    }
    let index = textures.len();
    textures.push(ModelTexture {
        path: path.to_string(),
        kind,
        width: decoded.width,
        height: decoded.height,
        array_size: decoded.array_size,
        array_layer_height: decoded.array_layer_height,
        rgba: decoded.rgba,
        rgba_f32: None,
    });
    push_loaded_path(loaded_paths, path.to_string());
    Ok(index)
}

#[cfg(feature = "game-data")]
async fn attach_shared_material_arrays_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    materials: &mut [ModelMaterial],
    textures: &mut Vec<ModelTexture>,
    loaded_paths: &mut Vec<String>,
) {
    let needs_tile = materials.iter().any(material_needs_tile_arrays);
    let needs_detail = materials.iter().any(material_needs_detail_arrays);
    let tile_normal = if needs_tile {
        Some(
            load_shared_texture_array_from_async_resource(
                resource,
                CHARACTER_TILE_NORMAL_ARRAY_PATH,
                ModelTextureKind::TileNormalArray,
                textures,
                loaded_paths,
            )
            .await,
        )
    } else {
        None
    };
    let tile_orb = if needs_tile {
        Some(
            load_shared_texture_array_from_async_resource(
                resource,
                CHARACTER_TILE_ORB_ARRAY_PATH,
                ModelTextureKind::TileOrbArray,
                textures,
                loaded_paths,
            )
            .await,
        )
    } else {
        None
    };
    let detail_diffuse = if needs_detail {
        Some(
            load_shared_texture_array_from_async_resource(
                resource,
                BG_DETAIL_DIFFUSE_ARRAY_PATH,
                ModelTextureKind::DetailDiffuseArray,
                textures,
                loaded_paths,
            )
            .await,
        )
    } else {
        None
    };
    let detail_normal = if needs_detail {
        Some(
            load_shared_texture_array_from_async_resource(
                resource,
                BG_DETAIL_NORMAL_ARRAY_PATH,
                ModelTextureKind::DetailNormalArray,
                textures,
                loaded_paths,
            )
            .await,
        )
    } else {
        None
    };

    for material in materials {
        if material_needs_tile_arrays(material) {
            apply_shared_array_result(material, &tile_normal, SharedArraySlot::TileNormal);
            apply_shared_array_result(material, &tile_orb, SharedArraySlot::TileOrb);
        }
        if material_needs_detail_arrays(material) {
            apply_shared_array_result(material, &detail_diffuse, SharedArraySlot::DetailDiffuse);
            apply_shared_array_result(material, &detail_normal, SharedArraySlot::DetailNormal);
        }
    }
}

#[cfg(feature = "game-data")]
async fn load_shared_texture_array_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    path: &str,
    kind: ModelTextureKind,
    textures: &mut Vec<ModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Result<usize, String> {
    if let Some(index) = textures.iter().position(|texture| texture.path == path) {
        return Ok(index);
    }
    let bytes = resource
        .read(path)
        .await
        .map_err(|error| format!("failed to read shared texture array {path}: {error}"))?;
    decode_and_push_shared_texture_array(
        resource.platform(),
        path,
        kind,
        &bytes,
        textures,
        loaded_paths,
    )
}

#[cfg(feature = "game-data")]
pub fn load_weapon_model_from_resource_request<R: physis::resource::Resource>(
    resource: &mut R,
    request: &WeaponModelLoadRequest,
) -> anyhow::Result<WeaponModelData> {
    let model_main = request.primary_model();
    let model_sub = request.secondary_model();
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let stain_ids = request.normalized_stain_ids();
    let staining =
        load_weapon_staining_templates_from_resource(resource, stain_ids, &mut loaded_paths);

    load_weapon_model_meshes_from_resource(
        resource,
        model_main,
        &staining,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
    )
    .map_err(WeaponModelMeshLoadFailure::into_error)?;

    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            if let Err(failure) = load_weapon_model_meshes_from_resource(
                resource,
                model_sub,
                &staining,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
            ) {
                load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
            }
        }
    }

    attach_shared_material_arrays_from_resource(
        resource,
        &mut materials,
        &mut textures,
        &mut loaded_paths,
    );

    if meshes.is_empty() {
        return Err(anyhow::anyhow!(
            "{} has no renderable model meshes",
            request.item_name
        ));
    }

    Ok(WeaponModelData {
        item_id: request.item_id,
        item_name: request.item_name.clone(),
        model_main,
        model_sub,
        stain_ids,
        load_diagnostics,
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

    let raw_meshes = crate::mdl_geometry::extract_mdl_lod0_geometry(path, bytes)
        .with_context(|| format!("failed to extract raw geometry from {path}"))?;

    let mut meshes = Vec::new();
    for mesh in raw_meshes {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            continue;
        }

        let color = material_color(mesh.material_index);
        for range in mesh_index_ranges(mesh.indices.len(), &mesh.submeshes) {
            let raw_indices = &mesh.indices[range.start..range.end];
            let Some((vertices, indices)) = remap_mesh_vertices(&mesh.vertices, raw_indices) else {
                continue;
            };
            meshes.push(WeaponModelMesh {
                path: mesh_path_with_submesh(path, mesh.mesh_index, range.submesh_index),
                part_index: mesh.mesh_index as u32,
                mesh_category: Some(mesh.category.clone()),
                submesh: range.submesh.clone(),
                shape_influences: mesh.shape_influences.clone(),
                material_index: mesh.material_index,
                material_slot: mesh.material_index as usize,
                material_name: mesh.material_name.clone(),
                color,
                bone_table: mesh.bone_table.clone(),
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
#[derive(Clone, Debug, PartialEq, Eq)]
struct MeshIndexRange {
    submesh_index: Option<usize>,
    submesh: Option<ModelSubmeshInfo>,
    start: usize,
    end: usize,
}

#[cfg(feature = "game-data")]
fn mesh_index_ranges(
    index_count: usize,
    submeshes: &[crate::mdl_geometry::MdlGeometrySubmesh],
) -> Vec<MeshIndexRange> {
    normalize_submesh_index_ranges(
        index_count,
        submeshes
            .iter()
            .enumerate()
            .map(|(submesh_index, submesh)| {
                (
                    submesh_index,
                    submesh.info.clone(),
                    submesh.index_offset as usize,
                    submesh.index_count as usize,
                )
            }),
    )
}

#[cfg(feature = "game-data")]
fn normalize_submesh_index_ranges<I>(index_count: usize, submeshes: I) -> Vec<MeshIndexRange>
where
    I: IntoIterator<Item = (usize, ModelSubmeshInfo, usize, usize)>,
{
    let raw = submeshes
        .into_iter()
        .filter(|(_, _, _, count)| *count != 0)
        .collect::<Vec<_>>();
    if raw.is_empty() || index_count == 0 {
        return full_mesh_index_range(index_count);
    }

    let base_index_offset = raw[0].2;
    let mut ranges = Vec::new();
    for (submesh_index, submesh, index_offset, count) in raw {
        let direct_start = (index_offset
            .checked_add(count)
            .is_some_and(|end| end <= index_count))
        .then_some(index_offset);
        let relative_start = index_offset.checked_sub(base_index_offset).filter(|start| {
            start
                .checked_add(count)
                .is_some_and(|end| end <= index_count)
        });
        let Some(start) = direct_start.or(relative_start) else {
            continue;
        };
        ranges.push(MeshIndexRange {
            submesh_index: Some(submesh_index),
            submesh: Some(submesh),
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
            submesh: None,
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
        remapped_indices.extend([a, b, c]);
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
pub fn material_debug_info_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    path: &str,
) -> anyhow::Result<MaterialDebugInfo> {
    use anyhow::{Context, anyhow};
    use physis::ReadableFile;

    let bytes = resource
        .read(path)
        .ok_or_else(|| anyhow!("failed to read material {path}"))?;
    let material = physis::mtrl::Material::from_existing(resource.platform(), &bytes)
        .ok_or_else(|| anyhow!("failed to parse material {path}"))?;
    let mut loaded_paths = Vec::new();
    let semantics = load_composed_material_semantics_from_resource(
        resource,
        &material.shader_package_name,
        &material,
        &bytes,
        &mut loaded_paths,
    );

    material_debug_info_from_parsed_material(path, &bytes, material, &semantics)
        .with_context(|| format!("failed to build material debug info for {path}"))
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
    let mut semantics = ComposedMaterialSemantics::default();
    semantics.apply_material(&material);
    semantics.apply_material_constants(bytes);
    material_debug_info_from_parsed_material(path, bytes, material, &semantics)
}

#[cfg(feature = "game-data")]
fn material_debug_info_from_parsed_material(
    path: &str,
    bytes: &[u8],
    material: physis::mtrl::Material,
    semantics: &ComposedMaterialSemantics,
) -> anyhow::Result<MaterialDebugInfo> {
    let sampler_records = parse_material_sampler_records(bytes, semantics);
    let shader_flags = parse_material_shader_flags(bytes);
    let low_level = material_low_level_debug(bytes, &material.texture_paths);
    let texture_offsets = low_level
        .as_ref()
        .map(|debug| debug.texture_offsets.clone())
        .unwrap_or_default();
    let shader_keys = material
        .shader_keys
        .iter()
        .map(|key| MaterialShaderKeyDebug {
            category: key.category,
            category_hex: hex_u32(key.category),
            value: key.value,
            value_hex: hex_u32(key.value),
        })
        .collect::<Vec<_>>();
    let constants = material_constant_debug(bytes);
    let samplers = sampler_records
        .into_iter()
        .map(|record| MaterialSamplerDebug {
            texture_index: record.texture_index,
            texture_path: material.texture_paths.get(record.texture_index).cloned(),
            texture_usage: record.texture_usage,
            texture_usage_hex: hex_u32(record.texture_usage),
            texture_usage_name: record.texture_usage_name,
            flags: record.flags,
            flags_hex: hex_u32(record.flags),
            kind: record.kind,
            kind_source: record.kind_source.map(ToString::to_string),
        })
        .collect::<Vec<_>>();
    let summary = material_semantic_summary(
        shader_flags,
        &shader_keys,
        semantics,
        &texture_offsets,
        &samplers,
    );

    Ok(MaterialDebugInfo {
        path: path.to_string(),
        summary,
        file_header: low_level.as_ref().map(|debug| debug.file_header.clone()),
        shader_package_name: material.shader_package_name.clone(),
        shader_header: low_level
            .as_ref()
            .and_then(|debug| debug.shader_header.clone()),
        shader_flags,
        shader_flags_hex: hex_u32(shader_flags),
        texture_paths: material.texture_paths.clone(),
        texture_offsets,
        uv_color_sets: low_level
            .as_ref()
            .map(|debug| debug.uv_color_sets.clone())
            .unwrap_or_default(),
        color_sets: low_level
            .as_ref()
            .map(|debug| debug.color_sets.clone())
            .unwrap_or_default(),
        additional_data: low_level
            .as_ref()
            .map(|debug| debug.additional_data.clone())
            .unwrap_or_default(),
        data_set_size: low_level
            .as_ref()
            .map(|debug| debug.data_set_size)
            .unwrap_or_default(),
        shader_keys,
        shader_value_list_size: low_level
            .as_ref()
            .map(|debug| debug.shader_value_list_size)
            .unwrap_or_default(),
        shader_value_count: low_level
            .as_ref()
            .map(|debug| debug.shader_value_count)
            .unwrap_or_default(),
        constants,
        constants_debug: material
            .constants
            .iter()
            .map(|constant| format!("{constant:?}"))
            .collect(),
        samplers,
        color_table: material_color_table_debug(material.color_table.as_ref()),
        color_dye_table_kind: material
            .color_dye_table
            .as_ref()
            .map(material_color_dye_table_kind),
        color_dye_table: material_color_dye_table_debug(material.color_dye_table.as_ref()),
    })
}

#[cfg(feature = "game-data")]
fn material_semantic_summary(
    shader_flags: u32,
    material_shader_keys: &[MaterialShaderKeyDebug],
    semantics: &ComposedMaterialSemantics,
    texture_offsets: &[MaterialTextureOffsetDebug],
    samplers: &[MaterialSamplerDebug],
) -> MaterialSemanticSummaryDebug {
    let mut shader_keys = semantics
        .material_keys
        .iter()
        .map(|(category, entry)| MaterialResolvedShaderKeyDebug {
            category: *category,
            category_hex: hex_u32(*category),
            category_name: known_shader_label(*category),
            value: entry.value,
            value_hex: hex_u32(entry.value),
            value_name: known_shader_label(entry.value),
            source: entry.source.to_string(),
        })
        .collect::<Vec<_>>();
    shader_keys.sort_by(|left, right| left.category.cmp(&right.category));

    let mut constants = semantics
        .material_constants
        .iter()
        .map(|(id, entry)| MaterialResolvedConstantDebug {
            id: *id,
            id_hex: hex_u32(*id),
            name: known_material_constant_name(*id),
            value_count: entry.value.len(),
            values: entry.value.clone(),
            source: entry.source.to_string(),
        })
        .collect::<Vec<_>>();
    constants.sort_by(|left, right| left.id.cmp(&right.id));

    let texture_flags = texture_offsets
        .iter()
        .map(|texture| MaterialTextureFlagSummaryDebug {
            index: texture.index,
            flags: texture.flags,
            flags_hex: texture.flags_hex.clone(),
            path: texture.path.clone(),
        })
        .collect::<Vec<_>>();

    let sampler_flags = samplers
        .iter()
        .map(|sampler| MaterialSamplerFlagSummaryDebug {
            texture_index: sampler.texture_index,
            texture_path: sampler.texture_path.clone(),
            texture_usage: sampler.texture_usage,
            texture_usage_hex: sampler.texture_usage_hex.clone(),
            texture_usage_name: sampler.texture_usage_name.clone(),
            flags: sampler.flags,
            flags_hex: sampler.flags_hex.clone(),
            kind: sampler.kind,
            kind_source: sampler.kind_source.clone(),
        })
        .collect::<Vec<_>>();

    MaterialSemanticSummaryDebug {
        shader_flags,
        shader_flags_hex: hex_u32(shader_flags),
        shader_key_count: material_shader_keys.len(),
        resolved_shader_key_count: shader_keys.len(),
        resolved_constant_count: constants.len(),
        texture_flag_count: texture_flags.len(),
        sampler_flag_count: sampler_flags.len(),
        shader_keys,
        constants,
        texture_flags,
        sampler_flags,
    }
}

#[cfg(feature = "game-data")]
fn known_material_constant_name(id: u32) -> Option<String> {
    if id == 0x9A69_6A17 {
        return Some("UvScrollMapping".to_string());
    }
    known_crc_label(
        id,
        &[
            "g_NormalScale",
            "g_MultiNormalScale",
            "g_AlphaThreshold",
            "g_TileIndex",
            "g_TileAlpha",
            "g_TileScale",
            "g_ToonIndex",
            "g_ToonLightScale",
            "g_ToonLightSpecAperture",
            "g_ToonReflectionScale",
            "g_ToonSpecIndex",
            "g_SheenAperture",
            "g_SheenRate",
            "g_SheenTintRate",
            "g_SphereMapIndex",
            "g_DetailID",
            "g_MultiDetailID",
            "g_DetailColorUvScale",
            "g_DetailNormalUvScale",
            "g_DetailColor",
            "g_MultiDetailColor",
            "g_DetailNormalScale",
            "g_MultiDetailNormalScale",
            "g_AlphaAperture",
            "g_AlphaOffset",
            "g_ShadowAlphaThreshold",
            "g_Transparency",
            "g_WaterDeepColor",
            "g_RefractionColor",
            "g_WhitecapColor",
            "g_TexAnim",
            "g_TexU",
            "g_TexV",
            "g_Ray",
            "g_Color",
            "g_DiffuseColor",
            "g_EmissiveColor",
            "g_MultiDiffuseColor",
            "g_MultiEmissiveColor",
            "g_OutlineColor",
            "g_OutlineWidth",
            "g_SpecularColorMask",
            "g_SSAOMask",
            "g_TextureMipBias",
            "g_ShadowPosOffset",
            "g_GlassIOR",
            "g_GlassThicknessMax",
        ],
    )
}

#[cfg(feature = "game-data")]
fn known_shader_label(id: u32) -> Option<String> {
    known_crc_label(
        id,
        &[
            "ApplyAlphaTest",
            "ApplyAlphaTestOn",
            "ApplyAlphaTestOff",
            "ApplyVertexColor",
            "ApplyVertexColorOn",
            "ApplyVertexColorOff",
            "GetValues",
            "GetSingleValues",
            "GetMultiValues",
            "GetAlphaMultiValues",
            "GetAlphaMultiValues2",
            "GetAlphaMultiValues3",
            "GetValuesTextureType",
            "GetValuesCompatibility",
            "Compatibility",
            "GetMaterialValue",
            "GetMaterialValueFace",
            "GetMaterialValueBody",
            "GetMaterialValueBodyJJM",
            "GetMaterialValueFaceEmissive",
            "GetDecalColor",
            "GetDecalColorAlpha",
            "GetSubColor",
            "GetSubColorFace",
            "GetSubColorHair",
            "CategoryFlowMapType",
            "Standard",
            "Flow",
        ],
    )
}

#[cfg(feature = "game-data")]
fn known_crc_label(id: u32, labels: &[&'static str]) -> Option<String> {
    labels
        .iter()
        .find(|label| physis::shpk::ShaderPackage::crc(label) == id)
        .map(|label| (*label).to_string())
}

#[cfg(feature = "game-data")]
fn hex_u32(value: u32) -> String {
    format!("0x{value:08x}")
}

#[cfg(feature = "game-data")]
fn hex_u16(value: u16) -> String {
    format!("0x{value:04x}")
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq)]
struct MaterialLowLevelDebug {
    file_header: MaterialFileHeaderDebug,
    shader_header: Option<MaterialShaderHeaderDebug>,
    texture_offsets: Vec<MaterialTextureOffsetDebug>,
    uv_color_sets: Vec<MaterialNamedSetDebug>,
    color_sets: Vec<MaterialNamedSetDebug>,
    additional_data: Vec<u8>,
    data_set_size: usize,
    shader_value_list_size: usize,
    shader_value_count: usize,
}

#[cfg(feature = "game-data")]
fn material_low_level_debug(
    bytes: &[u8],
    texture_paths: &[String],
) -> Option<MaterialLowLevelDebug> {
    let version = read_u32_le(bytes, 0)?;
    let file_size = read_u16_le(bytes, 4)?;
    let data_set_size = read_u16_le(bytes, 6)?;
    let string_table_size = read_u16_le(bytes, 8)?;
    let shader_package_name_offset = read_u16_le(bytes, 10)?;
    let texture_count = *bytes.get(12)?;
    let uv_set_count = *bytes.get(13)?;
    let color_set_count = *bytes.get(14)?;
    let additional_data_size = *bytes.get(15)?;

    let mut offset = 16_usize;
    let mut texture_offsets_raw = Vec::with_capacity(usize::from(texture_count));
    for index in 0..usize::from(texture_count) {
        let raw = read_u32_le(bytes, offset)?;
        texture_offsets_raw.push((index, raw as u16, (raw >> 16) as u16));
        offset = checked_advance(offset, 4, bytes.len())?;
    }

    let mut uv_color_sets_raw = Vec::with_capacity(usize::from(uv_set_count));
    for index in 0..usize::from(uv_set_count) {
        uv_color_sets_raw.push((
            index,
            read_u16_le(bytes, offset)?,
            *bytes.get(offset + 2)?,
            *bytes.get(offset + 3)?,
        ));
        offset = checked_advance(offset, 4, bytes.len())?;
    }

    let mut color_sets_raw = Vec::with_capacity(usize::from(color_set_count));
    for index in 0..usize::from(color_set_count) {
        color_sets_raw.push((
            index,
            read_u16_le(bytes, offset)?,
            *bytes.get(offset + 2)?,
            *bytes.get(offset + 3)?,
        ));
        offset = checked_advance(offset, 4, bytes.len())?;
    }

    let string_table = read_bytes(bytes, offset, usize::from(string_table_size))?;
    offset = checked_advance(offset, usize::from(string_table_size), bytes.len())?;

    let additional_data = read_bytes(bytes, offset, usize::from(additional_data_size))?.to_vec();
    offset = checked_advance(offset, usize::from(additional_data_size), bytes.len())?;

    offset = checked_advance(offset, usize::from(data_set_size), bytes.len())?;

    let shader_header = if offset < bytes.len() {
        let shader_value_list_size = read_u16_le(bytes, offset)?;
        let shader_key_count = read_u16_le(bytes, offset + 2)?;
        let constant_count = read_u16_le(bytes, offset + 4)?;
        let sampler_count = read_u16_le(bytes, offset + 6)?;
        let flags = read_u32_le(bytes, offset + 8)?;
        let mut shader_offset = checked_advance(offset, 12, bytes.len())?;
        shader_offset = checked_advance(
            shader_offset,
            usize::from(shader_key_count) * 8,
            bytes.len(),
        )?;
        shader_offset =
            checked_advance(shader_offset, usize::from(constant_count) * 8, bytes.len())?;
        shader_offset =
            checked_advance(shader_offset, usize::from(sampler_count) * 12, bytes.len())?;
        checked_advance(
            shader_offset,
            usize::from(shader_value_list_size),
            bytes.len(),
        )?;

        Some(MaterialShaderHeaderDebug {
            shader_value_list_size,
            shader_key_count,
            constant_count,
            sampler_count,
            flags,
            flags_hex: hex_u32(flags),
        })
    } else {
        None
    };

    let texture_offsets = texture_offsets_raw
        .into_iter()
        .map(|(index, string_offset, flags)| MaterialTextureOffsetDebug {
            index,
            offset: string_offset,
            flags,
            flags_hex: hex_u16(flags),
            path: read_string_at(string_table, usize::from(string_offset))
                .or_else(|| texture_paths.get(index).cloned()),
        })
        .collect();
    let uv_color_sets = uv_color_sets_raw
        .into_iter()
        .map(
            |(index, name_offset, set_index, unknown1)| MaterialNamedSetDebug {
                index,
                name_offset,
                set_index,
                unknown1,
                name: read_string_at(string_table, usize::from(name_offset)),
            },
        )
        .collect();
    let color_sets = color_sets_raw
        .into_iter()
        .map(
            |(index, name_offset, set_index, unknown1)| MaterialNamedSetDebug {
                index,
                name_offset,
                set_index,
                unknown1,
                name: read_string_at(string_table, usize::from(name_offset)),
            },
        )
        .collect();
    let shader_value_list_size = shader_header
        .as_ref()
        .map(|header| usize::from(header.shader_value_list_size))
        .unwrap_or_default();

    Some(MaterialLowLevelDebug {
        file_header: MaterialFileHeaderDebug {
            version,
            version_hex: hex_u32(version),
            file_size,
            data_set_size,
            string_table_size,
            shader_package_name_offset,
            texture_count,
            uv_set_count,
            color_set_count,
            additional_data_size,
        },
        shader_header,
        texture_offsets,
        uv_color_sets,
        color_sets,
        additional_data,
        data_set_size: usize::from(data_set_size),
        shader_value_list_size,
        shader_value_count: shader_value_list_size / 4,
    })
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
                    anisotropy: None,
                    tile_alpha: None,
                    tile_index: Some(f32::from(row.tile_set)),
                    sheen_rate: None,
                    sheen_tint: None,
                    sheen_aperture: None,
                    sphere_mask: None,
                    tile_set: Some(row.tile_set),
                    shader_index: None,
                    sphere_index: None,
                    tile_matrix: Some([
                        row.material_repeat_x,
                        row.material_repeat_y,
                        row.material_skew[0],
                        row.material_skew[1],
                    ]),
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
                    anisotropy: Some(row.anisotropy),
                    tile_alpha: Some(row.tile_alpha),
                    tile_index: Some(dawntrail_tile_index(row.tile_set)),
                    sheen_rate: Some(row.sheen_rate),
                    sheen_tint: Some(row.sheen_tint),
                    sheen_aperture: Some(row.sheen_aperture),
                    sphere_mask: Some(row.sphere_mask),
                    tile_set: Some(row.tile_set),
                    shader_index: Some(row.shader_index),
                    sphere_index: Some(row.sphere_index),
                    tile_matrix: Some([
                        row.material_repeat[0],
                        row.material_repeat[1],
                        row.material_skew[0],
                        row.material_skew[1],
                    ]),
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
fn material_color_dye_table_debug(
    color_dye_table: Option<&physis::mtrl::ColorDyeTable>,
) -> Option<MaterialColorDyeTableDebug> {
    match model_color_dye_table(color_dye_table)? {
        ModelColorDyeTable::Legacy(rows) => Some(MaterialColorDyeTableDebug {
            kind: "Legacy".to_string(),
            row_count: rows.len(),
            rows: rows
                .into_iter()
                .enumerate()
                .map(|(index, row)| MaterialColorDyeTableRowDebug {
                    index,
                    template: row.template,
                    channel: None,
                    diffuse: row.diffuse,
                    specular: row.specular,
                    emissive: row.emissive,
                    gloss: Some(row.gloss),
                    specular_strength: Some(row.specular_strength),
                    scalar3: None,
                    metalness: None,
                    roughness: None,
                    sheen_rate: None,
                    sheen_tint_rate: None,
                    sheen_aperture: None,
                    anisotropy: None,
                    sphere_map_index: None,
                    sphere_map_mask: None,
                })
                .collect(),
        }),
        ModelColorDyeTable::Dawntrail(rows) => Some(MaterialColorDyeTableDebug {
            kind: "Dawntrail".to_string(),
            row_count: rows.len(),
            rows: rows
                .into_iter()
                .enumerate()
                .map(|(index, row)| MaterialColorDyeTableRowDebug {
                    index,
                    template: row.template,
                    channel: Some(row.channel),
                    diffuse: row.diffuse,
                    specular: row.specular,
                    emissive: row.emissive,
                    gloss: None,
                    specular_strength: None,
                    scalar3: Some(row.scalar3),
                    metalness: Some(row.metalness),
                    roughness: Some(row.roughness),
                    sheen_rate: Some(row.sheen_rate),
                    sheen_tint_rate: Some(row.sheen_tint_rate),
                    sheen_aperture: Some(row.sheen_aperture),
                    anisotropy: Some(row.anisotropy),
                    sphere_map_index: Some(row.sphere_map_index),
                    sphere_map_mask: Some(row.sphere_map_mask),
                })
                .collect(),
        }),
        ModelColorDyeTable::Opaque => Some(MaterialColorDyeTableDebug {
            kind: "Opaque".to_string(),
            row_count: 0,
            rows: Vec::new(),
        }),
    }
}

#[cfg(feature = "game-data")]
fn model_color_dye_table(
    color_dye_table: Option<&physis::mtrl::ColorDyeTable>,
) -> Option<ModelColorDyeTable> {
    match color_dye_table? {
        physis::mtrl::ColorDyeTable::LegacyColorDyeTable(table) => {
            Some(ModelColorDyeTable::Legacy(
                table
                    .rows
                    .iter()
                    .map(|row| ModelLegacyColorDyeTableRow {
                        template: row.template,
                        diffuse: row.diffuse,
                        specular: row.specular,
                        emissive: row.emissive,
                        gloss: row.gloss,
                        specular_strength: row.specular_strength,
                    })
                    .collect(),
            ))
        }
        physis::mtrl::ColorDyeTable::DawntrailColorDyeTable(table) => {
            Some(ModelColorDyeTable::Dawntrail(
                table
                    .rows
                    .iter()
                    .map(|row| ModelDawntrailColorDyeTableRow {
                        template: row.template,
                        channel: row.channel,
                        diffuse: row.diffuse,
                        specular: row.specular,
                        emissive: row.emissive,
                        scalar3: row.scalar3,
                        metalness: row.metalness,
                        roughness: row.roughness,
                        sheen_rate: row.sheen_rate,
                        sheen_tint_rate: row.sheen_tint_rate,
                        sheen_aperture: row.sheen_aperture,
                        anisotropy: row.anisotropy,
                        sphere_map_index: row.sphere_map_index,
                        sphere_map_mask: row.sphere_map_mask,
                    })
                    .collect(),
            ))
        }
        physis::mtrl::ColorDyeTable::OpaqueColorDyeTable(_) => Some(ModelColorDyeTable::Opaque),
    }
}

#[cfg(feature = "game-data")]
fn load_weapon_model_meshes_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    model: PackedModelId,
    staining: &WeaponStainingTemplates,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<WeaponModelMesh>,
) -> Result<(), WeaponModelMeshLoadFailure> {
    use anyhow::Context;

    let mut candidates = Vec::new();
    for path in weapon_model_candidate_paths(model) {
        let Some(bytes) = resource.read(&path) else {
            candidates.push(model_load_candidate(
                path,
                WeaponModelLoadCandidateStatus::Missing,
                "resource read returned no bytes",
            ));
            continue;
        };

        let mut path_meshes = match meshes_from_mdl_bytes(&path, &bytes)
            .with_context(|| format!("failed to load render meshes from {path}"))
        {
            Ok(path_meshes) => path_meshes,
            Err(error) => {
                candidates.push(model_load_candidate(
                    path,
                    WeaponModelLoadCandidateStatus::ParseError,
                    format!("{error:#}"),
                ));
                return Err(WeaponModelMeshLoadFailure::new(model, candidates));
            }
        };
        push_loaded_path(loaded_paths, path.clone());
        assign_weapon_materials_from_resource(
            resource,
            model,
            &path,
            staining,
            &mut path_meshes,
            materials,
            textures,
            loaded_paths,
        );
        meshes.append(&mut path_meshes);
        return Ok(());
    }

    Err(WeaponModelMeshLoadFailure::new(model, candidates))
}

#[cfg(feature = "game-data")]
fn assign_weapon_materials_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    model: PackedModelId,
    model_path: &str,
    staining: &WeaponStainingTemplates,
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
            staining,
            material_index,
            material_name,
            slot,
            textures,
            loaded_paths,
        );
        let material = reuse_loaded_material_for_missing_reference(material, materials);
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
    staining: &WeaponStainingTemplates,
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
        let color_dye_table = model_color_dye_table(material.color_dye_table.as_ref());
        let mut color_table_rows = material
            .color_table
            .as_ref()
            .and_then(weapon_color_table_rows);
        let staining_application = apply_weapon_staining(
            color_table_rows.as_deref_mut(),
            color_dye_table.as_ref(),
            staining,
        );
        let summary = summarize_material_colors(color_table_rows.as_deref(), fallback);
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
        let draw_depth_mode = composed_material_draw_depth_mode(&semantics);
        let lighting_mode = composed_material_lighting_mode(&semantics);
        let flow_mode = composed_material_flow_mode(&semantics);
        let value_mode = composed_material_value_mode(&semantics);
        let sub_color_mode = composed_material_sub_color_mode(&semantics);
        let transparency = composed_material_transparency(&semantics, &shader_package_name);
        let water_deep_color = composed_material_water_deep_color(&semantics);
        let water_refraction_color = composed_material_water_refraction_color(&semantics);
        let water_whitecap_color = composed_material_water_whitecap_color(&semantics);
        let alpha_aperture = composed_material_alpha_aperture(&semantics);
        let alpha_offset = composed_material_alpha_offset(&semantics);
        let shadow_alpha_threshold = composed_material_shadow_alpha_threshold(&semantics);
        let glass_ior = composed_material_glass_ior(&semantics);
        let glass_thickness_max = composed_material_glass_thickness_max(&semantics);
        let normal_scale = composed_material_normal_scale(&semantics);
        let multi_normal_scale = composed_material_multi_normal_scale(&semantics);
        let detail_normal_scale = composed_material_detail_normal_scale(&semantics);
        let multi_detail_normal_scale = composed_material_multi_detail_normal_scale(&semantics);
        let tile_index = composed_material_tile_index(&semantics);
        let tile_alpha = composed_material_tile_alpha(&semantics);
        let tile_scale = composed_material_tile_scale(&semantics);
        let toon_index = composed_material_toon_index(&semantics);
        let toon_light_scale = composed_material_toon_light_scale(&semantics);
        let toon_light_spec_aperture = composed_material_toon_light_spec_aperture(&semantics);
        let toon_reflection_scale = composed_material_toon_reflection_scale(&semantics);
        let toon_spec_index = composed_material_toon_spec_index(&semantics);
        let sheen_rate = composed_material_sheen_rate(&semantics);
        let sheen_tint_rate = composed_material_sheen_tint_rate(&semantics);
        let sheen_aperture = composed_material_sheen_aperture(&semantics);
        let sphere_map_index = composed_material_sphere_map_index(&semantics);
        let detail_id = composed_material_detail_id(&semantics);
        let multi_detail_id = composed_material_multi_detail_id(&semantics);
        let detail_color = composed_material_detail_color(&semantics);
        let multi_detail_color = composed_material_multi_detail_color(&semantics);
        let shader_diffuse_color = composed_material_shader_diffuse_color(&semantics);
        let shader_multi_diffuse_color = composed_material_shader_multi_diffuse_color(&semantics);
        let shader_emissive_color = composed_material_shader_emissive_color(&semantics);
        let shader_multi_emissive_color = composed_material_shader_multi_emissive_color(&semantics);
        let outline_color = composed_material_outline_color(&semantics);
        let outline_width = composed_material_outline_width(&semantics);
        let specular_color_mask = composed_material_specular_color_mask(&semantics);
        let ssao_mask = composed_material_ssao_mask(&semantics);
        let texture_mip_bias = composed_material_texture_mip_bias(&semantics);
        let shadow_pos_offset = composed_material_shadow_pos_offset(&semantics);
        let detail_color_uv_scale = composed_material_detail_color_uv_scale(&semantics);
        let detail_normal_uv_scale = composed_material_detail_normal_uv_scale(&semantics);
        let uv_scroll = composed_material_uv_scroll(&semantics);
        let lightshaft_color = composed_material_lightshaft_color(&semantics);
        let lightshaft_tex_anim = composed_material_lightshaft_tex_anim(&semantics);
        let lightshaft_tex_u = composed_material_lightshaft_tex_u(&semantics);
        let lightshaft_tex_v = composed_material_lightshaft_tex_v(&semantics);
        let lightshaft_ray = composed_material_lightshaft_ray(&semantics);
        let texture_set = load_weapon_material_textures_from_resource(
            resource,
            &path,
            &material,
            color_table_rows.as_deref(),
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
            draw_depth_mode,
            lighting_mode,
            flow_mode,
            value_mode,
            sub_color_mode,
            transparency,
            water_deep_color,
            water_refraction_color,
            water_whitecap_color,
            alpha_aperture,
            alpha_offset,
            shadow_alpha_threshold,
            glass_ior,
            glass_thickness_max,
            normal_scale,
            multi_normal_scale,
            detail_normal_scale,
            multi_detail_normal_scale,
            tile_index,
            tile_alpha,
            tile_scale,
            toon_index,
            toon_light_scale,
            toon_light_spec_aperture,
            toon_reflection_scale,
            toon_spec_index,
            sheen_rate,
            sheen_tint_rate,
            sheen_aperture,
            sphere_map_index,
            detail_id,
            multi_detail_id,
            detail_color,
            multi_detail_color,
            shader_diffuse_color,
            shader_multi_diffuse_color,
            shader_emissive_color,
            shader_multi_emissive_color,
            outline_color,
            outline_width,
            specular_color_mask,
            ssao_mask,
            texture_mip_bias,
            shadow_pos_offset,
            detail_color_uv_scale,
            detail_normal_uv_scale,
            uv_scroll,
            lightshaft_color,
            lightshaft_tex_anim,
            lightshaft_tex_u,
            lightshaft_tex_v,
            lightshaft_ray,
            opacity,
            render_backfaces,
            apply_vertex_color,
            has_color_dye_table: color_dye_table.is_some(),
            color_dye_table,
            staining_application,
            texture_arrays: ModelMaterialTextureArrays::default(),
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            secondary_base_color_texture: texture_set.secondary_base_color,
            normal_texture: texture_set.normal,
            secondary_normal_texture: texture_set.secondary_normal,
            mask_texture: texture_set.mask,
            material_map_texture: texture_set.material_map,
            multi_map_texture: texture_set.multi_map,
            specular_texture: texture_set.specular,
            secondary_specular_texture: texture_set.secondary_specular,
            emissive_texture: texture_set.emissive,
            environment_texture: texture_set.environment,
            material_properties_texture: texture_set.material_properties,
            tile_properties_texture: texture_set.tile_properties,
            sheen_properties_texture: texture_set.sheen_properties,
            sphere_properties_texture: texture_set.sphere_properties,
            tile_matrix_texture: texture_set.tile_matrix,
            index_texture: texture_set.index,
            water_wave_texture: texture_set.water_wave,
            water_wave1_texture: texture_set.water_wave1,
            water_whitecap_texture: texture_set.water_whitecap,
        };
    }

    fallback_weapon_material(slot, material_index, material_name, fallback)
}

#[cfg(feature = "game-data")]
fn load_weapon_material_textures_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    material_path: &str,
    material: &physis::mtrl::Material,
    color_table_rows: Option<&[ColorTableRowColors]>,
    sampler_roles: &[MaterialSamplerRole],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in material.texture_paths.iter().enumerate() {
        let sampler_kind = sampler_kind_for_texture(sampler_roles, texture_order);
        let kind = classify_weapon_texture(raw_texture_path, sampler_kind);
        let Some(texture_index) = load_weapon_texture_from_resource(
            resource,
            material_path,
            raw_texture_path,
            kind,
            sampler_kind,
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
            WeaponModelTextureKind::SecondaryBaseColor => {
                set.secondary_base_color.get_or_insert(texture_index);
                if texture_alpha_affects_material_transparency(&textures[texture_index]) {
                    set.has_alpha = true;
                }
            }
            WeaponModelTextureKind::Normal => {
                set.normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SecondaryNormal => {
                set.secondary_normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Mask => {
                set.mask.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MaterialMap => {
                set.material_map.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MultiMap => {
                set.multi_map.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Specular => {
                set.specular.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SecondarySpecular => {
                set.secondary_specular.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Emissive => {
                set.emissive.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Environment => {
                set.environment.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MaterialProperties => {
                set.material_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::TileProperties => {
                set.tile_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SheenProperties => {
                set.sheen_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SphereProperties => {
                set.sphere_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::TileMatrixProperties => {
                set.tile_matrix.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Index => {
                set.index.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::WaterWave => {
                set.water_wave.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::WaterWaveSecondary => {
                set.water_wave1.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::WaterWhitecap => {
                set.water_whitecap.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::TileNormalArray
            | WeaponModelTextureKind::TileOrbArray
            | WeaponModelTextureKind::DetailDiffuseArray
            | WeaponModelTextureKind::DetailNormalArray
            | WeaponModelTextureKind::Other => {}
        }
    }

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        color_table_rows,
        set.index,
        set.emissive.is_none(),
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
        set.tile_properties.get_or_insert(baked.tile_properties);
        add_unique_index(&mut set.indices, baked.tile_properties);
        set.sheen_properties.get_or_insert(baked.sheen_properties);
        add_unique_index(&mut set.indices, baked.sheen_properties);
        set.sphere_properties.get_or_insert(baked.sphere_properties);
        add_unique_index(&mut set.indices, baked.sphere_properties);
        set.tile_matrix.get_or_insert(baked.tile_matrix);
        add_unique_index(&mut set.indices, baked.tile_matrix);
    }

    if set.base_color.is_none() {
        set.base_color = choose_fallback_base_texture(&set.indices, textures);
    }
    refresh_texture_set_alpha(&mut set, textures);

    set
}

#[cfg(feature = "game-data")]
fn load_weapon_texture_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    material_path: &str,
    raw_texture_path: &str,
    kind: WeaponModelTextureKind,
    sampler_kind: Option<WeaponModelTextureKind>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    use physis::ReadableFile;

    for path in weapon_texture_candidate_paths(material_path, raw_texture_path) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
            textures[index].kind =
                merge_texture_kind(textures[index].kind, kind, sampler_kind.is_some());
            return Some(index);
        }

        let Some(bytes) = resource.read(&path) else {
            continue;
        };
        let Some(mut texture) = physis::tex::Texture::from_existing(resource.platform(), &bytes)
        else {
            continue;
        };
        let Some(decoded) =
            crate::texture_decode::decode_texture_rgba_with_layout(&mut texture, &bytes)
        else {
            continue;
        };
        let index = textures.len();
        textures.push(WeaponModelTexture {
            path: path.clone(),
            kind,
            width: decoded.width,
            height: decoded.height,
            array_size: decoded.array_size,
            array_layer_height: decoded.array_layer_height,
            rgba: decoded.rgba,
            rgba_f32: None,
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
    let model_main = request.primary_model();
    let model_sub = request.secondary_model();
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let stain_ids = request.normalized_stain_ids();
    let staining =
        load_weapon_staining_templates_from_async_resource(resource, stain_ids, &mut loaded_paths)
            .await;

    load_weapon_model_meshes_from_async_resource(
        resource,
        model_main,
        &staining,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
    )
    .await
    .map_err(WeaponModelMeshLoadFailure::into_error)?;

    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            if let Err(failure) = load_weapon_model_meshes_from_async_resource(
                resource,
                model_sub,
                &staining,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
            )
            .await
            {
                load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
            }
        }
    }

    attach_shared_material_arrays_from_async_resource(
        resource,
        &mut materials,
        &mut textures,
        &mut loaded_paths,
    )
    .await;

    if meshes.is_empty() {
        return Err(anyhow::anyhow!(
            "{} has no renderable model meshes",
            request.item_name
        ));
    }

    Ok(WeaponModelData {
        item_id: request.item_id,
        item_name: request.item_name.clone(),
        model_main,
        model_sub,
        stain_ids,
        load_diagnostics,
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
    staining: &WeaponStainingTemplates,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<WeaponModelMesh>,
) -> Result<(), WeaponModelMeshLoadFailure> {
    use anyhow::Context;

    let mut candidates = Vec::new();
    for path in weapon_model_candidate_paths(model) {
        let bytes = match resource.read(&path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                candidates.push(model_load_candidate(
                    path,
                    WeaponModelLoadCandidateStatus::ReadError,
                    error.to_string(),
                ));
                continue;
            }
        };

        let mut path_meshes = match meshes_from_mdl_bytes(&path, &bytes)
            .with_context(|| format!("failed to load render meshes from {path}"))
        {
            Ok(path_meshes) => path_meshes,
            Err(error) => {
                candidates.push(model_load_candidate(
                    path,
                    WeaponModelLoadCandidateStatus::ParseError,
                    format!("{error:#}"),
                ));
                return Err(WeaponModelMeshLoadFailure::new(model, candidates));
            }
        };
        push_loaded_path(loaded_paths, path.clone());
        assign_weapon_materials_from_async_resource(
            resource,
            model,
            &path,
            staining,
            &mut path_meshes,
            materials,
            textures,
            loaded_paths,
        )
        .await;
        meshes.append(&mut path_meshes);
        return Ok(());
    }

    Err(WeaponModelMeshLoadFailure::new(model, candidates))
}

#[cfg(feature = "game-data")]
async fn assign_weapon_materials_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    model: PackedModelId,
    model_path: &str,
    staining: &WeaponStainingTemplates,
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
            staining,
            material_index,
            material_name,
            slot,
            textures,
            loaded_paths,
        )
        .await;
        let material = reuse_loaded_material_for_missing_reference(material, materials);
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
    staining: &WeaponStainingTemplates,
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
        let color_dye_table = model_color_dye_table(material.color_dye_table.as_ref());
        let mut color_table_rows = material
            .color_table
            .as_ref()
            .and_then(weapon_color_table_rows);
        let staining_application = apply_weapon_staining(
            color_table_rows.as_deref_mut(),
            color_dye_table.as_ref(),
            staining,
        );
        let summary = summarize_material_colors(color_table_rows.as_deref(), fallback);
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
        let draw_depth_mode = composed_material_draw_depth_mode(&semantics);
        let lighting_mode = composed_material_lighting_mode(&semantics);
        let flow_mode = composed_material_flow_mode(&semantics);
        let value_mode = composed_material_value_mode(&semantics);
        let sub_color_mode = composed_material_sub_color_mode(&semantics);
        let transparency = composed_material_transparency(&semantics, &shader_package_name);
        let water_deep_color = composed_material_water_deep_color(&semantics);
        let water_refraction_color = composed_material_water_refraction_color(&semantics);
        let water_whitecap_color = composed_material_water_whitecap_color(&semantics);
        let alpha_aperture = composed_material_alpha_aperture(&semantics);
        let alpha_offset = composed_material_alpha_offset(&semantics);
        let shadow_alpha_threshold = composed_material_shadow_alpha_threshold(&semantics);
        let glass_ior = composed_material_glass_ior(&semantics);
        let glass_thickness_max = composed_material_glass_thickness_max(&semantics);
        let normal_scale = composed_material_normal_scale(&semantics);
        let multi_normal_scale = composed_material_multi_normal_scale(&semantics);
        let detail_normal_scale = composed_material_detail_normal_scale(&semantics);
        let multi_detail_normal_scale = composed_material_multi_detail_normal_scale(&semantics);
        let tile_index = composed_material_tile_index(&semantics);
        let tile_alpha = composed_material_tile_alpha(&semantics);
        let tile_scale = composed_material_tile_scale(&semantics);
        let toon_index = composed_material_toon_index(&semantics);
        let toon_light_scale = composed_material_toon_light_scale(&semantics);
        let toon_light_spec_aperture = composed_material_toon_light_spec_aperture(&semantics);
        let toon_reflection_scale = composed_material_toon_reflection_scale(&semantics);
        let toon_spec_index = composed_material_toon_spec_index(&semantics);
        let sheen_rate = composed_material_sheen_rate(&semantics);
        let sheen_tint_rate = composed_material_sheen_tint_rate(&semantics);
        let sheen_aperture = composed_material_sheen_aperture(&semantics);
        let sphere_map_index = composed_material_sphere_map_index(&semantics);
        let detail_id = composed_material_detail_id(&semantics);
        let multi_detail_id = composed_material_multi_detail_id(&semantics);
        let detail_color = composed_material_detail_color(&semantics);
        let multi_detail_color = composed_material_multi_detail_color(&semantics);
        let shader_diffuse_color = composed_material_shader_diffuse_color(&semantics);
        let shader_multi_diffuse_color = composed_material_shader_multi_diffuse_color(&semantics);
        let shader_emissive_color = composed_material_shader_emissive_color(&semantics);
        let shader_multi_emissive_color = composed_material_shader_multi_emissive_color(&semantics);
        let outline_color = composed_material_outline_color(&semantics);
        let outline_width = composed_material_outline_width(&semantics);
        let specular_color_mask = composed_material_specular_color_mask(&semantics);
        let ssao_mask = composed_material_ssao_mask(&semantics);
        let texture_mip_bias = composed_material_texture_mip_bias(&semantics);
        let shadow_pos_offset = composed_material_shadow_pos_offset(&semantics);
        let detail_color_uv_scale = composed_material_detail_color_uv_scale(&semantics);
        let detail_normal_uv_scale = composed_material_detail_normal_uv_scale(&semantics);
        let uv_scroll = composed_material_uv_scroll(&semantics);
        let lightshaft_color = composed_material_lightshaft_color(&semantics);
        let lightshaft_tex_anim = composed_material_lightshaft_tex_anim(&semantics);
        let lightshaft_tex_u = composed_material_lightshaft_tex_u(&semantics);
        let lightshaft_tex_v = composed_material_lightshaft_tex_v(&semantics);
        let lightshaft_ray = composed_material_lightshaft_ray(&semantics);
        let texture_set = load_weapon_material_textures_from_async_resource(
            resource,
            &path,
            &material,
            color_table_rows.as_deref(),
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
            draw_depth_mode,
            lighting_mode,
            flow_mode,
            value_mode,
            sub_color_mode,
            transparency,
            water_deep_color,
            water_refraction_color,
            water_whitecap_color,
            alpha_aperture,
            alpha_offset,
            shadow_alpha_threshold,
            glass_ior,
            glass_thickness_max,
            normal_scale,
            multi_normal_scale,
            detail_normal_scale,
            multi_detail_normal_scale,
            tile_index,
            tile_alpha,
            tile_scale,
            toon_index,
            toon_light_scale,
            toon_light_spec_aperture,
            toon_reflection_scale,
            toon_spec_index,
            sheen_rate,
            sheen_tint_rate,
            sheen_aperture,
            sphere_map_index,
            detail_id,
            multi_detail_id,
            detail_color,
            multi_detail_color,
            shader_diffuse_color,
            shader_multi_diffuse_color,
            shader_emissive_color,
            shader_multi_emissive_color,
            outline_color,
            outline_width,
            specular_color_mask,
            ssao_mask,
            texture_mip_bias,
            shadow_pos_offset,
            detail_color_uv_scale,
            detail_normal_uv_scale,
            uv_scroll,
            lightshaft_color,
            lightshaft_tex_anim,
            lightshaft_tex_u,
            lightshaft_tex_v,
            lightshaft_ray,
            opacity,
            render_backfaces,
            apply_vertex_color,
            has_color_dye_table: color_dye_table.is_some(),
            color_dye_table,
            staining_application,
            texture_arrays: ModelMaterialTextureArrays::default(),
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            secondary_base_color_texture: texture_set.secondary_base_color,
            normal_texture: texture_set.normal,
            secondary_normal_texture: texture_set.secondary_normal,
            mask_texture: texture_set.mask,
            material_map_texture: texture_set.material_map,
            multi_map_texture: texture_set.multi_map,
            specular_texture: texture_set.specular,
            secondary_specular_texture: texture_set.secondary_specular,
            emissive_texture: texture_set.emissive,
            environment_texture: texture_set.environment,
            material_properties_texture: texture_set.material_properties,
            tile_properties_texture: texture_set.tile_properties,
            sheen_properties_texture: texture_set.sheen_properties,
            sphere_properties_texture: texture_set.sphere_properties,
            tile_matrix_texture: texture_set.tile_matrix,
            index_texture: texture_set.index,
            water_wave_texture: texture_set.water_wave,
            water_wave1_texture: texture_set.water_wave1,
            water_whitecap_texture: texture_set.water_whitecap,
        };
    }

    fallback_weapon_material(slot, material_index, material_name, fallback)
}

#[cfg(feature = "game-data")]
async fn load_weapon_material_textures_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    material_path: &str,
    material: &physis::mtrl::Material,
    color_table_rows: Option<&[ColorTableRowColors]>,
    sampler_roles: &[MaterialSamplerRole],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in material.texture_paths.iter().enumerate() {
        let sampler_kind = sampler_kind_for_texture(sampler_roles, texture_order);
        let kind = classify_weapon_texture(raw_texture_path, sampler_kind);
        let Some(texture_index) = load_weapon_texture_from_async_resource(
            resource,
            material_path,
            raw_texture_path,
            kind,
            sampler_kind,
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
            WeaponModelTextureKind::SecondaryBaseColor => {
                set.secondary_base_color.get_or_insert(texture_index);
                if texture_alpha_affects_material_transparency(&textures[texture_index]) {
                    set.has_alpha = true;
                }
            }
            WeaponModelTextureKind::Normal => {
                set.normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SecondaryNormal => {
                set.secondary_normal.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Mask => {
                set.mask.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MaterialMap => {
                set.material_map.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MultiMap => {
                set.multi_map.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Specular => {
                set.specular.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SecondarySpecular => {
                set.secondary_specular.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Emissive => {
                set.emissive.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Environment => {
                set.environment.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::MaterialProperties => {
                set.material_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::TileProperties => {
                set.tile_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SheenProperties => {
                set.sheen_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::SphereProperties => {
                set.sphere_properties.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::TileMatrixProperties => {
                set.tile_matrix.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::Index => {
                set.index.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::WaterWave => {
                set.water_wave.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::WaterWaveSecondary => {
                set.water_wave1.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::WaterWhitecap => {
                set.water_whitecap.get_or_insert(texture_index);
            }
            WeaponModelTextureKind::TileNormalArray
            | WeaponModelTextureKind::TileOrbArray
            | WeaponModelTextureKind::DetailDiffuseArray
            | WeaponModelTextureKind::DetailNormalArray
            | WeaponModelTextureKind::Other => {}
        }
    }

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        color_table_rows,
        set.index,
        set.emissive.is_none(),
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
        set.tile_properties.get_or_insert(baked.tile_properties);
        add_unique_index(&mut set.indices, baked.tile_properties);
        set.sheen_properties.get_or_insert(baked.sheen_properties);
        add_unique_index(&mut set.indices, baked.sheen_properties);
        set.sphere_properties.get_or_insert(baked.sphere_properties);
        add_unique_index(&mut set.indices, baked.sphere_properties);
        set.tile_matrix.get_or_insert(baked.tile_matrix);
        add_unique_index(&mut set.indices, baked.tile_matrix);
    }

    if set.base_color.is_none() {
        set.base_color = choose_fallback_base_texture(&set.indices, textures);
    }
    refresh_texture_set_alpha(&mut set, textures);

    set
}

#[cfg(feature = "game-data")]
async fn load_weapon_texture_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    material_path: &str,
    raw_texture_path: &str,
    kind: WeaponModelTextureKind,
    sampler_kind: Option<WeaponModelTextureKind>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    use physis::ReadableFile;

    for path in weapon_texture_candidate_paths(material_path, raw_texture_path) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
            textures[index].kind =
                merge_texture_kind(textures[index].kind, kind, sampler_kind.is_some());
            return Some(index);
        }

        let Ok(bytes) = resource.read(&path).await else {
            continue;
        };
        let Some(mut texture) = physis::tex::Texture::from_existing(resource.platform(), &bytes)
        else {
            continue;
        };
        let Some(decoded) =
            crate::texture_decode::decode_texture_rgba_with_layout(&mut texture, &bytes)
        else {
            continue;
        };
        let index = textures.len();
        textures.push(WeaponModelTexture {
            path: path.clone(),
            kind,
            width: decoded.width,
            height: decoded.height,
            array_size: decoded.array_size,
            array_layer_height: decoded.array_layer_height,
            rgba: decoded.rgba,
            rgba_f32: None,
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
    secondary_base_color: Option<usize>,
    normal: Option<usize>,
    secondary_normal: Option<usize>,
    mask: Option<usize>,
    material_map: Option<usize>,
    multi_map: Option<usize>,
    specular: Option<usize>,
    secondary_specular: Option<usize>,
    emissive: Option<usize>,
    environment: Option<usize>,
    material_properties: Option<usize>,
    tile_properties: Option<usize>,
    sheen_properties: Option<usize>,
    sphere_properties: Option<usize>,
    tile_matrix: Option<usize>,
    index: Option<usize>,
    water_wave: Option<usize>,
    water_wave1: Option<usize>,
    water_whitecap: Option<usize>,
    has_alpha: bool,
}

#[cfg(feature = "game-data")]
struct BakedWeaponTextureIndices {
    base_color: usize,
    specular: usize,
    material_properties: usize,
    tile_properties: usize,
    sheen_properties: usize,
    sphere_properties: usize,
    tile_matrix: usize,
    emissive: Option<usize>,
}

#[cfg(feature = "game-data")]
fn texture_has_alpha(texture: &WeaponModelTexture) -> bool {
    texture.rgba.chunks_exact(4).any(|pixel| pixel[3] < 250)
}

#[cfg(feature = "game-data")]
fn texture_alpha_affects_material_transparency(texture: &WeaponModelTexture) -> bool {
    matches!(
        texture.kind,
        WeaponModelTextureKind::BaseColor | WeaponModelTextureKind::SecondaryBaseColor
    ) && texture_has_alpha(texture)
}

#[cfg(feature = "game-data")]
fn refresh_texture_set_alpha(set: &mut WeaponTextureSet, textures: &[WeaponModelTexture]) {
    set.has_alpha = set
        .base_color
        .and_then(|index| textures.get(index))
        .is_some_and(texture_alpha_affects_material_transparency)
        || set
            .secondary_base_color
            .and_then(|index| textures.get(index))
            .is_some_and(texture_alpha_affects_material_transparency);
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct MaterialSamplerRole {
    texture_index: usize,
    kind: WeaponModelTextureKind,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
struct MaterialSamplerRecord {
    texture_index: usize,
    texture_usage: u32,
    texture_usage_name: Option<String>,
    flags: u32,
    kind: Option<WeaponModelTextureKind>,
    kind_source: Option<&'static str>,
}

#[cfg(feature = "game-data")]
#[derive(Default)]
struct ComposedMaterialSemantics {
    material_keys: HashMap<u32, ResolvedMaterialValue<u32>>,
    material_constants: HashMap<u32, ResolvedMaterialValue<Vec<f32>>>,
    resource_names: HashMap<u32, String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq)]
struct ResolvedMaterialValue<T> {
    value: T,
    source: &'static str,
}

#[cfg(feature = "game-data")]
impl ComposedMaterialSemantics {
    fn has_material_key(&self, key: u32, value: u32) -> bool {
        self.material_keys.get(&key).map(|entry| entry.value) == Some(value)
    }

    fn material_key_value(&self, key: u32) -> Option<u32> {
        self.material_keys.get(&key).map(|entry| entry.value)
    }

    fn sampler_kind_resolution(&self, texture_usage: u32) -> MaterialSamplerKindResolution {
        if let Some(name) = self.resource_names.get(&texture_usage) {
            let kind = classify_sampler_name(name);
            return MaterialSamplerKindResolution {
                texture_usage_name: Some(name.clone()),
                kind,
                kind_source: kind.map(|_| "shpkResourceName"),
            };
        }

        if let Some((name, kind)) = known_sampler_names()
            .iter()
            .find(|(name, _)| physis::shpk::ShaderPackage::crc(name) == texture_usage)
        {
            return MaterialSamplerKindResolution {
                texture_usage_name: Some((*name).to_string()),
                kind: Some(*kind),
                kind_source: Some("knownCrc"),
            };
        }

        MaterialSamplerKindResolution {
            texture_usage_name: None,
            kind: None,
            kind_source: None,
        }
    }

    fn material_constant_first_f32(&self, constant_id: u32) -> Option<f32> {
        self.material_constants
            .get(&constant_id)
            .and_then(|entry| entry.value.first())
            .copied()
    }

    fn material_constant_f32_values(&self, constant_id: u32) -> Option<&[f32]> {
        self.material_constants
            .get(&constant_id)
            .map(|entry| entry.value.as_slice())
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
            self.material_constants
                .entry(id)
                .or_insert(ResolvedMaterialValue {
                    value: values,
                    source: "shaderPackageDefault",
                });
        }
    }

    fn apply_material_constants(&mut self, bytes: &[u8]) {
        for (id, values) in material_constants(bytes) {
            self.material_constants.insert(
                id,
                ResolvedMaterialValue {
                    value: values,
                    source: "materialOverride",
                },
            );
        }
    }

    fn apply_shader_package_key_default(&mut self, key: u32, value: u32) {
        self.material_keys
            .entry(key)
            .or_insert(ResolvedMaterialValue {
                value,
                source: "shaderPackageDefault",
            });
    }

    fn apply_material_key(&mut self, key: u32, value: u32) {
        self.material_keys.insert(
            key,
            ResolvedMaterialValue {
                value,
                source: "materialOverride",
            },
        );
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
#[derive(Clone, Debug, PartialEq, Eq)]
struct MaterialSamplerKindResolution {
    texture_usage_name: Option<String>,
    kind: Option<WeaponModelTextureKind>,
    kind_source: Option<&'static str>,
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
    rows: Option<&[ColorTableRowColors]>,
    index_texture: Option<usize>,
    bake_emissive: bool,
    textures: &mut Vec<WeaponModelTexture>,
) -> Option<BakedWeaponTextureIndices> {
    let rows = rows?;
    let index_texture = textures.get(index_texture?)?;
    let width = index_texture.width;
    let height = index_texture.height;
    let id_rgba = index_texture.rgba.clone();
    let baked = bake_color_table_maps(rows, &id_rgba)?;
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

    let tile_properties = push_or_replace_baked_texture(
        textures,
        format!("baked://{material_key}#colorset-tile-properties"),
        WeaponModelTextureKind::TileProperties,
        width,
        height,
        baked.tile_properties_rgba,
    );

    let sheen_properties = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-sheen-properties"),
        WeaponModelTextureKind::SheenProperties,
        width,
        height,
        baked.sheen_properties_rgba,
        Some(baked.sheen_properties_rgba_f32),
    );

    let sphere_properties = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-sphere-properties"),
        WeaponModelTextureKind::SphereProperties,
        width,
        height,
        baked.sphere_properties_rgba,
        Some(baked.sphere_properties_rgba_f32),
    );

    let tile_matrix = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-tile-matrix"),
        WeaponModelTextureKind::TileMatrixProperties,
        width,
        height,
        baked.tile_matrix_rgba,
        Some(baked.tile_matrix_rgba_f32),
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
        tile_properties,
        sheen_properties,
        sphere_properties,
        tile_matrix,
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
    } else if shader.contains("transparency") {
        WeaponMaterialAlphaMode::Blend
    } else if shader_flags & ENABLE_TRANSLUCENCY != 0 {
        WeaponMaterialAlphaMode::Blend
    } else if alpha_test && apply_alpha_test_material_key_applies(&shader) {
        WeaponMaterialAlphaMode::Mask
    } else if texture_set.has_alpha {
        WeaponMaterialAlphaMode::Blend
    } else {
        WeaponMaterialAlphaMode::Opaque
    }
}

#[cfg(feature = "game-data")]
fn apply_alpha_test_material_key_applies(shader_package_name: &str) -> bool {
    let shader = shader_package_name
        .rsplit('/')
        .next()
        .unwrap_or(shader_package_name)
        .to_ascii_lowercase();
    matches!(
        shader.as_str(),
        "bg.shpk"
            | "bgcolorchange.shpk"
            | "bgcrestchange.shpk"
            | "bgprop.shpk"
            | "bguvscroll.shpk"
            | "crystal.shpk"
            | "lightshaft.shpk"
    )
}

#[cfg(feature = "game-data")]
fn weapon_material_opacity(mode: WeaponMaterialRenderMode) -> f32 {
    match mode {
        WeaponMaterialRenderMode::Opaque => 1.0,
        WeaponMaterialRenderMode::Transparent => 1.0,
        WeaponMaterialRenderMode::Glass => 1.0,
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
fn composed_material_transparency(
    semantics: &ComposedMaterialSemantics,
    shader_package_name: &str,
) -> f32 {
    let default = matches!(
        crate::model::material_shader_family(Some(shader_package_name)),
        crate::model::MaterialShaderFamily::Water
    )
    .then_some(1.0)
    .unwrap_or(0.0);
    composed_material_finite_constant(semantics, G_TRANSPARENCY, default).clamp(0.0, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_water_deep_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(
        semantics,
        G_WATER_DEEP_COLOR,
        [0.3529, 0.372_549, 0.3921, 1.0],
    )
}

#[cfg(feature = "game-data")]
fn composed_material_water_refraction_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(
        semantics,
        G_WATER_REFRACTION_COLOR,
        [0.4117, 0.4313, 0.4509, 1.0],
    )
}

#[cfg(feature = "game-data")]
fn composed_material_water_whitecap_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(
        semantics,
        G_WATER_WHITECAP_COLOR,
        [0.4509, 0.4705, 0.4901, 0.3],
    )
}

#[cfg(feature = "game-data")]
fn composed_material_draw_depth_mode(
    semantics: &ComposedMaterialSemantics,
) -> MaterialDrawDepthMode {
    match semantics.material_key_value(DRAW_DEPTH_MODE) {
        None => MaterialDrawDepthMode::None,
        Some(DRAW_DEPTH_MODE_DITHER) => MaterialDrawDepthMode::Dither,
        Some(_) => MaterialDrawDepthMode::Unknown,
    }
}

#[cfg(feature = "game-data")]
fn composed_material_lighting_mode(semantics: &ComposedMaterialSemantics) -> MaterialLightingMode {
    match semantics.material_key_value(ENABLE_LIGHTING) {
        None => MaterialLightingMode::Default,
        Some(ENABLE_LIGHTING_ON) => MaterialLightingMode::Enabled,
        Some(ENABLE_LIGHTING_OFF) => MaterialLightingMode::Disabled,
        Some(_) => MaterialLightingMode::Unknown,
    }
}

#[cfg(feature = "game-data")]
fn composed_material_flow_mode(semantics: &ComposedMaterialSemantics) -> MaterialFlowMode {
    match semantics.material_key_value(CATEGORY_FLOW_MAP_TYPE) {
        None | Some(FLOW_MAP_STANDARD) => MaterialFlowMode::Standard,
        Some(FLOW_MAP_FLOW) => MaterialFlowMode::Flow,
        Some(_) => MaterialFlowMode::Unknown,
    }
}

#[cfg(feature = "game-data")]
fn composed_material_value_mode(semantics: &ComposedMaterialSemantics) -> MaterialValueMode {
    match semantics.material_key_value(GET_VALUES) {
        None | Some(GET_VALUES_SINGLE) => MaterialValueMode::Single,
        Some(GET_VALUES_MULTI) => MaterialValueMode::Multi,
        Some(GET_ALPHA_MULTI_VALUES) => MaterialValueMode::AlphaMulti,
        Some(GET_ALPHA_MULTI_VALUES2) => MaterialValueMode::AlphaMulti2,
        Some(GET_ALPHA_MULTI_VALUES3) => MaterialValueMode::AlphaMulti3,
        Some(GET_VALUES_MULTI_MATERIAL) => MaterialValueMode::MultiMaterial,
        Some(GET_VALUES_COMPATIBILITY) => MaterialValueMode::Compatibility,
        Some(_) => MaterialValueMode::Unknown,
    }
}

#[cfg(feature = "game-data")]
fn composed_material_sub_color_mode(semantics: &ComposedMaterialSemantics) -> MaterialSubColorMode {
    match semantics.material_key_value(GET_SUB_COLOR) {
        None => MaterialSubColorMode::None,
        Some(GET_SUB_COLOR_FACE) => MaterialSubColorMode::Face,
        Some(GET_SUB_COLOR_HAIR) => MaterialSubColorMode::Hair,
        Some(_) => MaterialSubColorMode::Unknown,
    }
}

#[cfg(feature = "game-data")]
fn composed_material_alpha_aperture(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_ALPHA_APERTURE, 2.0)
}

#[cfg(feature = "game-data")]
fn composed_material_alpha_offset(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_ALPHA_OFFSET, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_shadow_alpha_threshold(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SHADOW_ALPHA_THRESHOLD, 0.5).clamp(0.0, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_glass_ior(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_GLASS_IOR, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_glass_thickness_max(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_GLASS_THICKNESS_MAX, 0.01)
}

#[cfg(feature = "game-data")]
fn composed_material_normal_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_normal_scale_constant(semantics, G_NORMAL_SCALE)
}

#[cfg(feature = "game-data")]
fn composed_material_multi_normal_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_normal_scale_constant(semantics, G_MULTI_NORMAL_SCALE)
}

#[cfg(feature = "game-data")]
fn composed_material_detail_normal_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_normal_scale_constant(semantics, G_DETAIL_NORMAL_SCALE)
}

#[cfg(feature = "game-data")]
fn composed_material_multi_detail_normal_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_normal_scale_constant(semantics, G_MULTI_DETAIL_NORMAL_SCALE)
}

#[cfg(feature = "game-data")]
fn composed_material_normal_scale_constant(
    semantics: &ComposedMaterialSemantics,
    constant_id: u32,
) -> f32 {
    semantics
        .material_constant_first_f32(constant_id)
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 4.0))
        .unwrap_or(1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_tile_index(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TILE_INDEX, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_tile_alpha(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TILE_ALPHA, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_tile_scale(semantics: &ComposedMaterialSemantics) -> [f32; 2] {
    let mut scale = [16.0, 16.0];
    if let Some(values) = semantics.material_constant_f32_values(G_TILE_SCALE) {
        for (target, value) in scale.iter_mut().zip(values.iter().copied()) {
            if value.is_finite() {
                *target = value;
            }
        }
    }
    scale
}

#[cfg(feature = "game-data")]
fn composed_material_toon_index(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TOON_INDEX, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_toon_light_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TOON_LIGHT_SCALE, 2.0)
}

#[cfg(feature = "game-data")]
fn composed_material_toon_light_spec_aperture(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TOON_LIGHT_SPEC_APERTURE, 50.0)
}

#[cfg(feature = "game-data")]
fn composed_material_toon_reflection_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TOON_REFLECTION_SCALE, 2.5)
}

#[cfg(feature = "game-data")]
fn composed_material_toon_spec_index(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TOON_SPEC_INDEX, 4.0e-45)
}

#[cfg(feature = "game-data")]
fn composed_material_sheen_rate(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SHEEN_RATE, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_sheen_tint_rate(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SHEEN_TINT_RATE, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_sheen_aperture(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SHEEN_APERTURE, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_sphere_map_index(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SPHERE_MAP_INDEX, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_detail_id(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_DETAIL_ID, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_multi_detail_id(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_MULTI_DETAIL_ID, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_detail_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_DETAIL_COLOR, [0.5, 0.5, 0.5, 1.0])
}

#[cfg(feature = "game-data")]
fn composed_material_multi_detail_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_MULTI_DETAIL_COLOR, [0.5, 0.5, 0.5, 1.0])
}

#[cfg(feature = "game-data")]
fn composed_material_shader_diffuse_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_DIFFUSE_COLOR, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_shader_multi_diffuse_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_MULTI_DIFFUSE_COLOR, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_shader_emissive_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_EMISSIVE_COLOR, [0.0, 0.0, 0.0, 1.0])
}

#[cfg(feature = "game-data")]
fn composed_material_shader_multi_emissive_color(
    semantics: &ComposedMaterialSemantics,
) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_MULTI_EMISSIVE_COLOR, [0.0, 0.0, 0.0, 1.0])
}

#[cfg(feature = "game-data")]
fn composed_material_outline_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_OUTLINE_COLOR, [0.0, 0.0, 0.0, 1.0])
}

#[cfg(feature = "game-data")]
fn composed_material_outline_width(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_OUTLINE_WIDTH, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_specular_color_mask(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_SPECULAR_COLOR_MASK, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_ssao_mask(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SSAO_MASK, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_texture_mip_bias(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TEXTURE_MIP_BIAS, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_shadow_pos_offset(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_SHADOW_POS_OFFSET, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_detail_color_uv_scale(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_DETAIL_COLOR_UV_SCALE, [4.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_detail_normal_uv_scale(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_DETAIL_NORMAL_UV_SCALE, [4.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_uv_scroll(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    let raw = composed_material_finite_vec4_constant(semantics, G_UV_SCROLL_TIME, [0.0; 4]);
    [-raw[0], raw[1], -raw[2], raw[3]]
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_LIGHTSHAFT_COLOR, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_tex_anim(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_LIGHTSHAFT_TEX_ANIM, [0.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_tex_u(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_LIGHTSHAFT_TEX_U, [1.0, 0.0, 0.0, 0.0])
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_tex_v(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_LIGHTSHAFT_TEX_V, [0.0, 1.0, 0.0, 0.0])
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_ray(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_LIGHTSHAFT_RAY, [0.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_finite_vec4_constant(
    semantics: &ComposedMaterialSemantics,
    constant_id: u32,
    default: [f32; 4],
) -> [f32; 4] {
    let mut values = default;
    if let Some(source) = semantics.material_constant_f32_values(constant_id) {
        for (target, value) in values.iter_mut().zip(source.iter().copied()) {
            if value.is_finite() {
                *target = value;
            }
        }
    }
    values
}

#[cfg(feature = "game-data")]
fn composed_material_finite_constant(
    semantics: &ComposedMaterialSemantics,
    constant_id: u32,
    default: f32,
) -> f32 {
    semantics
        .material_constant_first_f32(constant_id)
        .filter(|value| value.is_finite())
        .unwrap_or(default)
}

#[cfg(feature = "game-data")]
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
            rgba.push(base[3]);
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
    push_or_replace_baked_texture_with_float_channels(
        textures, path, kind, width, height, rgba, None,
    )
}

#[cfg(feature = "game-data")]
fn push_or_replace_baked_texture_with_float_channels(
    textures: &mut Vec<WeaponModelTexture>,
    path: String,
    kind: WeaponModelTextureKind,
    width: u16,
    height: u16,
    rgba: Vec<u8>,
    rgba_f32: Option<Vec<[f32; 4]>>,
) -> usize {
    if let Some(index) = textures.iter().position(|texture| texture.path == path) {
        textures[index] = WeaponModelTexture {
            path,
            kind,
            width,
            height,
            array_size: 1,
            array_layer_height: height,
            rgba,
            rgba_f32,
        };
        return index;
    }

    let index = textures.len();
    textures.push(WeaponModelTexture {
        path,
        kind,
        width,
        height,
        array_size: 1,
        array_layer_height: height,
        rgba,
        rgba_f32,
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
                    scalar3: row.unknown3,
                    // physis still exposes these Dawntrail fields with placeholder names.
                    // Meddle names them GlossStrength and SpecularStrength respectively.
                    gloss_strength: row.unknown1,
                    specular_strength: row.unknown2,
                    roughness: row.roughness,
                    metalness: row.metalness,
                    anisotropy: row.anisotropy,
                    tile_alpha: row.tile_alpha,
                    tile_index: dawntrail_tile_index(row.tile_set),
                    sheen_rate: row.sheen_rate,
                    sheen_tint: row.sheen_tint,
                    sheen_aperture: row.sheen_aperture,
                    sphere_index: dawntrail_sphere_index(row.sphere_index),
                    sphere_mask: row.sphere_mask,
                    tile_matrix: [
                        row.material_repeat[0],
                        row.material_repeat[1],
                        row.material_skew[0],
                        row.material_skew[1],
                    ],
                })
                .collect(),
        ),
        physis::mtrl::ColorTable::LegacyColorTable(table) => Some(
            table
                .rows
                .iter()
                .map(|row| ColorTableRowColors {
                    diffuse: row.diffuse_color,
                    specular: row.specular_color,
                    emissive: row.emissive_color,
                    gloss_strength: row.gloss_strength,
                    specular_strength: row.specular_strength,
                    tile_index: f32::from(row.tile_set),
                    tile_matrix: [
                        row.material_repeat_x,
                        row.material_repeat_y,
                        row.material_skew[0],
                        row.material_skew[1],
                    ],
                    ..Default::default()
                })
                .collect(),
        ),
        physis::mtrl::ColorTable::OpaqueColorTable(_) => None,
    }
}

#[cfg(feature = "game-data")]
fn dawntrail_tile_index(tile_set: u16) -> f32 {
    half_to_f32(tile_set) * 64.0
}

#[cfg(feature = "game-data")]
fn dawntrail_sphere_index(sphere_index: u16) -> f32 {
    half_to_f32(sphere_index)
}

#[cfg(feature = "game-data")]
fn half_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let mantissa = u32::from(bits & 0x03ff);
    let value = match exponent {
        0 => {
            if mantissa == 0 {
                sign
            } else {
                let mut mantissa = mantissa;
                let mut exponent = -14_i32;
                while (mantissa & 0x0400) == 0 {
                    mantissa <<= 1;
                    exponent -= 1;
                }
                mantissa &= 0x03ff;
                let exponent = u32::try_from(exponent + 127).unwrap_or(0);
                sign | (exponent << 23) | (mantissa << 13)
            }
        }
        0x1f => sign | 0x7f80_0000 | (mantissa << 13),
        _ => {
            let exponent = u32::from(exponent) + 112;
            sign | (exponent << 23) | (mantissa << 13)
        }
    };
    f32::from_bits(value)
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
    rows: Option<&[ColorTableRowColors]>,
    fallback: [f32; 3],
) -> MaterialColorSummary {
    let mut diffuse = ColorAccumulator::default();
    let mut specular = ColorAccumulator::default();
    let mut emissive = [0.0; 3];
    let mut roughness_total = 0.0;
    let mut metalness_total = 0.0;
    let mut physical_rows = 0_u32;

    for row in rows.unwrap_or_default() {
        diffuse.add_nonzero(row.diffuse);
        specular.add_nonzero(row.specular);
        emissive = brighter_color(emissive, row.emissive);
        if row.roughness.is_finite() && row.metalness.is_finite() {
            roughness_total += row.roughness.clamp(0.0, 1.0);
            metalness_total += row.metalness.clamp(0.0, 1.0);
            physical_rows += 1;
        }
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
        draw_depth_mode: MaterialDrawDepthMode::None,
        lighting_mode: MaterialLightingMode::Default,
        flow_mode: MaterialFlowMode::Standard,
        value_mode: MaterialValueMode::Single,
        sub_color_mode: MaterialSubColorMode::None,
        transparency: 0.0,
        water_deep_color: [0.3529, 0.372_549, 0.3921, 1.0],
        water_refraction_color: [0.4117, 0.4313, 0.4509, 1.0],
        water_whitecap_color: [0.4509, 0.4705, 0.4901, 0.3],
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
        fallback_color: fallback,
        diffuse_color: fallback,
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        secondary_base_color_texture: None,
        normal_texture: None,
        secondary_normal_texture: None,
        mask_texture: None,
        material_map_texture: None,
        multi_map_texture: None,
        specular_texture: None,
        secondary_specular_texture: None,
        emissive_texture: None,
        environment_texture: None,
        material_properties_texture: None,
        tile_properties_texture: None,
        sheen_properties_texture: None,
        sphere_properties_texture: None,
        tile_matrix_texture: None,
        index_texture: None,
        water_wave_texture: None,
        water_wave1_texture: None,
        water_whitecap_texture: None,
    }
}

#[cfg(feature = "game-data")]
fn reuse_loaded_material_for_missing_reference(
    material: WeaponModelMaterial,
    loaded_materials: &[WeaponModelMaterial],
) -> WeaponModelMaterial {
    if material.path.is_some() {
        return material;
    }
    let Some(source) = loaded_materials
        .iter()
        .find(|source| source.material_index == material.material_index && source.path.is_some())
    else {
        return material;
    };

    let mut reused = source.clone();
    reused.slot = material.slot;
    reused.material_index = material.material_index;
    reused.name = material.name;
    reused
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
        let Some(flags) = read_u32_le(bytes, sampler_offset + 4) else {
            return records;
        };
        let Some(texture_index) = bytes.get(sampler_offset + 8).copied().map(usize::from) else {
            return records;
        };
        if texture_index < layout.texture_count {
            let resolution = semantics.sampler_kind_resolution(texture_usage);
            records.push(MaterialSamplerRecord {
                texture_index,
                texture_usage,
                texture_usage_name: resolution.texture_usage_name,
                flags,
                kind: resolution.kind,
                kind_source: resolution.kind_source,
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
        if value_size >= 4
            && value_offset.saturating_add(value_size) <= layout.shader_value_list_size
        {
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
fn material_constant_debug(bytes: &[u8]) -> Vec<MaterialConstantDebug> {
    let Some(layout) = material_shader_table_layout(bytes) else {
        return Vec::new();
    };
    let mut constant_offset = layout.constant_offset;
    let mut constants = Vec::new();

    for _ in 0..layout.constant_count {
        let Some(id) = read_u32_le(bytes, constant_offset) else {
            return constants;
        };
        let Some(value_offset) = read_u16_le(bytes, constant_offset + 4) else {
            return constants;
        };
        let Some(value_size) = read_u16_le(bytes, constant_offset + 6) else {
            return constants;
        };

        let value_offset_usize = usize::from(value_offset);
        let value_size_usize = usize::from(value_size);
        let mut raw_values = Vec::new();
        let mut values = Vec::new();

        if value_size_usize >= 4
            && value_offset_usize.saturating_add(value_size_usize) <= layout.shader_value_list_size
        {
            let Some(value_start) = layout.shader_values_offset.checked_add(value_offset_usize)
            else {
                return constants;
            };
            let value_count = value_size_usize / 4;
            for index in 0..value_count {
                let value_offset = value_start + index * 4;
                let Some(raw_value) = read_u32_le(bytes, value_offset) else {
                    return constants;
                };
                let Some(value) = read_f32_le(bytes, value_offset) else {
                    return constants;
                };
                raw_values.push(raw_value);
                values.push(value);
            }
        }

        constants.push(MaterialConstantDebug {
            id,
            id_hex: hex_u32(id),
            value_offset,
            value_size,
            value_count: raw_values.len(),
            raw_values_hex: raw_values.iter().copied().map(hex_u32).collect(),
            raw_values,
            values,
        });

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
    shader_value_list_size: usize,
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
        shader_value_list_size,
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
#[cfg(test)]
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
        (
            "g_SamplerNormalMap1",
            WeaponModelTextureKind::SecondaryNormal,
        ),
        ("g_SamplerSkinNormal", WeaponModelTextureKind::Normal),
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
        ("g_SamplerSkinMask", WeaponModelTextureKind::Mask),
        ("g_SamplerMaterial", WeaponModelTextureKind::MaterialMap),
        ("g_MaterialSampler", WeaponModelTextureKind::MaterialMap),
        ("g_SamplerMulti", WeaponModelTextureKind::MultiMap),
        ("g_MultiSampler", WeaponModelTextureKind::MultiMap),
        ("g_SamplerSpecular", WeaponModelTextureKind::Specular),
        ("g_SpecularSampler", WeaponModelTextureKind::Specular),
        ("g_SamplerSpecularMap", WeaponModelTextureKind::Specular),
        ("g_SpecularMapSampler", WeaponModelTextureKind::Specular),
        ("g_SamplerSpecularMap0", WeaponModelTextureKind::Specular),
        (
            "g_SamplerSpecularMap1",
            WeaponModelTextureKind::SecondarySpecular,
        ),
        ("g_SamplerReflect", WeaponModelTextureKind::Specular),
        ("g_ReflectSampler", WeaponModelTextureKind::Specular),
        ("g_SamplerDiffuse", WeaponModelTextureKind::BaseColor),
        ("g_DiffuseSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColor", WeaponModelTextureKind::BaseColor),
        ("g_ColorSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColorMap", WeaponModelTextureKind::BaseColor),
        ("g_ColorMapSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerColorMap0", WeaponModelTextureKind::BaseColor),
        (
            "g_SamplerColorMap1",
            WeaponModelTextureKind::SecondaryBaseColor,
        ),
        ("g_SamplerSkinDiffuse", WeaponModelTextureKind::BaseColor),
        ("g_SamplerAlbedo", WeaponModelTextureKind::BaseColor),
        ("g_AlbedoSampler", WeaponModelTextureKind::BaseColor),
        ("g_SamplerBaseColor", WeaponModelTextureKind::BaseColor),
        ("g_BaseColorSampler", WeaponModelTextureKind::BaseColor),
        ("g_Sampler0", WeaponModelTextureKind::BaseColor),
        ("g_Sampler1", WeaponModelTextureKind::BaseColor),
        ("g_SamplerEnvMap", WeaponModelTextureKind::Environment),
        ("g_SamplerWaveMap", WeaponModelTextureKind::WaterWave),
        (
            "g_SamplerWaveMap1",
            WeaponModelTextureKind::WaterWaveSecondary,
        ),
        (
            "g_SamplerWhitecapMap",
            WeaponModelTextureKind::WaterWhitecap,
        ),
    ]
}

#[cfg(feature = "game-data")]
fn classify_weapon_texture(
    path: &str,
    sampler_kind: Option<WeaponModelTextureKind>,
) -> WeaponModelTextureKind {
    if let Some(kind) = sampler_kind {
        return kind;
    }

    let path = path.to_ascii_lowercase();
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path.as_str())
        .trim_end_matches(".tex");

    if stem.ends_with("_id") || stem.contains("_id_") || stem.contains("index") {
        return WeaponModelTextureKind::Index;
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
    incoming_from_sampler: bool,
) -> WeaponModelTextureKind {
    if incoming_from_sampler {
        return incoming;
    }

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
fn read_bytes(bytes: &[u8], offset: usize, len: usize) -> Option<&[u8]> {
    bytes.get(offset..offset.checked_add(len)?)
}

#[cfg(feature = "game-data")]
fn read_string_at(bytes: &[u8], offset: usize) -> Option<String> {
    let bytes = bytes.get(offset..)?;
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    std::str::from_utf8(&bytes[..end])
        .ok()
        .map(ToString::to_string)
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
    fn weapon_model_load_request_normalizes_stain_ids() {
        let request = WeaponModelLoadRequest {
            item_id: 1,
            item_name: "test".to_string(),
            model_main: 2,
            model_sub: 3,
            stain_ids: [MAX_STAIN_ID, u8::MAX],
        };

        assert_eq!(request.normalized_stain_ids(), [MAX_STAIN_ID, 0]);
        assert_eq!(request.clone().with_stain_ids([17, 93]).stain_ids, [17, 93]);
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_shared_texture_arrays_decode_as_vertical_atlases() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let cases = [
            (
                CHARACTER_TILE_NORMAL_ARRAY_PATH,
                ModelTextureKind::TileNormalArray,
            ),
            (
                CHARACTER_TILE_ORB_ARRAY_PATH,
                ModelTextureKind::TileOrbArray,
            ),
            (
                BG_DETAIL_DIFFUSE_ARRAY_PATH,
                ModelTextureKind::DetailDiffuseArray,
            ),
            (
                BG_DETAIL_NORMAL_ARRAY_PATH,
                ModelTextureKind::DetailNormalArray,
            ),
        ];

        for (path, kind) in cases {
            let mut textures = Vec::new();
            let mut loaded_paths = Vec::new();
            let index = load_shared_texture_array_from_resource(
                &mut resource,
                path,
                kind,
                &mut textures,
                &mut loaded_paths,
            )
            .unwrap_or_else(|error| panic!("{path}: {error}"));
            let texture = &textures[index];

            eprintln!(
                "{path}: kind={:?}, {}x{}, layers={}, layer_height={}, rgba={}",
                texture.kind,
                texture.width,
                texture.height,
                texture.array_size,
                texture.array_layer_height,
                texture.rgba.len()
            );
            assert_eq!(texture.path, path);
            assert_eq!(texture.kind, kind);
            assert!(texture.array_size > 1);
            assert_eq!(
                u32::from(texture.height),
                u32::from(texture.array_layer_height) * u32::from(texture.array_size)
            );
            assert_eq!(
                texture.rgba.len(),
                usize::from(texture.width) * usize::from(texture.height) * 4
            );
            assert_eq!(loaded_paths, vec![path.to_string()]);
        }
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_character_weapon_attaches_tile_texture_arrays() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 45052,
            item_name: "奶油之幻梦".to_string(),
            model_main: 4_295_295_803,
            model_sub: 0,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let material = model
            .materials
            .iter()
            .find(|material| material.texture_arrays.tile_normal.is_some())
            .expect("material using character tile arrays");
        let prepared =
            crate::model::prepare_material_for_draw_role(Some(material), ModelMeshDrawRole::Normal);

        assert!(material.texture_arrays.tile_orb.is_some());
        assert!(material.texture_arrays.errors.is_empty());
        assert!(prepared.resource_availability.tile_array_complete);
        assert!(prepared.unsupported_inputs.tile_array);
        assert!(
            model
                .loaded_paths
                .iter()
                .any(|path| path == CHARACTER_TILE_NORMAL_ARRAY_PATH)
        );
        assert!(
            model
                .loaded_paths
                .iter()
                .any(|path| path == CHARACTER_TILE_ORB_ARRAY_PATH)
        );
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_character_glass_uses_normal_blue_alpha_policy() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 45059,
            item_name: "冬雪之幻梦".to_string(),
            model_main: 4_295_034_963,
            model_sub: 773_094_181_015,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let material = model
            .materials
            .iter()
            .find(|material| material.shader_package_name.as_deref() == Some("characterglass.shpk"))
            .expect("character glass material");
        let normal = &model.textures[material.normal_texture.expect("glass normal texture")];
        let (blue_min, blue_max) = normal
            .rgba
            .chunks_exact(4)
            .fold((u8::MAX, u8::MIN), |(min, max), pixel| {
                (min.min(pixel[2]), max.max(pixel[2]))
            });
        let prepared =
            crate::model::prepare_material_for_draw_role(Some(material), ModelMeshDrawRole::Normal);

        eprintln!(
            "glass normal blue range={blue_min}..{blue_max}, depth={:?}, lighting={:?}",
            material.draw_depth_mode, material.lighting_mode
        );
        assert_eq!(material.draw_depth_mode, MaterialDrawDepthMode::Dither);
        assert_eq!(material.lighting_mode, MaterialLightingMode::Default);
        assert_eq!(
            prepared.render_pass,
            crate::model::PreparedRenderPass::Glass
        );
        assert_eq!(
            prepared.alpha_policy.source,
            crate::model::PreparedAlphaSource::NormalBlue
        );
        assert!(blue_min < blue_max);
        assert!(blue_min < u8::MAX);
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_weapon_stain_changes_baked_color_table() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 45052,
            item_name: "奶油之幻梦".to_string(),
            model_main: 4_295_295_803,
            model_sub: 0,
            stain_ids: [0, 0],
        };

        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let unstained =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("unstained");
        let stained = load_weapon_model_from_resource_request(
            &mut resource,
            &request.clone().with_stain_ids([1, 0]),
        )
        .expect("stained");

        assert_eq!(unstained.stain_ids, [0, 0]);
        assert_eq!(stained.stain_ids, [1, 0]);
        assert!(
            stained
                .loaded_paths
                .iter()
                .any(|path| path == DAWNTRAIL_STAINING_TEMPLATE_PATH)
        );

        let stained_material = stained
            .materials
            .iter()
            .find(|material| {
                material
                    .staining_application
                    .as_ref()
                    .is_some_and(|application| {
                        application.error.is_none() && application.report.rows_changed != 0
                    })
            })
            .expect("stained material");
        let unstained_material = unstained
            .materials
            .iter()
            .find(|material| material.path == stained_material.path)
            .expect("matching unstained material");
        let stained_texture = &stained.textures[stained_material
            .base_color_texture
            .expect("stained base texture")];
        let unstained_texture = &unstained.textures[unstained_material
            .base_color_texture
            .expect("unstained base texture")];

        eprintln!(
            "staining application: {:#?}",
            stained_material.staining_application
        );
        assert_ne!(stained_texture.rgba, unstained_texture.rgba);
        assert!(
            !crate::model::prepare_material_for_draw_role(
                Some(stained_material),
                ModelMeshDrawRole::Normal
            )
            .unsupported_inputs
            .dye_application
        );
    }

    #[test]
    fn sub_model_load_failure_diagnostic_preserves_candidate_errors() {
        let model = PackedModelId::from_raw(0x0001_0002_0064);
        let failure = WeaponModelMeshLoadFailure::new(
            model,
            vec![
                model_load_candidate(
                    "chara/weapon/w0064/obj/body/b0002/model/w0064b0002.mdl".to_string(),
                    WeaponModelLoadCandidateStatus::Missing,
                    "resource read returned no bytes",
                ),
                model_load_candidate(
                    "chara/weapon/w0064/obj/body/b0002/model/w0064b0002_damaged.mdl".to_string(),
                    WeaponModelLoadCandidateStatus::ParseError,
                    "failed to load render meshes",
                ),
            ],
        );

        let diagnostic = failure.into_diagnostic(WeaponModelLoadRole::Secondary);

        assert_eq!(diagnostic.role, WeaponModelLoadRole::Secondary);
        assert_eq!(diagnostic.model, model);
        assert!(diagnostic.error.contains("unable to read weapon model"));
        assert_eq!(diagnostic.candidates.len(), 2);
        assert_eq!(
            diagnostic.candidates[0].status,
            WeaponModelLoadCandidateStatus::Missing
        );
        assert_eq!(
            diagnostic.candidates[1].status,
            WeaponModelLoadCandidateStatus::ParseError
        );
        assert!(
            diagnostic.candidates[1]
                .error
                .contains("failed to load render meshes")
        );
    }

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

        let records = parse_material_sampler_records(&bytes, &semantics);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].texture_usage_name.as_deref(), Some(sampler_name));
        assert_eq!(records[0].kind, Some(WeaponModelTextureKind::Index));
        assert_eq!(records[0].kind_source, Some("shpkResourceName"));
    }

    #[test]
    fn parse_material_sampler_records_preserves_sampler_flags() {
        let texture_usage = physis::shpk::ShaderPackage::crc("g_SamplerNormal");
        let mut bytes = vec![0; 16];
        bytes[12] = 1;
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0x10_u32.to_le_bytes());
        bytes.extend_from_slice(&texture_usage.to_le_bytes());
        bytes.extend_from_slice(&0x1234_5678_u32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0; 3]);

        let records = parse_material_sampler_records(&bytes, &ComposedMaterialSemantics::default());

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].texture_usage, texture_usage);
        assert_eq!(
            records[0].texture_usage_name.as_deref(),
            Some("g_SamplerNormal")
        );
        assert_eq!(records[0].flags, 0x1234_5678);
        assert_eq!(records[0].texture_index, 0);
        assert_eq!(records[0].kind, Some(WeaponModelTextureKind::Normal));
        assert_eq!(records[0].kind_source, Some("knownCrc"));
    }

    #[test]
    fn parse_material_sampler_records_marks_unknown_sampler_source() {
        let texture_usage = 0x1234_5678_u32;
        let mut bytes = vec![0; 16];
        bytes[12] = 1;
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&texture_usage.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0; 3]);

        let records = parse_material_sampler_records(&bytes, &ComposedMaterialSemantics::default());

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].texture_usage, texture_usage);
        assert_eq!(records[0].texture_usage_name, None);
        assert_eq!(records[0].kind, None);
        assert_eq!(records[0].kind_source, None);
    }

    #[test]
    fn material_low_level_debug_preserves_meddle_mtrl_fields() {
        let bytes = test_mtrl_with_low_level_fields();
        let debug = material_low_level_debug(&bytes, &[]).expect("debug info");

        assert_eq!(debug.file_header.version, 0x0103_0000);
        assert_eq!(debug.file_header.version_hex, "0x01030000");
        assert_eq!(debug.file_header.file_size, bytes.len() as u16);
        assert_eq!(debug.file_header.data_set_size, 3);
        assert_eq!(debug.file_header.texture_count, 1);
        assert_eq!(debug.file_header.uv_set_count, 1);
        assert_eq!(debug.file_header.color_set_count, 1);
        assert_eq!(debug.file_header.additional_data_size, 2);

        assert_eq!(debug.texture_offsets.len(), 1);
        assert_eq!(debug.texture_offsets[0].offset, 0);
        assert_eq!(debug.texture_offsets[0].flags, 0x00f0);
        assert_eq!(debug.texture_offsets[0].flags_hex, "0x00f0");
        assert_eq!(
            debug.texture_offsets[0].path.as_deref(),
            Some("texture/base.tex")
        );

        assert_eq!(debug.uv_color_sets.len(), 1);
        assert_eq!(debug.uv_color_sets[0].name.as_deref(), Some("uv0"));
        assert_eq!(debug.uv_color_sets[0].set_index, 2);
        assert_eq!(debug.uv_color_sets[0].unknown1, 3);

        assert_eq!(debug.color_sets.len(), 1);
        assert_eq!(debug.color_sets[0].name.as_deref(), Some("color0"));
        assert_eq!(debug.color_sets[0].set_index, 4);
        assert_eq!(debug.color_sets[0].unknown1, 5);

        assert_eq!(debug.additional_data, vec![0x30, 0x05]);
        assert_eq!(debug.data_set_size, 3);

        let shader_header = debug.shader_header.expect("shader header");
        assert_eq!(shader_header.shader_value_list_size, 8);
        assert_eq!(shader_header.shader_key_count, 1);
        assert_eq!(shader_header.constant_count, 1);
        assert_eq!(shader_header.sampler_count, 1);
        assert_eq!(shader_header.flags, 0x11);
        assert_eq!(shader_header.flags_hex, "0x00000011");
        assert_eq!(debug.shader_value_list_size, 8);
        assert_eq!(debug.shader_value_count, 2);
    }

    #[test]
    fn material_semantic_summary_compacts_keys_constants_and_flags() {
        let bytes = test_mtrl_with_low_level_fields();
        let low_level = material_low_level_debug(&bytes, &[]).expect("debug info");
        let mut semantics = ComposedMaterialSemantics::default();
        semantics.apply_shader_package_key_default(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_OFF);
        semantics.apply_material_key(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON);
        semantics.apply_material_constants(&bytes);
        let samplers = parse_material_sampler_records(&bytes, &semantics)
            .into_iter()
            .map(|record| MaterialSamplerDebug {
                texture_index: record.texture_index,
                texture_path: Some("texture/base.tex".to_string()),
                texture_usage: record.texture_usage,
                texture_usage_hex: hex_u32(record.texture_usage),
                texture_usage_name: record.texture_usage_name,
                flags: record.flags,
                flags_hex: hex_u32(record.flags),
                kind: record.kind,
                kind_source: record.kind_source.map(ToString::to_string),
            })
            .collect::<Vec<_>>();
        let material_shader_keys = vec![MaterialShaderKeyDebug {
            category: APPLY_ALPHA_TEST,
            category_hex: hex_u32(APPLY_ALPHA_TEST),
            value: APPLY_ALPHA_TEST_ON,
            value_hex: hex_u32(APPLY_ALPHA_TEST_ON),
        }];

        let summary = material_semantic_summary(
            0x11,
            &material_shader_keys,
            &semantics,
            &low_level.texture_offsets,
            &samplers,
        );

        assert_eq!(summary.shader_flags, 0x11);
        assert_eq!(summary.shader_flags_hex, "0x00000011");
        assert_eq!(summary.shader_key_count, 1);
        assert_eq!(summary.resolved_shader_key_count, 1);
        assert_eq!(
            summary.shader_keys[0].category_name.as_deref(),
            Some("ApplyAlphaTest")
        );
        assert_eq!(
            summary.shader_keys[0].value_name.as_deref(),
            Some("ApplyAlphaTestOn")
        );
        assert_eq!(summary.shader_keys[0].source, "materialOverride");
        assert_eq!(summary.resolved_constant_count, 1);
        assert_eq!(
            summary.constants[0].name.as_deref(),
            Some("g_AlphaThreshold")
        );
        assert_eq!(summary.constants[0].values, vec![0.25]);
        assert_eq!(summary.constants[0].source, "materialOverride");
        assert_eq!(summary.texture_flags[0].flags, 0x00f0);
        assert_eq!(summary.sampler_flags[0].flags, 0x1234_5678);
        assert_eq!(
            summary.sampler_flags[0].kind,
            Some(WeaponModelTextureKind::Normal)
        );
        assert_eq!(
            summary.sampler_flags[0].kind_source.as_deref(),
            Some("knownCrc")
        );
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
    fn material_constant_debug_preserves_raw_constant_entries() {
        let bytes = test_mtrl_with_constant(G_ALPHA_THRESHOLD, &[0.42], 0);

        let constants = material_constant_debug(&bytes);

        assert_eq!(constants.len(), 1);
        assert_eq!(constants[0].id, G_ALPHA_THRESHOLD);
        assert_eq!(constants[0].id_hex, hex_u32(G_ALPHA_THRESHOLD));
        assert_eq!(constants[0].value_offset, 0);
        assert_eq!(constants[0].value_size, 4);
        assert_eq!(constants[0].value_count, 1);
        assert_eq!(constants[0].raw_values, vec![0.42_f32.to_bits()]);
        assert_eq!(
            constants[0].raw_values_hex,
            vec![hex_u32(0.42_f32.to_bits())]
        );
        assert_eq!(constants[0].values, vec![0.42]);
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
    fn material_constants_reject_values_outside_shader_value_list() {
        let mut bytes = test_mtrl_with_constant(G_ALPHA_THRESHOLD, &[], 0);
        let constant_offset = material_shader_table_layout(&bytes)
            .expect("layout")
            .constant_offset;
        bytes[constant_offset + 4..constant_offset + 6].copy_from_slice(&4_u16.to_le_bytes());
        bytes[constant_offset + 6..constant_offset + 8].copy_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&0.75_f32.to_le_bytes());

        assert_eq!(material_constants(&bytes), Vec::<(u32, Vec<f32>)>::new());
        let constants = material_constant_debug(&bytes);
        assert_eq!(constants.len(), 1);
        assert_eq!(constants[0].value_offset, 4);
        assert_eq!(constants[0].value_size, 4);
        assert!(constants[0].values.is_empty());
        assert!(constants[0].raw_values.is_empty());
    }

    #[test]
    fn dawntrail_color_table_rows_use_meddle_strength_names() {
        let row = test_dawntrail_color_table_row();
        let color_table =
            physis::mtrl::ColorTable::DawntrailColorTable(physis::mtrl::DawntrailColorTableData {
                rows: vec![row],
            });

        let rows = weapon_color_table_rows(&color_table).expect("dawntrail rows");

        assert_eq!(rows[0].gloss_strength, row.unknown1);
        assert_eq!(rows[0].specular_strength, row.unknown2);
        assert_eq!(rows[0].anisotropy, row.anisotropy);
        assert_eq!(rows[0].tile_alpha, row.tile_alpha);
        assert_eq!(rows[0].tile_index, dawntrail_tile_index(row.tile_set));
        assert_eq!(rows[0].sheen_rate, row.sheen_rate);
        assert_eq!(rows[0].sheen_tint, row.sheen_tint);
        assert_eq!(rows[0].sheen_aperture, row.sheen_aperture);
        assert_eq!(rows[0].sphere_index, 2.0);
        assert_eq!(rows[0].sphere_mask, row.sphere_mask);
        assert_eq!(
            rows[0].tile_matrix,
            [
                row.material_repeat[0],
                row.material_repeat[1],
                row.material_skew[0],
                row.material_skew[1],
            ]
        );

        let baked = bake_color_table_maps(&[rows[0], rows[0]], &[0, 0, 0, 255])
            .expect("bake Dawntrail sphere properties");
        assert_eq!(baked.sphere_properties_rgba[0], 2);
    }

    #[test]
    fn dawntrail_color_table_debug_exposes_tile_properties() {
        let row = test_dawntrail_color_table_row();
        let color_table =
            physis::mtrl::ColorTable::DawntrailColorTable(physis::mtrl::DawntrailColorTableData {
                rows: vec![row],
            });

        let debug = material_color_table_debug(Some(&color_table)).expect("debug");

        assert_eq!(debug.kind, "Dawntrail");
        assert_eq!(debug.rows[0].tile_alpha, Some(row.tile_alpha));
        assert_eq!(
            debug.rows[0].tile_index,
            Some(dawntrail_tile_index(row.tile_set))
        );
        assert_eq!(debug.rows[0].sheen_rate, Some(row.sheen_rate));
        assert_eq!(debug.rows[0].sheen_tint, Some(row.sheen_tint));
        assert_eq!(debug.rows[0].sheen_aperture, Some(row.sheen_aperture));
        assert_eq!(debug.rows[0].sphere_mask, Some(row.sphere_mask));
        assert_eq!(debug.rows[0].sphere_index, Some(0x4000));
        assert_eq!(
            debug.rows[0].tile_matrix,
            Some([
                row.material_repeat[0],
                row.material_repeat[1],
                row.material_skew[0],
                row.material_skew[1],
            ])
        );
    }

    #[test]
    fn legacy_color_table_rows_use_meddle_strength_names() {
        let row = test_legacy_color_table_row();
        let color_table =
            physis::mtrl::ColorTable::LegacyColorTable(physis::mtrl::LegacyColorTableData {
                rows: vec![row],
            });

        let rows = weapon_color_table_rows(&color_table).expect("legacy rows");

        assert_eq!(rows[0].diffuse, row.diffuse_color);
        assert_eq!(rows[0].specular, row.specular_color);
        assert_eq!(rows[0].emissive, row.emissive_color);
        assert_eq!(rows[0].gloss_strength, row.gloss_strength);
        assert_eq!(rows[0].specular_strength, row.specular_strength);
        assert_eq!(rows[0].roughness, 0.5);
        assert_eq!(rows[0].metalness, 0.0);
        assert_eq!(rows[0].anisotropy, 0.0);
        assert_eq!(rows[0].tile_alpha, 1.0);
        assert_eq!(rows[0].tile_index, f32::from(row.tile_set));
        assert_eq!(rows[0].sheen_rate, 0.0);
        assert_eq!(rows[0].sheen_tint, 0.0);
        assert_eq!(rows[0].sheen_aperture, 0.0);
        assert_eq!(rows[0].sphere_index, 0.0);
        assert_eq!(rows[0].sphere_mask, 0.0);
        assert_eq!(
            rows[0].tile_matrix,
            [
                row.material_repeat_x,
                row.material_repeat_y,
                row.material_skew[0],
                row.material_skew[1],
            ]
        );
    }

    #[test]
    fn legacy_color_table_debug_uses_meddle_tile_matrix_order() {
        let row = test_legacy_color_table_row();
        let color_table =
            physis::mtrl::ColorTable::LegacyColorTable(physis::mtrl::LegacyColorTableData {
                rows: vec![row],
            });

        let debug = material_color_table_debug(Some(&color_table)).expect("debug");

        assert_eq!(debug.kind, "Legacy");
        assert_eq!(debug.rows[0].tile_index, Some(f32::from(row.tile_set)));
        assert_eq!(
            debug.rows[0].tile_matrix,
            Some([
                row.material_repeat_x,
                row.material_repeat_y,
                row.material_skew[0],
                row.material_skew[1],
            ])
        );
        assert_eq!(
            debug.rows[0].material_repeat,
            Some([row.material_repeat_x, row.material_repeat_y])
        );
        assert_eq!(debug.rows[0].material_skew, Some(row.material_skew));
    }

    #[test]
    fn baked_tile_matrix_texture_preserves_float_channels() {
        let mut row_a = test_dawntrail_color_table_row();
        row_a.material_repeat = [2.0, -0.5];
        row_a.material_skew = [0.25, 1.5];
        let mut row_b = row_a;
        row_b.material_repeat = [0.0, 0.0];
        row_b.material_skew = [0.0, 0.0];
        let color_table =
            physis::mtrl::ColorTable::DawntrailColorTable(physis::mtrl::DawntrailColorTableData {
                rows: vec![row_a, row_b],
            });
        let mut textures = vec![WeaponModelTexture {
            path: "index.tex".to_string(),
            kind: WeaponModelTextureKind::Index,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 1,
            rgba: vec![0, 0, 0, 255],
            rgba_f32: None,
        }];
        let rows = weapon_color_table_rows(&color_table).expect("color table rows");

        let baked = bake_weapon_color_table_textures(
            "material.mtrl",
            Some(&rows),
            Some(0),
            true,
            &mut textures,
        )
        .expect("bake");

        let tile_matrix = &textures[baked.tile_matrix];
        assert_eq!(
            tile_matrix.kind,
            WeaponModelTextureKind::TileMatrixProperties
        );
        assert_eq!(&tile_matrix.rgba[0..4], &[255, 0, 64, 255]);
        assert_eq!(tile_matrix.rgba_f32, Some(vec![[2.0, -0.5, 0.25, 1.5]]));
        assert_eq!(textures[baked.base_color].rgba[3], 255);
    }

    #[test]
    fn baked_sheen_and_sphere_textures_preserve_float_channels() {
        let rows = vec![
            ColorTableRowColors {
                sheen_aperture: 4.0,
                sphere_index: 2.0,
                ..Default::default()
            },
            ColorTableRowColors::default(),
        ];
        let mut textures = vec![WeaponModelTexture {
            path: "index.tex".to_string(),
            kind: WeaponModelTextureKind::Index,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 1,
            rgba: vec![0, 0, 0, 255],
            rgba_f32: None,
        }];

        let baked = bake_weapon_color_table_textures(
            "material.mtrl",
            Some(&rows),
            Some(0),
            true,
            &mut textures,
        )
        .expect("bake");

        assert_eq!(textures[baked.sheen_properties].rgba[2], 255);
        assert_eq!(
            textures[baked.sheen_properties].rgba_f32,
            Some(vec![[0.0, 0.0, 4.0, 1.0]])
        );
        assert_eq!(textures[baked.sphere_properties].rgba[0], 2);
        assert_eq!(
            textures[baked.sphere_properties].rgba_f32,
            Some(vec![[2.0 / 255.0, 0.0, 1.0, 1.0]])
        );
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_45059_preserves_hdr_sheen_ramp() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 45059,
            item_name: "冬雪之幻梦".to_string(),
            model_main: 4_295_034_963,
            model_sub: 773_094_181_015,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let sheen = model
            .textures
            .iter()
            .find(|texture| {
                texture.kind == WeaponModelTextureKind::SheenProperties
                    && texture
                        .rgba_f32
                        .as_deref()
                        .is_some_and(|pixels| pixels.iter().any(|pixel| pixel[2] == 4.0))
            })
            .expect("45059 HDR sheen properties");

        assert!(sheen.rgba.chunks_exact(4).any(|pixel| pixel[2] == 255));
        assert!(
            sheen
                .rgba_f32
                .as_deref()
                .expect("float sheen payload")
                .iter()
                .any(|pixel| pixel[2] == 4.0)
        );
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_equipment_style_fist_loads_default_human_glove() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 49_100,
            item_name: "幻境指虎·半影（复制品）".to_string(),
            model_main: 0x0000_0000_0001_2276,
            model_sub: 0,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");

        assert!(!model.meshes.is_empty());
        assert!(
            model
                .loaded_paths
                .iter()
                .any(|path| { path == "chara/equipment/e8822/model/c0101e8822_glv.mdl" })
        );
        assert!(model.loaded_paths.iter().any(|path| {
            path == "chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl"
        }));
        assert!(
            model.materials.iter().any(|material| {
                material.shader_package_name.as_deref() == Some("character.shpk")
            })
        );
        let skin_material = model
            .materials
            .iter()
            .find(|material| material.shader_package_name.as_deref() == Some("skin.shpk"))
            .expect("skin material");
        let prepared = crate::model::prepare_material_for_draw_role(
            Some(skin_material),
            ModelMeshDrawRole::Normal,
        );
        assert_eq!(
            prepared.shader_family,
            crate::model::MaterialShaderFamily::Skin
        );
        assert!(prepared.unsupported_inputs.runtime_skin_color);
        assert!(model.bounds.radius.is_finite() && model.bounds.radius > 0.0);
    }

    #[test]
    fn missing_material_reference_reuses_loaded_same_index() {
        let mut source =
            fallback_weapon_material(0, 0, "/mt_w3004b0001_a.mtrl".to_string(), [0.1, 0.2, 0.3]);
        source.path = Some(
            "chara/weapon/w3004/obj/body/b0001/material/v0001/mt_w3004b0001_a.mtrl".to_string(),
        );
        source.shader_package_name = Some("character.shpk".to_string());
        let missing =
            fallback_weapon_material(1, 0, "/mt_w3103b0001_a.mtrl".to_string(), [0.4, 0.5, 0.6]);

        let reused = reuse_loaded_material_for_missing_reference(missing, &[source]);

        assert_eq!(reused.slot, 1);
        assert_eq!(reused.material_index, 0);
        assert_eq!(reused.name, "/mt_w3103b0001_a.mtrl");
        assert_eq!(
            reused.shader_package_name.as_deref(),
            Some("character.shpk")
        );
        assert_eq!(
            reused.path.as_deref(),
            Some("chara/weapon/w3004/obj/body/b0001/material/v0001/mt_w3004b0001_a.mtrl")
        );
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_43624_reuses_primary_material_for_stale_secondary_reference() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 43_624,
            item_name: "帝国魔导双牙".to_string(),
            model_main: 0x0000_0002_0001_0BBC,
            model_sub: 0x0000_0002_0001_0BEE,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let secondary = model
            .materials
            .iter()
            .find(|material| material.name == "/mt_w3103b0001_a.mtrl")
            .expect("secondary material");

        assert_eq!(
            secondary.shader_package_name.as_deref(),
            Some("character.shpk")
        );
        assert_eq!(
            secondary.path.as_deref(),
            Some("chara/weapon/w3004/obj/body/b0001/material/v0002/mt_w3004b0001_a.mtrl")
        );
        assert!(!secondary.texture_indices.is_empty());
    }

    #[test]
    fn color_dye_table_debug_preserves_legacy_rows() {
        let color_dye_table = physis::mtrl::ColorDyeTable::LegacyColorDyeTable(
            physis::mtrl::LegacyColorDyeTableData {
                rows: vec![physis::mtrl::LegacyColorDyeTableRow {
                    template: 42,
                    diffuse: true,
                    specular: false,
                    emissive: true,
                    gloss: true,
                    specular_strength: false,
                }],
            },
        );

        let debug = material_color_dye_table_debug(Some(&color_dye_table)).expect("debug");
        assert_eq!(
            model_color_dye_table(Some(&color_dye_table)),
            Some(ModelColorDyeTable::Legacy(vec![
                ModelLegacyColorDyeTableRow {
                    template: 42,
                    diffuse: true,
                    specular: false,
                    emissive: true,
                    gloss: true,
                    specular_strength: false,
                }
            ]))
        );

        assert_eq!(debug.kind, "Legacy");
        assert_eq!(debug.row_count, 1);
        assert_eq!(debug.rows[0].index, 0);
        assert_eq!(debug.rows[0].template, 42);
        assert_eq!(debug.rows[0].channel, None);
        assert!(debug.rows[0].diffuse);
        assert!(!debug.rows[0].specular);
        assert!(debug.rows[0].emissive);
        assert_eq!(debug.rows[0].gloss, Some(true));
        assert_eq!(debug.rows[0].specular_strength, Some(false));
        assert_eq!(debug.rows[0].metalness, None);
        assert_eq!(debug.rows[0].sphere_map_mask, None);
    }

    #[test]
    fn color_dye_table_debug_preserves_dawntrail_rows() {
        let color_dye_table = physis::mtrl::ColorDyeTable::DawntrailColorDyeTable(
            physis::mtrl::DawntrailColorDyeTableData {
                rows: vec![physis::mtrl::DawntrailColorDyeTableRow {
                    template: 77,
                    channel: 2,
                    diffuse: true,
                    specular: true,
                    emissive: false,
                    scalar3: true,
                    metalness: false,
                    roughness: true,
                    sheen_rate: true,
                    sheen_tint_rate: false,
                    sheen_aperture: true,
                    anisotropy: false,
                    sphere_map_index: true,
                    sphere_map_mask: true,
                }],
            },
        );

        let debug = material_color_dye_table_debug(Some(&color_dye_table)).expect("debug");
        assert_eq!(
            model_color_dye_table(Some(&color_dye_table)),
            Some(ModelColorDyeTable::Dawntrail(vec![
                ModelDawntrailColorDyeTableRow {
                    template: 77,
                    channel: 2,
                    diffuse: true,
                    specular: true,
                    emissive: false,
                    scalar3: true,
                    metalness: false,
                    roughness: true,
                    sheen_rate: true,
                    sheen_tint_rate: false,
                    sheen_aperture: true,
                    anisotropy: false,
                    sphere_map_index: true,
                    sphere_map_mask: true,
                }
            ]))
        );

        assert_eq!(debug.kind, "Dawntrail");
        assert_eq!(debug.row_count, 1);
        assert_eq!(debug.rows[0].index, 0);
        assert_eq!(debug.rows[0].template, 77);
        assert_eq!(debug.rows[0].channel, Some(2));
        assert!(debug.rows[0].diffuse);
        assert!(debug.rows[0].specular);
        assert!(!debug.rows[0].emissive);
        assert_eq!(debug.rows[0].gloss, None);
        assert_eq!(debug.rows[0].specular_strength, None);
        assert_eq!(debug.rows[0].scalar3, Some(true));
        assert_eq!(debug.rows[0].metalness, Some(false));
        assert_eq!(debug.rows[0].roughness, Some(true));
        assert_eq!(debug.rows[0].sheen_rate, Some(true));
        assert_eq!(debug.rows[0].sheen_tint_rate, Some(false));
        assert_eq!(debug.rows[0].sheen_aperture, Some(true));
        assert_eq!(debug.rows[0].anisotropy, Some(false));
        assert_eq!(debug.rows[0].sphere_map_index, Some(true));
        assert_eq!(debug.rows[0].sphere_map_mask, Some(true));
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
    fn composed_material_transparency_uses_resolved_material_constant() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[(G_TRANSPARENCY, &[0.35])]);

        assert_eq!(
            composed_material_transparency(&semantics, "character.shpk"),
            0.0
        );
        assert_eq!(
            composed_material_transparency(&semantics, "water.shpk"),
            1.0
        );

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_transparency(&semantics, "water.shpk"),
            0.35
        );

        let material = test_mtrl_with_constant(G_TRANSPARENCY, &[0.72], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_transparency(&semantics, "water.shpk"),
            0.72
        );

        let material = test_mtrl_with_constant(G_TRANSPARENCY, &[8.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_transparency(&semantics, "water.shpk"),
            1.0
        );
    }

    #[test]
    fn composed_material_water_colors_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_water_deep_color(&semantics),
            [0.3529, 0.372_549, 0.3921, 1.0]
        );
        assert_eq!(
            composed_material_water_refraction_color(&semantics),
            [0.4117, 0.4313, 0.4509, 1.0]
        );
        assert_eq!(
            composed_material_water_whitecap_color(&semantics),
            [0.4509, 0.4705, 0.4901, 0.3]
        );

        let shader_package = test_shpk_with_material_defaults(&[
            (G_WATER_DEEP_COLOR, &[0.1, 0.2, 0.3]),
            (G_WATER_REFRACTION_COLOR, &[0.4, 0.5, 0.6]),
            (G_WATER_WHITECAP_COLOR, &[0.7, 0.8, 0.9, 0.25]),
        ]);
        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_water_deep_color(&semantics),
            [0.1, 0.2, 0.3, 1.0]
        );
        assert_eq!(
            composed_material_water_refraction_color(&semantics),
            [0.4, 0.5, 0.6, 1.0]
        );
        assert_eq!(
            composed_material_water_whitecap_color(&semantics),
            [0.7, 0.8, 0.9, 0.25]
        );

        let material = test_mtrl_with_constant(G_WATER_DEEP_COLOR, &[0.9, f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_water_deep_color(&semantics),
            [0.9, 0.372_549, 0.3921, 1.0]
        );
    }

    #[test]
    fn composed_character_transparency_keys_preserve_depth_and_lighting_policy() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_draw_depth_mode(&semantics),
            MaterialDrawDepthMode::None
        );
        assert_eq!(
            composed_material_lighting_mode(&semantics),
            MaterialLightingMode::Default
        );

        semantics.apply_shader_package_key_default(DRAW_DEPTH_MODE, DRAW_DEPTH_MODE_DITHER);
        semantics.apply_shader_package_key_default(ENABLE_LIGHTING, ENABLE_LIGHTING_ON);
        assert_eq!(
            composed_material_draw_depth_mode(&semantics),
            MaterialDrawDepthMode::Dither
        );
        assert_eq!(
            composed_material_lighting_mode(&semantics),
            MaterialLightingMode::Enabled
        );

        semantics.apply_material_key(ENABLE_LIGHTING, ENABLE_LIGHTING_OFF);
        assert_eq!(
            composed_material_lighting_mode(&semantics),
            MaterialLightingMode::Disabled
        );

        semantics.apply_material_key(DRAW_DEPTH_MODE, 0xDEAD_BEEF);
        semantics.apply_material_key(ENABLE_LIGHTING, 0xCAFE_BABE);
        assert_eq!(
            composed_material_draw_depth_mode(&semantics),
            MaterialDrawDepthMode::Unknown
        );
        assert_eq!(
            composed_material_lighting_mode(&semantics),
            MaterialLightingMode::Unknown
        );
    }

    #[test]
    fn composed_character_flow_mode_preserves_default_override_and_unknown() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_flow_mode(&semantics),
            MaterialFlowMode::Standard
        );

        semantics.apply_shader_package_key_default(CATEGORY_FLOW_MAP_TYPE, FLOW_MAP_STANDARD);
        assert_eq!(
            composed_material_flow_mode(&semantics),
            MaterialFlowMode::Standard
        );

        semantics.apply_material_key(CATEGORY_FLOW_MAP_TYPE, FLOW_MAP_FLOW);
        assert_eq!(
            composed_material_flow_mode(&semantics),
            MaterialFlowMode::Flow
        );

        semantics.apply_material_key(CATEGORY_FLOW_MAP_TYPE, 0xDEAD_BEEF);
        assert_eq!(
            composed_material_flow_mode(&semantics),
            MaterialFlowMode::Unknown
        );
    }

    #[test]
    fn composed_get_values_mode_preserves_known_and_unknown_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_value_mode(&semantics),
            MaterialValueMode::Single
        );

        for (value, expected) in [
            (GET_VALUES_SINGLE, MaterialValueMode::Single),
            (GET_VALUES_MULTI, MaterialValueMode::Multi),
            (GET_ALPHA_MULTI_VALUES, MaterialValueMode::AlphaMulti),
            (GET_ALPHA_MULTI_VALUES2, MaterialValueMode::AlphaMulti2),
            (GET_ALPHA_MULTI_VALUES3, MaterialValueMode::AlphaMulti3),
            (GET_VALUES_MULTI_MATERIAL, MaterialValueMode::MultiMaterial),
            (GET_VALUES_COMPATIBILITY, MaterialValueMode::Compatibility),
            (0xDEAD_BEEF, MaterialValueMode::Unknown),
        ] {
            semantics.apply_material_key(GET_VALUES, value);
            assert_eq!(composed_material_value_mode(&semantics), expected);
        }
    }

    #[test]
    fn composed_sub_color_mode_preserves_known_and_unknown_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_sub_color_mode(&semantics),
            MaterialSubColorMode::None
        );

        semantics.apply_shader_package_key_default(GET_SUB_COLOR, GET_SUB_COLOR_FACE);
        assert_eq!(
            composed_material_sub_color_mode(&semantics),
            MaterialSubColorMode::Face
        );

        semantics.apply_material_key(GET_SUB_COLOR, GET_SUB_COLOR_HAIR);
        assert_eq!(
            composed_material_sub_color_mode(&semantics),
            MaterialSubColorMode::Hair
        );

        semantics.apply_material_key(GET_SUB_COLOR, 0xDEAD_BEEF);
        assert_eq!(
            composed_material_sub_color_mode(&semantics),
            MaterialSubColorMode::Unknown
        );
    }

    #[test]
    fn composed_material_alpha_params_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_ALPHA_APERTURE, &[2.5]),
            (G_ALPHA_OFFSET, &[-0.25]),
            (G_SHADOW_ALPHA_THRESHOLD, &[0.35]),
        ]);

        assert_eq!(composed_material_alpha_aperture(&semantics), 2.0);
        assert_eq!(composed_material_alpha_offset(&semantics), 0.0);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 0.5);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_alpha_aperture(&semantics), 2.5);
        assert_eq!(composed_material_alpha_offset(&semantics), -0.25);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 0.35);

        let material = test_mtrl_with_constant(G_ALPHA_APERTURE, &[4.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_aperture(&semantics), 4.0);

        let material = test_mtrl_with_constant(G_ALPHA_OFFSET, &[0.2], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_offset(&semantics), 0.2);

        let material = test_mtrl_with_constant(G_SHADOW_ALPHA_THRESHOLD, &[1.5], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 1.0);

        let material = test_mtrl_with_constant(G_ALPHA_APERTURE, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_aperture(&semantics), 2.0);

        let material = test_mtrl_with_constant(G_ALPHA_OFFSET, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_offset(&semantics), 0.0);

        let material = test_mtrl_with_constant(G_SHADOW_ALPHA_THRESHOLD, &[f32::NEG_INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 0.5);
    }

    #[test]
    fn composed_material_glass_params_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_GLASS_IOR, &[1.33]),
            (G_GLASS_THICKNESS_MAX, &[0.08]),
        ]);

        assert_eq!(composed_material_glass_ior(&semantics), 1.0);
        assert_eq!(composed_material_glass_thickness_max(&semantics), 0.01);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_glass_ior(&semantics), 1.33);
        assert_eq!(composed_material_glass_thickness_max(&semantics), 0.08);

        let material = test_mtrl_with_constant(G_GLASS_IOR, &[1.52], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_glass_ior(&semantics), 1.52);

        let material = test_mtrl_with_constant(G_GLASS_THICKNESS_MAX, &[0.125], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_glass_thickness_max(&semantics), 0.125);

        let material = test_mtrl_with_constant(G_GLASS_IOR, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_glass_ior(&semantics), 1.0);

        let material = test_mtrl_with_constant(G_GLASS_THICKNESS_MAX, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_glass_thickness_max(&semantics), 0.01);
    }

    #[test]
    fn composed_material_normal_scales_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_NORMAL_SCALE, &[0.65]),
            (G_MULTI_NORMAL_SCALE, &[0.75]),
            (G_DETAIL_NORMAL_SCALE, &[0.85]),
            (G_MULTI_DETAIL_NORMAL_SCALE, &[0.95]),
        ]);

        assert_eq!(composed_material_normal_scale(&semantics), 1.0);
        assert_eq!(composed_material_multi_normal_scale(&semantics), 1.0);
        assert_eq!(composed_material_detail_normal_scale(&semantics), 1.0);
        assert_eq!(composed_material_multi_detail_normal_scale(&semantics), 1.0);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_normal_scale(&semantics), 0.65);
        assert_eq!(composed_material_multi_normal_scale(&semantics), 0.75);
        assert_eq!(composed_material_detail_normal_scale(&semantics), 0.85);
        assert_eq!(
            composed_material_multi_detail_normal_scale(&semantics),
            0.95
        );

        let material = test_mtrl_with_constant(G_NORMAL_SCALE, &[1.75], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_normal_scale(&semantics), 1.75);

        let material = test_mtrl_with_constant(G_MULTI_NORMAL_SCALE, &[2.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_multi_normal_scale(&semantics), 2.25);

        let material = test_mtrl_with_constant(G_DETAIL_NORMAL_SCALE, &[3.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_detail_normal_scale(&semantics), 3.25);

        let material = test_mtrl_with_constant(G_MULTI_DETAIL_NORMAL_SCALE, &[8.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_multi_detail_normal_scale(&semantics), 4.0);
    }

    #[test]
    fn composed_material_tile_select_uses_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_TILE_INDEX, &[3.0]),
            (G_TILE_ALPHA, &[0.75]),
            (G_TILE_SCALE, &[16.0, 8.0]),
        ]);

        assert_eq!(composed_material_tile_index(&semantics), 0.0);
        assert_eq!(composed_material_tile_alpha(&semantics), 1.0);
        assert_eq!(composed_material_tile_scale(&semantics), [16.0, 16.0]);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_tile_index(&semantics), 3.0);
        assert_eq!(composed_material_tile_alpha(&semantics), 0.75);
        assert_eq!(composed_material_tile_scale(&semantics), [16.0, 8.0]);

        let material = test_mtrl_with_constant(G_TILE_INDEX, &[9.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_tile_index(&semantics), 9.0);

        let material = test_mtrl_with_constant(G_TILE_ALPHA, &[0.35], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_tile_alpha(&semantics), 0.35);

        let material = test_mtrl_with_constant(G_TILE_SCALE, &[4.0, 2.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_tile_scale(&semantics), [4.0, 2.0]);
    }

    #[test]
    fn composed_material_toon_sheen_sphere_params_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_TOON_INDEX, &[3.0]),
            (G_TOON_LIGHT_SCALE, &[1.5]),
            (G_TOON_LIGHT_SPEC_APERTURE, &[42.0]),
            (G_TOON_REFLECTION_SCALE, &[3.25]),
            (G_TOON_SPEC_INDEX, &[2.0]),
            (G_SHEEN_RATE, &[0.25]),
            (G_SHEEN_TINT_RATE, &[0.35]),
            (G_SHEEN_APERTURE, &[0.8]),
            (G_SPHERE_MAP_INDEX, &[2.0]),
        ]);

        assert_eq!(composed_material_toon_index(&semantics), 0.0);
        assert_eq!(composed_material_toon_light_scale(&semantics), 2.0);
        assert_eq!(composed_material_toon_light_spec_aperture(&semantics), 50.0);
        assert_eq!(composed_material_toon_reflection_scale(&semantics), 2.5);
        assert_eq!(composed_material_toon_spec_index(&semantics), 4.0e-45);
        assert_eq!(composed_material_sheen_rate(&semantics), 0.0);
        assert_eq!(composed_material_sheen_tint_rate(&semantics), 0.0);
        assert_eq!(composed_material_sheen_aperture(&semantics), 1.0);
        assert_eq!(composed_material_sphere_map_index(&semantics), 0.0);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_toon_index(&semantics), 3.0);
        assert_eq!(composed_material_toon_light_scale(&semantics), 1.5);
        assert_eq!(composed_material_toon_light_spec_aperture(&semantics), 42.0);
        assert_eq!(composed_material_toon_reflection_scale(&semantics), 3.25);
        assert_eq!(composed_material_toon_spec_index(&semantics), 2.0);
        assert_eq!(composed_material_sheen_rate(&semantics), 0.25);
        assert_eq!(composed_material_sheen_tint_rate(&semantics), 0.35);
        assert_eq!(composed_material_sheen_aperture(&semantics), 0.8);
        assert_eq!(composed_material_sphere_map_index(&semantics), 2.0);

        let material = test_mtrl_with_constant(G_TOON_INDEX, &[5.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_index(&semantics), 5.0);

        let material = test_mtrl_with_constant(G_TOON_LIGHT_SCALE, &[2.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_light_scale(&semantics), 2.25);

        let material = test_mtrl_with_constant(G_TOON_LIGHT_SPEC_APERTURE, &[64.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_light_spec_aperture(&semantics), 64.0);

        let material = test_mtrl_with_constant(G_TOON_REFLECTION_SCALE, &[4.5], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_reflection_scale(&semantics), 4.5);

        let material = test_mtrl_with_constant(G_TOON_SPEC_INDEX, &[6.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_spec_index(&semantics), 6.0);

        let material = test_mtrl_with_constant(G_SHEEN_RATE, &[0.45], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_sheen_rate(&semantics), 0.45);

        let material = test_mtrl_with_constant(G_SHEEN_TINT_RATE, &[0.55], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_sheen_tint_rate(&semantics), 0.55);

        let material = test_mtrl_with_constant(G_SHEEN_APERTURE, &[1.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_sheen_aperture(&semantics), 1.25);

        let material = test_mtrl_with_constant(G_SPHERE_MAP_INDEX, &[4.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_sphere_map_index(&semantics), 4.0);

        let material = test_mtrl_with_constant(G_TOON_INDEX, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_index(&semantics), 0.0);

        let material = test_mtrl_with_constant(G_TOON_LIGHT_SCALE, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_light_scale(&semantics), 2.0);

        let material = test_mtrl_with_constant(G_TOON_LIGHT_SPEC_APERTURE, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_light_spec_aperture(&semantics), 50.0);

        let material = test_mtrl_with_constant(G_TOON_REFLECTION_SCALE, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_reflection_scale(&semantics), 2.5);

        let material = test_mtrl_with_constant(G_TOON_SPEC_INDEX, &[f32::NEG_INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_toon_spec_index(&semantics), 4.0e-45);

        let material = test_mtrl_with_constant(G_SHEEN_APERTURE, &[f32::NEG_INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_sheen_aperture(&semantics), 1.0);
    }

    #[test]
    fn composed_material_detail_uv_uses_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_DETAIL_ID, &[2.0]),
            (G_MULTI_DETAIL_ID, &[4.0]),
            (G_DETAIL_COLOR_UV_SCALE, &[8.0, 6.0, 4.0, 2.0]),
            (G_DETAIL_NORMAL_UV_SCALE, &[7.0, 5.0, 3.0, 1.0]),
        ]);

        assert_eq!(composed_material_detail_id(&semantics), 0.0);
        assert_eq!(composed_material_multi_detail_id(&semantics), 0.0);
        assert_eq!(
            composed_material_detail_color_uv_scale(&semantics),
            [4.0; 4]
        );
        assert_eq!(
            composed_material_detail_normal_uv_scale(&semantics),
            [4.0; 4]
        );

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_detail_id(&semantics), 2.0);
        assert_eq!(composed_material_multi_detail_id(&semantics), 4.0);
        assert_eq!(
            composed_material_detail_color_uv_scale(&semantics),
            [8.0, 6.0, 4.0, 2.0]
        );
        assert_eq!(
            composed_material_detail_normal_uv_scale(&semantics),
            [7.0, 5.0, 3.0, 1.0]
        );

        let material = test_mtrl_with_constant(G_DETAIL_ID, &[9.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_detail_id(&semantics), 9.0);

        let material = test_mtrl_with_constant(G_MULTI_DETAIL_ID, &[11.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_multi_detail_id(&semantics), 11.0);

        let material = test_mtrl_with_constant(G_DETAIL_COLOR_UV_SCALE, &[1.0, 2.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_detail_color_uv_scale(&semantics),
            [1.0, 2.0, 4.0, 4.0]
        );

        let material = test_mtrl_with_constant(G_DETAIL_NORMAL_UV_SCALE, &[3.0, 4.0, 5.0, 6.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_detail_normal_uv_scale(&semantics),
            [3.0, 4.0, 5.0, 6.0]
        );
    }

    #[test]
    fn composed_material_detail_colors_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_DETAIL_COLOR, &[0.2, 0.4, 0.6, 0.8]),
            (G_MULTI_DETAIL_COLOR, &[0.1, 0.3, 0.5, 0.7]),
        ]);

        assert_eq!(
            composed_material_detail_color(&semantics),
            [0.5, 0.5, 0.5, 1.0]
        );
        assert_eq!(
            composed_material_multi_detail_color(&semantics),
            [0.5, 0.5, 0.5, 1.0]
        );

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_detail_color(&semantics),
            [0.2, 0.4, 0.6, 0.8]
        );
        assert_eq!(
            composed_material_multi_detail_color(&semantics),
            [0.1, 0.3, 0.5, 0.7]
        );

        let material = test_mtrl_with_constant(G_DETAIL_COLOR, &[0.9, 0.8, 0.7, 0.6], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_detail_color(&semantics),
            [0.9, 0.8, 0.7, 0.6]
        );

        let material = test_mtrl_with_constant(G_MULTI_DETAIL_COLOR, &[0.25, 0.5], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_multi_detail_color(&semantics),
            [0.25, 0.5, 0.5, 1.0]
        );

        let material =
            test_mtrl_with_constant(G_DETAIL_COLOR, &[0.3, f32::NAN, f32::INFINITY, 0.4], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_detail_color(&semantics),
            [0.3, 0.5, 0.5, 0.4]
        );
    }

    #[test]
    fn composed_material_shader_colors_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_DIFFUSE_COLOR, &[0.8, 0.7, 0.6, 0.5]),
            (G_MULTI_DIFFUSE_COLOR, &[0.6, 0.7, 0.8, 0.9]),
            (G_EMISSIVE_COLOR, &[0.1, 0.2, 0.3, 1.0]),
            (G_MULTI_EMISSIVE_COLOR, &[0.4, 0.5, 0.6, 1.0]),
        ]);

        assert_eq!(composed_material_shader_diffuse_color(&semantics), [1.0; 4]);
        assert_eq!(
            composed_material_shader_multi_diffuse_color(&semantics),
            [1.0; 4]
        );
        assert_eq!(
            composed_material_shader_emissive_color(&semantics),
            [0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            composed_material_shader_multi_emissive_color(&semantics),
            [0.0, 0.0, 0.0, 1.0]
        );

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_shader_diffuse_color(&semantics),
            [0.8, 0.7, 0.6, 0.5]
        );
        assert_eq!(
            composed_material_shader_multi_diffuse_color(&semantics),
            [0.6, 0.7, 0.8, 0.9]
        );
        assert_eq!(
            composed_material_shader_emissive_color(&semantics),
            [0.1, 0.2, 0.3, 1.0]
        );
        assert_eq!(
            composed_material_shader_multi_emissive_color(&semantics),
            [0.4, 0.5, 0.6, 1.0]
        );

        let material = test_mtrl_with_constant(G_DIFFUSE_COLOR, &[0.25, 0.5], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_shader_diffuse_color(&semantics),
            [0.25, 0.5, 1.0, 1.0]
        );

        let material =
            test_mtrl_with_constant(G_EMISSIVE_COLOR, &[0.75, f32::NAN, f32::INFINITY, 0.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_shader_emissive_color(&semantics),
            [0.75, 0.0, 0.0, 0.25]
        );
    }

    #[test]
    fn composed_material_outline_specular_occlusion_params_use_resolved_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_OUTLINE_COLOR, &[0.1, 0.2, 0.3, 0.4]),
            (G_OUTLINE_WIDTH, &[0.05]),
            (G_SPECULAR_COLOR_MASK, &[0.7, 0.8, 0.9, 1.0]),
            (G_SSAO_MASK, &[0.6]),
            (G_TEXTURE_MIP_BIAS, &[-0.75]),
            (G_SHADOW_POS_OFFSET, &[0.125]),
        ]);

        assert_eq!(
            composed_material_outline_color(&semantics),
            [0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(composed_material_outline_width(&semantics), 0.0);
        assert_eq!(composed_material_specular_color_mask(&semantics), [1.0; 4]);
        assert_eq!(composed_material_ssao_mask(&semantics), 1.0);
        assert_eq!(composed_material_texture_mip_bias(&semantics), 0.0);
        assert_eq!(composed_material_shadow_pos_offset(&semantics), 0.0);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_outline_color(&semantics),
            [0.1, 0.2, 0.3, 0.4]
        );
        assert_eq!(composed_material_outline_width(&semantics), 0.05);
        assert_eq!(
            composed_material_specular_color_mask(&semantics),
            [0.7, 0.8, 0.9, 1.0]
        );
        assert_eq!(composed_material_ssao_mask(&semantics), 0.6);
        assert_eq!(composed_material_texture_mip_bias(&semantics), -0.75);
        assert_eq!(composed_material_shadow_pos_offset(&semantics), 0.125);

        let material = test_mtrl_with_constant(G_OUTLINE_COLOR, &[0.9, 0.8], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_outline_color(&semantics),
            [0.9, 0.8, 0.0, 1.0]
        );

        let material = test_mtrl_with_constant(G_OUTLINE_WIDTH, &[0.2], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_outline_width(&semantics), 0.2);

        let material = test_mtrl_with_constant(G_SPECULAR_COLOR_MASK, &[0.25, 0.5], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_specular_color_mask(&semantics),
            [0.25, 0.5, 1.0, 1.0]
        );

        let material = test_mtrl_with_constant(G_SSAO_MASK, &[0.35], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_ssao_mask(&semantics), 0.35);

        let material = test_mtrl_with_constant(G_TEXTURE_MIP_BIAS, &[1.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_texture_mip_bias(&semantics), 1.25);

        let material = test_mtrl_with_constant(G_SHADOW_POS_OFFSET, &[-0.2], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_shadow_pos_offset(&semantics), -0.2);

        let material =
            test_mtrl_with_constant(G_OUTLINE_COLOR, &[0.3, f32::NAN, f32::INFINITY, 0.4], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_outline_color(&semantics),
            [0.3, 0.0, 0.0, 0.4]
        );

        let material = test_mtrl_with_constant(G_SSAO_MASK, &[f32::NEG_INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_ssao_mask(&semantics), 1.0);
    }

    #[test]
    fn composed_material_uv_scroll_uses_meddletools_multiplier_mapping() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package =
            test_shpk_with_material_defaults(&[(G_UV_SCROLL_TIME, &[10.0, 20.0, 30.0, 40.0])]);

        assert_eq!(composed_material_uv_scroll(&semantics), [0.0; 4]);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_uv_scroll(&semantics),
            [-10.0, 20.0, -30.0, 40.0]
        );

        let material = test_mtrl_with_constant(G_UV_SCROLL_TIME, &[1.0, 2.0, 3.0, 4.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_uv_scroll(&semantics),
            [-1.0, 2.0, -3.0, 4.0]
        );

        let material = test_mtrl_with_constant(G_UV_SCROLL_TIME, &[5.0, 6.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_uv_scroll(&semantics),
            [-5.0, 6.0, 0.0, 0.0]
        );
    }

    #[test]
    fn composed_material_lightshaft_params_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_LIGHTSHAFT_COLOR, &[0.2, 0.4, 0.6, 0.8]),
            (G_LIGHTSHAFT_TEX_ANIM, &[0.1, 0.2, 0.3, 0.4]),
            (G_LIGHTSHAFT_TEX_U, &[1.5, 0.5, 0.25]),
            (G_LIGHTSHAFT_TEX_V, &[0.25, 1.75, 0.5]),
            (G_LIGHTSHAFT_RAY, &[2.0, 3.0, 4.0, 5.0]),
        ]);

        assert_eq!(composed_material_lightshaft_color(&semantics), [1.0; 4]);
        assert_eq!(composed_material_lightshaft_tex_anim(&semantics), [0.0; 4]);
        assert_eq!(
            composed_material_lightshaft_tex_u(&semantics),
            [1.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            composed_material_lightshaft_tex_v(&semantics),
            [0.0, 1.0, 0.0, 0.0]
        );
        assert_eq!(composed_material_lightshaft_ray(&semantics), [0.0; 4]);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_lightshaft_color(&semantics),
            [0.2, 0.4, 0.6, 0.8]
        );
        assert_eq!(
            composed_material_lightshaft_tex_anim(&semantics),
            [0.1, 0.2, 0.3, 0.4]
        );
        assert_eq!(
            composed_material_lightshaft_tex_u(&semantics),
            [1.5, 0.5, 0.25, 0.0]
        );
        assert_eq!(
            composed_material_lightshaft_tex_v(&semantics),
            [0.25, 1.75, 0.5, 0.0]
        );
        assert_eq!(
            composed_material_lightshaft_ray(&semantics),
            [2.0, 3.0, 4.0, 5.0]
        );

        let material = test_mtrl_with_constant(G_LIGHTSHAFT_COLOR, &[1.0, 0.5, 0.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_lightshaft_color(&semantics),
            [1.0, 0.5, 0.25, 1.0]
        );

        let material = test_mtrl_with_constant(G_LIGHTSHAFT_TEX_U, &[2.0, 3.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_lightshaft_tex_u(&semantics),
            [2.0, 3.0, 0.0, 0.0]
        );
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
    fn explicit_sampler_kind_overrides_index_like_filename() {
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w0001/obj/body/b0001/texture/w0001_index.tex",
                Some(WeaponModelTextureKind::Normal),
            ),
            WeaponModelTextureKind::Normal
        );
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w0001/obj/body/b0001/texture/w0001_id.tex",
                Some(WeaponModelTextureKind::MaterialMap),
            ),
            WeaponModelTextureKind::MaterialMap
        );
    }

    #[test]
    fn explicit_sampler_kind_reclassifies_cached_filename_guess() {
        assert_eq!(
            merge_texture_kind(
                WeaponModelTextureKind::Index,
                WeaponModelTextureKind::Normal,
                true,
            ),
            WeaponModelTextureKind::Normal
        );
        assert_eq!(
            merge_texture_kind(
                WeaponModelTextureKind::Normal,
                WeaponModelTextureKind::Index,
                false,
            ),
            WeaponModelTextureKind::Normal
        );
    }

    #[test]
    fn sampler_classification_preserves_material_and_multi_roles() {
        assert_eq!(
            classify_sampler_name("g_SamplerMaterial"),
            Some(WeaponModelTextureKind::MaterialMap)
        );
        assert_eq!(
            classify_sampler_name("g_MaterialSampler"),
            Some(WeaponModelTextureKind::MaterialMap)
        );
        assert_eq!(
            classify_sampler_usage(physis::shpk::ShaderPackage::crc("g_SamplerMulti")),
            Some(WeaponModelTextureKind::MultiMap)
        );
        assert_eq!(
            classify_sampler_name("g_MultiSampler"),
            Some(WeaponModelTextureKind::MultiMap)
        );
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w0001/obj/body/b0001/texture/unknown.tex",
                Some(WeaponModelTextureKind::MaterialMap),
            ),
            WeaponModelTextureKind::MaterialMap
        );
        assert_eq!(
            classify_weapon_texture(
                "chara/weapon/w0001/obj/body/b0001/texture/unknown.tex",
                Some(WeaponModelTextureKind::MultiMap),
            ),
            WeaponModelTextureKind::MultiMap
        );
    }

    #[test]
    fn sampler_classification_covers_meddletools_texture_roles() {
        assert_eq!(
            classify_sampler_name("g_SamplerColorMap1"),
            Some(WeaponModelTextureKind::SecondaryBaseColor)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerNormalMap1"),
            Some(WeaponModelTextureKind::SecondaryNormal)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerSpecularMap1"),
            Some(WeaponModelTextureKind::SecondarySpecular)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerSkinDiffuse"),
            Some(WeaponModelTextureKind::BaseColor)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerSkinNormal"),
            Some(WeaponModelTextureKind::Normal)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerSkinMask"),
            Some(WeaponModelTextureKind::Mask)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerEnvMap"),
            Some(WeaponModelTextureKind::Environment)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerWaveMap"),
            Some(WeaponModelTextureKind::WaterWave)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerWaveMap1"),
            Some(WeaponModelTextureKind::WaterWaveSecondary)
        );
        assert_eq!(
            classify_sampler_usage(physis::shpk::ShaderPackage::crc("g_SamplerWhitecapMap")),
            Some(WeaponModelTextureKind::WaterWhitecap)
        );
    }

    #[test]
    fn fallback_base_texture_ignores_specialized_maps() {
        let textures = vec![
            test_texture("emissive.tex", WeaponModelTextureKind::Emissive),
            test_texture("normal.tex", WeaponModelTextureKind::Normal),
            test_texture("mask.tex", WeaponModelTextureKind::Mask),
            test_texture("material.tex", WeaponModelTextureKind::MaterialMap),
            test_texture("multi.tex", WeaponModelTextureKind::MultiMap),
            test_texture("specular.tex", WeaponModelTextureKind::Specular),
            test_texture("color1.tex", WeaponModelTextureKind::SecondaryBaseColor),
            test_texture("normal1.tex", WeaponModelTextureKind::SecondaryNormal),
            test_texture("specular1.tex", WeaponModelTextureKind::SecondarySpecular),
            test_texture("id.tex", WeaponModelTextureKind::Index),
            test_texture("wave.tex", WeaponModelTextureKind::WaterWave),
            test_texture("wave1.tex", WeaponModelTextureKind::WaterWaveSecondary),
            test_texture("whitecap.tex", WeaponModelTextureKind::WaterWhitecap),
            test_texture("environment.tex", WeaponModelTextureKind::Environment),
        ];
        assert_eq!(
            choose_fallback_base_texture(&(0..textures.len()).collect::<Vec<_>>(), &textures),
            None
        );
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
    fn base_texture_alpha_drives_generic_alpha_classification() {
        for kind in [
            WeaponModelTextureKind::Normal,
            WeaponModelTextureKind::Mask,
            WeaponModelTextureKind::MaterialMap,
            WeaponModelTextureKind::MultiMap,
            WeaponModelTextureKind::Specular,
            WeaponModelTextureKind::Emissive,
            WeaponModelTextureKind::MaterialProperties,
            WeaponModelTextureKind::TileProperties,
            WeaponModelTextureKind::SheenProperties,
            WeaponModelTextureKind::SphereProperties,
            WeaponModelTextureKind::TileMatrixProperties,
            WeaponModelTextureKind::Index,
            WeaponModelTextureKind::WaterWave,
            WeaponModelTextureKind::WaterWaveSecondary,
            WeaponModelTextureKind::WaterWhitecap,
            WeaponModelTextureKind::Environment,
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

        let alpha_secondary = test_texture_with_alpha(
            "base1-alpha.tex",
            WeaponModelTextureKind::SecondaryBaseColor,
            128,
        );
        assert!(texture_alpha_affects_material_transparency(
            &alpha_secondary
        ));
    }

    #[test]
    fn refresh_texture_set_alpha_uses_final_base_texture() {
        let textures = vec![
            test_texture_with_alpha("opaque-base.tex", WeaponModelTextureKind::BaseColor, 255),
            test_texture_with_alpha("baked-base.tex", WeaponModelTextureKind::BaseColor, 128),
        ];
        let mut set = WeaponTextureSet {
            base_color: Some(0),
            has_alpha: true,
            ..Default::default()
        };

        refresh_texture_set_alpha(&mut set, &textures);
        assert!(!set.has_alpha);

        set.base_color = Some(1);
        refresh_texture_set_alpha(&mut set, &textures);
        assert!(set.has_alpha);

        let secondary_index = textures.len();
        let mut textures = textures;
        textures.push(test_texture_with_alpha(
            "secondary-base.tex",
            WeaponModelTextureKind::SecondaryBaseColor,
            64,
        ));
        set.base_color = Some(0);
        set.secondary_base_color = Some(secondary_index);
        refresh_texture_set_alpha(&mut set, &textures);
        assert!(set.has_alpha);
    }

    #[test]
    fn final_base_alpha_uses_blend_without_alpha_test() {
        let texture_set = WeaponTextureSet {
            has_alpha: true,
            ..Default::default()
        };

        assert_eq!(
            weapon_material_alpha_mode("character.shpk", 0, &texture_set, false),
            WeaponMaterialAlphaMode::Blend
        );
    }

    #[test]
    fn character_transparency_and_glass_packages_force_transparent_passes() {
        let texture_set = WeaponTextureSet::default();
        assert_eq!(
            weapon_material_alpha_mode("charactertransparency.shpk", 0, &texture_set, false),
            WeaponMaterialAlphaMode::Blend
        );
        assert_eq!(
            weapon_material_alpha_mode("characterglass.shpk", 0, &texture_set, false),
            WeaponMaterialAlphaMode::Glass
        );
        assert_eq!(
            weapon_material_opacity(WeaponMaterialRenderMode::Glass),
            1.0
        );
    }

    #[test]
    fn alpha_test_only_masks_supported_shader_packages() {
        let texture_set = WeaponTextureSet {
            has_alpha: true,
            ..Default::default()
        };

        assert_eq!(
            weapon_material_alpha_mode("bg.shpk", 0, &texture_set, true),
            WeaponMaterialAlphaMode::Mask
        );
        assert_eq!(
            weapon_material_alpha_mode("character.shpk", 0, &texture_set, true),
            WeaponMaterialAlphaMode::Blend
        );
        assert_eq!(
            weapon_material_alpha_mode(
                "chara/weapon/material/character.shpk",
                0,
                &texture_set,
                true
            ),
            WeaponMaterialAlphaMode::Blend
        );
    }

    #[test]
    fn srgb_multiply_uses_linear_space() {
        assert_eq!(multiply_srgb_channels(255, 128), 128);
        assert_ne!(multiply_srgb_channels(128, 128), 64);
    }

    #[test]
    fn base_colorset_multiply_preserves_base_alpha() {
        let mut textures = vec![
            WeaponModelTexture {
                path: "base.tex".to_string(),
                kind: WeaponModelTextureKind::BaseColor,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![255, 128, 64, 255],
                rgba_f32: None,
            },
            WeaponModelTexture {
                path: "colorset.tex".to_string(),
                kind: WeaponModelTextureKind::BaseColor,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![255, 255, 255, 32],
                rgba_f32: None,
            },
        ];

        let index =
            combine_base_with_colorset_texture("material.mtrl", 0, 1, &mut textures).expect("bake");

        assert_eq!(textures[index].rgba[3], 255);
        assert!(!texture_alpha_affects_material_transparency(
            &textures[index]
        ));
    }

    #[test]
    fn submesh_index_ranges_are_made_part_local() {
        let ranges = normalize_submesh_index_ranges(
            12,
            [
                test_submesh_range(0, 100, 6),
                test_submesh_range(1, 106, 6),
                test_submesh_range(2, 112, 3),
            ],
        );

        assert_eq!(
            ranges,
            vec![
                MeshIndexRange {
                    submesh_index: Some(0),
                    submesh: Some(test_submesh_info(0)),
                    start: 0,
                    end: 6,
                },
                MeshIndexRange {
                    submesh_index: Some(1),
                    submesh: Some(test_submesh_info(1)),
                    start: 6,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn submesh_index_ranges_accept_already_local_offsets() {
        let ranges = normalize_submesh_index_ranges(
            12,
            [test_submesh_range(0, 0, 3), test_submesh_range(1, 3, 9)],
        );

        assert_eq!(
            ranges,
            vec![
                MeshIndexRange {
                    submesh_index: Some(0),
                    submesh: Some(test_submesh_info(0)),
                    start: 0,
                    end: 3,
                },
                MeshIndexRange {
                    submesh_index: Some(1),
                    submesh: Some(test_submesh_info(1)),
                    start: 3,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn submesh_index_ranges_keep_nonzero_local_offsets() {
        let ranges = normalize_submesh_index_ranges(
            12,
            [test_submesh_range(0, 3, 3), test_submesh_range(1, 6, 6)],
        );

        assert_eq!(
            ranges,
            vec![
                MeshIndexRange {
                    submesh_index: Some(0),
                    submesh: Some(test_submesh_info(0)),
                    start: 3,
                    end: 6,
                },
                MeshIndexRange {
                    submesh_index: Some(1),
                    submesh: Some(test_submesh_info(1)),
                    start: 6,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn submesh_index_ranges_keep_attribute_info_for_full_mesh_range() {
        let ranges = normalize_submesh_index_ranges(12, [test_submesh_range(2, 0, 12)]);

        assert_eq!(
            ranges,
            vec![MeshIndexRange {
                submesh_index: None,
                submesh: Some(test_submesh_info(2)),
                start: 0,
                end: 12,
            }]
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
    fn remap_mesh_vertices_preserves_winding_when_normals_are_opposed() {
        let vertices = vec![
            test_vertex_at([0.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            test_vertex_at([1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
            test_vertex_at([0.0, 1.0, 0.0], [0.0, 0.0, -1.0]),
        ];

        let (_, remapped_indices) =
            remap_mesh_vertices(&vertices, &[0, 1, 2]).expect("valid remap");

        assert_eq!(remapped_indices, vec![0, 1, 2]);
    }

    #[test]
    fn remap_mesh_vertices_rejects_partial_triangles() {
        let vertices = vec![test_vertex(0.0), test_vertex(1.0), test_vertex(2.0)];

        assert!(remap_mesh_vertices(&vertices, &[0, 1]).is_none());
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
            array_size: 1,
            array_layer_height: 1,
            rgba: vec![255, 255, 255, alpha],
            rgba_f32: None,
        }
    }

    fn test_submesh_range(
        index: usize,
        index_offset: usize,
        index_count: usize,
    ) -> (usize, ModelSubmeshInfo, usize, usize) {
        (index, test_submesh_info(index), index_offset, index_count)
    }

    fn test_submesh_info(index: usize) -> ModelSubmeshInfo {
        let mask = 1_u32 << index;
        ModelSubmeshInfo {
            index,
            table_index: index + 10,
            attribute_index_mask: mask,
            attribute_index_mask_hex: format!("0x{mask:08x}"),
            attribute_names: vec![format!("attr_{index}")],
            bone_start_index: index as u16,
            bone_count: 1,
        }
    }

    fn test_dawntrail_color_table_row() -> physis::mtrl::DawntrailColorTableRow {
        physis::mtrl::DawntrailColorTableRow {
            diffuse_color: [0.1, 0.2, 0.3],
            unknown1: 0.31,
            specular_color: [0.4, 0.5, 0.6],
            unknown2: 0.62,
            emissive_color: [0.7, 0.8, 0.9],
            unknown3: 0.0,
            sheen_rate: 0.11,
            sheen_tint: 0.22,
            sheen_aperture: 0.33,
            unknown4: 0.0,
            roughness: 0.44,
            unknown5: 0.0,
            metalness: 0.55,
            anisotropy: 0.66,
            unknown6: 0.0,
            sphere_mask: 0.77,
            unknown7: 0.0,
            unknown8: 0.0,
            shader_index: 3,
            tile_set: 0x3400,
            tile_alpha: 0.88,
            sphere_index: 0x4000,
            material_repeat: [1.25, 1.5],
            material_skew: [0.25, 0.5],
        }
    }

    fn test_legacy_color_table_row() -> physis::mtrl::LegacyColorTableRow {
        physis::mtrl::LegacyColorTableRow {
            diffuse_color: [0.1, 0.2, 0.3],
            specular_strength: 0.62,
            specular_color: [0.4, 0.5, 0.6],
            gloss_strength: 0.31,
            emissive_color: [0.7, 0.8, 0.9],
            tile_set: 7,
            material_repeat_x: 1.25,
            material_skew: [0.25, 0.5],
            material_repeat_y: 1.5,
        }
    }

    fn test_vertex(x: f32) -> WeaponModelVertex {
        WeaponModelVertex {
            position: [x, 0.0, 0.0],
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

    fn test_mtrl_with_low_level_fields() -> Vec<u8> {
        let mut strings = Vec::new();
        let texture_offset = strings.len() as u16;
        strings.extend_from_slice(b"texture/base.tex\0");
        let shader_package_name_offset = strings.len() as u16;
        strings.extend_from_slice(b"character.shpk\0");
        let uv_name_offset = strings.len() as u16;
        strings.extend_from_slice(b"uv0\0");
        let color_name_offset = strings.len() as u16;
        strings.extend_from_slice(b"color0\0");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x0103_0000_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&3_u16.to_le_bytes());
        bytes.extend_from_slice(&(strings.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&shader_package_name_offset.to_le_bytes());
        bytes.push(1);
        bytes.push(1);
        bytes.push(1);
        bytes.push(2);

        let packed_texture_offset = u32::from(texture_offset) | (0x00f0_u32 << 16);
        bytes.extend_from_slice(&packed_texture_offset.to_le_bytes());
        bytes.extend_from_slice(&uv_name_offset.to_le_bytes());
        bytes.push(2);
        bytes.push(3);
        bytes.extend_from_slice(&color_name_offset.to_le_bytes());
        bytes.push(4);
        bytes.push(5);
        bytes.extend_from_slice(&strings);
        bytes.extend_from_slice(&[0x30, 0x05]);
        bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC]);

        bytes.extend_from_slice(&8_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&0x11_u32.to_le_bytes());
        bytes.extend_from_slice(&0xAAAA_0001_u32.to_le_bytes());
        bytes.extend_from_slice(&0xBBBB_0002_u32.to_le_bytes());
        bytes.extend_from_slice(&G_ALPHA_THRESHOLD.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&physis::shpk::ShaderPackage::crc("g_SamplerNormal").to_le_bytes());
        bytes.extend_from_slice(&0x1234_5678_u32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(&0.25_f32.to_le_bytes());
        bytes.extend_from_slice(&0.5_f32.to_le_bytes());

        let file_size = bytes.len() as u16;
        bytes[4..6].copy_from_slice(&file_size.to_le_bytes());
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
