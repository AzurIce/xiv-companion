pub use crate::model::{
    BakedColorTableMaps, ColorTableRowColors, EQUIPMENT_MODEL_FALLBACK_RACE_ID, EquipmentSlotInfo,
    MaterialCharacterScrollVariant, MaterialDecalColorMode, MaterialDrawDepthMode,
    MaterialFlowMode, MaterialLightShaftType, MaterialLightingMode, MaterialRenderMode,
    MaterialSkinValueMode, MaterialSpecularType, MaterialSubColorMode, MaterialValueMode,
    ModelBounds, ModelColorDyeTable, ModelData, ModelDawntrailColorDyeTableRow,
    ModelLegacyColorDyeTableRow, ModelMaterial, ModelMaterialReferenceFallback,
    ModelMaterialReferenceFallbackKind, ModelMaterialTextureArrays, ModelMesh, ModelMeshDrawRole,
    ModelRenderData, ModelShapeTarget, ModelShapeVertexDelta, ModelStainingApplication,
    ModelSubmeshInfo, ModelTexture, ModelTextureKind, ModelTextureTexelLayout, ModelVertex,
    PackedEquipmentModelId, PackedModelId, PreparedMeshVisibility, PreparedModelOptions,
    StainingApplicationReport, WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage,
    WeaponMaterialAlphaMode, WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData,
    WeaponModelLoadCandidateDiagnostic, WeaponModelLoadCandidateStatus, WeaponModelLoadDiagnostic,
    WeaponModelLoadRole, WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture,
    WeaponModelTextureKind, WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    equipment_material_candidate_paths, equipment_material_candidate_paths_with_version,
    equipment_model_candidate_paths, equipment_slot_info, is_weapon_equip_slot_category,
    material_color, mesh_draw_role_for_category, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "game-data")]
use std::collections::{HashMap, HashSet};
#[cfg(feature = "game-data")]
use std::rc::Rc;

#[cfg(feature = "game-data")]
use crate::furniture::{
    FurnitureCatalogItem, FurnitureModelKind, extract_sgb_asset_paths,
    furniture_material_candidate_paths, furniture_sgb_path,
};

#[cfg(feature = "game-data")]
use crate::chara_models::{
    CharaCatalogItem, CharaModelKind, PackedCharaModelId, chara_material_candidate_paths,
    chara_model_candidate_paths,
};

#[cfg(feature = "game-data")]
use crate::skeleton::{
    ModelSkeleton, RaceDeform, bake_race_deform, character_skeleton_path,
    load_skeleton_from_sklb_bytes, skeleton_path_for_chara_model,
};

#[cfg(feature = "game-data")]
use crate::racial_scaling::{BodyScaling, HUMAN_CMP_PATH, RacialScalingTable};

#[cfg(feature = "game-data")]
use crate::chara_assemble::{
    CharacterCustomize, CharacterPartKind, EarConcealment, character_material_candidate_paths,
    character_part_paths, close_bare_limb_junctions, face_paint_decal_texture_candidates,
    race_code_from_character_model_path, snap_bare_hand_cuff_to_forearm,
};

#[cfg(feature = "game-data")]
use crate::character_make::CharacterAppearanceColors;

#[cfg(feature = "game-data")]
use crate::equipment_params::{
    EQUIPMENT_PARAMETER_PATH, EquipmentParameterEntry, EquipmentParameterTable, ImcEntry, ImcFile,
    equipment_imc_path, imc_slot_offset,
};

#[cfg(feature = "game-data")]
use crate::model::{
    MaterialShaderFamily, ModelBlendIndices, ModelBlendWeights, ModelBoneTable,
    ModelMaterialCharacterColors, material_shader_family,
};

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
const G_VERTEX_ALPHA_TO_ONE: u32 = 0xAD94_E254;
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
const CATEGORY_SPECULAR_TYPE: u32 = 0xC8BD_1DEF;
#[cfg(feature = "game-data")]
const SPECULAR_TYPE_DEFAULT: u32 = 0x198D_11CD;
#[cfg(feature = "game-data")]
const SPECULAR_TYPE_MASK: u32 = 0xA02F_4828;
#[cfg(feature = "game-data")]
const GET_VALUES: u32 = 0xB616_DC5A;
#[cfg(feature = "game-data")]
const GET_VALUES_TEXTURE_TYPE: u32 = 0x2877_1DF9;
#[cfg(feature = "game-data")]
const GET_VALUES_TEXTURE_TYPE_COMPATIBILITY: u32 = 0xCFC6_2513;
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
const GET_DECAL_COLOR: u32 = 0xD277_7173;
#[cfg(feature = "game-data")]
const GET_DECAL_COLOR_OFF: u32 = 0x4242_B842;
#[cfg(feature = "game-data")]
const GET_DECAL_COLOR_ALPHA: u32 = 0x5842_65DD;
#[cfg(feature = "game-data")]
const GET_DECAL_COLOR_RGBA: u32 = 0xF35F_5131;
#[cfg(feature = "game-data")]
const GET_MATERIAL_VALUE: u32 = 0x380C_AED0;
#[cfg(feature = "game-data")]
const GET_MATERIAL_VALUE_BODY: u32 = 0x2BDB_45F1;
#[cfg(feature = "game-data")]
const GET_MATERIAL_VALUE_BODY_JJM: u32 = 0x57FF_3B64;
#[cfg(feature = "game-data")]
const GET_MATERIAL_VALUE_FACE_EMISSIVE: u32 = 0x72E6_97CD;
#[cfg(feature = "game-data")]
const GET_MATERIAL_VALUE_FACE: u32 = 0xF567_3524;
#[cfg(feature = "game-data")]
const CHARACTER_SCROLL_VARIANT: u32 = 0xF886_E10E;
#[cfg(feature = "game-data")]
const CHARACTER_SCROLL_VARIANT_69EB4AE0: u32 = 0x69EB_4AE0;
#[cfg(feature = "game-data")]
const CHARACTER_SCROLL_VARIANT_9A8A46F5: u32 = 0x9A8A_46F5;
#[cfg(feature = "game-data")]
const LIGHTSHAFT_TYPE: u32 = 0x0DA8_270B;
#[cfg(feature = "game-data")]
const LIGHTSHAFT_TYPE_0: u32 = 0xB106_4103;
#[cfg(feature = "game-data")]
const LIGHTSHAFT_TYPE_1: u32 = 0xC601_7195;
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
const G_AMBIENT_OCCLUSION_MASK: u32 = 0x575A_BFB2;
#[cfg(feature = "game-data")]
const G_TEXTURE_MIP_BIAS: u32 = 0x3955_1220;
#[cfg(feature = "game-data")]
const G_TILE_MIP_BIAS_OFFSET: u32 = 0x6421_DD30;
#[cfg(feature = "game-data")]
const G_VERTEX_MOVEMENT_SCALE: u32 = 0x641E_0F22;
#[cfg(feature = "game-data")]
const G_VERTEX_MOVEMENT_MAX_LENGTH: u32 = 0xD26F_F0AE;
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
// iris.shpk 眼白与角膜环参数(Meddle Names.cs CRC;缺省值同 MeddleTools 节点组)。
// bg 域逐贴图 UV 缩放(每常量 4 浮点 = map0.xy + map1.xy;缺省 1,1,1,1)。
const G_COLOR_UV_SCALE: u32 = 0xA5D0_2C52;
const G_NORMAL_UV_SCALE: u32 = 0xBB99_CF76;
const G_SPECULAR_UV_SCALE: u32 = 0x8D03_A782;
const G_WHITE_EYE_COLOR: u32 = 0x11C9_0091;
const G_IRIS_RING_COLOR: u32 = 0x50E3_6D56;
const G_IRIS_RING_EMISSIVE_INTENSITY: u32 = 0x7DAB_A471;
const G_IRIS_RING_UV_RADIUS: u32 = 0xE183_98AE;
const G_IRIS_RING_UV_FADE_WIDTH: u32 = 0x5B60_8CFE;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_ANGLE_CLIP: u32 = 0x71DB_DA81;
#[cfg(feature = "game-data")]
const G_LIGHTSHAFT_NEAR_CLIP: u32 = 0x17A5_2926;
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
        normalize_stain_ids(self.stain_ids)
    }
}

#[cfg(feature = "game-data")]
fn normalize_stain_ids(stain_ids: [u8; 2]) -> [u8; 2] {
    stain_ids.map(|stain_id| (stain_id <= MAX_STAIN_ID).then_some(stain_id).unwrap_or(0))
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

/// 装备模型加载请求。`model_main`/`model_sub` 是 Item 表的原始 u64，按
/// [`PackedEquipmentModelId`] 的装备语义解码（套装 id + IMC 子集 id）。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EquipmentModelLoadRequest {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: u64,
    pub model_sub: u64,
    pub equip_slot_category: u32,
    pub race_id: u16,
    pub stain_ids: [u8; 2],
}

#[cfg(feature = "game-data")]
impl EquipmentModelLoadRequest {
    pub fn primary_model(&self) -> PackedEquipmentModelId {
        PackedEquipmentModelId::from_raw(self.model_main)
    }

    pub fn secondary_model(&self) -> Option<PackedEquipmentModelId> {
        (self.model_sub != 0).then(|| PackedEquipmentModelId::from_raw(self.model_sub))
    }

    pub fn with_race_id(mut self, race_id: u16) -> Self {
        self.race_id = race_id;
        self
    }

    pub fn with_stain_ids(mut self, stain_ids: [u8; 2]) -> Self {
        self.stain_ids = stain_ids;
        self
    }

    fn normalized_stain_ids(&self) -> [u8; 2] {
        normalize_stain_ids(self.stain_ids)
    }
}

#[cfg(feature = "game-data")]
impl From<&WeaponCatalogItem> for EquipmentModelLoadRequest {
    fn from(item: &WeaponCatalogItem) -> Self {
        Self {
            item_id: item.id,
            item_name: item.name.clone(),
            model_main: item.model_main,
            model_sub: item.model_sub,
            equip_slot_category: item.equip_slot_category,
            race_id: EQUIPMENT_MODEL_FALLBACK_RACE_ID,
            stain_ids: [0, 0],
        }
    }
}

/// 装备模型加载结果直接复用武器的结果结构。`model_main`/`model_sub` 保留原始
/// raw 值，但 [`PackedModelId`] 的字段按武器三段语义解读；装备的套装 id 与
/// IMC 子集 id 应以 [`PackedEquipmentModelId`] 重新解码。
#[cfg(feature = "game-data")]
pub type EquipmentModelData = WeaponModelData;

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, Default)]
struct WeaponStainingTemplateLoad {
    template: Option<StainingTemplate>,
    error: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
pub struct WeaponStainingTemplates {
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

    pub fn from_load_results(
        legacy: Result<Vec<u8>, String>,
        dawntrail: Result<Vec<u8>, String>,
    ) -> Self {
        Self {
            stain_ids: [0, 0],
            legacy: staining_template_load_from_result(LEGACY_STAINING_TEMPLATE_PATH, legacy),
            dawntrail: staining_template_load_from_result(
                DAWNTRAIL_STAINING_TEMPLATE_PATH,
                dawntrail,
            ),
        }
    }
}

#[cfg(feature = "game-data")]
fn staining_template_load_from_result(
    path: &str,
    result: Result<Vec<u8>, String>,
) -> WeaponStainingTemplateLoad {
    match result {
        Ok(bytes) => match StainingTemplate::from_bytes(&bytes) {
            Ok(template) => WeaponStainingTemplateLoad {
                template: Some(template),
                error: None,
            },
            Err(error) => WeaponStainingTemplateLoad {
                template: None,
                error: Some(format!("failed to parse {path}: {error:#}")),
            },
        },
        Err(error) => WeaponStainingTemplateLoad {
            template: None,
            error: Some(format!("failed to read {path}: {error}")),
        },
    }
}

#[cfg(feature = "game-data")]
#[derive(Clone)]
struct LoadedMaterialColorTable {
    slot: usize,
    material_index: u16,
    name: String,
    path: String,
    rows: Vec<ColorTableRowColors>,
    dye_table: Option<ModelColorDyeTable>,
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
pub struct ShaderPackageSemanticDebug {
    pub path: String,
    pub name: String,
    pub sampler_resources: Vec<ShaderPackageSamplerResourceDebug>,
    pub material_keys: Vec<ShaderPackageKeyDefaultDebug>,
    pub system_keys: Vec<ShaderPackageKeyDefaultDebug>,
    pub scene_keys: Vec<ShaderPackageKeyDefaultDebug>,
    pub material_constants: Vec<ShaderPackageMaterialConstantDebug>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderPackageSamplerResourceDebug {
    pub name: String,
    pub crc: u32,
    pub crc_hex: String,
    pub slot: u16,
    pub size: u16,
    pub logical_role: Option<MaterialSamplerLogicalRole>,
    pub kind: Option<WeaponModelTextureKind>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderPackageKeyDefaultDebug {
    pub id: u32,
    pub id_hex: String,
    pub name: Option<String>,
    pub default_value: u32,
    pub default_value_hex: String,
    pub default_value_name: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShaderPackageMaterialConstantDebug {
    pub id: u32,
    pub id_hex: String,
    pub name: Option<String>,
    pub byte_offset: u16,
    pub byte_size: u16,
    pub default_values: Option<Vec<f32>>,
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
    pub logical_role: Option<MaterialSamplerLogicalRole>,
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
    pub logical_role: Option<MaterialSamplerLogicalRole>,
    pub kind: Option<WeaponModelTextureKind>,
    pub kind_source: Option<String>,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MaterialSamplerLogicalRole {
    BaseColor,
    SecondaryBaseColor,
    Normal,
    SecondaryNormal,
    Mask,
    SkinDiffuse,
    SkinNormal,
    SkinMask,
    MaterialMap,
    MultiMap,
    Specular,
    SecondarySpecular,
    Emissive,
    Index,
    Environment,
    WaterWave,
    WaterWaveSecondary,
    WaterWhitecap,
}

#[cfg(feature = "game-data")]
impl MaterialSamplerLogicalRole {
    fn texture_kind(self) -> WeaponModelTextureKind {
        match self {
            Self::BaseColor | Self::SkinDiffuse => WeaponModelTextureKind::BaseColor,
            Self::SecondaryBaseColor => WeaponModelTextureKind::SecondaryBaseColor,
            Self::Normal | Self::SkinNormal => WeaponModelTextureKind::Normal,
            Self::SecondaryNormal => WeaponModelTextureKind::SecondaryNormal,
            Self::Mask | Self::SkinMask => WeaponModelTextureKind::Mask,
            Self::MaterialMap => WeaponModelTextureKind::MaterialMap,
            Self::MultiMap => WeaponModelTextureKind::MultiMap,
            Self::Specular => WeaponModelTextureKind::Specular,
            Self::SecondarySpecular => WeaponModelTextureKind::SecondarySpecular,
            Self::Emissive => WeaponModelTextureKind::Emissive,
            Self::Index => WeaponModelTextureKind::Index,
            Self::Environment => WeaponModelTextureKind::Environment,
            Self::WaterWave => WeaponModelTextureKind::WaterWave,
            Self::WaterWaveSecondary => WeaponModelTextureKind::WaterWaveSecondary,
            Self::WaterWhitecap => WeaponModelTextureKind::WaterWhitecap,
        }
    }
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
    apply_weapon_staining_with_ids(rows, dye_table, staining.stain_ids, staining)
}

#[cfg(feature = "game-data")]
fn apply_weapon_staining_with_ids(
    rows: Option<&mut [ColorTableRowColors]>,
    dye_table: Option<&ModelColorDyeTable>,
    stain_ids: [u8; 2],
    staining: &WeaponStainingTemplates,
) -> Option<ModelStainingApplication> {
    if !stain_ids.iter().any(|stain_id| *stain_id != 0) {
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
                stain_ids,
                template_path: String::new(),
                report: StainingApplicationReport::default(),
                error: Some("opaque ColorDyeTable cannot be applied".to_string()),
            });
        }
    };

    let Some(rows) = rows else {
        return Some(ModelStainingApplication {
            stain_ids,
            template_path: template_path.to_string(),
            report: StainingApplicationReport::default(),
            error: Some("material has no supported ColorTable rows".to_string()),
        });
    };
    let Some(template) = template else {
        return Some(ModelStainingApplication {
            stain_ids,
            template_path: template_path.to_string(),
            report: StainingApplicationReport::default(),
            error: load_error.or_else(|| Some(format!("{template_path} is unavailable"))),
        });
    };

    Some(ModelStainingApplication {
        stain_ids,
        template_path: template_path.to_string(),
        report: apply_staining_template_to_rows(rows, dye_table, &stain_ids, template),
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
        texel_layout: ModelTextureTexelLayout::Standard,
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
    let mut color_table_sources = HashMap::new();
    let stain_ids = request.normalized_stain_ids();
    let staining =
        load_weapon_staining_templates_from_resource(resource, stain_ids, &mut loaded_paths);

    load_model_meshes_from_resource(
        resource,
        ModelPathContext::Weapon(model_main),
        &staining,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    )
    .map_err(WeaponModelMeshLoadFailure::into_error)?;

    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            if let Err(failure) = load_model_meshes_from_resource(
                resource,
                ModelPathContext::Weapon(model_sub),
                &staining,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
                &mut color_table_sources,
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
pub fn load_equipment_model_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &EquipmentModelLoadRequest,
) -> anyhow::Result<EquipmentModelData> {
    let Some(slot) = equipment_slot_info(request.equip_slot_category) else {
        return Err(anyhow::anyhow!(
            "equip slot category {} has no equipment model",
            request.equip_slot_category
        ));
    };
    let model_main = request.primary_model();
    let model_sub = request.secondary_model();
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let stain_ids = request.normalized_stain_ids();
    let staining =
        load_weapon_staining_templates_from_resource(resource, stain_ids, &mut loaded_paths);

    let main_context = ModelPathContext::Equipment(EquipmentModelPathContext {
        model: model_main,
        slot,
        race_id: request.race_id,
        material_version: None,
    });
    load_model_meshes_from_resource(
        resource,
        main_context.clone(),
        &staining,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    )
    .map_err(WeaponModelMeshLoadFailure::into_error)?;

    if let Some(model_sub) = model_sub {
        if model_sub.raw != model_main.raw {
            let sub_context = ModelPathContext::Equipment(EquipmentModelPathContext {
                model: model_sub,
                slot,
                race_id: request.race_id,
                material_version: None,
            });
            if let Err(failure) = load_model_meshes_from_resource(
                resource,
                sub_context,
                &staining,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
                &mut color_table_sources,
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
        model_main: main_context.diagnostic_model(),
        model_sub: model_sub.map(|model| PackedModelId::from_raw(model.raw)),
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
            let Some((vertices, indices, shape_targets)) = remap_mesh_vertices_with_shapes(
                &mesh.vertices,
                raw_indices,
                range.start,
                &mesh.shape_targets,
            ) else {
                continue;
            };
            let shape_influences = shape_targets
                .iter()
                .map(|target| target.shape.clone())
                .collect();
            meshes.push(WeaponModelMesh {
                path: mesh_path_with_submesh(path, mesh.mesh_index, range.submesh_index),
                part_index: mesh.mesh_index as u32,
                mesh_category: Some(mesh.category.clone()),
                submesh: range.submesh.clone(),
                shape_influences,
                shape_targets,
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
#[cfg(test)]
fn remap_mesh_vertices(
    vertices: &[WeaponModelVertex],
    indices: &[u16],
) -> Option<(Vec<WeaponModelVertex>, Vec<u32>)> {
    let (vertices, indices, _) = remap_mesh_vertices_with_shapes(vertices, indices, 0, &[])?;
    Some((vertices, indices))
}

#[cfg(feature = "game-data")]
fn remap_mesh_vertices_with_shapes(
    vertices: &[WeaponModelVertex],
    indices: &[u16],
    index_position_offset: usize,
    shape_targets: &[crate::mdl_geometry::MdlGeometryShapeTarget],
) -> Option<(Vec<WeaponModelVertex>, Vec<u32>, Vec<ModelShapeTarget>)> {
    if indices.len() % 3 != 0 {
        return None;
    }

    let mut remapped_vertices = Vec::new();
    let mut remap = HashMap::<(u16, Vec<(usize, usize)>), u32>::new();
    let mut remapped_indices = Vec::with_capacity(indices.len());
    let mut replacements_by_position = HashMap::<usize, Vec<(usize, usize)>>::new();
    for (target_index, target) in shape_targets.iter().enumerate() {
        for replacement in &target.replacements {
            if replacement.replacing_vertex_index >= vertices.len() {
                continue;
            }
            replacements_by_position
                .entry(replacement.base_indices_index)
                .or_default()
                .push((target_index, replacement.replacing_vertex_index));
        }
    }
    for replacements in replacements_by_position.values_mut() {
        replacements.sort_unstable();
        replacements.dedup_by_key(|(target_index, _)| *target_index);
    }
    let mut target_deltas = vec![Vec::new(); shape_targets.len()];

    for (local_position, index) in indices.iter().copied().enumerate() {
        let source_vertex = *vertices.get(usize::from(index))?;
        let replacements = replacements_by_position
            .get(&index_position_offset.saturating_add(local_position))
            .cloned()
            .unwrap_or_default();
        let key = (index, replacements.clone());
        let remapped_index = if let Some(remapped_index) = remap.get(&key) {
            *remapped_index
        } else {
            let remapped_index = remapped_vertices.len() as u32;
            remapped_vertices.push(source_vertex);
            for (target_index, replacing_vertex_index) in &replacements {
                let replacement = vertices[*replacing_vertex_index];
                target_deltas[*target_index].push(ModelShapeVertexDelta {
                    vertex_index: remapped_index,
                    position: subtract_vec3(replacement.position, source_vertex.position),
                    normal: subtract_vec3(replacement.normal, source_vertex.normal),
                });
            }
            remap.insert(key, remapped_index);
            remapped_index
        };
        remapped_indices.push(remapped_index);
    }

    let shape_targets = shape_targets
        .iter()
        .zip(target_deltas)
        .filter_map(|(target, vertex_deltas)| {
            (!vertex_deltas.is_empty()).then(|| ModelShapeTarget {
                shape: target.info.clone(),
                vertex_deltas,
            })
        })
        .collect();

    Some((remapped_vertices, remapped_indices, shape_targets))
}

#[cfg(feature = "game-data")]
fn subtract_vec3(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

#[cfg(feature = "game-data")]
pub fn shader_package_semantic_debug_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    shader_package_name: &str,
) -> anyhow::Result<ShaderPackageSemanticDebug> {
    use anyhow::anyhow;

    let name = normalize_game_resource_path(shader_package_name);
    if name.is_empty() {
        return Err(anyhow!("shader package name is empty"));
    }
    let path = if name.contains('/') {
        name
    } else {
        format!("shader/sm5/shpk/{name}")
    };
    let bytes = resource
        .read(&path)
        .ok_or_else(|| anyhow!("failed to read shader package {path}"))?;

    shader_package_semantic_debug_from_bytes_for_platform(&path, &bytes, resource.platform())
}

#[cfg(feature = "game-data")]
pub fn shader_package_semantic_debug_from_shpk_bytes(
    path: &str,
    bytes: &[u8],
) -> anyhow::Result<ShaderPackageSemanticDebug> {
    shader_package_semantic_debug_from_bytes_for_platform(path, bytes, physis::Platform::Win32)
}

#[cfg(feature = "game-data")]
fn shader_package_semantic_debug_from_bytes_for_platform(
    path: &str,
    bytes: &[u8],
    platform: physis::Platform,
) -> anyhow::Result<ShaderPackageSemanticDebug> {
    use anyhow::anyhow;
    use physis::ReadableFile;

    let path = normalize_game_resource_path(path);
    let name = path.rsplit('/').next().unwrap_or_default().to_string();
    let shader_package = physis::shpk::ShaderPackage::from_existing(platform, bytes)
        .ok_or_else(|| anyhow!("failed to parse shader package {path}"))?;

    Ok(ShaderPackageSemanticDebug {
        path,
        name,
        sampler_resources: shader_package
            .sampler_parameters
            .iter()
            .map(|parameter| {
                let crc = physis::shpk::ShaderPackage::crc(&parameter.name);
                let logical_role = classify_sampler_logical_role_name(&parameter.name);
                ShaderPackageSamplerResourceDebug {
                    name: parameter.name.clone(),
                    crc,
                    crc_hex: hex_u32(crc),
                    slot: parameter.slot,
                    size: parameter.size,
                    logical_role,
                    kind: logical_role.map(MaterialSamplerLogicalRole::texture_kind),
                }
            })
            .collect(),
        material_keys: shader_package_key_defaults_debug(&shader_package.material_keys),
        system_keys: shader_package_key_defaults_debug(&shader_package.system_keys),
        scene_keys: shader_package_key_defaults_debug(&shader_package.scene_keys),
        material_constants: shader_package_material_parameters_for_platform(bytes, platform)
            .into_iter()
            .map(|parameter| ShaderPackageMaterialConstantDebug {
                id: parameter.id,
                id_hex: hex_u32(parameter.id),
                name: known_material_constant_name(parameter.id),
                byte_offset: parameter.byte_offset,
                byte_size: parameter.byte_size,
                default_values: parameter.default_values,
            })
            .collect(),
    })
}

#[cfg(feature = "game-data")]
fn shader_package_key_defaults_debug(
    keys: &[physis::shpk::Key],
) -> Vec<ShaderPackageKeyDefaultDebug> {
    keys.iter()
        .map(|key| ShaderPackageKeyDefaultDebug {
            id: key.id,
            id_hex: hex_u32(key.id),
            name: known_shader_label(key.id),
            default_value: key.default_value,
            default_value_hex: hex_u32(key.default_value),
            default_value_name: known_shader_label(key.default_value),
        })
        .collect()
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
    let texture_paths = material_texture_paths_from_offsets(bytes, &material.texture_paths);
    let low_level = material_low_level_debug(bytes, &texture_paths);
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
            texture_path: texture_paths.get(record.texture_index).cloned(),
            texture_usage: record.texture_usage,
            texture_usage_hex: hex_u32(record.texture_usage),
            texture_usage_name: record.texture_usage_name,
            flags: record.flags,
            flags_hex: hex_u32(record.flags),
            logical_role: record.logical_role,
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
        texture_paths,
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
            logical_role: sampler.logical_role,
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
    if id == G_VERTEX_ALPHA_TO_ONE {
        // Meddle still records this CRC as unknown. The descriptive label is
        // based on the installed DXBC formula, not claimed as an official name.
        return Some("VertexAlphaToOne".to_string());
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
            "g_AmbientOcclusionMask",
            "g_TextureMipBias",
            "g_TileMipBiasOffset",
            "g_VertexMovementScale",
            "g_VertexMovementMaxLength",
            "g_ShadowPosOffset",
            "g_GlassIOR",
            "g_GlassThicknessMax",
        ],
    )
}

#[cfg(feature = "game-data")]
fn known_shader_label(id: u32) -> Option<String> {
    let fixed_label = match id {
        CATEGORY_FLOW_MAP_TYPE => Some("CategoryFlowMapType"),
        FLOW_MAP_STANDARD => Some("Standard"),
        FLOW_MAP_FLOW => Some("Flow"),
        CATEGORY_SPECULAR_TYPE => Some("CategorySpecularType"),
        SPECULAR_TYPE_DEFAULT => Some("Default"),
        SPECULAR_TYPE_MASK => Some("Mask"),
        _ => None,
    };
    if let Some(label) = fixed_label {
        return Some(label.to_string());
    }
    known_crc_label(
        id,
        &[
            "ApplyAlphaTest",
            "ApplyAlphaTestOn",
            "ApplyAlphaTestOff",
            "ApplyAlphaClip",
            "ApplyAlphaClipOn",
            "ApplyAlphaClipOff",
            "ApplyVertexColor",
            "ApplyVertexColorOn",
            "ApplyVertexColorOff",
            "ApplyVertexMovement",
            "ApplyVertexMovementOff",
            "ApplyVertexMovementOn",
            "DrawDepthMode",
            "DrawDepthMode_Dither",
            "EnableLighting",
            "EnableLightingOff",
            "EnableLightingOn",
            "GetValues",
            "GetSingleValues",
            "GetMultiValues",
            "GetValuesMultiMaterial",
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
            "GetDecalColorOff",
            "GetDecalColorAlpha",
            "GetDecalColorRGBA",
            "GetSubColor",
            "GetSubColorFace",
            "GetSubColorHair",
            "CategorySpecularType",
            "Default",
            "Mask",
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
fn material_texture_paths_from_offsets(bytes: &[u8], fallback: &[String]) -> Vec<String> {
    let Some(string_table_size) = read_u16_le(bytes, 8).map(usize::from) else {
        return fallback.to_vec();
    };
    let Some(texture_count) = bytes.get(12).copied().map(usize::from) else {
        return fallback.to_vec();
    };
    let Some(uv_set_count) = bytes.get(13).copied().map(usize::from) else {
        return fallback.to_vec();
    };
    let Some(color_set_count) = bytes.get(14).copied().map(usize::from) else {
        return fallback.to_vec();
    };

    let mut texture_offsets = Vec::with_capacity(texture_count);
    let mut offset = 16_usize;
    for _ in 0..texture_count {
        let Some(raw) = read_u32_le(bytes, offset) else {
            return fallback.to_vec();
        };
        texture_offsets.push(raw as u16);
        let Some(next) = checked_advance(offset, 4, bytes.len()) else {
            return fallback.to_vec();
        };
        offset = next;
    }

    let named_set_bytes = uv_set_count
        .checked_add(color_set_count)
        .and_then(|count| count.checked_mul(4));
    let Some(string_table_offset) =
        named_set_bytes.and_then(|size| checked_advance(offset, size, bytes.len()))
    else {
        return fallback.to_vec();
    };
    let Some(string_table) = read_bytes(bytes, string_table_offset, string_table_size) else {
        return fallback.to_vec();
    };

    let resolved = texture_offsets
        .into_iter()
        .enumerate()
        .map(|(index, string_offset)| {
            read_string_at(string_table, usize::from(string_offset))
                .or_else(|| fallback.get(index).cloned())
        })
        .collect::<Option<Vec<_>>>();

    resolved.unwrap_or_else(|| fallback.to_vec())
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

/// 模型/材质候选路径的来源：武器按 [`PackedModelId`]，装备按套装、槽位与
/// race，家具按 SGB 给出的精确 MDL 路径（材质优先匹配 SGB 引用文件列表），
/// 宠物/坐骑按 ModelChara 三元组（demihuman 多 MDL 由加载方逐槽位探测），
/// 角色拼装按部件类别 + 捏脸数据（human 域材质特例）。
/// 五条加载链共用同一套 MDL/MTRL 读取与染色逻辑，仅在路径候选上分叉。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
enum ModelPathContext {
    Weapon(PackedModelId),
    Equipment(EquipmentModelPathContext),
    Furniture(FurnitureModelPathContext),
    Chara(CharaModelPathContext),
    Character(CharacterModelPathContext),
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct EquipmentModelPathContext {
    model: PackedEquipmentModelId,
    slot: EquipmentSlotInfo,
    race_id: u16,
    /// 材质版本目录覆盖（着装装配从 IMC 条目解析的 MaterialSet）；None 时按
    /// variant 猜测（`equipment_material_candidate_paths` 原行为）。
    material_version: Option<u16>,
}

/// 家具路径上下文：SGB 给出的精确 MDL 路径 + 同一家具全部 SGB 引用文件
/// （.mtrl/.tex 等），材质候选先按文件名匹配后者，缺失再按 MDL 目录推导。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
struct FurnitureModelPathContext {
    model_key: u16,
    model_path: String,
    sgb_files: Rc<[String]>,
}

/// 宠物/坐骑路径上下文：单个精确 MDL 路径（monster 唯一，demihuman 每槽位
/// 一个，由加载方循环探测合并）+ ModelChara 三元组（材质版本回退与诊断用）。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
struct CharaModelPathContext {
    model: PackedCharaModelId,
    model_path: String,
}

/// 角色拼装路径上下文：单个部件的全部模型候选（首选 + 备选，共享发型）+
/// 部件类别与捏脸数据（材质候选的 human 域特例由 `chara_assemble` 按部件类别
/// 处理）。材质候选按实际命中的模型路径推导。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
struct CharacterModelPathContext {
    customize: CharacterCustomize,
    part: CharacterPartKind,
    model_candidates: Vec<String>,
}

#[cfg(feature = "game-data")]
impl ModelPathContext {
    fn model_candidate_paths(&self) -> Vec<String> {
        match self {
            Self::Weapon(model) => weapon_model_candidate_paths(*model),
            Self::Equipment(context) => {
                equipment_model_candidate_paths(context.model, context.slot, context.race_id)
            }
            Self::Furniture(context) => vec![context.model_path.clone()],
            Self::Chara(context) => vec![context.model_path.clone()],
            Self::Character(context) => context.model_candidates.clone(),
        }
    }

    fn material_candidate_paths(&self, model_path: &str, material_name: &str) -> Vec<String> {
        match self {
            Self::Weapon(model) => {
                weapon_material_candidate_paths(*model, model_path, material_name)
            }
            Self::Equipment(context) => equipment_material_candidate_paths_with_version(
                context.model,
                model_path,
                material_name,
                context.material_version,
            ),
            Self::Furniture(context) => {
                furniture_material_candidate_paths(model_path, material_name, &context.sgb_files)
            }
            Self::Chara(context) => {
                chara_material_candidate_paths(context.model, model_path, material_name)
            }
            Self::Character(context) => character_material_candidate_paths(
                &context.customize,
                context.part,
                model_path,
                material_name,
            ),
        }
    }

    /// 诊断信息用的模型标识；装备的 raw 原样保留（字段按武器语义解读），
    /// 家具打包 housing ModelKey（model_id = ModelKey，其余段为 0），宠物/坐骑
    /// 打包 ModelChara 三元组（model/base/variant 对应武器三段布局），角色拼装
    /// 打包 race code（c 编码）。
    fn diagnostic_model(&self) -> PackedModelId {
        match self {
            Self::Weapon(model) => *model,
            Self::Equipment(context) => PackedModelId::from_raw(context.model.raw),
            Self::Furniture(context) => PackedModelId::from_raw(context.model_key as u64),
            Self::Chara(context) => chara_diagnostic_model(context.model),
            Self::Character(context) => {
                PackedModelId::from_raw(u64::from(context.customize.race_code()))
            }
        }
    }
}

/// 宠物/坐骑模型标识打包成武器三段布局的 raw（model | base<<16 | variant<<32）。
#[cfg(feature = "game-data")]
fn chara_diagnostic_model(model: PackedCharaModelId) -> PackedModelId {
    PackedModelId::from_raw(
        u64::from(model.model_id)
            | (u64::from(model.base_id) << 16)
            | (u64::from(model.variant_id) << 32),
    )
}

#[cfg(feature = "game-data")]
fn load_model_meshes_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    paths: ModelPathContext,
    staining: &WeaponStainingTemplates,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<WeaponModelMesh>,
    color_table_sources: &mut HashMap<u16, LoadedMaterialColorTable>,
) -> Result<(), WeaponModelMeshLoadFailure> {
    use anyhow::Context;

    let mut candidates = Vec::new();
    for path in paths.model_candidate_paths() {
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
                return Err(WeaponModelMeshLoadFailure::new(
                    paths.diagnostic_model(),
                    candidates,
                ));
            }
        };
        push_loaded_path(loaded_paths, path.clone());
        assign_model_materials_from_resource(
            resource,
            paths,
            &path,
            staining,
            &mut path_meshes,
            materials,
            textures,
            loaded_paths,
            color_table_sources,
        );
        meshes.append(&mut path_meshes);
        return Ok(());
    }

    Err(WeaponModelMeshLoadFailure::new(
        paths.diagnostic_model(),
        candidates,
    ))
}

#[cfg(feature = "game-data")]
fn assign_model_materials_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    paths: ModelPathContext,
    model_path: &str,
    staining: &WeaponStainingTemplates,
    meshes: &mut [WeaponModelMesh],
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
    color_table_sources: &mut HashMap<u16, LoadedMaterialColorTable>,
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
        let material = load_model_material_from_resource(
            resource,
            paths.clone(),
            model_path,
            staining,
            material_index,
            material_name,
            slot,
            materials,
            textures,
            loaded_paths,
            color_table_sources,
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
fn load_model_material_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    paths: ModelPathContext,
    model_path: &str,
    staining: &WeaponStainingTemplates,
    material_index: u16,
    material_name: String,
    slot: usize,
    loaded_materials: &[WeaponModelMaterial],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
    color_table_sources: &mut HashMap<u16, LoadedMaterialColorTable>,
) -> WeaponModelMaterial {
    use physis::ReadableFile;

    let fallback = material_color(material_index);
    let candidates = paths.material_candidate_paths(model_path, &material_name);
    for path in candidates {
        let Some(bytes) = resource.read(&path) else {
            continue;
        };
        let Some(material) = physis::mtrl::Material::from_existing(resource.platform(), &bytes)
        else {
            continue;
        };
        let texture_paths = material_texture_paths_from_offsets(&bytes, &material.texture_paths);

        push_loaded_path(loaded_paths, path.clone());
        let shader_package_name = material.shader_package_name.clone();
        let mut color_dye_table = model_color_dye_table(material.color_dye_table.as_ref());
        let mut color_table_rows = material
            .color_table
            .as_ref()
            .and_then(weapon_color_table_rows);
        let reference_fallback = resolve_loaded_color_table_reference(
            slot,
            material_index,
            &material_name,
            &path,
            &mut color_table_rows,
            &mut color_dye_table,
            color_table_sources,
        );
        let fallback_index_texture = loaded_color_table_reference_index_texture(
            reference_fallback.as_ref(),
            loaded_materials,
        );
        let base_color_table_rows = color_table_rows.clone();
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
        let (specular_type, specular_type_raw) = composed_material_specular_type(&semantics);
        let (value_mode, value_mode_raw) = composed_material_value_mode(&semantics);
        let color_table_diffuse_composition = color_table_diffuse_composition(
            material_shader_family(Some(&shader_package_name)),
            composed_material_uses_compatibility_values(&semantics),
        );
        let sub_color_mode = composed_material_sub_color_mode(&semantics);
        let (decal_color_mode, decal_color_mode_raw) =
            composed_material_decal_color_mode(&semantics);
        let skin_value_mode = composed_material_skin_value_mode(&semantics);
        let (character_scroll_variant, character_scroll_variant_raw) =
            composed_material_character_scroll_variant(&semantics);
        let (lightshaft_type, lightshaft_type_raw) = composed_material_lightshaft_type(&semantics);
        let transparency = composed_material_transparency(&semantics, &shader_package_name);
        let water_deep_color = composed_material_water_deep_color(&semantics);
        let water_refraction_color = composed_material_water_refraction_color(&semantics);
        let water_whitecap_color = composed_material_water_whitecap_color(&semantics);
        let alpha_aperture = composed_material_alpha_aperture(&semantics);
        let alpha_offset = composed_material_alpha_offset(&semantics);
        let vertex_alpha_to_one = composed_material_vertex_alpha_to_one(&semantics);
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
        let ambient_occlusion_mask = composed_material_ambient_occlusion_mask(&semantics);
        let texture_mip_bias = composed_material_texture_mip_bias(&semantics);
        let tile_mip_bias_offset = composed_material_tile_mip_bias_offset(&semantics);
        let vertex_movement_scale = composed_material_vertex_movement_scale(&semantics);
        let vertex_movement_max_length = composed_material_vertex_movement_max_length(&semantics);
        let shadow_pos_offset = composed_material_shadow_pos_offset(&semantics);
        let detail_color_uv_scale = composed_material_detail_color_uv_scale(&semantics);
        let detail_normal_uv_scale = composed_material_detail_normal_uv_scale(&semantics);
        let uv_scroll = composed_material_uv_scroll(&semantics);
        let color_uv_scale = composed_material_color_uv_scale(&semantics);
        let normal_uv_scale = composed_material_normal_uv_scale(&semantics);
        let specular_uv_scale = composed_material_specular_uv_scale(&semantics);
        let white_eye_color = composed_material_white_eye_color(&semantics);
        let iris_ring_color = composed_material_iris_ring_color(&semantics);
        let iris_ring_emissive_intensity =
            composed_material_iris_ring_emissive_intensity(&semantics);
        let iris_ring_uv_radius = composed_material_iris_ring_uv_radius(&semantics);
        let iris_ring_uv_fade_width = composed_material_iris_ring_uv_fade_width(&semantics);
        let lightshaft_color = composed_material_lightshaft_color(&semantics);
        let lightshaft_tex_anim = composed_material_lightshaft_tex_anim(&semantics);
        let lightshaft_tex_u = composed_material_lightshaft_tex_u(&semantics);
        let lightshaft_tex_v = composed_material_lightshaft_tex_v(&semantics);
        let lightshaft_ray = composed_material_lightshaft_ray(&semantics);
        let lightshaft_angle_clip = composed_material_lightshaft_angle_clip(&semantics);
        let lightshaft_near_clip = composed_material_lightshaft_near_clip(&semantics);
        let texture_set = load_weapon_material_textures_from_resource(
            resource,
            &path,
            &texture_paths,
            color_table_rows.as_deref(),
            &sampler_roles,
            color_table_diffuse_composition,
            fallback_index_texture,
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
            reference_fallback,
            shader_package_name: Some(shader_package_name),
            render_mode,
            alpha_mode,
            alpha_threshold,
            draw_depth_mode,
            lighting_mode,
            flow_mode,
            specular_type,
            specular_type_raw,
            value_mode,
            value_mode_raw,
            sub_color_mode,
            decal_color_mode,
            decal_color_mode_raw,
            skin_value_mode,
            character_scroll_variant,
            character_scroll_variant_raw,
            lightshaft_type,
            lightshaft_type_raw,
            transparency,
            water_deep_color,
            water_refraction_color,
            water_whitecap_color,
            alpha_aperture,
            alpha_offset,
            vertex_alpha_to_one,
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
            ambient_occlusion_mask,
            texture_mip_bias,
            tile_mip_bias_offset,
            vertex_movement_scale,
            vertex_movement_max_length,
            shadow_pos_offset,
            detail_color_uv_scale,
            detail_normal_uv_scale,
            uv_scroll,
            color_uv_scale,
            normal_uv_scale,
            specular_uv_scale,
            white_eye_color,
            iris_ring_color,
            iris_ring_emissive_intensity,
            iris_ring_uv_radius,
            iris_ring_uv_fade_width,
            lightshaft_color,
            lightshaft_tex_anim,
            lightshaft_tex_u,
            lightshaft_tex_v,
            lightshaft_ray,
            lightshaft_angle_clip,
            lightshaft_near_clip,
            opacity,
            render_backfaces,
            apply_vertex_color,
            has_color_dye_table: color_dye_table.is_some(),
            color_dye_table,
            color_table_rows: base_color_table_rows,
            staining_application,
            character_colors: None,
            texture_arrays: ModelMaterialTextureArrays::default(),
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            colorset_diffuse_texture: texture_set.colorset_diffuse,
            secondary_base_color_texture: texture_set.secondary_base_color,
            normal_texture: texture_set.normal,
            secondary_normal_texture: texture_set.secondary_normal,
            mask_texture: texture_set.mask,
            skin_diffuse_texture: texture_set.skin_diffuse,
            skin_normal_texture: texture_set.skin_normal,
            skin_mask_texture: texture_set.skin_mask,
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
    texture_paths: &[String],
    color_table_rows: Option<&[ColorTableRowColors]>,
    sampler_roles: &[MaterialSamplerRole],
    color_table_diffuse_composition: ColorTableDiffuseComposition,
    fallback_index_texture: Option<usize>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in texture_paths.iter().enumerate() {
        let sampler_role = sampler_role_for_texture(sampler_roles, texture_order);
        let sampler_kind = sampler_role.map(|role| role.kind);
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
        assign_weapon_texture_slot(
            &mut set,
            texture_index,
            &textures[texture_index],
            sampler_role.map(|role| role.logical_role),
        );
    }
    apply_color_table_index_fallback(&mut set, fallback_index_texture, textures);

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        color_table_rows,
        set.index,
        set.emissive.is_none(),
        textures,
    ) {
        let resolved_base = resolve_color_table_base_texture(
            set.base_color,
            baked.base_color,
            color_table_diffuse_composition,
        );
        set.base_color = Some(resolved_base.base_color);
        add_unique_index(&mut set.indices, resolved_base.base_color);
        set.colorset_diffuse = resolved_base.colorset_diffuse;
        if let Some(colorset_diffuse) = resolved_base.colorset_diffuse {
            add_unique_index(&mut set.indices, colorset_diffuse);
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
    let mut color_table_sources = HashMap::new();
    let stain_ids = request.normalized_stain_ids();
    let staining =
        load_weapon_staining_templates_from_async_resource(resource, stain_ids, &mut loaded_paths)
            .await;

    load_model_meshes_from_async_resource(
        resource,
        ModelPathContext::Weapon(model_main),
        &staining,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    )
    .await
    .map_err(WeaponModelMeshLoadFailure::into_error)?;

    if let Some(model_sub) = model_sub {
        if model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw {
            if let Err(failure) = load_model_meshes_from_async_resource(
                resource,
                ModelPathContext::Weapon(model_sub),
                &staining,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
                &mut color_table_sources,
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
pub async fn load_equipment_model_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &EquipmentModelLoadRequest,
) -> anyhow::Result<EquipmentModelData> {
    let Some(slot) = equipment_slot_info(request.equip_slot_category) else {
        return Err(anyhow::anyhow!(
            "equip slot category {} has no equipment model",
            request.equip_slot_category
        ));
    };
    let model_main = request.primary_model();
    let model_sub = request.secondary_model();
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let stain_ids = request.normalized_stain_ids();
    let staining =
        load_weapon_staining_templates_from_async_resource(resource, stain_ids, &mut loaded_paths)
            .await;

    let main_context = ModelPathContext::Equipment(EquipmentModelPathContext {
        model: model_main,
        slot,
        race_id: request.race_id,
        material_version: None,
    });
    load_model_meshes_from_async_resource(
        resource,
        main_context.clone(),
        &staining,
        &mut loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    )
    .await
    .map_err(WeaponModelMeshLoadFailure::into_error)?;

    if let Some(model_sub) = model_sub {
        if model_sub.raw != model_main.raw {
            let sub_context = ModelPathContext::Equipment(EquipmentModelPathContext {
                model: model_sub,
                slot,
                race_id: request.race_id,
                material_version: None,
            });
            if let Err(failure) = load_model_meshes_from_async_resource(
                resource,
                sub_context,
                &staining,
                &mut loaded_paths,
                &mut materials,
                &mut textures,
                &mut meshes,
                &mut color_table_sources,
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
        model_main: main_context.diagnostic_model(),
        model_sub: model_sub.map(|model| PackedModelId::from_raw(model.raw)),
        stain_ids,
        load_diagnostics,
        loaded_paths,
        bounds: calculate_model_bounds(&meshes),
        materials,
        textures,
        meshes,
    })
}

/// 家具/庭具模型加载请求。`model_key` 是 HousingFurniture/HousingYardObject 的
/// ModelKey（模型 id），`kind` 决定室内/庭具 SGB 根路径。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FurnitureModelLoadRequest {
    pub item_id: u32,
    pub item_name: String,
    pub kind: FurnitureModelKind,
    pub model_key: u16,
}

#[cfg(feature = "game-data")]
impl FurnitureModelLoadRequest {
    pub fn sgb_path(&self) -> String {
        furniture_sgb_path(self.kind, self.model_key)
    }
}

#[cfg(feature = "game-data")]
impl From<&FurnitureCatalogItem> for FurnitureModelLoadRequest {
    fn from(item: &FurnitureCatalogItem) -> Self {
        Self {
            item_id: item.id,
            item_name: item.name.clone(),
            kind: item.kind,
            model_key: item.model_key,
        }
    }
}

/// 家具模型加载结果复用武器的结果结构。`model_main` 打包 housing ModelKey
/// （model_id = ModelKey，其余段为 0），`model_sub` 恒为 None，染色固定关闭；
/// SGB 引用的全部 MDL 按顺序合并进 `meshes`，渲染时 component 按 MDL 序号区分
/// （见 [`weapon_model_mesh_component_index`]）。
#[cfg(feature = "game-data")]
pub type FurnitureModelData = WeaponModelData;

/// 递归提取一个 housing SGB 及其关联 SGB 引用的全部资源。`models` 按提取
/// 顺序去重，`scanned_sgbs` 记录实际读取的 SGB（含根），scanned set 防止
/// 关联 SGB 成环导致死循环。根 SGB 读不到时报错；关联 SGB 读不到时按上游
/// 语义（`FileExists` 检查）静默跳过。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FurnitureSgbAssets {
    models: Vec<String>,
    /// SGB 引用的其余文件（.mtrl/.tex 等），供材质候选按文件名匹配。
    files: Vec<String>,
    scanned_sgbs: Vec<String>,
}

#[cfg(feature = "game-data")]
async fn collect_furniture_sgb_assets_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    root_sgb_path: &str,
) -> Result<FurnitureSgbAssets, String> {
    let mut assets = FurnitureSgbAssets::default();
    let mut scanned = HashSet::new();
    let mut pending = vec![root_sgb_path.to_string()];
    let mut is_root = true;
    while let Some(sgb_path) = pending.pop() {
        if !scanned.insert(sgb_path.clone()) {
            continue;
        }
        let bytes = match resource.read(&sgb_path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                if is_root {
                    return Err(format!("failed to read {sgb_path}: {error}"));
                }
                continue;
            }
        };
        is_root = false;
        let extracted = extract_sgb_asset_paths(&bytes);
        assets.scanned_sgbs.push(sgb_path);
        for model in extracted.models {
            push_unique_path(&mut assets.models, model);
        }
        for file in extracted.others {
            push_unique_path(&mut assets.files, file);
        }
        for related in extracted.related_sgbs.into_iter().rev() {
            pending.push(related);
        }
    }
    Ok(assets)
}

#[cfg(feature = "game-data")]
fn collect_furniture_sgb_assets_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    root_sgb_path: &str,
) -> Result<FurnitureSgbAssets, String> {
    let mut assets = FurnitureSgbAssets::default();
    let mut scanned = HashSet::new();
    let mut pending = vec![root_sgb_path.to_string()];
    let mut is_root = true;
    while let Some(sgb_path) = pending.pop() {
        if !scanned.insert(sgb_path.clone()) {
            continue;
        }
        let Some(bytes) = resource.read(&sgb_path) else {
            if is_root {
                return Err(format!(
                    "failed to read {sgb_path}: resource read returned no bytes"
                ));
            }
            continue;
        };
        is_root = false;
        let extracted = extract_sgb_asset_paths(&bytes);
        assets.scanned_sgbs.push(sgb_path);
        for model in extracted.models {
            push_unique_path(&mut assets.models, model);
        }
        for file in extracted.others {
            push_unique_path(&mut assets.files, file);
        }
        for related in extracted.related_sgbs.into_iter().rev() {
            pending.push(related);
        }
    }
    Ok(assets)
}

/// 家具模型加载：读 SGB → 递归收集全部 MDL → 逐个加载合并网格（首个 MDL
/// 失败报错，其余失败记为 Secondary 诊断，对齐武器 main/sub 处理）→ 材质优先
/// 按 SGB 引用文件列表匹配，缺失回退 MDL 目录推导 → 纹理走通用候选推导。
#[cfg(feature = "game-data")]
pub async fn load_furniture_model_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &FurnitureModelLoadRequest,
) -> anyhow::Result<FurnitureModelData> {
    let sgb_path = request.sgb_path();
    let sgb_assets = collect_furniture_sgb_assets_from_async_resource(resource, &sgb_path)
        .await
        .map_err(|error| anyhow::anyhow!("failed to collect housing assets: {error}"))?;

    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = sgb_assets.scanned_sgbs.clone();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    if sgb_assets.models.is_empty() {
        return Err(anyhow::anyhow!(
            "{} has no renderable model meshes (SGB {sgb_path} references no MDL)",
            request.item_name
        ));
    }
    let sgb_files: Rc<[String]> = sgb_assets.files.into();
    let mut is_primary = true;
    for model_path in &sgb_assets.models {
        let context = ModelPathContext::Furniture(FurnitureModelPathContext {
            model_key: request.model_key,
            model_path: model_path.clone(),
            sgb_files: sgb_files.clone(),
        });
        let result = load_model_meshes_from_async_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await;
        if is_primary {
            result.map_err(WeaponModelMeshLoadFailure::into_error)?;
            is_primary = false;
        } else if let Err(failure) = result {
            load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
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
        model_main: PackedModelId::from_raw(request.model_key as u64),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics,
        loaded_paths,
        bounds: calculate_model_bounds(&meshes),
        materials,
        textures,
        meshes,
    })
}

/// [`load_furniture_model_from_async_resource`] 的同步 Resource 版本，对齐
/// [`load_equipment_model_from_resource`]。
#[cfg(feature = "game-data")]
pub fn load_furniture_model_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &FurnitureModelLoadRequest,
) -> anyhow::Result<FurnitureModelData> {
    let sgb_path = request.sgb_path();
    let sgb_assets = collect_furniture_sgb_assets_from_resource(resource, &sgb_path)
        .map_err(|error| anyhow::anyhow!("failed to collect housing assets: {error}"))?;

    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = sgb_assets.scanned_sgbs.clone();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    if sgb_assets.models.is_empty() {
        return Err(anyhow::anyhow!(
            "{} has no renderable model meshes (SGB {sgb_path} references no MDL)",
            request.item_name
        ));
    }
    let sgb_files: Rc<[String]> = sgb_assets.files.into();
    let mut is_primary = true;
    for model_path in &sgb_assets.models {
        let context = ModelPathContext::Furniture(FurnitureModelPathContext {
            model_key: request.model_key,
            model_path: model_path.clone(),
            sgb_files: sgb_files.clone(),
        });
        let result = load_model_meshes_from_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        );
        if is_primary {
            result.map_err(WeaponModelMeshLoadFailure::into_error)?;
            is_primary = false;
        } else if let Err(failure) = result {
            load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
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
        model_main: PackedModelId::from_raw(request.model_key as u64),
        model_sub: None,
        stain_ids: [0, 0],
        load_diagnostics,
        loaded_paths,
        bounds: calculate_model_bounds(&meshes),
        materials,
        textures,
        meshes,
    })
}

/// 宠物/坐骑模型加载请求。`model` 是 ModelChara 三元组（目录条目直接携带），
/// `kind` 仅用于展示与诊断；monster 单 MDL 必须读到，demihuman 逐槽位探测
/// 合并所有存在的 MDL。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharaModelLoadRequest {
    pub item_id: u32,
    pub item_name: String,
    pub kind: CharaModelKind,
    pub model: PackedCharaModelId,
}

#[cfg(feature = "game-data")]
impl From<&CharaCatalogItem> for CharaModelLoadRequest {
    fn from(item: &CharaCatalogItem) -> Self {
        Self {
            item_id: item.id,
            item_name: item.name.clone(),
            kind: item.kind,
            model: item.model,
        }
    }
}

/// 宠物/坐骑模型加载结果复用武器的结果结构。`model_main` 打包 ModelChara
/// 三元组（model/base/variant 对应武器三段布局），`model_sub` 恒为 None，
/// 染色固定关闭；demihuman 的全部槽位 MDL 按顺序合并进 `meshes`，渲染时
/// component 按 MDL 序号区分（见 [`weapon_model_mesh_component_index`]）。
#[cfg(feature = "game-data")]
pub type CharaModelData = WeaponModelData;

/// demihuman 槽位探测时的“文件不存在”失败：全部候选都是读不到（Missing/
/// ReadError），与 MDL 存在但解析失败（ParseError，记诊断）区分。
#[cfg(feature = "game-data")]
fn chara_probe_failure_is_absent(failure: &WeaponModelMeshLoadFailure) -> bool {
    failure.candidates.iter().all(|candidate| {
        matches!(
            candidate.status,
            WeaponModelLoadCandidateStatus::Missing | WeaponModelLoadCandidateStatus::ReadError
        )
    })
}

/// 宠物/坐骑模型加载：monster 单 MDL 必须成功；demihuman 按槽位候选逐个
/// 探测，读不到的槽位静默跳过，解析失败记 Secondary 诊断，全部槽位 MDL
/// 合并网格；材质按 ModelChara 三元组多版本回退，纹理走通用候选推导。
#[cfg(feature = "game-data")]
pub async fn load_chara_model_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &CharaModelLoadRequest,
) -> anyhow::Result<CharaModelData> {
    load_chara_model_with_skeleton_from_async_resource(resource, request)
        .await
        .map(|(data, _skeleton)| data)
}

/// [`load_chara_model_with_skeleton_from_resource`] 的异步 Resource 版本：
/// 随模型返回 rest pose 骨架（sklb 缺失/解析失败 → `None` + 诊断日志）。
#[cfg(feature = "game-data")]
pub async fn load_chara_model_with_skeleton_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &CharaModelLoadRequest,
) -> anyhow::Result<(CharaModelData, Option<ModelSkeleton>)> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    let model_paths = chara_model_candidate_paths(request.model);
    let single_model = model_paths.len() == 1;
    for model_path in &model_paths {
        let context = ModelPathContext::Chara(CharaModelPathContext {
            model: request.model,
            model_path: model_path.clone(),
        });
        let result = load_model_meshes_from_async_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await;
        match (single_model, result) {
            (true, Err(failure)) => return Err(failure.into_error()),
            (false, Err(failure)) => {
                if !chara_probe_failure_is_absent(&failure) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
            (_, Ok(())) => {}
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

    let skeleton = load_optional_skeleton_from_async_resource(
        resource,
        &skeleton_path_for_chara_model(request.model),
        &request.item_name,
    )
    .await;

    Ok((
        WeaponModelData {
            item_id: request.item_id,
            item_name: request.item_name.clone(),
            model_main: chara_diagnostic_model(request.model),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics,
            loaded_paths,
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        },
        skeleton,
    ))
}

/// [`load_chara_model_from_async_resource`] 的同步 Resource 版本，对齐
/// [`load_furniture_model_from_resource`]。
#[cfg(feature = "game-data")]
pub fn load_chara_model_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &CharaModelLoadRequest,
) -> anyhow::Result<CharaModelData> {
    load_chara_model_with_skeleton_from_resource(resource, request).map(|(data, _skeleton)| data)
}

/// [`load_chara_model_from_resource`] 的骨架并行版：加载模型后顺手读取对应
/// sklb（monster/demihuman 按模型 id 推导路径）构建 rest pose 骨架一并返回。
/// sklb 缺失或解析失败返回 `None` 并输出诊断，不视为加载错误（静态预览可降级
/// 为非蒙皮渲染）。骨架不进 serde/IndexedDB，仅随本次加载内存存活。
#[cfg(feature = "game-data")]
pub fn load_chara_model_with_skeleton_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &CharaModelLoadRequest,
) -> anyhow::Result<(CharaModelData, Option<ModelSkeleton>)> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    let model_paths = chara_model_candidate_paths(request.model);
    let single_model = model_paths.len() == 1;
    for model_path in &model_paths {
        let context = ModelPathContext::Chara(CharaModelPathContext {
            model: request.model,
            model_path: model_path.clone(),
        });
        let result = load_model_meshes_from_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        );
        match (single_model, result) {
            (true, Err(failure)) => return Err(failure.into_error()),
            (false, Err(failure)) => {
                if !chara_probe_failure_is_absent(&failure) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
            (_, Ok(())) => {}
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

    let skeleton = load_optional_skeleton_from_resource(
        resource,
        &skeleton_path_for_chara_model(request.model),
        &request.item_name,
    );

    Ok((
        WeaponModelData {
            item_id: request.item_id,
            item_name: request.item_name.clone(),
            model_main: chara_diagnostic_model(request.model),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics,
            loaded_paths,
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        },
        skeleton,
    ))
}

/// 读取 sklb 并解析为 rest pose 骨架；文件缺失/解析失败输出诊断并返回 None。
/// vendored havok 解析的 panic 已在 `load_skeleton_from_sklb_bytes` 内转为 Err。
#[cfg(feature = "game-data")]
fn load_optional_skeleton_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    path: &str,
    owner: &str,
) -> Option<ModelSkeleton> {
    let Some(bytes) = resource.read(path) else {
        eprintln!("skeleton unavailable for {owner}: sklb missing at {path}");
        return None;
    };
    match load_skeleton_from_sklb_bytes(&bytes) {
        Ok(skeleton) => Some(skeleton),
        Err(error) => {
            eprintln!("skeleton unavailable for {owner}: {path}: {error}");
            None
        }
    }
}

/// [`load_optional_skeleton_from_resource`] 的异步版本：读不到（缺失或读取
/// 失败）同样静默降级为 None（动画/蒙皮是 best-effort 增强）。
#[cfg(feature = "game-data")]
async fn load_optional_skeleton_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    path: &str,
    owner: &str,
) -> Option<ModelSkeleton> {
    let bytes = match resource.read(path).await {
        Ok(bytes) => bytes,
        Err(_) => {
            eprintln!("skeleton unavailable for {owner}: sklb missing at {path}");
            return None;
        }
    };
    match load_skeleton_from_sklb_bytes(&bytes) {
        Ok(skeleton) => Some(skeleton),
        Err(error) => {
            eprintln!("skeleton unavailable for {owner}: {path}: {error}");
            None
        }
    }
}

/// 由 human.cmp 尾部缩放参数表换算捏脸字节对应的体型缩放（RGSP）。表缺失/
/// 解析失败返回 None（回退恒等，与无缩放一致）；恒等缩放同样返回 None
/// （调用方不附着，骨架保持 body_scaling=None 的未缩放语义）。
#[cfg(feature = "game-data")]
fn body_scaling_from_cmp_bytes(
    bytes: Option<&[u8]>,
    customize: &CharacterCustomize,
) -> Option<BodyScaling> {
    let table = RacialScalingTable::from_cmp_bytes(bytes?)
        .map_err(|error| eprintln!("body scaling unavailable: {HUMAN_CMP_PATH}: {error}"))
        .ok()?;
    let scaling = table.body_scaling(customize);
    (!scaling.is_identity()).then_some(scaling)
}

/// 读取 human.cmp 并把体型缩放附着到骨架（`ModelSkeleton::body_scaling`）。
/// human.cmp 缺失/解析失败或缩放为恒等时不附着（回退恒等，不影响渲染）。
#[cfg(feature = "game-data")]
fn attach_body_scaling_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    customize: &CharacterCustomize,
    skeleton: &mut ModelSkeleton,
) {
    let bytes = resource.read(HUMAN_CMP_PATH);
    skeleton.body_scaling = body_scaling_from_cmp_bytes(bytes.as_deref(), customize);
}

/// [`attach_body_scaling_from_resource`] 的异步版本。
#[cfg(feature = "game-data")]
async fn attach_body_scaling_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    customize: &CharacterCustomize,
    skeleton: &mut ModelSkeleton,
) {
    let bytes = resource.read(HUMAN_CMP_PATH).await.ok();
    skeleton.body_scaling = body_scaling_from_cmp_bytes(bytes.as_deref(), customize);
}

/// 角色拼装加载请求。`customize` 决定全部部件路径与材质候选（身体 5 槽 +
/// 脸 + 发 + 种族可选尾/兔耳）；`name` 仅用于展示与诊断；`appearance` 提供时
/// 在材质合成阶段把角色级颜色写入对应部件材质（按 shader family 落地，
/// 见 `ModelMaterial::character_colors`），不提供则保持未染色语义。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterAssemblyLoadRequest {
    pub customize: CharacterCustomize,
    pub name: String,
    pub appearance: Option<CharacterAppearanceColors>,
}

#[cfg(feature = "game-data")]
impl CharacterAssemblyLoadRequest {
    pub fn new(customize: CharacterCustomize, name: impl Into<String>) -> Self {
        Self {
            customize,
            name: name.into(),
            appearance: None,
        }
    }

    pub fn with_appearance(mut self, appearance: CharacterAppearanceColors) -> Self {
        self.appearance = Some(appearance);
        self
    }
}

/// 角色拼装结果复用武器的结果结构（对齐装备/家具/宠物坐骑的别名模式）：
/// `model_main` 打包 race code（c 编码，如 101 = 中原人男），`model_sub` 恒为
/// None（无主次模型，所有部件拼在原位），染色固定关闭；小衣 top/dwn（+sho）+
/// 裸肤 glv（+sho）+ 脸 + 发 + 尾/兔耳的全部 MDL 按部件顺序合并进 `meshes`。
/// attribute 显隐按名判定（位是各 MDL 本地表序），启用集合见
/// [`crate::chara_assemble::character_enabled_attribute_names`]；可用选项见
/// [`crate::chara_assemble::character_assembly_attribute_options`]。
#[cfg(feature = "game-data")]
pub type CharacterAssemblyData = WeaponModelData;

/// 角色拼装：按部件请求逐个加载合并。小衣 top/dwn、裸肤 glv、脸、发为必需
/// 部件，失败即整体失败；sho/尾/兔耳为可选部件，文件缺失静默跳过，解析失败
/// 记 Secondary 诊断。材质按 human 域特例推导（皮肤肤族根/脸与兔耳无版本
/// 目录/头发共享根），纹理走通用候选推导。
#[cfg(feature = "game-data")]
pub fn load_character_assembly_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &CharacterAssemblyLoadRequest,
) -> anyhow::Result<CharacterAssemblyData> {
    load_character_assembly_with_skeleton_from_resource(resource, request)
        .map(|(data, _skeleton)| data)
}

/// [`load_character_assembly_from_resource`] 的骨架并行版：部件合并后读取角色
/// race code（`customize.race_code()`）对应 sklb 构建 rest pose 骨架一并返回。
/// sklb 缺失或解析失败返回 `None` 并输出诊断，不视为加载错误。
#[cfg(feature = "game-data")]
pub fn load_character_assembly_with_skeleton_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &CharacterAssemblyLoadRequest,
) -> anyhow::Result<(CharacterAssemblyData, Option<ModelSkeleton>)> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    for part in character_part_paths(&request.customize) {
        let mut model_candidates = vec![part.model_path.clone()];
        model_candidates.extend(part.alternate_model_paths.clone());
        let context = ModelPathContext::Character(CharacterModelPathContext {
            customize: request.customize,
            part: part.kind,
            model_candidates,
        });
        let materials_before = materials.len();
        let result = load_model_meshes_from_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        );
        match (part.required, result) {
            (true, Err(failure)) => return Err(failure.into_error()),
            (false, Err(failure)) => {
                if !chara_probe_failure_is_absent(&failure) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
            (_, Ok(())) => {
                if let Some(appearance) = &request.appearance {
                    let decal_texture = load_character_decal_texture_from_resource(
                        resource,
                        request.customize.face_paint,
                        part.kind,
                        &mut textures,
                        &mut loaded_paths,
                    );
                    apply_character_appearance_to_materials(
                        &mut materials[materials_before..],
                        appearance,
                        decal_texture,
                    );
                }
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
            request.name
        ));
    }

    let skeleton = load_optional_skeleton_from_resource(
        resource,
        &character_skeleton_path(request.customize.race_code()),
        &request.name,
    )
    .map(|mut skeleton| {
        attach_body_scaling_from_resource(resource, &request.customize, &mut skeleton);
        skeleton
    });
    if let Some(target) = &skeleton {
        bake_assembly_race_deforms_from_resource(
            resource,
            &mut meshes,
            request.customize.race_code(),
            target,
            &request.name,
        );
    }

    Ok((
        WeaponModelData {
            item_id: 0,
            item_name: request.name.clone(),
            model_main: PackedModelId::from_raw(u64::from(request.customize.race_code())),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics,
            loaded_paths,
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        },
        skeleton,
    ))
}

/// 角色装配的种族骨变形烘焙：回退文件（mesh.path 的 race code ≠ 角色自身）
/// 的网格从回退族骨架 rest 烘焙到自身骨架 rest——离线近似游戏的 PBD 骨变形
/// （维埃拉/硌狮女小衣来自中原女文件、裸肤手来自中原/鲁加/拉拉男文件等）。
/// 源族骨架缺失时跳过该族（保持回退族比例，与未烘焙一致），不作为加载错误。
#[cfg(feature = "game-data")]
fn bake_assembly_race_deforms_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    meshes: &mut Vec<crate::model::ModelMesh>,
    race_code: u16,
    target_skeleton: &ModelSkeleton,
    owner: &str,
) {
    let mut meshes_by_source_race: HashMap<u16, Vec<usize>> = HashMap::new();
    for (index, mesh) in meshes.iter().enumerate() {
        let Some(source_race) = race_code_from_character_model_path(&mesh.path) else {
            continue;
        };
        if source_race != race_code {
            meshes_by_source_race
                .entry(source_race)
                .or_default()
                .push(index);
        }
    }
    for (source_race, indices) in meshes_by_source_race {
        let source_path = character_skeleton_path(source_race);
        let Some(source_skeleton) =
            load_optional_skeleton_from_resource(resource, &source_path, owner)
        else {
            eprintln!(
                "race deform skipped for {owner}: {indices:?} meshes from c{source_race:04} (skeleton unavailable)"
            );
            continue;
        };
        let deform = RaceDeform::new(&source_skeleton, target_skeleton);
        for index in indices {
            let skipped = bake_race_deform(&mut meshes[index], &source_skeleton, &deform);
            if skipped > 0 {
                eprintln!(
                    "race deform for {owner}: {} keeps {} unskinned vertices (c{source_race:04})",
                    meshes[index].path, skipped
                );
            }
        }
    }
    close_bare_limb_junctions(meshes, race_code);
    snap_bare_hand_cuff_to_forearm(meshes, race_code);
}

/// [`load_character_assembly_from_resource`] 的异步 Resource 版本，对齐
/// [`load_chara_model_from_async_resource`]。
#[cfg(feature = "game-data")]
pub async fn load_character_assembly_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &CharacterAssemblyLoadRequest,
) -> anyhow::Result<CharacterAssemblyData> {
    load_character_assembly_with_skeleton_from_async_resource(resource, request)
        .await
        .map(|(data, _skeleton)| data)
}

/// [`load_character_assembly_with_skeleton_from_resource`] 的异步 Resource
/// 版本：随装配返回 race code 对应 sklb 的 rest pose 骨架（缺失 → None）。
#[cfg(feature = "game-data")]
pub async fn load_character_assembly_with_skeleton_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &CharacterAssemblyLoadRequest,
) -> anyhow::Result<(CharacterAssemblyData, Option<ModelSkeleton>)> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    for part in character_part_paths(&request.customize) {
        let mut model_candidates = vec![part.model_path.clone()];
        model_candidates.extend(part.alternate_model_paths.clone());
        let context = ModelPathContext::Character(CharacterModelPathContext {
            customize: request.customize,
            part: part.kind,
            model_candidates,
        });
        let materials_before = materials.len();
        let result = load_model_meshes_from_async_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await;
        match (part.required, result) {
            (true, Err(failure)) => return Err(failure.into_error()),
            (false, Err(failure)) => {
                if !chara_probe_failure_is_absent(&failure) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
            (_, Ok(())) => {
                if let Some(appearance) = &request.appearance {
                    let decal_texture = load_character_decal_texture_from_async_resource(
                        resource,
                        request.customize.face_paint,
                        part.kind,
                        &mut textures,
                        &mut loaded_paths,
                    )
                    .await;
                    apply_character_appearance_to_materials(
                        &mut materials[materials_before..],
                        appearance,
                        decal_texture,
                    );
                }
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
            request.name
        ));
    }

    let skeleton = load_optional_skeleton_from_async_resource(
        resource,
        &character_skeleton_path(request.customize.race_code()),
        &request.name,
    )
    .await;
    let skeleton = match skeleton {
        Some(mut skeleton) => {
            attach_body_scaling_from_async_resource(resource, &request.customize, &mut skeleton)
                .await;
            Some(skeleton)
        }
        None => None,
    };
    if let Some(target) = &skeleton {
        bake_assembly_race_deforms_from_async_resource(
            resource,
            &mut meshes,
            request.customize.race_code(),
            target,
            &request.name,
        )
        .await;
    }

    Ok((
        WeaponModelData {
            item_id: 0,
            item_name: request.name.clone(),
            model_main: PackedModelId::from_raw(u64::from(request.customize.race_code())),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics,
            loaded_paths,
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        },
        skeleton,
    ))
}

/// [`bake_assembly_race_deforms_from_resource`] 的异步版本。
#[cfg(feature = "game-data")]
async fn bake_assembly_race_deforms_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    meshes: &mut Vec<crate::model::ModelMesh>,
    race_code: u16,
    target_skeleton: &ModelSkeleton,
    owner: &str,
) {
    let mut meshes_by_source_race: HashMap<u16, Vec<usize>> = HashMap::new();
    for (index, mesh) in meshes.iter().enumerate() {
        let Some(source_race) = race_code_from_character_model_path(&mesh.path) else {
            continue;
        };
        if source_race != race_code {
            meshes_by_source_race
                .entry(source_race)
                .or_default()
                .push(index);
        }
    }
    for (source_race, indices) in meshes_by_source_race {
        let source_path = character_skeleton_path(source_race);
        let Some(source_skeleton) =
            load_optional_skeleton_from_async_resource(resource, &source_path, owner).await
        else {
            eprintln!(
                "race deform skipped for {owner}: {indices:?} meshes from c{source_race:04} (skeleton unavailable)"
            );
            continue;
        };
        let deform = RaceDeform::new(&source_skeleton, target_skeleton);
        for index in indices {
            let skipped = bake_race_deform(&mut meshes[index], &source_skeleton, &deform);
            if skipped > 0 {
                eprintln!(
                    "race deform for {owner}: {} keeps {} unskinned vertices (c{source_race:04})",
                    meshes[index].path, skipped
                );
            }
        }
    }
    close_bare_limb_junctions(meshes, race_code);
    snap_bare_hand_cuff_to_forearm(meshes, race_code);
}

/// 面妆 decal 贴图加载：按 [`face_paint_decal_texture_candidates`] 降序尝试，
/// 命中即解码为 BaseColor 纹理追加到模型纹理列表；无候选/全部缺失时返回 None
/// （调用方按缺失降级，不报错）。仅脸部件携带面妆（`face_paint & 0x7F != 0`）。
#[cfg(feature = "game-data")]
fn load_character_decal_texture_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    face_paint: u8,
    part: CharacterPartKind,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    use physis::ReadableFile;

    if !matches!(part, CharacterPartKind::Face) || face_paint & 0x7F == 0 {
        return None;
    }
    for path in face_paint_decal_texture_candidates(face_paint) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
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
            kind: WeaponModelTextureKind::BaseColor,
            texel_layout: ModelTextureTexelLayout::Standard,
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
async fn load_character_decal_texture_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    face_paint: u8,
    part: CharacterPartKind,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> Option<usize> {
    use physis::ReadableFile;

    if !matches!(part, CharacterPartKind::Face) || face_paint & 0x7F == 0 {
        return None;
    }
    for path in face_paint_decal_texture_candidates(face_paint) {
        if let Some(index) = textures.iter().position(|texture| texture.path == path) {
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
            kind: WeaponModelTextureKind::BaseColor,
            texel_layout: ModelTextureTexelLayout::Standard,
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

/// 把角色级颜色写入单个部件新增的材质（材质合成阶段的落地）。数据侧只按
/// shader family 判断"该材质是否消费角色色"（skin 全收；hair 收发色/挑染；
/// iris 收眼色；charactertattoo 收特征色），各通道的具体消费方式由渲染侧
/// 按 family/GetMaterialValue 决定；其余 family（装备 character 等）不写入。
#[cfg(feature = "game-data")]
fn apply_character_appearance_to_materials(
    materials: &mut [WeaponModelMaterial],
    appearance: &CharacterAppearanceColors,
    decal_texture: Option<usize>,
) {
    for material in materials.iter_mut() {
        let consumes = matches!(
            material_shader_family(material.shader_package_name.as_deref()),
            MaterialShaderFamily::Skin
                | MaterialShaderFamily::Hair
                | MaterialShaderFamily::Iris
                | MaterialShaderFamily::CharacterTattoo
        );
        if !consumes {
            continue;
        }
        material.character_colors = Some(ModelMaterialCharacterColors {
            colors: *appearance,
            decal_texture,
        });
    }
}

/// 从本地游戏目录加载角色拼装，对齐 [`load_weapon_model_from_game_dir`]。
#[cfg(feature = "game-data")]
pub fn load_character_assembly_from_game_dir(
    game_dir: &std::path::Path,
    request: &CharacterAssemblyLoadRequest,
) -> anyhow::Result<CharacterAssemblyData> {
    use anyhow::{Context, anyhow};

    let game_dir = normalize_game_dir(game_dir)?;
    let game_dir = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = physis::resource::SqPackResource::from_existing(game_dir);
    load_character_assembly_from_resource(&mut resource, request)
        .with_context(|| format!("failed to load character assembly for {}", request.name))
}

/// 着装角色的单件装备请求。字段语义同 [`EquipmentModelLoadRequest`]（无
/// `race_id`：装备模型种族恒取角色自身 race code，[race, 101] 候选回退不变）；
/// `stain_ids` 双通道染色在加载时落地到该件的材质切片（材质仍保留未染色
/// 基准色表，运行时改色走 [`apply_dressed_piece_stains`]）。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DressedEquipmentPiece {
    pub item_id: u32,
    pub item_name: String,
    pub model_main: u64,
    pub model_sub: u64,
    pub equip_slot_category: u32,
    pub stain_ids: [u8; 2],
}

/// 着装角色加载请求：裸装身体（`customize`/`appearance` 语义同
/// [`CharacterAssemblyLoadRequest`]）+ 装备件列表。每槽位至多一件（由调用方
/// 保证）；加载按槽位排序，传入顺序不影响结果。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq)]
pub struct DressedCharacterLoadRequest {
    pub customize: CharacterCustomize,
    pub name: String,
    pub appearance: Option<CharacterAppearanceColors>,
    pub equipment: Vec<DressedEquipmentPiece>,
}

#[cfg(feature = "game-data")]
impl DressedCharacterLoadRequest {
    pub fn new(customize: CharacterCustomize, name: impl Into<String>) -> Self {
        Self {
            customize,
            name: name.into(),
            appearance: None,
            equipment: Vec::new(),
        }
    }

    pub fn with_appearance(mut self, appearance: CharacterAppearanceColors) -> Self {
        self.appearance = Some(appearance);
        self
    }

    pub fn with_equipment(mut self, equipment: Vec<DressedEquipmentPiece>) -> Self {
        self.equipment = equipment;
        self
    }
}

/// 单件装备在着装结果里的材质区间（`model.materials` 切片），增量染色
/// （[`apply_dressed_piece_stains`]）按它定位。
#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquipmentMaterialRange {
    pub item_id: u32,
    pub equip_slot_category: u32,
    pub material_start: usize,
    pub material_end: usize,
}

/// 着装角色加载结果：合并模型复用角色拼装结果结构（`model_main` 打包 race
/// code、`model_sub` 恒 None、模型级 `stain_ids` 恒 `[0, 0]`——每件装备的
/// 染色已落地到各自材质切片）。`equipment_material_ranges` 按槽位序；
/// `hidden_body_attributes` 记录被装备遮蔽的身体/头发 attribute（诊断用，
/// 条目格式 `{部位}:{细节}`）。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DressedCharacterData {
    pub model: CharacterAssemblyData,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub equipment_material_ranges: Vec<EquipmentMaterialRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_body_attributes: Vec<String>,
}

/// 逐件着装场景的单件装备：该件独立网格/材质/纹理（种族骨变形已烘焙到请求
/// 种族），附带遮蔽判定所需的 IMC/EQP 解析结果。`stain_ids` 传 `[0, 0]` 时
/// 结果对（model_main, model_sub, 槽位, race）恒定，可做件级缓存；染色经
/// [`apply_weapon_model_stains`] 在缓存副本上增量落地。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
pub struct DressedPieceModel {
    pub item_id: u32,
    pub item_name: String,
    pub equip_slot_category: u32,
    pub is_accessory: bool,
    pub stain_ids: [u8; 2],
    /// 该件 IMC 条目 attribute 可见性位（该件 MDL 本地表序）；缺失 → None
    /// （全显示）。
    pub imc_mask: Option<u16>,
    /// 该件套装 EQP 条目（饰品恒 None；表/块缺失 → None，按不遮蔽降级）。
    pub eqp: Option<EquipmentParameterEntry>,
    /// 武器挂接信息（仅武器件 Some）：该件网格已烘焙为挂点骨单骨蒙皮，
    /// 渲染/动画驱动按 [`crate::weapon_attach_joint_matrices`] 出关节矩阵
    /// （姿势世界 × 校正），不走 inverse bind。
    pub attach: Option<WeaponAttachInfo>,
    /// 该件模型（Rc 共享：免染基准可跨染色组合复用，染色副本由调用方经
    /// [`apply_weapon_model_stains`] 另建）。
    pub model: std::rc::Rc<WeaponModelData>,
}

/// 主手武器挂点骨（人体骨架；`n_buki_tate_r/l` 是收纳位，不用）。
#[cfg(feature = "game-data")]
pub const WEAPON_ATTACH_BONE_MAIN_HAND: &str = "n_buki_r";
/// 副手武器挂点骨（盾/副手工具）。
#[cfg(feature = "game-data")]
pub const WEAPON_ATTACH_BONE_OFF_HAND: &str = "n_buki_l";

/// 武器挂接的常量校正（武器模型局部 → 挂点骨局部）。武器 MDL 是原点在握
/// 把、轴向对齐挂点骨局部轴的刚性模型，校正为恒等（native 快照目验确认
/// 剑/盾/大剑在 rest 手下落位正确，见 tests/native_dressed_character.rs）。
#[cfg(feature = "game-data")]
pub const WEAPON_ATTACH_CORRECTION: [f32; 16] = crate::skeleton::IDENTITY_MAT4;

/// 武器挂接规则：该件作为刚性模型挂到人体骨架的武器挂点骨上。每帧关节
/// 矩阵 = 挂点骨姿势世界矩阵 × `correction`（无 inverse bind——武器顶点
/// 在挂点骨局部空间而非绑定空间）。
#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponAttachInfo {
    pub correction: [f32; 16],
}

#[cfg(feature = "game-data")]
impl Default for WeaponAttachInfo {
    fn default() -> Self {
        Self {
            correction: WEAPON_ATTACH_CORRECTION,
        }
    }
}

/// 逐件着装场景：裸装身体（含外观色）+ 各装备件独立模型 + race 骨架。与
/// 合并版（[`DressedCharacterData`]）的差异：网格/材质不跨件合并（件级缓存
/// 与逐件 GPU 实例重建的前提），遮蔽不在加载期落地——由
/// [`plan_dressed_concealment`] 按当前件组合计算隐藏标签。身体管线
/// （部件循环/外观色/decal/种族变形烘焙）与合并版逐字节一致。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
pub struct DressedCharacterScene {
    pub name: String,
    pub body: WeaponModelData,
    /// 按槽位序（与请求排序一致）。
    pub pieces: Vec<DressedPieceModel>,
    pub skeleton: Option<ModelSkeleton>,
    pub load_diagnostics: Vec<WeaponModelLoadDiagnostic>,
    pub loaded_paths: Vec<String>,
}

/// 逐件遮蔽计划：对当前件组合（身体 + 各件）的隐藏标签，模型数据本体不动，
/// 创建 GPU 实例时经 [`crate::PreparedModelOptions`] 落地——整网格隐藏用
/// `hidden_mesh_indices`，submesh 级隐藏用按名启用集合减去隐藏名。
/// 语义与合并版加载期过滤（[`DressedCharacterData`] 的网格删除）一致；
/// `hidden_notes` 条目格式同 `hidden_body_attributes`。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DressedConcealmentPlan {
    /// 身体整网格隐藏（`body.meshes` 下标）。
    pub body_hidden_meshes: Vec<usize>,
    /// 身体隐藏的 attribute 名（从 `character_enabled_attribute_names` 结果中
    /// 去除；含 top/dwn 皮肤遮蔽位、头发 atr_top、脸部耳/角 attribute）。
    pub body_hidden_attributes: Vec<String>,
    /// 件最终启用的 attribute 名（(equip_slot_category, item_id) → 名单；IMC
    /// 变体位命名 + 跨件规则。双耳环等同类多件按 item_id 区分）。含
    /// attribute submesh 的件必须传给渲染选项；不含的件名单为空、渲染层忽略。
    pub piece_enabled_attributes: Vec<((u32, u32), Vec<String>)>,
    /// 整件隐藏的件（(equip_slot_category, item_id)；当前仅耳饰件的头部
    /// EQP 耳饰位门控）。渲染层跳过该件的实例创建。
    pub hidden_pieces: Vec<(u32, u32)>,
    /// 遮蔽诊断（`{部位}:{细节}`）。
    pub hidden_notes: Vec<String>,
}

/// 单件已加载装备的合并记录：网格/材质区间 + 该件的 IMC/EQP 解析结果。
#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
struct DressedPieceLoad {
    item_id: u32,
    equip_slot_category: u32,
    is_accessory: bool,
    mesh_start: usize,
    mesh_end: usize,
    material_start: usize,
    material_end: usize,
    /// IMC 条目 attribute 可见性位（该件 MDL 本地表序）；缺失 → None（全显示）。
    imc_mask: Option<u16>,
    /// 套装 EQP 条目（饰品无 EQP；表/块缺失 → None，按不遮蔽降级）。
    eqp: Option<EquipmentParameterEntry>,
}

/// 装备参数文件（IMC/EQP）读取失败的 Secondary 诊断（不阻断装配）。
#[cfg(feature = "game-data")]
fn equipment_params_diagnostic(
    model: PackedModelId,
    path: String,
    status: WeaponModelLoadCandidateStatus,
    error: impl Into<String>,
) -> WeaponModelLoadDiagnostic {
    let candidate = model_load_candidate(path, status, error.into());
    WeaponModelLoadDiagnostic {
        role: WeaponModelLoadRole::Secondary,
        model,
        error: format!(
            "unable to read equipment parameter file (tried: {}: {})",
            candidate.path, candidate.error
        ),
        candidates: vec![candidate],
    }
}

/// 读取 EQP 表（全装配一次）。文件缺失 → None + Secondary 诊断，调用方按
/// 空表降级（不遮蔽任何身体网格）。
#[cfg(feature = "game-data")]
fn load_equipment_parameter_table_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    loaded_paths: &mut Vec<String>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
) -> Option<EquipmentParameterTable> {
    let Some(bytes) = resource.read(EQUIPMENT_PARAMETER_PATH) else {
        load_diagnostics.push(equipment_params_diagnostic(
            PackedModelId::from_raw(0),
            EQUIPMENT_PARAMETER_PATH.to_string(),
            WeaponModelLoadCandidateStatus::Missing,
            "resource read returned no bytes",
        ));
        return None;
    };
    push_loaded_path(loaded_paths, EQUIPMENT_PARAMETER_PATH.to_string());
    Some(EquipmentParameterTable::from_bytes(&bytes))
}

/// [`load_equipment_parameter_table_from_resource`] 的异步版本。
#[cfg(feature = "game-data")]
async fn load_equipment_parameter_table_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    loaded_paths: &mut Vec<String>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
) -> Option<EquipmentParameterTable> {
    let bytes = match resource.read(EQUIPMENT_PARAMETER_PATH).await {
        Ok(bytes) => bytes,
        Err(error) => {
            load_diagnostics.push(equipment_params_diagnostic(
                PackedModelId::from_raw(0),
                EQUIPMENT_PARAMETER_PATH.to_string(),
                WeaponModelLoadCandidateStatus::ReadError,
                error.to_string(),
            ));
            return None;
        }
    };
    push_loaded_path(loaded_paths, EQUIPMENT_PARAMETER_PATH.to_string());
    Some(EquipmentParameterTable::from_bytes(&bytes))
}

/// 读取并解析装备/饰品 set 的 IMC（按 set 缓存），返回该模型（variant 子集
/// × 槽位）的条目。文件缺失/解析失败 → None + Secondary 诊断（该件按全显示
/// 降级、材质版本回退 variant 猜测）。
#[cfg(feature = "game-data")]
fn load_imc_entry_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    cache: &mut HashMap<(bool, u16), Option<Rc<ImcFile>>>,
    model: PackedEquipmentModelId,
    slot: EquipmentSlotInfo,
    loaded_paths: &mut Vec<String>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
) -> Option<ImcEntry> {
    let key = (slot.is_accessory, model.set_id);
    if !cache.contains_key(&key) {
        let path = equipment_imc_path(model.set_id, slot.is_accessory);
        let parsed = match resource.read(&path) {
            Some(bytes) => match ImcFile::from_bytes(&bytes) {
                Ok(file) => {
                    push_loaded_path(loaded_paths, path);
                    Some(Rc::new(file))
                }
                Err(error) => {
                    load_diagnostics.push(equipment_params_diagnostic(
                        PackedModelId::from_raw(model.raw),
                        path,
                        WeaponModelLoadCandidateStatus::ParseError,
                        format!("{error:#}"),
                    ));
                    None
                }
            },
            None => {
                load_diagnostics.push(equipment_params_diagnostic(
                    PackedModelId::from_raw(model.raw),
                    path,
                    WeaponModelLoadCandidateStatus::Missing,
                    "resource read returned no bytes",
                ));
                None
            }
        };
        cache.insert(key, parsed);
    }
    imc_entry_from_cache(&cache, key, model, slot)
}

#[cfg(feature = "game-data")]
fn imc_entry_from_cache(
    cache: &HashMap<(bool, u16), Option<Rc<ImcFile>>>,
    key: (bool, u16),
    model: PackedEquipmentModelId,
    slot: EquipmentSlotInfo,
) -> Option<ImcEntry> {
    let slot_offset = imc_slot_offset(slot.abbreviation).unwrap_or(0);
    cache
        .get(&key)
        .and_then(|cached| cached.as_ref())
        .map(|file| *file.entry(model.variant_id, slot_offset))
}

/// [`load_imc_entry_from_resource`] 的异步版本。
#[cfg(feature = "game-data")]
async fn load_imc_entry_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    cache: &mut HashMap<(bool, u16), Option<Rc<ImcFile>>>,
    model: PackedEquipmentModelId,
    slot: EquipmentSlotInfo,
    loaded_paths: &mut Vec<String>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
) -> Option<ImcEntry> {
    let key = (slot.is_accessory, model.set_id);
    if !cache.contains_key(&key) {
        let path = equipment_imc_path(model.set_id, slot.is_accessory);
        let parsed = match resource.read(&path).await {
            Ok(bytes) => match ImcFile::from_bytes(&bytes) {
                Ok(file) => {
                    push_loaded_path(loaded_paths, path);
                    Some(Rc::new(file))
                }
                Err(error) => {
                    load_diagnostics.push(equipment_params_diagnostic(
                        PackedModelId::from_raw(model.raw),
                        path,
                        WeaponModelLoadCandidateStatus::ParseError,
                        format!("{error:#}"),
                    ));
                    None
                }
            },
            Err(error) => {
                load_diagnostics.push(equipment_params_diagnostic(
                    PackedModelId::from_raw(model.raw),
                    path,
                    WeaponModelLoadCandidateStatus::ReadError,
                    error.to_string(),
                ));
                None
            }
        };
        cache.insert(key, parsed);
    }
    imc_entry_from_cache(&cache, key, model, slot)
}

/// 共享染色模板按件克隆并覆写 stain_ids（模板本体 stain-agnostic，全装配
/// 只读取一次 STM 文件）。
#[cfg(feature = "game-data")]
fn staining_templates_for_piece(
    shared: &WeaponStainingTemplates,
    stain_ids: [u8; 2],
) -> WeaponStainingTemplates {
    WeaponStainingTemplates {
        stain_ids: normalize_stain_ids(stain_ids),
        legacy: shared.legacy.clone(),
        dawntrail: shared.dawntrail.clone(),
    }
}

/// [`load_dressed_character_from_resource`] 的骨架并行版：部件合并后读取角色
/// race code 对应 sklb 构建 rest pose 骨架一并返回（缺失 → None，同角色装配）。
#[cfg(feature = "game-data")]
pub fn load_dressed_character_with_skeleton_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<(DressedCharacterData, Option<ModelSkeleton>)> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    // 身体部件循环与裸装装配一致（禁用染色 + per-part 外观切片写入）。
    for part in character_part_paths(&request.customize) {
        let mut model_candidates = vec![part.model_path.clone()];
        model_candidates.extend(part.alternate_model_paths.clone());
        let context = ModelPathContext::Character(CharacterModelPathContext {
            customize: request.customize,
            part: part.kind,
            model_candidates,
        });
        let materials_before = materials.len();
        let result = load_model_meshes_from_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        );
        match (part.required, result) {
            (true, Err(failure)) => return Err(failure.into_error()),
            (false, Err(failure)) => {
                if !chara_probe_failure_is_absent(&failure) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
            (_, Ok(())) => {
                if let Some(appearance) = &request.appearance {
                    let decal_texture = load_character_decal_texture_from_resource(
                        resource,
                        request.customize.face_paint,
                        part.kind,
                        &mut textures,
                        &mut loaded_paths,
                    );
                    apply_character_appearance_to_materials(
                        &mut materials[materials_before..],
                        appearance,
                        decal_texture,
                    );
                }
            }
        }
    }
    let body_mesh_count = meshes.len();

    // 任一装备件染色才读取 STM 模板（全装配一次）；EQP 表缺失按空表降级。
    let any_stains = request
        .equipment
        .iter()
        .any(|piece| piece.stain_ids.iter().any(|stain_id| *stain_id != 0));
    let equipment_staining = if any_stains {
        load_weapon_staining_templates_from_resource(resource, [1, 0], &mut loaded_paths)
    } else {
        WeaponStainingTemplates::disabled([0, 0])
    };
    let eqp_table = load_equipment_parameter_table_from_resource(
        resource,
        &mut loaded_paths,
        &mut load_diagnostics,
    );
    let mut imc_cache = HashMap::new();

    let mut pieces: Vec<&DressedEquipmentPiece> = request.equipment.iter().collect();
    pieces.sort_by_key(|piece| piece.equip_slot_category);
    let mut loaded_pieces = Vec::new();
    for piece in pieces {
        let Some(slot) = equipment_slot_info(piece.equip_slot_category) else {
            load_diagnostics.push(WeaponModelLoadDiagnostic {
                role: WeaponModelLoadRole::Secondary,
                model: PackedModelId::from_raw(piece.model_main),
                candidates: Vec::new(),
                error: format!(
                    "equip slot category {} has no equipment model",
                    piece.equip_slot_category
                ),
            });
            continue;
        };
        let model_main = PackedEquipmentModelId::from_raw(piece.model_main);
        if model_main.set_id == 0 {
            continue;
        }
        let imc_entry = load_imc_entry_from_resource(
            resource,
            &mut imc_cache,
            model_main,
            slot,
            &mut loaded_paths,
            &mut load_diagnostics,
        );
        let material_version = imc_entry
            .map(|entry| u16::from(entry.material_set))
            .filter(|version| *version > 0);
        let piece_staining = staining_templates_for_piece(&equipment_staining, piece.stain_ids);
        let materials_before = materials.len();
        let meshes_before = meshes.len();
        let result = load_model_meshes_from_resource(
            resource,
            ModelPathContext::Equipment(EquipmentModelPathContext {
                model: model_main,
                slot,
                race_id: request.customize.race_code(),
                material_version,
            }),
            &piece_staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        );
        if let Err(failure) = result {
            load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
            continue;
        }
        if let Some(model_sub) =
            (piece.model_sub != 0).then(|| PackedEquipmentModelId::from_raw(piece.model_sub))
        {
            if model_sub.raw != model_main.raw {
                // 次模型按自身 set 解析 IMC（材质版本），失败记 Secondary 诊断。
                let sub_version = load_imc_entry_from_resource(
                    resource,
                    &mut imc_cache,
                    model_sub,
                    slot,
                    &mut loaded_paths,
                    &mut load_diagnostics,
                )
                .map(|entry| u16::from(entry.material_set))
                .filter(|version| *version > 0);
                if let Err(failure) = load_model_meshes_from_resource(
                    resource,
                    ModelPathContext::Equipment(EquipmentModelPathContext {
                        model: model_sub,
                        slot,
                        race_id: request.customize.race_code(),
                        material_version: sub_version,
                    }),
                    &piece_staining,
                    &mut loaded_paths,
                    &mut materials,
                    &mut textures,
                    &mut meshes,
                    &mut color_table_sources,
                ) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
        }
        loaded_pieces.push(DressedPieceLoad {
            item_id: piece.item_id,
            equip_slot_category: piece.equip_slot_category,
            is_accessory: slot.is_accessory,
            mesh_start: meshes_before,
            mesh_end: meshes.len(),
            material_start: materials_before,
            material_end: materials.len(),
            imc_mask: imc_entry.map(|entry| entry.attribute_mask()),
            eqp: if slot.is_accessory {
                None
            } else {
                eqp_table
                    .as_ref()
                    .and_then(|table| table.entry(model_main.set_id))
            },
        });
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
            request.name
        ));
    }

    let skeleton = load_optional_skeleton_from_resource(
        resource,
        &character_skeleton_path(request.customize.race_code()),
        &request.name,
    )
    .map(|mut skeleton| {
        attach_body_scaling_from_resource(resource, &request.customize, &mut skeleton);
        skeleton
    });
    if let Some(target) = &skeleton {
        bake_assembly_race_deforms_from_resource(
            resource,
            &mut meshes,
            request.customize.race_code(),
            target,
            &request.name,
        );
    }

    // 可见性按网格过滤落地（骨变形之后、包围盒之前）。
    let hidden_body_attributes = apply_dressed_visibility(
        &mut meshes,
        body_mesh_count,
        &loaded_pieces,
        &request.customize,
    );
    let equipment_material_ranges = loaded_pieces
        .iter()
        .map(|piece| EquipmentMaterialRange {
            item_id: piece.item_id,
            equip_slot_category: piece.equip_slot_category,
            material_start: piece.material_start,
            material_end: piece.material_end,
        })
        .collect();

    Ok((
        DressedCharacterData {
            model: WeaponModelData {
                item_id: 0,
                item_name: request.name.clone(),
                model_main: PackedModelId::from_raw(u64::from(request.customize.race_code())),
                model_sub: None,
                stain_ids: [0, 0],
                load_diagnostics,
                loaded_paths,
                bounds: calculate_model_bounds(&meshes),
                materials,
                textures,
                meshes,
            },
            equipment_material_ranges,
            hidden_body_attributes,
        },
        skeleton,
    ))
}

/// 着装角色装配：裸装身体 + 装备件逐件合并（共享累加器，同角色装配的部件
/// 循环）。装备件按槽位排序加载；单件失败记 Secondary 诊断并跳过（不阻断
/// 整体）。可见性按网格过滤落地（合并后 attribute 名在装备/身体间全局冲突，
/// 不能用按名启用表）：装备件按自身 IMC 条目 attribute 位裁剪变体子网格，
/// 被覆盖槽位的身体网格按各套装 EQP 条目隐藏。
#[cfg(feature = "game-data")]
pub fn load_dressed_character_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<DressedCharacterData> {
    load_dressed_character_with_skeleton_from_resource(resource, request)
        .map(|(data, _skeleton)| data)
}

/// [`load_dressed_character_from_resource`] 的异步 Resource 版本。
#[cfg(feature = "game-data")]
pub async fn load_dressed_character_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<DressedCharacterData> {
    load_dressed_character_with_skeleton_from_async_resource(resource, request)
        .await
        .map(|(data, _skeleton)| data)
}

/// [`load_dressed_character_with_skeleton_from_resource`] 的异步 Resource
/// 版本：随装配返回 race code 对应 sklb 的 rest pose 骨架（缺失 → None）。
#[cfg(feature = "game-data")]
pub async fn load_dressed_character_with_skeleton_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<(DressedCharacterData, Option<ModelSkeleton>)> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let staining = WeaponStainingTemplates::disabled([0, 0]);

    for part in character_part_paths(&request.customize) {
        let mut model_candidates = vec![part.model_path.clone()];
        model_candidates.extend(part.alternate_model_paths.clone());
        let context = ModelPathContext::Character(CharacterModelPathContext {
            customize: request.customize,
            part: part.kind,
            model_candidates,
        });
        let materials_before = materials.len();
        let result = load_model_meshes_from_async_resource(
            resource,
            context,
            &staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await;
        match (part.required, result) {
            (true, Err(failure)) => return Err(failure.into_error()),
            (false, Err(failure)) => {
                if !chara_probe_failure_is_absent(&failure) {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
            (_, Ok(())) => {
                if let Some(appearance) = &request.appearance {
                    let decal_texture = load_character_decal_texture_from_async_resource(
                        resource,
                        request.customize.face_paint,
                        part.kind,
                        &mut textures,
                        &mut loaded_paths,
                    )
                    .await;
                    apply_character_appearance_to_materials(
                        &mut materials[materials_before..],
                        appearance,
                        decal_texture,
                    );
                }
            }
        }
    }
    let body_mesh_count = meshes.len();

    let any_stains = request
        .equipment
        .iter()
        .any(|piece| piece.stain_ids.iter().any(|stain_id| *stain_id != 0));
    let equipment_staining = if any_stains {
        load_weapon_staining_templates_from_async_resource(resource, [1, 0], &mut loaded_paths)
            .await
    } else {
        WeaponStainingTemplates::disabled([0, 0])
    };
    let eqp_table = load_equipment_parameter_table_from_async_resource(
        resource,
        &mut loaded_paths,
        &mut load_diagnostics,
    )
    .await;
    let mut imc_cache = HashMap::new();

    let mut pieces: Vec<&DressedEquipmentPiece> = request.equipment.iter().collect();
    pieces.sort_by_key(|piece| piece.equip_slot_category);
    let mut loaded_pieces = Vec::new();
    for piece in pieces {
        let Some(slot) = equipment_slot_info(piece.equip_slot_category) else {
            load_diagnostics.push(WeaponModelLoadDiagnostic {
                role: WeaponModelLoadRole::Secondary,
                model: PackedModelId::from_raw(piece.model_main),
                candidates: Vec::new(),
                error: format!(
                    "equip slot category {} has no equipment model",
                    piece.equip_slot_category
                ),
            });
            continue;
        };
        let model_main = PackedEquipmentModelId::from_raw(piece.model_main);
        if model_main.set_id == 0 {
            continue;
        }
        let imc_entry = load_imc_entry_from_async_resource(
            resource,
            &mut imc_cache,
            model_main,
            slot,
            &mut loaded_paths,
            &mut load_diagnostics,
        )
        .await;
        let material_version = imc_entry
            .map(|entry| u16::from(entry.material_set))
            .filter(|version| *version > 0);
        let piece_staining = staining_templates_for_piece(&equipment_staining, piece.stain_ids);
        let materials_before = materials.len();
        let meshes_before = meshes.len();
        let result = load_model_meshes_from_async_resource(
            resource,
            ModelPathContext::Equipment(EquipmentModelPathContext {
                model: model_main,
                slot,
                race_id: request.customize.race_code(),
                material_version,
            }),
            &piece_staining,
            &mut loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await;
        if let Err(failure) = result {
            load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
            continue;
        }
        if let Some(model_sub) =
            (piece.model_sub != 0).then(|| PackedEquipmentModelId::from_raw(piece.model_sub))
        {
            if model_sub.raw != model_main.raw {
                let sub_version = load_imc_entry_from_async_resource(
                    resource,
                    &mut imc_cache,
                    model_sub,
                    slot,
                    &mut loaded_paths,
                    &mut load_diagnostics,
                )
                .await
                .map(|entry| u16::from(entry.material_set))
                .filter(|version| *version > 0);
                if let Err(failure) = load_model_meshes_from_async_resource(
                    resource,
                    ModelPathContext::Equipment(EquipmentModelPathContext {
                        model: model_sub,
                        slot,
                        race_id: request.customize.race_code(),
                        material_version: sub_version,
                    }),
                    &piece_staining,
                    &mut loaded_paths,
                    &mut materials,
                    &mut textures,
                    &mut meshes,
                    &mut color_table_sources,
                )
                .await
                {
                    load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
                }
            }
        }
        loaded_pieces.push(DressedPieceLoad {
            item_id: piece.item_id,
            equip_slot_category: piece.equip_slot_category,
            is_accessory: slot.is_accessory,
            mesh_start: meshes_before,
            mesh_end: meshes.len(),
            material_start: materials_before,
            material_end: materials.len(),
            imc_mask: imc_entry.map(|entry| entry.attribute_mask()),
            eqp: if slot.is_accessory {
                None
            } else {
                eqp_table
                    .as_ref()
                    .and_then(|table| table.entry(model_main.set_id))
            },
        });
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
            request.name
        ));
    }

    let skeleton = load_optional_skeleton_from_async_resource(
        resource,
        &character_skeleton_path(request.customize.race_code()),
        &request.name,
    )
    .await;
    let skeleton = match skeleton {
        Some(mut skeleton) => {
            attach_body_scaling_from_async_resource(resource, &request.customize, &mut skeleton)
                .await;
            Some(skeleton)
        }
        None => None,
    };
    if let Some(target) = &skeleton {
        bake_assembly_race_deforms_from_async_resource(
            resource,
            &mut meshes,
            request.customize.race_code(),
            target,
            &request.name,
        )
        .await;
    }

    let hidden_body_attributes = apply_dressed_visibility(
        &mut meshes,
        body_mesh_count,
        &loaded_pieces,
        &request.customize,
    );
    let equipment_material_ranges = loaded_pieces
        .iter()
        .map(|piece| EquipmentMaterialRange {
            item_id: piece.item_id,
            equip_slot_category: piece.equip_slot_category,
            material_start: piece.material_start,
            material_end: piece.material_end,
        })
        .collect();

    Ok((
        DressedCharacterData {
            model: WeaponModelData {
                item_id: 0,
                item_name: request.name.clone(),
                model_main: PackedModelId::from_raw(u64::from(request.customize.race_code())),
                model_sub: None,
                stain_ids: [0, 0],
                load_diagnostics,
                loaded_paths,
                bounds: calculate_model_bounds(&meshes),
                materials,
                textures,
                meshes,
            },
            equipment_material_ranges,
            hidden_body_attributes,
        },
        skeleton,
    ))
}

/// 从本地游戏目录加载着装角色，对齐 [`load_character_assembly_from_game_dir`]。
#[cfg(feature = "game-data")]
pub fn load_dressed_character_from_game_dir(
    game_dir: &std::path::Path,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<DressedCharacterData> {
    use anyhow::{Context, anyhow};

    let game_dir = normalize_game_dir(game_dir)?;
    let game_dir = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = physis::resource::SqPackResource::from_existing(game_dir);
    load_dressed_character_from_resource(&mut resource, request)
        .with_context(|| format!("failed to load dressed character for {}", request.name))
}

/// 逐件加载的共享上下文（场景与单件加载路径共用）。
#[cfg(feature = "game-data")]
struct DressedPieceLoadContext<'a> {
    race_code: u16,
    skeleton: Option<&'a ModelSkeleton>,
    equipment_staining: &'a WeaponStainingTemplates,
    eqp_table: Option<&'a EquipmentParameterTable>,
}

/// 武器网格的单骨挂接烘焙：bone table 覆写为 `[挂点骨]`，全部顶点 blend
/// 强制关节 0、权重 1.0。武器 MDL 是刚性独立模型（原点在握把、顶点在挂点
/// 骨局部空间），烘焙后实例 joint 表 = `[挂点骨]`，joint 矩阵按
/// [`crate::weapon_attach_joint_matrices`]（姿势世界 × 校正）驱动。
/// 返回内部可动的网格数（有顶点蒙皮到多于一根武器骨——单骨化后其内部
/// 动画丢失，罕见如部分书的内页；调用方记诊断）。
#[cfg(feature = "game-data")]
fn bake_weapon_attach(meshes: &mut [WeaponModelMesh], bone_name: &str) -> usize {
    let mut articulated = 0;
    for mesh in meshes {
        let skinned_to_multiple_bones = mesh.vertices.iter().any(|vertex| {
            let (Some(weights), Some(indices)) = (vertex.blend_weights, vertex.blend_indices)
            else {
                return false;
            };
            let count = usize::from(weights.count.min(indices.count).min(8));
            (1..count)
                .any(|slot| weights.values[slot] > 0.0 && indices.values[slot] != indices.values[0])
        });
        if skinned_to_multiple_bones {
            articulated += 1;
        }
        mesh.bone_table = Some(ModelBoneTable {
            index: 0,
            bone_count: 1,
            bone_indices: vec![0],
            bone_names: vec![Some(bone_name.to_string())],
        });
        for vertex in &mut mesh.vertices {
            vertex.blend_indices = Some(ModelBlendIndices {
                count: 1,
                values: [0; 8],
            });
            vertex.blend_weights = Some(ModelBlendWeights {
                count: 1,
                values: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            });
        }
    }
    articulated
}

/// 加载逐件着装场景的一件武器（主手槽位 1/13/14、副手槽位 2）：走武器路
/// 径（[`ModelPathContext::Weapon`]，与武器预览页同链），网格经
/// [`bake_weapon_attach`] 烘焙为挂点骨单骨蒙皮——主手件挂
/// [`WEAPON_ATTACH_BONE_MAIN_HAND`]，副手件（盾等）挂
/// [`WEAPON_ATTACH_BONE_OFF_HAND`]；次模型（model_sub 与主模型不同：成对
/// 副武器/刀鞘等）挂 [`WEAPON_ATTACH_BONE_OFF_HAND`]。武器是刚性模型
/// （顶点在挂点骨局部空间），不做种族骨变形烘焙。单件失败记 Secondary
/// 诊断并返回 None（不阻断整体）。
#[cfg(feature = "game-data")]
fn load_dressed_weapon_piece_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    piece: &DressedEquipmentPiece,
    context: &DressedPieceLoadContext<'_>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
    loaded_paths: &mut Vec<String>,
) -> Option<DressedPieceModel> {
    let model_main = PackedModelId::from_raw(piece.model_main);
    if model_main.model_id == 0 {
        return None;
    }
    let piece_staining = staining_templates_for_piece(context.equipment_staining, piece.stain_ids);
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let mut articulated_meshes = 0_usize;

    let main_bone = if piece.equip_slot_category == 2 {
        WEAPON_ATTACH_BONE_OFF_HAND
    } else {
        WEAPON_ATTACH_BONE_MAIN_HAND
    };
    if let Err(failure) = load_model_meshes_from_resource(
        resource,
        ModelPathContext::Weapon(model_main),
        &piece_staining,
        loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    ) {
        load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
        return None;
    }
    articulated_meshes += bake_weapon_attach(&mut meshes, main_bone);

    let model_sub = (piece.model_sub != 0)
        .then(|| PackedModelId::from_raw(piece.model_sub))
        .filter(|model_sub| {
            model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw
        });
    if let Some(model_sub) = model_sub {
        let sub_start = meshes.len();
        match load_model_meshes_from_resource(
            resource,
            ModelPathContext::Weapon(model_sub),
            &piece_staining,
            loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        ) {
            Ok(()) => {
                articulated_meshes +=
                    bake_weapon_attach(&mut meshes[sub_start..], WEAPON_ATTACH_BONE_OFF_HAND);
            }
            Err(failure) => {
                load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
            }
        }
    }

    attach_shared_material_arrays_from_resource(
        resource,
        &mut materials,
        &mut textures,
        loaded_paths,
    );

    if meshes.is_empty() {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: model_main,
            candidates: Vec::new(),
            error: format!("{} has no renderable model meshes", piece.item_name),
        });
        return None;
    }
    if articulated_meshes > 0 {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: model_main,
            candidates: Vec::new(),
            error: format!(
                "{}: {articulated_meshes} mesh(es) skinned to multiple weapon bones; single-joint attach drops their internal articulation",
                piece.item_name
            ),
        });
    }

    Some(DressedPieceModel {
        item_id: piece.item_id,
        item_name: piece.item_name.clone(),
        equip_slot_category: piece.equip_slot_category,
        is_accessory: false,
        stain_ids: piece.stain_ids,
        imc_mask: None,
        eqp: None,
        attach: Some(WeaponAttachInfo::default()),
        model: Rc::new(WeaponModelData {
            item_id: piece.item_id,
            item_name: piece.item_name.clone(),
            model_main,
            model_sub,
            stain_ids: normalize_stain_ids(piece.stain_ids),
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        }),
    })
}

/// [`load_dressed_weapon_piece_from_resource`] 的异步 Resource 版本。
#[cfg(feature = "game-data")]
async fn load_dressed_weapon_piece_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    piece: &DressedEquipmentPiece,
    context: &DressedPieceLoadContext<'_>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
    loaded_paths: &mut Vec<String>,
) -> Option<DressedPieceModel> {
    let model_main = PackedModelId::from_raw(piece.model_main);
    if model_main.model_id == 0 {
        return None;
    }
    let piece_staining = staining_templates_for_piece(context.equipment_staining, piece.stain_ids);
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let mut articulated_meshes = 0_usize;

    let main_bone = if piece.equip_slot_category == 2 {
        WEAPON_ATTACH_BONE_OFF_HAND
    } else {
        WEAPON_ATTACH_BONE_MAIN_HAND
    };
    if let Err(failure) = load_model_meshes_from_async_resource(
        resource,
        ModelPathContext::Weapon(model_main),
        &piece_staining,
        loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    )
    .await
    {
        load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
        return None;
    }
    articulated_meshes += bake_weapon_attach(&mut meshes, main_bone);

    let model_sub = (piece.model_sub != 0)
        .then(|| PackedModelId::from_raw(piece.model_sub))
        .filter(|model_sub| {
            model_sub.model_id != model_main.model_id || model_sub.raw != model_main.raw
        });
    if let Some(model_sub) = model_sub {
        let sub_start = meshes.len();
        match load_model_meshes_from_async_resource(
            resource,
            ModelPathContext::Weapon(model_sub),
            &piece_staining,
            loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await
        {
            Ok(()) => {
                articulated_meshes +=
                    bake_weapon_attach(&mut meshes[sub_start..], WEAPON_ATTACH_BONE_OFF_HAND);
            }
            Err(failure) => {
                load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
            }
        }
    }

    attach_shared_material_arrays_from_async_resource(
        resource,
        &mut materials,
        &mut textures,
        loaded_paths,
    )
    .await;

    if meshes.is_empty() {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: model_main,
            candidates: Vec::new(),
            error: format!("{} has no renderable model meshes", piece.item_name),
        });
        return None;
    }
    if articulated_meshes > 0 {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: model_main,
            candidates: Vec::new(),
            error: format!(
                "{}: {articulated_meshes} mesh(es) skinned to multiple weapon bones; single-joint attach drops their internal articulation",
                piece.item_name
            ),
        });
    }

    Some(DressedPieceModel {
        item_id: piece.item_id,
        item_name: piece.item_name.clone(),
        equip_slot_category: piece.equip_slot_category,
        is_accessory: false,
        stain_ids: piece.stain_ids,
        imc_mask: None,
        eqp: None,
        attach: Some(WeaponAttachInfo::default()),
        model: Rc::new(WeaponModelData {
            item_id: piece.item_id,
            item_name: piece.item_name.clone(),
            model_main,
            model_sub,
            stain_ids: normalize_stain_ids(piece.stain_ids),
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        }),
    })
}

/// 加载逐件着装场景的一件装备：独立网格/材质/纹理（IMC 材质版本解析、
/// 主/副模型、共享材质数组、种族骨变形烘焙到 `skeleton`）。单件失败记
/// Secondary 诊断并返回 None（不阻断整体）。网格/材质语义与合并版该件
/// 区间逐字节一致（烘焙目标骨架相同）。武器槽位（1/13/14/2）走
/// [`load_dressed_weapon_piece_from_resource`]（挂点骨单骨蒙皮）。
#[cfg(feature = "game-data")]
fn load_dressed_piece_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    piece: &DressedEquipmentPiece,
    context: &DressedPieceLoadContext<'_>,
    imc_cache: &mut HashMap<(bool, u16), Option<Rc<ImcFile>>>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
    loaded_paths: &mut Vec<String>,
) -> Option<DressedPieceModel> {
    if is_weapon_equip_slot_category(piece.equip_slot_category) {
        return load_dressed_weapon_piece_from_resource(
            resource,
            piece,
            context,
            load_diagnostics,
            loaded_paths,
        );
    }
    let Some(slot) = equipment_slot_info(piece.equip_slot_category) else {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: PackedModelId::from_raw(piece.model_main),
            candidates: Vec::new(),
            error: format!(
                "equip slot category {} has no equipment model",
                piece.equip_slot_category
            ),
        });
        return None;
    };
    let model_main = PackedEquipmentModelId::from_raw(piece.model_main);
    if model_main.set_id == 0 {
        return None;
    }
    let imc_entry = load_imc_entry_from_resource(
        resource,
        imc_cache,
        model_main,
        slot,
        loaded_paths,
        load_diagnostics,
    );
    let material_version = imc_entry
        .map(|entry| u16::from(entry.material_set))
        .filter(|version| *version > 0);
    let piece_staining = staining_templates_for_piece(context.equipment_staining, piece.stain_ids);
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let result = load_model_meshes_from_resource(
        resource,
        ModelPathContext::Equipment(EquipmentModelPathContext {
            model: model_main,
            slot,
            race_id: context.race_code,
            material_version,
        }),
        &piece_staining,
        loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    );
    if let Err(failure) = result {
        load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
        return None;
    }
    if let Some(model_sub) =
        (piece.model_sub != 0).then(|| PackedEquipmentModelId::from_raw(piece.model_sub))
        && model_sub.raw != model_main.raw
    {
        // 次模型按自身 set 解析 IMC（材质版本），失败记 Secondary 诊断。
        let sub_version = load_imc_entry_from_resource(
            resource,
            imc_cache,
            model_sub,
            slot,
            loaded_paths,
            load_diagnostics,
        )
        .map(|entry| u16::from(entry.material_set))
        .filter(|version| *version > 0);
        if let Err(failure) = load_model_meshes_from_resource(
            resource,
            ModelPathContext::Equipment(EquipmentModelPathContext {
                model: model_sub,
                slot,
                race_id: context.race_code,
                material_version: sub_version,
            }),
            &piece_staining,
            loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        ) {
            load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
        }
    }

    attach_shared_material_arrays_from_resource(
        resource,
        &mut materials,
        &mut textures,
        loaded_paths,
    );

    if meshes.is_empty() {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: PackedModelId::from_raw(model_main.raw),
            candidates: Vec::new(),
            error: format!("{} has no renderable model meshes", piece.item_name),
        });
        return None;
    }

    if let Some(target) = context.skeleton {
        bake_assembly_race_deforms_from_resource(
            resource,
            &mut meshes,
            context.race_code,
            target,
            &piece.item_name,
        );
    }

    Some(DressedPieceModel {
        item_id: piece.item_id,
        item_name: piece.item_name.clone(),
        equip_slot_category: piece.equip_slot_category,
        is_accessory: slot.is_accessory,
        stain_ids: piece.stain_ids,
        imc_mask: imc_entry.map(|entry| entry.attribute_mask()),
        attach: None,
        eqp: if slot.is_accessory {
            None
        } else {
            context
                .eqp_table
                .and_then(|table| table.entry(model_main.set_id))
        },
        model: std::rc::Rc::new(WeaponModelData {
            item_id: piece.item_id,
            item_name: piece.item_name.clone(),
            model_main: PackedModelId::from_raw(model_main.raw),
            model_sub: (piece.model_sub != 0).then(|| PackedModelId::from_raw(piece.model_sub)),
            stain_ids: piece.stain_ids,
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        }),
    })
}

/// [`load_dressed_piece_from_resource`] 的异步 Resource 版本。
#[cfg(feature = "game-data")]
async fn load_dressed_piece_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    piece: &DressedEquipmentPiece,
    context: &DressedPieceLoadContext<'_>,
    imc_cache: &mut HashMap<(bool, u16), Option<Rc<ImcFile>>>,
    load_diagnostics: &mut Vec<WeaponModelLoadDiagnostic>,
    loaded_paths: &mut Vec<String>,
) -> Option<DressedPieceModel> {
    if is_weapon_equip_slot_category(piece.equip_slot_category) {
        return load_dressed_weapon_piece_from_async_resource(
            resource,
            piece,
            context,
            load_diagnostics,
            loaded_paths,
        )
        .await;
    }
    let Some(slot) = equipment_slot_info(piece.equip_slot_category) else {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: PackedModelId::from_raw(piece.model_main),
            candidates: Vec::new(),
            error: format!(
                "equip slot category {} has no equipment model",
                piece.equip_slot_category
            ),
        });
        return None;
    };
    let model_main = PackedEquipmentModelId::from_raw(piece.model_main);
    if model_main.set_id == 0 {
        return None;
    }
    let imc_entry = load_imc_entry_from_async_resource(
        resource,
        imc_cache,
        model_main,
        slot,
        loaded_paths,
        load_diagnostics,
    )
    .await;
    let material_version = imc_entry
        .map(|entry| u16::from(entry.material_set))
        .filter(|version| *version > 0);
    let piece_staining = staining_templates_for_piece(context.equipment_staining, piece.stain_ids);
    let mut materials = Vec::new();
    let mut textures = Vec::new();
    let mut meshes = Vec::new();
    let mut color_table_sources = HashMap::new();
    let result = load_model_meshes_from_async_resource(
        resource,
        ModelPathContext::Equipment(EquipmentModelPathContext {
            model: model_main,
            slot,
            race_id: context.race_code,
            material_version,
        }),
        &piece_staining,
        loaded_paths,
        &mut materials,
        &mut textures,
        &mut meshes,
        &mut color_table_sources,
    )
    .await;
    if let Err(failure) = result {
        load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
        return None;
    }
    if let Some(model_sub) =
        (piece.model_sub != 0).then(|| PackedEquipmentModelId::from_raw(piece.model_sub))
        && model_sub.raw != model_main.raw
    {
        let sub_version = load_imc_entry_from_async_resource(
            resource,
            imc_cache,
            model_sub,
            slot,
            loaded_paths,
            load_diagnostics,
        )
        .await
        .map(|entry| u16::from(entry.material_set))
        .filter(|version| *version > 0);
        if let Err(failure) = load_model_meshes_from_async_resource(
            resource,
            ModelPathContext::Equipment(EquipmentModelPathContext {
                model: model_sub,
                slot,
                race_id: context.race_code,
                material_version: sub_version,
            }),
            &piece_staining,
            loaded_paths,
            &mut materials,
            &mut textures,
            &mut meshes,
            &mut color_table_sources,
        )
        .await
        {
            load_diagnostics.push(failure.into_diagnostic(WeaponModelLoadRole::Secondary));
        }
    }

    attach_shared_material_arrays_from_async_resource(
        resource,
        &mut materials,
        &mut textures,
        loaded_paths,
    )
    .await;

    if meshes.is_empty() {
        load_diagnostics.push(WeaponModelLoadDiagnostic {
            role: WeaponModelLoadRole::Secondary,
            model: PackedModelId::from_raw(model_main.raw),
            candidates: Vec::new(),
            error: format!("{} has no renderable model meshes", piece.item_name),
        });
        return None;
    }

    if let Some(target) = context.skeleton {
        bake_assembly_race_deforms_from_async_resource(
            resource,
            &mut meshes,
            context.race_code,
            target,
            &piece.item_name,
        )
        .await;
    }

    Some(DressedPieceModel {
        item_id: piece.item_id,
        item_name: piece.item_name.clone(),
        equip_slot_category: piece.equip_slot_category,
        is_accessory: slot.is_accessory,
        stain_ids: piece.stain_ids,
        imc_mask: imc_entry.map(|entry| entry.attribute_mask()),
        attach: None,
        eqp: if slot.is_accessory {
            None
        } else {
            context
                .eqp_table
                .and_then(|table| table.entry(model_main.set_id))
        },
        model: std::rc::Rc::new(WeaponModelData {
            item_id: piece.item_id,
            item_name: piece.item_name.clone(),
            model_main: PackedModelId::from_raw(model_main.raw),
            model_sub: (piece.model_sub != 0).then(|| PackedModelId::from_raw(piece.model_sub)),
            stain_ids: piece.stain_ids,
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: calculate_model_bounds(&meshes),
            materials,
            textures,
            meshes,
        }),
    })
}

/// 加载逐件着装场景：身体走角色装配管线（与合并版逐字节一致），装备逐件
/// 独立加载（按槽位序），遮蔽不在加载期落地（调用方按
/// [`plan_dressed_concealment`] 组装隐藏标签）。件染色为 `[0, 0]` 时各件
/// 结果可跨染色组合复用（件级缓存），染色经 [`apply_weapon_model_stains`]
/// 增量落地。
#[cfg(feature = "game-data")]
pub fn load_dressed_character_scene_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<DressedCharacterScene> {
    let mut body_request =
        CharacterAssemblyLoadRequest::new(request.customize, request.name.clone());
    if let Some(appearance) = &request.appearance {
        body_request = body_request.with_appearance(*appearance);
    }
    let (body, skeleton) =
        load_character_assembly_with_skeleton_from_resource(resource, &body_request)?;
    let mut load_diagnostics = body.load_diagnostics.clone();
    let mut loaded_paths = body.loaded_paths.clone();

    let any_stains = request
        .equipment
        .iter()
        .any(|piece| piece.stain_ids.iter().any(|stain_id| *stain_id != 0));
    let equipment_staining = if any_stains {
        load_weapon_staining_templates_from_resource(resource, [1, 0], &mut loaded_paths)
    } else {
        WeaponStainingTemplates::disabled([0, 0])
    };
    let eqp_table = load_equipment_parameter_table_from_resource(
        resource,
        &mut loaded_paths,
        &mut load_diagnostics,
    );
    let mut imc_cache = HashMap::new();

    let mut pieces: Vec<&DressedEquipmentPiece> = request.equipment.iter().collect();
    pieces.sort_by_key(|piece| piece.equip_slot_category);
    let mut piece_models = Vec::new();
    for piece in pieces {
        let context = DressedPieceLoadContext {
            race_code: request.customize.race_code(),
            skeleton: skeleton.as_ref(),
            equipment_staining: &equipment_staining,
            eqp_table: eqp_table.as_ref(),
        };
        if let Some(loaded) = load_dressed_piece_from_resource(
            resource,
            piece,
            &context,
            &mut imc_cache,
            &mut load_diagnostics,
            &mut loaded_paths,
        ) {
            piece_models.push(loaded);
        }
    }

    Ok(DressedCharacterScene {
        name: request.name.clone(),
        body,
        pieces: piece_models,
        skeleton,
        load_diagnostics,
        loaded_paths,
    })
}

/// [`load_dressed_character_scene_from_resource`] 的异步 Resource 版本。
#[cfg(feature = "game-data")]
pub async fn load_dressed_character_scene_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    request: &DressedCharacterLoadRequest,
) -> anyhow::Result<DressedCharacterScene> {
    let mut body_request =
        CharacterAssemblyLoadRequest::new(request.customize, request.name.clone());
    if let Some(appearance) = &request.appearance {
        body_request = body_request.with_appearance(*appearance);
    }
    let (body, skeleton) =
        load_character_assembly_with_skeleton_from_async_resource(resource, &body_request).await?;
    let mut load_diagnostics = body.load_diagnostics.clone();
    let mut loaded_paths = body.loaded_paths.clone();

    let any_stains = request
        .equipment
        .iter()
        .any(|piece| piece.stain_ids.iter().any(|stain_id| *stain_id != 0));
    let equipment_staining = if any_stains {
        load_weapon_staining_templates_from_async_resource(resource, [1, 0], &mut loaded_paths)
            .await
    } else {
        WeaponStainingTemplates::disabled([0, 0])
    };
    let eqp_table = load_equipment_parameter_table_from_async_resource(
        resource,
        &mut loaded_paths,
        &mut load_diagnostics,
    )
    .await;
    let mut imc_cache = HashMap::new();

    let mut pieces: Vec<&DressedEquipmentPiece> = request.equipment.iter().collect();
    pieces.sort_by_key(|piece| piece.equip_slot_category);
    let mut piece_models = Vec::new();
    for piece in pieces {
        let context = DressedPieceLoadContext {
            race_code: request.customize.race_code(),
            skeleton: skeleton.as_ref(),
            equipment_staining: &equipment_staining,
            eqp_table: eqp_table.as_ref(),
        };
        if let Some(loaded) = load_dressed_piece_from_async_resource(
            resource,
            piece,
            &context,
            &mut imc_cache,
            &mut load_diagnostics,
            &mut loaded_paths,
        )
        .await
        {
            piece_models.push(loaded);
        }
    }

    Ok(DressedCharacterScene {
        name: request.name.clone(),
        body,
        pieces: piece_models,
        skeleton,
        load_diagnostics,
        loaded_paths,
    })
}

/// 单件装备加载（增量换装）：独立的 IMC/EQP 解析（调用间不共享缓存，EQP
/// 表与染色模板按需重读），`skeleton` 传入时种族变形烘焙到该骨架。语义与
/// [`load_dressed_character_scene_from_resource`] 中该件的产出一致；
/// `stain_ids` 为 `[0, 0]` 时结果对（model_main, model_sub, 槽位, race）
/// 恒定，适合件级缓存。诊断/路径写进返回件的 `model` 字段；单件失败返回
/// None（诊断随件丢弃）。
#[cfg(feature = "game-data")]
pub fn load_dressed_piece_model_from_resource<R: physis::resource::Resource>(
    resource: &mut R,
    piece: &DressedEquipmentPiece,
    race_code: u16,
    skeleton: Option<&ModelSkeleton>,
) -> Option<DressedPieceModel> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let equipment_staining = if piece.stain_ids.iter().any(|stain_id| *stain_id != 0) {
        load_weapon_staining_templates_from_resource(resource, [1, 0], &mut loaded_paths)
    } else {
        WeaponStainingTemplates::disabled([0, 0])
    };
    let eqp_table = load_equipment_parameter_table_from_resource(
        resource,
        &mut loaded_paths,
        &mut load_diagnostics,
    );
    let mut imc_cache = HashMap::new();
    let context = DressedPieceLoadContext {
        race_code,
        skeleton,
        equipment_staining: &equipment_staining,
        eqp_table: eqp_table.as_ref(),
    };
    let mut loaded = load_dressed_piece_from_resource(
        resource,
        piece,
        &context,
        &mut imc_cache,
        &mut load_diagnostics,
        &mut loaded_paths,
    );
    if let Some(loaded) = &mut loaded {
        Rc::make_mut(&mut loaded.model).load_diagnostics = load_diagnostics;
        Rc::make_mut(&mut loaded.model).loaded_paths = loaded_paths;
    }
    loaded
}

/// [`load_dressed_piece_model_from_resource`] 的异步 Resource 版本。
#[cfg(feature = "game-data")]
pub async fn load_dressed_piece_model_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    piece: &DressedEquipmentPiece,
    race_code: u16,
    skeleton: Option<&ModelSkeleton>,
) -> Option<DressedPieceModel> {
    let mut load_diagnostics = Vec::new();
    let mut loaded_paths = Vec::new();
    let equipment_staining = if piece.stain_ids.iter().any(|stain_id| *stain_id != 0) {
        load_weapon_staining_templates_from_async_resource(resource, [1, 0], &mut loaded_paths)
            .await
    } else {
        WeaponStainingTemplates::disabled([0, 0])
    };
    let eqp_table = load_equipment_parameter_table_from_async_resource(
        resource,
        &mut loaded_paths,
        &mut load_diagnostics,
    )
    .await;
    let mut imc_cache = HashMap::new();
    let context = DressedPieceLoadContext {
        race_code,
        skeleton,
        equipment_staining: &equipment_staining,
        eqp_table: eqp_table.as_ref(),
    };
    let mut loaded = load_dressed_piece_from_async_resource(
        resource,
        piece,
        &context,
        &mut imc_cache,
        &mut load_diagnostics,
        &mut loaded_paths,
    )
    .await;
    if let Some(loaded) = &mut loaded {
        Rc::make_mut(&mut loaded.model).load_diagnostics = load_diagnostics;
        Rc::make_mut(&mut loaded.model).loaded_paths = loaded_paths;
    }
    loaded
}

/// 计算逐件着装场景的遮蔽计划（纯函数，不动模型数据）：规则与合并版
/// [`DressedCharacterData`] 的加载期网格过滤一致（EQP 位语义见
/// [`EquipmentParameterEntry`]），以隐藏标签表达——身体整网格隐藏记
/// `body_hidden_meshes` 下标，top/dwn 皮肤、头发 atr_top、脸部耳/角遮蔽记
/// `body_hidden_attributes` 名单（从默认启用名集合去除后按名过滤，网格粒
/// 度与"任一所需名被隐藏即整网格隐藏"等价），耳饰件整件隐藏记
/// `hidden_pieces`，各件的 IMC 变体位映射回该件 MDL 本地 attribute 名得到
/// 启用名单（`sho` 在身时另去除 `dwn` 件的 `atr_leg`）。
///
/// 头部规则（发 41-43/颈 44/耳饰 46-49/耳 50-53）的数据源：默认读 met
/// 条目；top 条目 BodyShowHead（10）关闭时改读 top 条目，且脸部网格整体
/// 隐藏（全身套装遮头）。
#[cfg(feature = "game-data")]
pub fn plan_dressed_concealment(
    customize: &CharacterCustomize,
    body: &WeaponModelData,
    pieces: &[DressedPieceModel],
) -> DressedConcealmentPlan {
    use EquipmentParameterEntry as E;

    let piece_in_slot = |category: u32| {
        pieces
            .iter()
            .find(|piece| piece.equip_slot_category == category && !piece.is_accessory)
    };
    let met = piece_in_slot(3);
    let top = piece_in_slot(4);
    let glv = piece_in_slot(5);
    let dwn = piece_in_slot(7);
    let sho = piece_in_slot(8);
    // 耳饰件（slot 9 饰品）：头部条目的种族组耳饰位门控。
    let earring = pieces
        .iter()
        .find(|piece| piece.equip_slot_category == 9 && piece.is_accessory);
    let top_eqp = top.and_then(|piece| piece.eqp);
    // 身体显示位关闭时，对应区域的遮蔽数据改从 top 套装条目解析；条目缺失
    // 按显示处理（同合并版）。
    let body_show_leg = top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_LEG));
    let body_show_hand = top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_HAND));
    let body_show_head = top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_HEAD));
    // 头部遮蔽数据源（发/颈/耳饰/耳）：BodyShowHead 关闭时改读 top 条目。
    let head_eqp = if body_show_head {
        met.and_then(|piece| piece.eqp)
    } else {
        top_eqp
    };

    let mut top_skin_hide: Vec<&'static str> = Vec::new();
    // top_eqp 为 Some 蕴含 top 在场（top_eqp = top.and_then(eqp)）。
    if let Some(eqp) = top_eqp {
        if eqp.flag(E::BODY_HIDE_GORGET) {
            top_skin_hide.push("atr_nek");
        }
        if eqp.flag(E::BODY_HIDE_SHORT_GLOVES) || eqp.flag(E::BODY_HIDE_MID_GLOVES) {
            top_skin_hide.push("atr_ude");
        }
        if eqp.flag(E::BODY_HIDE_LONG_GLOVES) {
            top_skin_hide.extend(["atr_hij", "atr_ude"]);
        }
    }
    let hand_source = if body_show_hand {
        glv.and_then(|piece| piece.eqp)
    } else {
        top_eqp
    };
    if let Some(source) = hand_source {
        if source.flag(E::HAND_HIDE_FOREARM) {
            top_skin_hide.push("atr_ude");
        }
        if source.flag(E::HAND_HIDE_ELBOW) && source.flag(E::HAND_HIDE_FOREARM) {
            top_skin_hide.push("atr_hij");
        }
    }

    let mut dwn_skin_hide: Vec<&'static str> = Vec::new();
    let (dwn_source, sho_source) = if body_show_leg {
        (
            dwn.and_then(|piece| piece.eqp),
            sho.and_then(|piece| piece.eqp),
        )
    } else {
        (top_eqp, top_eqp)
    };
    if let Some(source) = dwn_source {
        if source.flag(E::LEG_HIDE_KNEE_PADS) {
            dwn_skin_hide.push("atr_hiz");
        }
        if source.flag(E::LEG_HIDE_SHORT_BOOT) || source.flag(E::LEG_HIDE_HALF_BOOT) {
            dwn_skin_hide.push("atr_sne");
        }
    }
    if let Some(source) = sho_source {
        if source.flag(E::FOOT_HIDE_KNEE) && source.flag(E::FOOT_HIDE_CALF) {
            dwn_skin_hide.push("atr_hiz");
        }
        if source.flag(E::FOOT_HIDE_ANKLE) {
            dwn_skin_hide.push("atr_sne");
        }
    }

    let mut hair_hide_scalp = false;
    let mut hair_hide_all = false;
    let mut face_hide_attributes: Vec<&'static str> = Vec::new();
    let mut zear_hide = false;
    let mut earring_hide = false;
    let mut ear_unsupported = false;
    if let Some(eqp) = head_eqp {
        if eqp.flag(E::HEAD_HIDE_SCALP) {
            hair_hide_scalp = true;
        }
        if eqp.flag(E::HEAD_HIDE_HAIR) && !eqp.flag(E::HEAD_SHOW_HAIR_OVERRIDE) {
            hair_hide_all = true;
        }
        if eqp.flag(E::HEAD_HIDE_NECK) {
            top_skin_hide.push("atr_nek");
        }
        // 耳饰件（slot 9）：角色种族组的耳饰位关闭 → 整件隐藏（无 met 且
        // BodyShowHead 未关闭时 head_eqp 为 None，耳饰恒显示）。
        earring_hide = earring.is_some() && !eqp.flag(customize.earring_eqp_bit());
        // 耳朵几何：按种族机制（脸部 attribute / zear 部件 / 不可隔离记诊断）。
        match customize.ear_concealment() {
            EarConcealment::FaceAttribute(bit, name) => {
                if !eqp.flag(bit) {
                    face_hide_attributes.push(name);
                }
            }
            EarConcealment::ZearPart(bit) => {
                zear_hide = !eqp.flag(bit);
            }
            EarConcealment::Unsupported(bit) => {
                ear_unsupported = !eqp.flag(bit);
            }
            EarConcealment::None => {}
        }
    }
    // 全身套装（top 条目 BodyShowHead 关闭）：脸部网格整体隐藏。
    let face_hide_all = top.is_some() && !body_show_head;
    // 尾部显隐（猫魅/敖龙）：top 条目 ShowTail(13) 或腿部来源（dwn 条目，
    // BodyShowLeg 关闭时改读 top 条目）LegShowTail(22) 任一关闭即隐藏
    // （任一说藏即藏；TT 未规定优先级，取保守交集）。
    let leg_eqp = if body_show_leg {
        dwn.and_then(|piece| piece.eqp)
    } else {
        top_eqp
    };
    let tail_hide = customize.has_tail()
        && (!top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_TAIL))
            || !leg_eqp.is_none_or(|eqp| eqp.flag(E::LEG_SHOW_TAIL)));

    let mut plan = DressedConcealmentPlan::default();
    fn record_note(plan: &mut DressedConcealmentPlan, note: String) {
        if !plan.hidden_notes.contains(&note) {
            plan.hidden_notes.push(note);
        }
    }
    fn push_hidden_attribute(plan: &mut DressedConcealmentPlan, name: &str) {
        let owned = name.to_string();
        if !plan.body_hidden_attributes.contains(&owned) {
            plan.body_hidden_attributes.push(owned);
        }
    }

    for (index, mesh) in body.meshes.iter().enumerate() {
        match classify_dressed_body_mesh(mesh) {
            DressedBodyRegion::TopCloth if top.is_some() => {
                plan.body_hidden_meshes.push(index);
                record_note(&mut plan, "top:cloth".to_string());
            }
            DressedBodyRegion::TopSkin => {
                for name in submesh_zipped_attribute_names(mesh) {
                    if top_skin_hide.contains(&name) {
                        push_hidden_attribute(&mut plan, name);
                        record_note(&mut plan, format!("top:skin:{name}"));
                    }
                }
            }
            DressedBodyRegion::DwnCloth if dwn.is_some() => {
                plan.body_hidden_meshes.push(index);
                record_note(&mut plan, "dwn:cloth".to_string());
            }
            DressedBodyRegion::DwnSkin => {
                for name in submesh_zipped_attribute_names(mesh) {
                    if dwn_skin_hide.contains(&name) {
                        push_hidden_attribute(&mut plan, name);
                        record_note(&mut plan, format!("dwn:skin:{name}"));
                    }
                }
            }
            DressedBodyRegion::BodySho if sho.is_some() => {
                plan.body_hidden_meshes.push(index);
                record_note(&mut plan, "sho:body".to_string());
            }
            DressedBodyRegion::BodyGlv if glv.is_some() => {
                plan.body_hidden_meshes.push(index);
                record_note(&mut plan, "glv:body".to_string());
            }
            DressedBodyRegion::Hair => {
                if hair_hide_all {
                    plan.body_hidden_meshes.push(index);
                    record_note(&mut plan, "hair:all".to_string());
                } else if hair_hide_scalp
                    && submesh_zipped_attribute_names(mesh).contains(&"atr_top")
                {
                    push_hidden_attribute(&mut plan, "atr_top");
                    record_note(&mut plan, "hair:atr_top".to_string());
                }
            }
            DressedBodyRegion::Face => {
                if face_hide_all {
                    plan.body_hidden_meshes.push(index);
                    record_note(&mut plan, "face:all".to_string());
                } else {
                    for name in submesh_zipped_attribute_names(mesh) {
                        if face_hide_attributes.contains(&name) {
                            push_hidden_attribute(&mut plan, name);
                            record_note(&mut plan, format!("face:{name}"));
                        }
                    }
                }
            }
            DressedBodyRegion::Zear if zear_hide => {
                plan.body_hidden_meshes.push(index);
                record_note(&mut plan, "zear:all".to_string());
            }
            DressedBodyRegion::Tail if tail_hide => {
                plan.body_hidden_meshes.push(index);
                record_note(&mut plan, "tail:all".to_string());
            }
            _ => {}
        }
    }
    if ear_unsupported {
        // 猫魅耳在脸部基础网格内、无 attribute 隔离（真实探测结论）：不隐藏，
        // 仅记诊断。
        record_note(&mut plan, "face:ear-miqo-unsupported".to_string());
    }

    for piece in pieces {
        // 武器件不参与遮蔽：无 IMC/EQP，网格全显示（挂点骨挂接与身体遮蔽
        // 规则无关）。
        if piece.attach.is_some() {
            continue;
        }
        // IMC 变体位 → 该件 MDL 本地 attribute 名启用名单（位是该件本地表序，
        // 名单跨件不可比，只能逐件判定）；IMC 缺失 → 全部名启用。
        let mut enabled = Vec::new();
        for mesh in &piece.model.meshes {
            let Some(submesh) = &mesh.submesh else {
                continue;
            };
            let mut names = submesh.attribute_names.iter();
            for bit in 0..u32::BITS {
                if submesh.attribute_index_mask & (1 << bit) == 0 {
                    continue;
                }
                let Some(name) = names.next() else {
                    break;
                };
                let visible = piece
                    .imc_mask
                    .map(|mask| u32::from(mask) & (1 << bit) != 0)
                    .unwrap_or(true);
                if visible && !enabled.iter().any(|existing| existing == name) {
                    enabled.push(name.clone());
                }
            }
        }
        // 跨件规则：sho 在身时 dwn 件自身的 atr_leg 子网格隐藏（裤脚塞靴）。
        if piece.equip_slot_category == 7 && sho.is_some() {
            enabled.retain(|name| name != "atr_leg");
            record_note(&mut plan, "dwn-gear:atr_leg".to_string());
        }
        plan.piece_enabled_attributes
            .push(((piece.equip_slot_category, piece.item_id), enabled));
        // 耳饰件整件隐藏（头部条目耳饰位门控）。
        if earring_hide && piece.is_accessory && piece.equip_slot_category == 9 {
            plan.hidden_pieces
                .push((piece.equip_slot_category, piece.item_id));
            record_note(&mut plan, "ear:gear".to_string());
        }
    }

    plan
}

/// 着装合并的身体区域判定。身体网格按 `mesh.path` 识别（小衣 e0001 / 裸肤
/// e0000 装备域路径 + `/obj/hair/` 头发 + `/obj/face/` 脸 + `/obj/tail/` 尾 +
/// `/obj/zear/` 兔耳；布料/皮肤按内嵌材质名 `b0001` 区分，同
/// [`close_bare_limb_junctions`] 的判定）。跨族回退件路径含回退 race
/// code，但 `e0001_top` 等标记段不变。
#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DressedBodyRegion {
    TopCloth,
    TopSkin,
    DwnCloth,
    DwnSkin,
    BodySho,
    BodyGlv,
    Hair,
    Face,
    Tail,
    Zear,
    Other,
}

#[cfg(feature = "game-data")]
fn classify_dressed_body_mesh(mesh: &WeaponModelMesh) -> DressedBodyRegion {
    let path = mesh.path.as_str();
    if path.contains("/obj/hair/") {
        return DressedBodyRegion::Hair;
    }
    if path.contains("/obj/face/") {
        return DressedBodyRegion::Face;
    }
    if path.contains("/obj/tail/") {
        return DressedBodyRegion::Tail;
    }
    if path.contains("/obj/zear/") {
        return DressedBodyRegion::Zear;
    }
    let is_skin = mesh.material_name.contains("b0001");
    if path.contains("e0001_top") {
        return if is_skin {
            DressedBodyRegion::TopSkin
        } else {
            DressedBodyRegion::TopCloth
        };
    }
    if path.contains("e0001_dwn") {
        return if is_skin {
            DressedBodyRegion::DwnSkin
        } else {
            DressedBodyRegion::DwnCloth
        };
    }
    if path.contains("e0001_sho") || path.contains("e0000_sho") {
        return DressedBodyRegion::BodySho;
    }
    if path.contains("e0000_glv") {
        return DressedBodyRegion::BodyGlv;
    }
    DressedBodyRegion::Other
}

/// submesh 的 attribute 名集合（`attribute_names` 与掩码置位按位升序 zip
/// 对应，同 [`character_enabled_attribute_names`] 的判读）。
#[cfg(feature = "game-data")]
fn submesh_zipped_attribute_names(mesh: &WeaponModelMesh) -> Vec<&str> {
    let Some(submesh) = &mesh.submesh else {
        return Vec::new();
    };
    let mut names = submesh.attribute_names.iter();
    let mut zipped = Vec::new();
    for bit in 0..u32::BITS {
        if submesh.attribute_index_mask & (1 << bit) == 0 {
            continue;
        }
        let Some(name) = names.next() else {
            break;
        };
        zipped.push(name.as_str());
    }
    zipped
}

/// 着装可见性落地：按网格过滤（合并后 attribute 名在装备/身体间全局冲突
/// ——如 atr_ude 两边都有——按名启用表是全局的、无法表达槽位差异），返回
/// 身体遮蔽诊断记录。
///
/// 规则（EQP 位语义见 [`EquipmentParameterEntry`]；身体 attribute 名为真实
/// 数据探测结论：top 皮肤 atr_hij 肘/atr_ude 腕前臂/atr_nek 颈，dwn 皮肤
/// atr_hiz 膝/atr_sne 小腿踝，e0000 裸肤手足无 attribute；人族耳 atr_mim、
/// 敖龙角 atr_hrn 在脸部 MDL）：
/// - top 装备：丢弃全部 e0001_top 布料网格；皮肤按 top 条目 HideGorget→
///   atr_nek、HideShortGloves|HideMidGloves→atr_ude、HideLongGloves→atr_hij
///   +atr_ude；手部遮蔽（HandHideForearm→atr_ude、HandHideElbow+Forearm→
///   atr_hij）默认读 glv 条目，top 条目 BodyShowHand 关闭时改读 top 条目。
///   猫魅/敖龙的尾（`/obj/tail/` 网格）在 top 条目 ShowTail 关闭时丢弃。
/// - glv 装备：丢弃全部 e0000_glv 裸肤手网格（露指手套自带皮肤网格）。
/// - dwn 装备：丢弃全部 e0001_dwn 布料网格；皮肤按 dwn 条目
///   LegHideKneePads→atr_hiz、LegHideShortBoot|LegHideHalfBoot→atr_sne，加
///   sho 条目 FootHideKnee(+Calf)→atr_hiz、FootHideAnkle→atr_sne；top 条目
///   BodyShowLeg 关闭时两者都改读 top 条目。sho 装备时额外丢弃 dwn 装备自身
///   的 atr_leg 子网格（裤脚塞靴）。尾部另受腿部来源（dwn 条目，
///   BodyShowLeg 关闭时改读 top 条目）LegShowTail 关闭影响；top/腿任一
///   关闭即隐藏。
/// - sho 装备：丢弃全部 e0001_sho/e0000_sho 网格。
/// - met 装备：身体不动；HeadHideScalp→丢弃带 atr_top 的头发子网格，
///   HeadHideHair 且无 HeadShowHairOverride→丢弃全部头发网格，
///   HeadHideNeck→top 皮肤 atr_nek；耳饰位（46-49，按种族分组）关闭时丢弃
///   耳饰件（slot 9 饰品）网格；耳部位（50-53）关闭时按种族机制丢弃脸部
///   耳/角子网格（atr_mim/atr_hrn）或维埃拉 zear 部件网格（猫魅耳在脸部
///   基础网格内无法隔离，仅记诊断不隐藏）。top 条目 BodyShowHead 关闭时
///   上述头部规则（发/颈/耳饰/耳）全部改读 top 条目，且脸部网格整体丢弃
///   （全身套装遮头）。
/// - 每件装备再按自身 IMC 条目 attribute 位裁剪变体子网格（位是该件 MDL
///   本地表序，数值比较仅在同 MDL 内有效）；IMC 缺失 → 全显示。
#[cfg(feature = "game-data")]
fn apply_dressed_visibility(
    meshes: &mut Vec<WeaponModelMesh>,
    body_mesh_count: usize,
    pieces: &[DressedPieceLoad],
    customize: &CharacterCustomize,
) -> Vec<String> {
    use EquipmentParameterEntry as E;

    let piece_in_slot = |category: u32| {
        pieces
            .iter()
            .find(|piece| piece.equip_slot_category == category && !piece.is_accessory)
    };
    let met = piece_in_slot(3);
    let top = piece_in_slot(4);
    let glv = piece_in_slot(5);
    let dwn = piece_in_slot(7);
    let sho = piece_in_slot(8);
    let earring = pieces
        .iter()
        .find(|piece| piece.equip_slot_category == 9 && piece.is_accessory);
    let top_eqp = top.and_then(|piece| piece.eqp);
    // 身体显示位关闭时，对应区域的遮蔽数据改从 top 套装条目解析
    // （xivModdingFramework `EquipmentParameterFlag` 注释）；条目缺失按显示处理。
    let body_show_leg = top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_LEG));
    let body_show_hand = top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_HAND));
    let body_show_head = top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_HEAD));
    // 头部遮蔽数据源（发/颈/耳饰/耳）：BodyShowHead 关闭时改读 top 条目。
    let head_eqp = if body_show_head {
        met.and_then(|piece| piece.eqp)
    } else {
        top_eqp
    };

    let mut top_skin_hide: Vec<&'static str> = Vec::new();
    if top.is_some() {
        if let Some(eqp) = top_eqp {
            if eqp.flag(E::BODY_HIDE_GORGET) {
                top_skin_hide.push("atr_nek");
            }
            if eqp.flag(E::BODY_HIDE_SHORT_GLOVES) || eqp.flag(E::BODY_HIDE_MID_GLOVES) {
                top_skin_hide.push("atr_ude");
            }
            if eqp.flag(E::BODY_HIDE_LONG_GLOVES) {
                top_skin_hide.extend(["atr_hij", "atr_ude"]);
            }
        }
    }
    let hand_source = if body_show_hand {
        glv.and_then(|piece| piece.eqp)
    } else {
        top_eqp
    };
    if let Some(source) = hand_source {
        if source.flag(E::HAND_HIDE_FOREARM) {
            top_skin_hide.push("atr_ude");
        }
        if source.flag(E::HAND_HIDE_ELBOW) && source.flag(E::HAND_HIDE_FOREARM) {
            top_skin_hide.push("atr_hij");
        }
    }

    let mut dwn_skin_hide: Vec<&'static str> = Vec::new();
    let (dwn_source, sho_source) = if body_show_leg {
        (
            dwn.and_then(|piece| piece.eqp),
            sho.and_then(|piece| piece.eqp),
        )
    } else {
        (top_eqp, top_eqp)
    };
    if let Some(source) = dwn_source {
        if source.flag(E::LEG_HIDE_KNEE_PADS) {
            dwn_skin_hide.push("atr_hiz");
        }
        if source.flag(E::LEG_HIDE_SHORT_BOOT) || source.flag(E::LEG_HIDE_HALF_BOOT) {
            dwn_skin_hide.push("atr_sne");
        }
    }
    if let Some(source) = sho_source {
        if source.flag(E::FOOT_HIDE_KNEE) && source.flag(E::FOOT_HIDE_CALF) {
            dwn_skin_hide.push("atr_hiz");
        }
        if source.flag(E::FOOT_HIDE_ANKLE) {
            dwn_skin_hide.push("atr_sne");
        }
    }

    let mut hair_hide_scalp = false;
    let mut hair_hide_all = false;
    let mut face_hide_attributes: Vec<&'static str> = Vec::new();
    let mut zear_hide = false;
    let mut earring_hide = false;
    let mut ear_unsupported = false;
    if let Some(eqp) = head_eqp {
        if eqp.flag(E::HEAD_HIDE_SCALP) {
            hair_hide_scalp = true;
        }
        if eqp.flag(E::HEAD_HIDE_HAIR) && !eqp.flag(E::HEAD_SHOW_HAIR_OVERRIDE) {
            hair_hide_all = true;
        }
        if eqp.flag(E::HEAD_HIDE_NECK) {
            top_skin_hide.push("atr_nek");
        }
        // 耳饰件（slot 9 饰品）：角色种族组耳饰位关闭 → 整件网格丢弃。
        earring_hide = earring.is_some() && !eqp.flag(customize.earring_eqp_bit());
        // 耳朵几何：按种族机制（脸部 attribute / zear 部件 / 不可隔离记诊断）。
        match customize.ear_concealment() {
            EarConcealment::FaceAttribute(bit, name) => {
                if !eqp.flag(bit) {
                    face_hide_attributes.push(name);
                }
            }
            EarConcealment::ZearPart(bit) => {
                zear_hide = !eqp.flag(bit);
            }
            EarConcealment::Unsupported(bit) => {
                ear_unsupported = !eqp.flag(bit);
            }
            EarConcealment::None => {}
        }
    }
    // 全身套装（top 条目 BodyShowHead 关闭）：脸部网格整体丢弃。
    let face_hide_all = top.is_some() && !body_show_head;
    // 尾部（猫魅/敖龙）：top 条目 ShowTail 或腿部来源 LegShowTail 任一关闭
    // 即丢弃（任一说藏即藏；TT 未规定优先级，取保守交集）。
    let leg_eqp = if body_show_leg {
        dwn.and_then(|piece| piece.eqp)
    } else {
        top_eqp
    };
    let tail_hide = customize.has_tail()
        && (!top_eqp.is_none_or(|eqp| eqp.flag(E::BODY_SHOW_TAIL))
            || !leg_eqp.is_none_or(|eqp| eqp.flag(E::LEG_SHOW_TAIL)));

    let mut dropped = vec![false; meshes.len()];
    let mut hidden: Vec<String> = Vec::new();
    let record = |hidden: &mut Vec<String>, entry: String| {
        if !hidden.contains(&entry) {
            hidden.push(entry);
        }
    };
    for (index, mesh) in meshes.iter().enumerate().take(body_mesh_count) {
        match classify_dressed_body_mesh(mesh) {
            DressedBodyRegion::TopCloth if top.is_some() => {
                dropped[index] = true;
                record(&mut hidden, "top:cloth".to_string());
            }
            DressedBodyRegion::TopSkin => {
                for name in submesh_zipped_attribute_names(mesh) {
                    if top_skin_hide.contains(&name) {
                        dropped[index] = true;
                        record(&mut hidden, format!("top:skin:{name}"));
                    }
                }
            }
            DressedBodyRegion::DwnCloth if dwn.is_some() => {
                dropped[index] = true;
                record(&mut hidden, "dwn:cloth".to_string());
            }
            DressedBodyRegion::DwnSkin => {
                for name in submesh_zipped_attribute_names(mesh) {
                    if dwn_skin_hide.contains(&name) {
                        dropped[index] = true;
                        record(&mut hidden, format!("dwn:skin:{name}"));
                    }
                }
            }
            DressedBodyRegion::BodySho if sho.is_some() => {
                dropped[index] = true;
                record(&mut hidden, "sho:body".to_string());
            }
            DressedBodyRegion::BodyGlv if glv.is_some() => {
                dropped[index] = true;
                record(&mut hidden, "glv:body".to_string());
            }
            DressedBodyRegion::Hair => {
                if hair_hide_all {
                    dropped[index] = true;
                    record(&mut hidden, "hair:all".to_string());
                } else if hair_hide_scalp
                    && submesh_zipped_attribute_names(mesh).contains(&"atr_top")
                {
                    dropped[index] = true;
                    record(&mut hidden, "hair:atr_top".to_string());
                }
            }
            DressedBodyRegion::Face => {
                if face_hide_all {
                    dropped[index] = true;
                    record(&mut hidden, "face:all".to_string());
                } else {
                    for name in submesh_zipped_attribute_names(mesh) {
                        if face_hide_attributes.contains(&name) {
                            dropped[index] = true;
                            record(&mut hidden, format!("face:{name}"));
                        }
                    }
                }
            }
            DressedBodyRegion::Zear if zear_hide => {
                dropped[index] = true;
                record(&mut hidden, "zear:all".to_string());
            }
            DressedBodyRegion::Tail if tail_hide => {
                dropped[index] = true;
                record(&mut hidden, "tail:all".to_string());
            }
            _ => {}
        }
    }
    if ear_unsupported {
        record(&mut hidden, "face:ear-miqo-unsupported".to_string());
    }

    for piece in pieces {
        // 耳饰件整件丢弃（头部条目耳饰位门控）。
        if earring_hide && piece.is_accessory && piece.equip_slot_category == 9 {
            for index in piece.mesh_start..piece.mesh_end {
                dropped[index] = true;
            }
            record(&mut hidden, "ear:gear".to_string());
            continue;
        }
        if let Some(mask) = piece.imc_mask {
            for index in piece.mesh_start..piece.mesh_end {
                let Some(submesh) = &meshes[index].submesh else {
                    continue;
                };
                if submesh.attribute_index_mask & !u32::from(mask) != 0 {
                    dropped[index] = true;
                }
            }
        }
        if piece.equip_slot_category == 7 && sho.is_some() {
            for index in piece.mesh_start..piece.mesh_end {
                if submesh_zipped_attribute_names(&meshes[index]).contains(&"atr_leg") {
                    dropped[index] = true;
                    record(&mut hidden, "dwn-gear:atr_leg".to_string());
                }
            }
        }
    }

    if dropped.iter().any(|dropped| *dropped) {
        let mut kept = Vec::with_capacity(meshes.len());
        for (index, mesh) in meshes.drain(..).enumerate() {
            if !dropped[index] {
                kept.push(mesh);
            }
        }
        *meshes = kept;
    }
    hidden
}

/// 着装结果的单件增量染色：克隆合并模型，仅从各材质的未染色基准色表重新
/// 染色该件材质切片并重烘焙其 ColorTable 派生贴图（其余部件不动）。
/// `item_id` 不在 `equipment_material_ranges` 中时原样返回克隆。
#[cfg(feature = "game-data")]
pub fn apply_dressed_piece_stains(
    base: &DressedCharacterData,
    item_id: u32,
    stain_ids: [u8; 2],
    staining: &WeaponStainingTemplates,
) -> WeaponModelData {
    let mut model = base.model.clone();
    let Some(range) = base
        .equipment_material_ranges
        .iter()
        .find(|range| range.item_id == item_id)
    else {
        return model;
    };
    let start = range.material_start.min(model.materials.len());
    let end = range.material_end.min(model.materials.len());
    restain_materials_with_templates(
        &mut model.materials[start..end],
        &mut model.textures,
        normalize_stain_ids(stain_ids),
        staining,
    );
    model
}

#[cfg(feature = "game-data")]
pub fn apply_weapon_model_stains(
    base: &WeaponModelData,
    stain_ids: [u8; 2],
    staining: &WeaponStainingTemplates,
) -> WeaponModelData {
    let stain_ids =
        stain_ids.map(|stain_id| (stain_id <= MAX_STAIN_ID).then_some(stain_id).unwrap_or(0));
    let mut model = base.clone();
    model.stain_ids = stain_ids;
    restain_materials_with_templates(
        &mut model.materials,
        &mut model.textures,
        stain_ids,
        staining,
    );
    model
}

/// 运行时染色落地的材质区间版：从各材质的未染色基准色表（`color_table_rows`）
/// 重新应用染色并重烘焙 ColorTable 派生贴图，仅作用于给定切片（着装装配的
/// 单件增量染色用；整模型染色 = 全切片）。
#[cfg(feature = "game-data")]
fn restain_materials_with_templates(
    materials: &mut [WeaponModelMaterial],
    textures: &mut Vec<WeaponModelTexture>,
    stain_ids: [u8; 2],
    staining: &WeaponStainingTemplates,
) {
    for material in materials {
        let mut rows = material.color_table_rows.clone();
        material.staining_application = apply_weapon_staining_with_ids(
            rows.as_deref_mut(),
            material.color_dye_table.as_ref(),
            stain_ids,
            staining,
        );

        let Some(rows) = rows.as_deref() else {
            continue;
        };
        let Some(material_path) = material.path.clone() else {
            continue;
        };
        let bake_emissive = material
            .emissive_texture
            .and_then(|index| textures.get(index))
            .is_none_or(|texture| is_color_table_derived_texture_path(&texture.path));
        let Some(baked) = bake_weapon_color_table_textures(
            &material_path,
            Some(rows),
            material.index_texture,
            bake_emissive,
            textures,
        ) else {
            continue;
        };

        refresh_stained_material_textures(material, baked, textures);
        let summary = summarize_material_colors(Some(rows), material.fallback_color);
        material.diffuse_color = if material.base_color_texture.is_some() {
            [1.0, 1.0, 1.0]
        } else {
            summary.diffuse
        };
        material.specular_color = summary.specular;
        material.emissive_color = preview_emissive_color_for_material(summary.emissive, material);
        material.roughness = summary.roughness;
        material.metalness = summary.metalness;
    }
}

#[cfg(feature = "game-data")]
fn refresh_stained_material_textures(
    material: &mut WeaponModelMaterial,
    baked: BakedWeaponTextureIndices,
    textures: &mut [WeaponModelTexture],
) {
    // A previously composed material keeps its original full-resolution diffuse
    // in `base_color_texture` and only swaps the ColorTable ramp; materials
    // without shader composition keep the ramp itself as the base.
    if material.colorset_diffuse_texture.is_none() {
        material.base_color_texture = Some(baked.base_color);
    }
    material.colorset_diffuse_texture = None;
    if let Some(base_color) = material.base_color_texture {
        add_unique_index(&mut material.texture_indices, base_color);
    }

    replace_color_table_derived_slot(&mut material.specular_texture, baked.specular, textures);
    replace_color_table_derived_slot(
        &mut material.material_properties_texture,
        baked.material_properties,
        textures,
    );
    replace_color_table_derived_slot(
        &mut material.tile_properties_texture,
        baked.tile_properties,
        textures,
    );
    replace_color_table_derived_slot(
        &mut material.sheen_properties_texture,
        baked.sheen_properties,
        textures,
    );
    replace_color_table_derived_slot(
        &mut material.sphere_properties_texture,
        baked.sphere_properties,
        textures,
    );
    replace_color_table_derived_slot(
        &mut material.tile_matrix_texture,
        baked.tile_matrix,
        textures,
    );
    for index in [
        baked.specular,
        baked.material_properties,
        baked.tile_properties,
        baked.sheen_properties,
        baked.sphere_properties,
        baked.tile_matrix,
    ] {
        add_unique_index(&mut material.texture_indices, index);
    }

    let emissive_is_derived = material
        .emissive_texture
        .and_then(|index| textures.get(index))
        .is_some_and(|texture| is_color_table_derived_texture_path(&texture.path));
    match baked.emissive {
        Some(emissive) if material.emissive_texture.is_none() || emissive_is_derived => {
            material.emissive_texture = Some(emissive);
            add_unique_index(&mut material.texture_indices, emissive);
        }
        None if emissive_is_derived => material.emissive_texture = None,
        _ => {}
    }
}

#[cfg(feature = "game-data")]
fn replace_color_table_derived_slot(
    slot: &mut Option<usize>,
    replacement: usize,
    textures: &[WeaponModelTexture],
) {
    let replace = slot
        .and_then(|index| textures.get(index))
        .is_none_or(|texture| is_color_table_derived_texture_path(&texture.path));
    if replace {
        *slot = Some(replacement);
    }
}

#[cfg(feature = "game-data")]
fn is_color_table_derived_texture_path(path: &str) -> bool {
    path.starts_with("baked://") && path.contains("#colorset-")
}

#[cfg(feature = "game-data")]
fn preview_emissive_color_for_material(
    emissive: [f32; 3],
    material: &WeaponModelMaterial,
) -> [f32; 3] {
    let scale = if material.emissive_texture.is_some() {
        1.0
    } else if material.mask_texture.is_some() {
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
async fn load_model_meshes_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    paths: ModelPathContext,
    staining: &WeaponStainingTemplates,
    loaded_paths: &mut Vec<String>,
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    meshes: &mut Vec<WeaponModelMesh>,
    color_table_sources: &mut HashMap<u16, LoadedMaterialColorTable>,
) -> Result<(), WeaponModelMeshLoadFailure> {
    use anyhow::Context;

    let mut candidates = Vec::new();
    for path in paths.model_candidate_paths() {
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
                return Err(WeaponModelMeshLoadFailure::new(
                    paths.diagnostic_model(),
                    candidates,
                ));
            }
        };
        push_loaded_path(loaded_paths, path.clone());
        assign_model_materials_from_async_resource(
            resource,
            paths,
            &path,
            staining,
            &mut path_meshes,
            materials,
            textures,
            loaded_paths,
            color_table_sources,
        )
        .await;
        meshes.append(&mut path_meshes);
        return Ok(());
    }

    Err(WeaponModelMeshLoadFailure::new(
        paths.diagnostic_model(),
        candidates,
    ))
}

#[cfg(feature = "game-data")]
async fn assign_model_materials_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    paths: ModelPathContext,
    model_path: &str,
    staining: &WeaponStainingTemplates,
    meshes: &mut [WeaponModelMesh],
    materials: &mut Vec<WeaponModelMaterial>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
    color_table_sources: &mut HashMap<u16, LoadedMaterialColorTable>,
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
        let material = load_model_material_from_async_resource(
            resource,
            paths.clone(),
            model_path,
            staining,
            material_index,
            material_name,
            slot,
            materials,
            textures,
            loaded_paths,
            color_table_sources,
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
async fn load_model_material_from_async_resource<R: AsyncGameResource>(
    resource: &mut R,
    paths: ModelPathContext,
    model_path: &str,
    staining: &WeaponStainingTemplates,
    material_index: u16,
    material_name: String,
    slot: usize,
    loaded_materials: &[WeaponModelMaterial],
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
    color_table_sources: &mut HashMap<u16, LoadedMaterialColorTable>,
) -> WeaponModelMaterial {
    use physis::ReadableFile;

    let fallback = material_color(material_index);
    let candidates = paths.material_candidate_paths(model_path, &material_name);
    for path in candidates {
        let Ok(bytes) = resource.read(&path).await else {
            continue;
        };
        let Some(material) = physis::mtrl::Material::from_existing(resource.platform(), &bytes)
        else {
            continue;
        };
        let texture_paths = material_texture_paths_from_offsets(&bytes, &material.texture_paths);

        push_loaded_path(loaded_paths, path.clone());
        let shader_package_name = material.shader_package_name.clone();
        let mut color_dye_table = model_color_dye_table(material.color_dye_table.as_ref());
        let mut color_table_rows = material
            .color_table
            .as_ref()
            .and_then(weapon_color_table_rows);
        let reference_fallback = resolve_loaded_color_table_reference(
            slot,
            material_index,
            &material_name,
            &path,
            &mut color_table_rows,
            &mut color_dye_table,
            color_table_sources,
        );
        let fallback_index_texture = loaded_color_table_reference_index_texture(
            reference_fallback.as_ref(),
            loaded_materials,
        );
        let base_color_table_rows = color_table_rows.clone();
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
        let (specular_type, specular_type_raw) = composed_material_specular_type(&semantics);
        let (value_mode, value_mode_raw) = composed_material_value_mode(&semantics);
        let color_table_diffuse_composition = color_table_diffuse_composition(
            material_shader_family(Some(&shader_package_name)),
            composed_material_uses_compatibility_values(&semantics),
        );
        let sub_color_mode = composed_material_sub_color_mode(&semantics);
        let (decal_color_mode, decal_color_mode_raw) =
            composed_material_decal_color_mode(&semantics);
        let skin_value_mode = composed_material_skin_value_mode(&semantics);
        let (character_scroll_variant, character_scroll_variant_raw) =
            composed_material_character_scroll_variant(&semantics);
        let (lightshaft_type, lightshaft_type_raw) = composed_material_lightshaft_type(&semantics);
        let transparency = composed_material_transparency(&semantics, &shader_package_name);
        let water_deep_color = composed_material_water_deep_color(&semantics);
        let water_refraction_color = composed_material_water_refraction_color(&semantics);
        let water_whitecap_color = composed_material_water_whitecap_color(&semantics);
        let alpha_aperture = composed_material_alpha_aperture(&semantics);
        let alpha_offset = composed_material_alpha_offset(&semantics);
        let vertex_alpha_to_one = composed_material_vertex_alpha_to_one(&semantics);
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
        let ambient_occlusion_mask = composed_material_ambient_occlusion_mask(&semantics);
        let texture_mip_bias = composed_material_texture_mip_bias(&semantics);
        let tile_mip_bias_offset = composed_material_tile_mip_bias_offset(&semantics);
        let vertex_movement_scale = composed_material_vertex_movement_scale(&semantics);
        let vertex_movement_max_length = composed_material_vertex_movement_max_length(&semantics);
        let shadow_pos_offset = composed_material_shadow_pos_offset(&semantics);
        let detail_color_uv_scale = composed_material_detail_color_uv_scale(&semantics);
        let detail_normal_uv_scale = composed_material_detail_normal_uv_scale(&semantics);
        let uv_scroll = composed_material_uv_scroll(&semantics);
        let color_uv_scale = composed_material_color_uv_scale(&semantics);
        let normal_uv_scale = composed_material_normal_uv_scale(&semantics);
        let specular_uv_scale = composed_material_specular_uv_scale(&semantics);
        let white_eye_color = composed_material_white_eye_color(&semantics);
        let iris_ring_color = composed_material_iris_ring_color(&semantics);
        let iris_ring_emissive_intensity =
            composed_material_iris_ring_emissive_intensity(&semantics);
        let iris_ring_uv_radius = composed_material_iris_ring_uv_radius(&semantics);
        let iris_ring_uv_fade_width = composed_material_iris_ring_uv_fade_width(&semantics);
        let lightshaft_color = composed_material_lightshaft_color(&semantics);
        let lightshaft_tex_anim = composed_material_lightshaft_tex_anim(&semantics);
        let lightshaft_tex_u = composed_material_lightshaft_tex_u(&semantics);
        let lightshaft_tex_v = composed_material_lightshaft_tex_v(&semantics);
        let lightshaft_ray = composed_material_lightshaft_ray(&semantics);
        let lightshaft_angle_clip = composed_material_lightshaft_angle_clip(&semantics);
        let lightshaft_near_clip = composed_material_lightshaft_near_clip(&semantics);
        let texture_set = load_weapon_material_textures_from_async_resource(
            resource,
            &path,
            &texture_paths,
            color_table_rows.as_deref(),
            &sampler_roles,
            color_table_diffuse_composition,
            fallback_index_texture,
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
            reference_fallback,
            shader_package_name: Some(shader_package_name),
            render_mode,
            alpha_mode,
            alpha_threshold,
            draw_depth_mode,
            lighting_mode,
            flow_mode,
            specular_type,
            specular_type_raw,
            value_mode,
            value_mode_raw,
            sub_color_mode,
            decal_color_mode,
            decal_color_mode_raw,
            skin_value_mode,
            character_scroll_variant,
            character_scroll_variant_raw,
            lightshaft_type,
            lightshaft_type_raw,
            transparency,
            water_deep_color,
            water_refraction_color,
            water_whitecap_color,
            alpha_aperture,
            alpha_offset,
            vertex_alpha_to_one,
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
            ambient_occlusion_mask,
            texture_mip_bias,
            tile_mip_bias_offset,
            vertex_movement_scale,
            vertex_movement_max_length,
            shadow_pos_offset,
            detail_color_uv_scale,
            detail_normal_uv_scale,
            uv_scroll,
            color_uv_scale,
            normal_uv_scale,
            specular_uv_scale,
            white_eye_color,
            iris_ring_color,
            iris_ring_emissive_intensity,
            iris_ring_uv_radius,
            iris_ring_uv_fade_width,
            lightshaft_color,
            lightshaft_tex_anim,
            lightshaft_tex_u,
            lightshaft_tex_v,
            lightshaft_ray,
            lightshaft_angle_clip,
            lightshaft_near_clip,
            opacity,
            render_backfaces,
            apply_vertex_color,
            has_color_dye_table: color_dye_table.is_some(),
            color_dye_table,
            color_table_rows: base_color_table_rows,
            staining_application,
            character_colors: None,
            texture_arrays: ModelMaterialTextureArrays::default(),
            fallback_color: fallback,
            diffuse_color,
            specular_color: summary.specular,
            emissive_color,
            roughness: summary.roughness,
            metalness: summary.metalness,
            texture_indices: texture_set.indices,
            base_color_texture: texture_set.base_color,
            colorset_diffuse_texture: texture_set.colorset_diffuse,
            secondary_base_color_texture: texture_set.secondary_base_color,
            normal_texture: texture_set.normal,
            secondary_normal_texture: texture_set.secondary_normal,
            mask_texture: texture_set.mask,
            skin_diffuse_texture: texture_set.skin_diffuse,
            skin_normal_texture: texture_set.skin_normal,
            skin_mask_texture: texture_set.skin_mask,
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
    texture_paths: &[String],
    color_table_rows: Option<&[ColorTableRowColors]>,
    sampler_roles: &[MaterialSamplerRole],
    color_table_diffuse_composition: ColorTableDiffuseComposition,
    fallback_index_texture: Option<usize>,
    textures: &mut Vec<WeaponModelTexture>,
    loaded_paths: &mut Vec<String>,
) -> WeaponTextureSet {
    let mut set = WeaponTextureSet::default();
    for (texture_order, raw_texture_path) in texture_paths.iter().enumerate() {
        let sampler_role = sampler_role_for_texture(sampler_roles, texture_order);
        let sampler_kind = sampler_role.map(|role| role.kind);
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
        assign_weapon_texture_slot(
            &mut set,
            texture_index,
            &textures[texture_index],
            sampler_role.map(|role| role.logical_role),
        );
    }
    apply_color_table_index_fallback(&mut set, fallback_index_texture, textures);

    if let Some(baked) = bake_weapon_color_table_textures(
        material_path,
        color_table_rows,
        set.index,
        set.emissive.is_none(),
        textures,
    ) {
        let resolved_base = resolve_color_table_base_texture(
            set.base_color,
            baked.base_color,
            color_table_diffuse_composition,
        );
        set.base_color = Some(resolved_base.base_color);
        add_unique_index(&mut set.indices, resolved_base.base_color);
        set.colorset_diffuse = resolved_base.colorset_diffuse;
        if let Some(colorset_diffuse) = resolved_base.colorset_diffuse {
            add_unique_index(&mut set.indices, colorset_diffuse);
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
    colorset_diffuse: Option<usize>,
    secondary_base_color: Option<usize>,
    normal: Option<usize>,
    secondary_normal: Option<usize>,
    mask: Option<usize>,
    skin_diffuse: Option<usize>,
    skin_normal: Option<usize>,
    skin_mask: Option<usize>,
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
fn apply_color_table_index_fallback(
    set: &mut WeaponTextureSet,
    fallback_index_texture: Option<usize>,
    textures: &[WeaponModelTexture],
) {
    if set.index.is_some() {
        return;
    }
    let Some(texture_index) = fallback_index_texture.filter(|texture_index| {
        textures
            .get(*texture_index)
            .is_some_and(|texture| texture.kind == WeaponModelTextureKind::Index)
    }) else {
        return;
    };

    set.index = Some(texture_index);
    add_unique_index(&mut set.indices, texture_index);
}

#[cfg(feature = "game-data")]
fn assign_weapon_texture_slot(
    set: &mut WeaponTextureSet,
    texture_index: usize,
    texture: &WeaponModelTexture,
    logical_role: Option<MaterialSamplerLogicalRole>,
) {
    use MaterialSamplerLogicalRole as Role;

    match logical_role {
        Some(Role::BaseColor) => {
            set.base_color.get_or_insert(texture_index);
            set.has_alpha |= texture_has_alpha(texture);
        }
        Some(Role::SecondaryBaseColor) => {
            set.secondary_base_color.get_or_insert(texture_index);
            set.has_alpha |= texture_has_alpha(texture);
        }
        Some(Role::Normal) => {
            set.normal.get_or_insert(texture_index);
        }
        Some(Role::SecondaryNormal) => {
            set.secondary_normal.get_or_insert(texture_index);
        }
        Some(Role::Mask) => {
            set.mask.get_or_insert(texture_index);
        }
        Some(Role::SkinDiffuse) => {
            set.skin_diffuse.get_or_insert(texture_index);
        }
        Some(Role::SkinNormal) => {
            set.skin_normal.get_or_insert(texture_index);
        }
        Some(Role::SkinMask) => {
            set.skin_mask.get_or_insert(texture_index);
        }
        Some(Role::MaterialMap) => {
            set.material_map.get_or_insert(texture_index);
        }
        Some(Role::MultiMap) => {
            set.multi_map.get_or_insert(texture_index);
        }
        Some(Role::Specular) => {
            set.specular.get_or_insert(texture_index);
        }
        Some(Role::SecondarySpecular) => {
            set.secondary_specular.get_or_insert(texture_index);
        }
        Some(Role::Emissive) => {
            set.emissive.get_or_insert(texture_index);
        }
        Some(Role::Index) => {
            set.index.get_or_insert(texture_index);
        }
        Some(Role::Environment) => {
            set.environment.get_or_insert(texture_index);
        }
        Some(Role::WaterWave) => {
            set.water_wave.get_or_insert(texture_index);
        }
        Some(Role::WaterWaveSecondary) => {
            set.water_wave1.get_or_insert(texture_index);
        }
        Some(Role::WaterWhitecap) => {
            set.water_whitecap.get_or_insert(texture_index);
        }
        None => assign_weapon_texture_slot_from_kind(set, texture_index, texture),
    }
}

#[cfg(feature = "game-data")]
fn assign_weapon_texture_slot_from_kind(
    set: &mut WeaponTextureSet,
    texture_index: usize,
    texture: &WeaponModelTexture,
) {
    match texture.kind {
        WeaponModelTextureKind::BaseColor => {
            set.base_color.get_or_insert(texture_index);
            if texture_alpha_affects_material_transparency(texture) {
                set.has_alpha = true;
            }
        }
        WeaponModelTextureKind::SecondaryBaseColor => {
            set.secondary_base_color.get_or_insert(texture_index);
            if texture_alpha_affects_material_transparency(texture) {
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorTableDiffuseComposition {
    Replace,
    Multiply,
}

#[cfg(feature = "game-data")]
fn color_table_diffuse_composition(
    shader_family: MaterialShaderFamily,
    uses_compatibility_values: bool,
) -> ColorTableDiffuseComposition {
    // These are the packages MeddleTools maps to its character.shpk node group.
    let uses_character_compatibility_gate = matches!(
        shader_family,
        MaterialShaderFamily::Character
            | MaterialShaderFamily::CharacterStockings
            | MaterialShaderFamily::CharacterGlass
            | MaterialShaderFamily::CharacterTransparency
            | MaterialShaderFamily::CharacterScroll
    );
    if uses_character_compatibility_gate && uses_compatibility_values {
        ColorTableDiffuseComposition::Multiply
    } else if uses_character_compatibility_gate {
        ColorTableDiffuseComposition::Replace
    } else {
        ColorTableDiffuseComposition::Multiply
    }
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
    logical_role: MaterialSamplerLogicalRole,
    kind: WeaponModelTextureKind,
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug)]
struct MaterialSamplerRecord {
    texture_index: usize,
    texture_usage: u32,
    texture_usage_name: Option<String>,
    flags: u32,
    logical_role: Option<MaterialSamplerLogicalRole>,
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
            let logical_role = classify_sampler_logical_role_name(name);
            return MaterialSamplerKindResolution {
                texture_usage_name: Some(name.clone()),
                logical_role,
                kind: logical_role.map(MaterialSamplerLogicalRole::texture_kind),
                kind_source: logical_role.map(|_| "shpkResourceName"),
            };
        }

        if let Some((name, logical_role)) = known_sampler_names()
            .iter()
            .find(|(name, _)| physis::shpk::ShaderPackage::crc(name) == texture_usage)
        {
            return MaterialSamplerKindResolution {
                texture_usage_name: Some((*name).to_string()),
                logical_role: Some(*logical_role),
                kind: Some(logical_role.texture_kind()),
                kind_source: Some("knownCrc"),
            };
        }

        MaterialSamplerKindResolution {
            texture_usage_name: None,
            logical_role: None,
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
    logical_role: Option<MaterialSamplerLogicalRole>,
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
    let tile_ramp_width = width.checked_mul(2)?;
    let id_rgba = index_texture.rgba.clone();
    let baked = bake_color_table_maps(rows, &id_rgba)?;
    let material_key = normalize_game_resource_path(material_path);

    let base_path = format!("baked://{material_key}#colorset-diffuse");
    let base_color = push_or_replace_baked_texture_with_float_channels(
        textures,
        base_path,
        WeaponModelTextureKind::BaseColor,
        tile_ramp_width,
        height,
        baked.diffuse_ab_rgba,
        Some(baked.diffuse_ab_rgba_f32),
    );
    textures[base_color].texel_layout = ModelTextureTexelLayout::ColorTableRampAb;

    let specular = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-specular"),
        WeaponModelTextureKind::Specular,
        tile_ramp_width,
        height,
        baked.specular_ab_rgba,
        Some(baked.specular_ab_rgba_f32),
    );
    textures[specular].texel_layout = ModelTextureTexelLayout::ColorTableRampAb;

    let material_properties = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-material-properties"),
        WeaponModelTextureKind::MaterialProperties,
        tile_ramp_width,
        height,
        baked.material_ab_rgba,
        Some(baked.material_ab_rgba_f32),
    );
    textures[material_properties].texel_layout = ModelTextureTexelLayout::ColorTableRampAb;

    let tile_properties = push_or_replace_baked_texture(
        textures,
        format!("baked://{material_key}#colorset-tile-properties-ab"),
        WeaponModelTextureKind::TileProperties,
        tile_ramp_width,
        height,
        baked.tile_properties_ab_rgba,
    );
    textures[tile_properties].texel_layout = ModelTextureTexelLayout::ColorTableTileRampAb;

    let sheen_properties = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-sheen-properties"),
        WeaponModelTextureKind::SheenProperties,
        tile_ramp_width,
        height,
        baked.sheen_properties_ab_rgba,
        Some(baked.sheen_properties_ab_rgba_f32),
    );
    textures[sheen_properties].texel_layout = ModelTextureTexelLayout::ColorTableRampAb;

    let sphere_properties = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-sphere-properties"),
        WeaponModelTextureKind::SphereProperties,
        tile_ramp_width,
        height,
        baked.sphere_properties_ab_rgba,
        Some(baked.sphere_properties_ab_rgba_f32),
    );
    textures[sphere_properties].texel_layout = ModelTextureTexelLayout::ColorTableRampAb;

    let tile_matrix = push_or_replace_baked_texture_with_float_channels(
        textures,
        format!("baked://{material_key}#colorset-tile-matrix-ab"),
        WeaponModelTextureKind::TileMatrixProperties,
        tile_ramp_width,
        height,
        baked.tile_matrix_ab_rgba,
        Some(baked.tile_matrix_ab_rgba_f32),
    );
    textures[tile_matrix].texel_layout = ModelTextureTexelLayout::ColorTableTileRampAb;

    let emissive = if bake_emissive {
        baked.emissive_rgba.map(|rgba| {
            let float_channels = baked
                .emissive_ab_rgba_f32
                .clone()
                .or_else(|| baked.emissive_rgba_f32.clone());
            let index = push_or_replace_baked_texture_with_float_channels(
                textures,
                format!("baked://{material_key}#colorset-emissive"),
                WeaponModelTextureKind::Emissive,
                tile_ramp_width,
                height,
                baked.emissive_ab_rgba.clone().unwrap_or(rgba),
                float_channels,
            );
            textures[index].texel_layout = ModelTextureTexelLayout::ColorTableRampAb;
            index
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
    } else if texture_set.has_alpha && !bg_color_change_alpha_is_dye_mask(&shader) {
        WeaponMaterialAlphaMode::Blend
    } else {
        WeaponMaterialAlphaMode::Opaque
    }
}

/// bg 变色（家具染色）材质的漫反射 alpha 是染色遮罩而非透明度：
/// 遮罩为 0 的区域（非可染部件）在游戏中照常不透明渲染，
/// 不能因纹理含 alpha 通道而落入 Blend 透明显示。
#[cfg(feature = "game-data")]
fn bg_color_change_alpha_is_dye_mask(shader_package_name: &str) -> bool {
    let shader = shader_package_name
        .rsplit('/')
        .next()
        .unwrap_or(shader_package_name)
        .to_ascii_lowercase();
    matches!(shader.as_str(), "bgcolorchange.shpk" | "bgcrestchange.shpk")
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
fn composed_material_specular_type(
    semantics: &ComposedMaterialSemantics,
) -> (MaterialSpecularType, Option<u32>) {
    let raw = semantics.material_key_value(CATEGORY_SPECULAR_TYPE);
    let value = match raw {
        None | Some(SPECULAR_TYPE_DEFAULT) => MaterialSpecularType::Default,
        Some(SPECULAR_TYPE_MASK) => MaterialSpecularType::Mask,
        Some(_) => MaterialSpecularType::Unknown,
    };
    (value, raw)
}

#[cfg(feature = "game-data")]
fn composed_material_value_mode(
    semantics: &ComposedMaterialSemantics,
) -> (MaterialValueMode, Option<u32>) {
    let raw = semantics.material_key_value(GET_VALUES);
    let value = match raw {
        None | Some(GET_VALUES_SINGLE) => MaterialValueMode::Single,
        Some(GET_VALUES_MULTI) => MaterialValueMode::Multi,
        Some(GET_ALPHA_MULTI_VALUES) => MaterialValueMode::AlphaMulti,
        Some(GET_ALPHA_MULTI_VALUES2) => MaterialValueMode::AlphaMulti2,
        Some(GET_ALPHA_MULTI_VALUES3) => MaterialValueMode::AlphaMulti3,
        Some(GET_VALUES_MULTI_MATERIAL) => MaterialValueMode::MultiMaterial,
        Some(GET_VALUES_COMPATIBILITY) => MaterialValueMode::Compatibility,
        Some(_) => MaterialValueMode::Unknown,
    };
    (value, raw)
}

#[cfg(feature = "game-data")]
fn composed_material_uses_compatibility_values(semantics: &ComposedMaterialSemantics) -> bool {
    semantics.material_key_value(GET_VALUES) == Some(GET_VALUES_COMPATIBILITY)
        || semantics.material_key_value(GET_VALUES_TEXTURE_TYPE)
            == Some(GET_VALUES_TEXTURE_TYPE_COMPATIBILITY)
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
fn composed_material_decal_color_mode(
    semantics: &ComposedMaterialSemantics,
) -> (MaterialDecalColorMode, Option<u32>) {
    let raw = semantics.material_key_value(GET_DECAL_COLOR);
    let value = match raw {
        None | Some(GET_DECAL_COLOR_OFF) => MaterialDecalColorMode::Off,
        Some(GET_DECAL_COLOR_ALPHA) => MaterialDecalColorMode::Alpha,
        Some(GET_DECAL_COLOR_RGBA) => MaterialDecalColorMode::Rgba,
        Some(_) => MaterialDecalColorMode::Unknown,
    };
    (value, raw)
}

#[cfg(feature = "game-data")]
fn composed_material_skin_value_mode(
    semantics: &ComposedMaterialSemantics,
) -> MaterialSkinValueMode {
    match semantics.material_key_value(GET_MATERIAL_VALUE) {
        None => MaterialSkinValueMode::None,
        Some(GET_MATERIAL_VALUE_FACE) => MaterialSkinValueMode::Face,
        Some(GET_MATERIAL_VALUE_BODY) => MaterialSkinValueMode::Body,
        Some(GET_MATERIAL_VALUE_BODY_JJM) => MaterialSkinValueMode::BodyJjm,
        Some(GET_MATERIAL_VALUE_FACE_EMISSIVE) => MaterialSkinValueMode::FaceEmissive,
        Some(_) => MaterialSkinValueMode::Unknown,
    }
}

#[cfg(feature = "game-data")]
fn composed_material_character_scroll_variant(
    semantics: &ComposedMaterialSemantics,
) -> (MaterialCharacterScrollVariant, Option<u32>) {
    let raw = semantics.material_key_value(CHARACTER_SCROLL_VARIANT);
    let variant = match raw {
        None => MaterialCharacterScrollVariant::None,
        Some(CHARACTER_SCROLL_VARIANT_69EB4AE0) => MaterialCharacterScrollVariant::Value69eb4ae0,
        Some(CHARACTER_SCROLL_VARIANT_9A8A46F5) => MaterialCharacterScrollVariant::Value9a8a46f5,
        Some(_) => MaterialCharacterScrollVariant::Unknown,
    };
    (variant, raw)
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_type(
    semantics: &ComposedMaterialSemantics,
) -> (MaterialLightShaftType, Option<u32>) {
    let raw = semantics.material_key_value(LIGHTSHAFT_TYPE);
    let value = match raw {
        None => MaterialLightShaftType::None,
        Some(LIGHTSHAFT_TYPE_0) => MaterialLightShaftType::Type0,
        Some(LIGHTSHAFT_TYPE_1) => MaterialLightShaftType::Type1,
        Some(_) => MaterialLightShaftType::Unknown,
    };
    (value, raw)
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
fn composed_material_vertex_alpha_to_one(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_VERTEX_ALPHA_TO_ONE, 0.0)
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
fn composed_material_ambient_occlusion_mask(semantics: &ComposedMaterialSemantics) -> Option<f32> {
    semantics
        .material_constant_f32_values(G_AMBIENT_OCCLUSION_MASK)
        .and_then(|values| values.first())
        .copied()
        .filter(|value| value.is_finite())
}

#[cfg(feature = "game-data")]
fn composed_material_texture_mip_bias(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TEXTURE_MIP_BIAS, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_tile_mip_bias_offset(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_TILE_MIP_BIAS_OFFSET, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_vertex_movement_scale(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_VERTEX_MOVEMENT_SCALE, 1.0)
}

#[cfg(feature = "game-data")]
fn composed_material_vertex_movement_max_length(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_VERTEX_MOVEMENT_MAX_LENGTH, 1.0)
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
fn composed_material_color_uv_scale(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_COLOR_UV_SCALE, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_normal_uv_scale(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_NORMAL_UV_SCALE, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_specular_uv_scale(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_SPECULAR_UV_SCALE, [1.0; 4])
}

#[cfg(feature = "game-data")]
fn composed_material_white_eye_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_WHITE_EYE_COLOR, [1.0, 1.0, 1.0, 0.0])
}

#[cfg(feature = "game-data")]
fn composed_material_iris_ring_color(semantics: &ComposedMaterialSemantics) -> [f32; 4] {
    composed_material_finite_vec4_constant(semantics, G_IRIS_RING_COLOR, [1.0, 1.0, 1.0, 1.0])
}

#[cfg(feature = "game-data")]
fn composed_material_iris_ring_emissive_intensity(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_vec4_constant(
        semantics,
        G_IRIS_RING_EMISSIVE_INTENSITY,
        [0.25, 0.0, 0.0, 0.0],
    )[0]
}

#[cfg(feature = "game-data")]
fn composed_material_iris_ring_uv_radius(semantics: &ComposedMaterialSemantics) -> [f32; 2] {
    let values = composed_material_finite_vec4_constant(
        semantics,
        G_IRIS_RING_UV_RADIUS,
        [0.158, 0.174, 0.0, 0.0],
    );
    [values[0], values[1]]
}

#[cfg(feature = "game-data")]
fn composed_material_iris_ring_uv_fade_width(semantics: &ComposedMaterialSemantics) -> [f32; 2] {
    let values = composed_material_finite_vec4_constant(
        semantics,
        G_IRIS_RING_UV_FADE_WIDTH,
        [0.04, 0.02, 0.0, 0.0],
    );
    [values[0], values[1]]
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
fn composed_material_lightshaft_angle_clip(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_LIGHTSHAFT_ANGLE_CLIP, 0.0)
}

#[cfg(feature = "game-data")]
fn composed_material_lightshaft_near_clip(semantics: &ComposedMaterialSemantics) -> f32 {
    composed_material_finite_constant(semantics, G_LIGHTSHAFT_NEAR_CLIP, 0.25)
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResolvedColorTableBase {
    base_color: usize,
    /// `Multiply` composition with a real diffuse: the renderer samples the
    /// diffuse at its own resolution and multiplies it by this ColorTable ramp
    /// in the shader instead of baking a colorset-resolution composite.
    colorset_diffuse: Option<usize>,
}

#[cfg(feature = "game-data")]
fn resolve_color_table_base_texture(
    base_index: Option<usize>,
    colorset_index: usize,
    composition: ColorTableDiffuseComposition,
) -> ResolvedColorTableBase {
    match (base_index, composition) {
        (Some(base_index), ColorTableDiffuseComposition::Multiply) => ResolvedColorTableBase {
            base_color: base_index,
            colorset_diffuse: Some(colorset_index),
        },
        _ => ResolvedColorTableBase {
            base_color: colorset_index,
            colorset_diffuse: None,
        },
    }
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
            texel_layout: ModelTextureTexelLayout::Standard,
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
        texel_layout: ModelTextureTexelLayout::Standard,
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
        reference_fallback: None,
        shader_package_name: None,
        render_mode: WeaponMaterialRenderMode::Opaque,
        alpha_mode: WeaponMaterialAlphaMode::Opaque,
        alpha_threshold: 0.0,
        draw_depth_mode: MaterialDrawDepthMode::None,
        lighting_mode: MaterialLightingMode::Default,
        flow_mode: MaterialFlowMode::Standard,
        specular_type: MaterialSpecularType::Default,
        specular_type_raw: None,
        value_mode: MaterialValueMode::Single,
        value_mode_raw: None,
        sub_color_mode: MaterialSubColorMode::None,
        decal_color_mode: MaterialDecalColorMode::Off,
        decal_color_mode_raw: None,
        skin_value_mode: MaterialSkinValueMode::None,
        character_scroll_variant: MaterialCharacterScrollVariant::None,
        character_scroll_variant_raw: None,
        lightshaft_type: MaterialLightShaftType::None,
        lightshaft_type_raw: None,
        transparency: 0.0,
        water_deep_color: [0.3529, 0.372_549, 0.3921, 1.0],
        water_refraction_color: [0.4117, 0.4313, 0.4509, 1.0],
        water_whitecap_color: [0.4509, 0.4705, 0.4901, 0.3],
        alpha_aperture: 2.0,
        alpha_offset: 0.0,
        vertex_alpha_to_one: 0.0,
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
        ambient_occlusion_mask: None,
        texture_mip_bias: 0.0,
        tile_mip_bias_offset: 0.0,
        vertex_movement_scale: 1.0,
        vertex_movement_max_length: 1.0,
        shadow_pos_offset: 0.0,
        detail_color_uv_scale: [4.0, 4.0, 4.0, 4.0],
        detail_normal_uv_scale: [4.0, 4.0, 4.0, 4.0],
        uv_scroll: [0.0, 0.0, 0.0, 0.0],
        color_uv_scale: [1.0; 4],
        normal_uv_scale: [1.0; 4],
        specular_uv_scale: [1.0; 4],
        white_eye_color: [1.0, 1.0, 1.0, 0.0],
        iris_ring_color: [1.0, 1.0, 1.0, 1.0],
        iris_ring_emissive_intensity: 0.25,
        iris_ring_uv_radius: [0.158, 0.174],
        iris_ring_uv_fade_width: [0.04, 0.02],
        lightshaft_color: [1.0, 1.0, 1.0, 1.0],
        lightshaft_tex_anim: [0.0, 0.0, 0.0, 0.0],
        lightshaft_tex_u: [1.0, 0.0, 0.0, 0.0],
        lightshaft_tex_v: [0.0, 1.0, 0.0, 0.0],
        lightshaft_ray: [0.0, 0.0, 0.0, 0.0],
        lightshaft_angle_clip: 0.0,
        lightshaft_near_clip: 0.25,
        opacity: 1.0,
        render_backfaces: true,
        apply_vertex_color: false,
        has_color_dye_table: false,
        color_dye_table: None,
        color_table_rows: None,
        staining_application: None,
        character_colors: None,
        texture_arrays: ModelMaterialTextureArrays::default(),
        fallback_color: fallback,
        diffuse_color: fallback,
        specular_color: [0.35, 0.35, 0.35],
        emissive_color: [0.0, 0.0, 0.0],
        roughness: 0.55,
        metalness: 0.0,
        texture_indices: Vec::new(),
        base_color_texture: None,
        colorset_diffuse_texture: None,
        secondary_base_color_texture: None,
        normal_texture: None,
        secondary_normal_texture: None,
        mask_texture: None,
        skin_diffuse_texture: None,
        skin_normal_texture: None,
        skin_mask_texture: None,
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
    let Some(source) = loaded_materials.iter().find(|source| {
        source.material_index == material.material_index
            && source.path.is_some()
            && source.reference_fallback.is_none()
    }) else {
        return material;
    };

    let reference_fallback = ModelMaterialReferenceFallback {
        kind: ModelMaterialReferenceFallbackKind::SameIndexLoadedMaterial,
        requested_name: material.name.clone(),
        source_slot: source.slot,
        source_material_index: source.material_index,
        source_name: source.name.clone(),
        source_path: source.path.clone().expect("filtered material path"),
    };
    let mut reused = source.clone();
    reused.slot = material.slot;
    reused.material_index = material.material_index;
    reused.name = material.name;
    reused.reference_fallback = Some(reference_fallback);
    reused
}

#[cfg(feature = "game-data")]
fn resolve_loaded_color_table_reference(
    slot: usize,
    material_index: u16,
    material_name: &str,
    material_path: &str,
    rows: &mut Option<Vec<ColorTableRowColors>>,
    dye_table: &mut Option<ModelColorDyeTable>,
    sources: &mut HashMap<u16, LoadedMaterialColorTable>,
) -> Option<ModelMaterialReferenceFallback> {
    if rows
        .as_deref()
        .is_some_and(color_table_rows_are_neutral_placeholder)
    {
        if let Some(source) = sources.get(&material_index).cloned() {
            *rows = Some(source.rows.clone());
            *dye_table = source.dye_table.clone();
            return Some(ModelMaterialReferenceFallback {
                kind: ModelMaterialReferenceFallbackKind::SameIndexLoadedColorTable,
                requested_name: material_name.to_string(),
                source_slot: source.slot,
                source_material_index: source.material_index,
                source_name: source.name,
                source_path: source.path,
            });
        }
    }

    if let Some(rows) = rows
        .as_ref()
        .filter(|rows| color_table_rows_have_visible_values(rows))
    {
        sources
            .entry(material_index)
            .or_insert_with(|| LoadedMaterialColorTable {
                slot,
                material_index,
                name: material_name.to_string(),
                path: material_path.to_string(),
                rows: rows.clone(),
                dye_table: dye_table.clone(),
            });
    }
    None
}

#[cfg(feature = "game-data")]
fn loaded_color_table_reference_index_texture(
    reference: Option<&ModelMaterialReferenceFallback>,
    loaded_materials: &[WeaponModelMaterial],
) -> Option<usize> {
    let reference = reference?;
    if !matches!(
        reference.kind,
        ModelMaterialReferenceFallbackKind::SameIndexLoadedColorTable
    ) {
        return None;
    }

    loaded_materials
        .iter()
        .find(|material| {
            material.slot == reference.source_slot
                && material.material_index == reference.source_material_index
        })
        .and_then(|material| material.index_texture)
}

#[cfg(feature = "game-data")]
fn color_table_rows_are_neutral_placeholder(rows: &[ColorTableRowColors]) -> bool {
    !rows.is_empty()
        && rows.iter().all(|row| {
            vec3_approximately(row.diffuse, [1.0; 3])
                && vec3_approximately(row.specular, [1.0; 3])
                && vec3_approximately(row.emissive, [0.0; 3])
        })
}

#[cfg(feature = "game-data")]
fn color_table_rows_have_visible_values(rows: &[ColorTableRowColors]) -> bool {
    rows.iter().any(|row| {
        !vec3_approximately(row.diffuse, [1.0; 3])
            || !vec3_approximately(row.specular, [1.0; 3])
            || !vec3_approximately(row.emissive, [0.0; 3])
    })
}

#[cfg(feature = "game-data")]
fn vec3_approximately(left: [f32; 3], right: [f32; 3]) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| (left - right).abs() <= 1.0e-4)
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
        .filter_map(|record| match (record.logical_role, record.kind) {
            (Some(logical_role), Some(kind)) => Some(MaterialSamplerRole {
                texture_index: record.texture_index,
                logical_role,
                kind,
            }),
            _ => None,
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
                logical_role: resolution.logical_role,
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
    shader_package_material_parameters_for_platform(bytes, physis::Platform::Win32)
        .into_iter()
        .filter_map(|parameter| {
            parameter
                .default_values
                .map(|values| (parameter.id, values))
        })
        .collect()
}

#[cfg(feature = "game-data")]
#[derive(Clone, Debug, PartialEq)]
struct ShaderPackageMaterialParameter {
    id: u32,
    byte_offset: u16,
    byte_size: u16,
    default_values: Option<Vec<f32>>,
}

#[cfg(feature = "game-data")]
fn shader_package_material_parameters_for_platform(
    bytes: &[u8],
    platform: physis::Platform,
) -> Vec<ShaderPackageMaterialParameter> {
    let Some(layout) = shader_package_material_defaults_layout(bytes, platform) else {
        return Vec::new();
    };

    let mut parameters = Vec::new();
    let mut parameter_offset = layout.parameter_offset;
    let defaults_offset = layout.defaults_offset;

    for _ in 0..layout.parameter_count {
        let Some(id) = read_shpk_u32(bytes, parameter_offset, platform) else {
            return parameters;
        };
        let Some(byte_offset) = read_shpk_u16(bytes, parameter_offset + 4, platform) else {
            return parameters;
        };
        let Some(byte_size) = read_shpk_u16(bytes, parameter_offset + 6, platform) else {
            return parameters;
        };
        let byte_offset_usize = usize::from(byte_offset);
        let byte_size_usize = usize::from(byte_size);
        let default_values = (layout.has_defaults
            && byte_size_usize % 4 == 0
            && byte_offset_usize.saturating_add(byte_size_usize) <= layout.defaults_size)
            .then(|| {
                defaults_offset
                    .checked_add(byte_offset_usize)
                    .and_then(|value_start| {
                        read_shpk_f32_values(bytes, value_start, byte_size_usize / 4, platform)
                    })
            })
            .flatten();
        parameters.push(ShaderPackageMaterialParameter {
            id,
            byte_offset,
            byte_size,
            default_values,
        });
        let Some(next) = checked_advance(parameter_offset, 8, bytes.len()) else {
            return parameters;
        };
        parameter_offset = next;
    }

    parameters
}

#[cfg(feature = "game-data")]
#[derive(Clone, Copy, Debug)]
struct ShaderPackageMaterialDefaultsLayout {
    parameter_offset: usize,
    defaults_offset: usize,
    defaults_size: usize,
    parameter_count: usize,
    has_defaults: bool,
}

#[cfg(feature = "game-data")]
fn shader_package_material_defaults_layout(
    bytes: &[u8],
    platform: physis::Platform,
) -> Option<ShaderPackageMaterialDefaultsLayout> {
    if bytes.get(0..4)? != b"ShPk" {
        return None;
    }

    let version = read_shpk_u32(bytes, 4, platform)?;
    let vertex_shader_count = usize::try_from(read_shpk_u32(bytes, 24, platform)?).ok()?;
    let pixel_shader_count = usize::try_from(read_shpk_u32(bytes, 28, platform)?).ok()?;
    let defaults_size = usize::try_from(read_shpk_u32(bytes, 32, platform)?).ok()?;
    let parameter_count = usize::from(read_shpk_u16(bytes, 36, platform)?);
    let has_defaults = read_shpk_u16(bytes, 38, platform)? != 0;
    if parameter_count == 0 {
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
        offset = shader_package_skip_shader(bytes, offset, version, platform)?;
    }

    let parameter_offset = offset;
    let defaults_offset = checked_advance(
        parameter_offset,
        parameter_count.saturating_mul(8),
        bytes.len(),
    )?;
    let stored_defaults_size = if has_defaults { defaults_size } else { 0 };
    checked_advance(defaults_offset, stored_defaults_size, bytes.len())?;

    Some(ShaderPackageMaterialDefaultsLayout {
        parameter_offset,
        defaults_offset,
        defaults_size,
        parameter_count,
        has_defaults,
    })
}

#[cfg(feature = "game-data")]
fn shader_package_skip_shader(
    bytes: &[u8],
    offset: usize,
    version: u32,
    platform: physis::Platform,
) -> Option<usize> {
    let scalar_count = usize::from(read_shpk_u16(bytes, offset + 8, platform)?);
    let resource_count = usize::from(read_shpk_u16(bytes, offset + 10, platform)?);
    let uav_count = usize::from(read_shpk_u16(bytes, offset + 12, platform)?);
    let texture_count = usize::from(read_shpk_u16(bytes, offset + 14, platform)?);
    let header_size = if version >= 0x0D01 { 20 } else { 16 };
    let parameter_count = scalar_count
        .saturating_add(resource_count)
        .saturating_add(uav_count)
        .saturating_add(texture_count);
    let offset = checked_advance(offset, header_size, bytes.len())?;
    checked_advance(offset, parameter_count.saturating_mul(16), bytes.len())
}

#[cfg(feature = "game-data")]
fn read_shpk_u16(bytes: &[u8], offset: usize, platform: physis::Platform) -> Option<u16> {
    let raw = bytes.get(offset..offset + 2)?.try_into().ok()?;
    Some(match platform {
        physis::Platform::PS3 => u16::from_be_bytes(raw),
        _ => u16::from_le_bytes(raw),
    })
}

#[cfg(feature = "game-data")]
fn read_shpk_u32(bytes: &[u8], offset: usize, platform: physis::Platform) -> Option<u32> {
    let raw = bytes.get(offset..offset + 4)?.try_into().ok()?;
    Some(match platform {
        physis::Platform::PS3 => u32::from_be_bytes(raw),
        _ => u32::from_le_bytes(raw),
    })
}

#[cfg(feature = "game-data")]
fn read_shpk_f32_values(
    bytes: &[u8],
    offset: usize,
    count: usize,
    platform: physis::Platform,
) -> Option<Vec<f32>> {
    (0..count)
        .map(|index| read_shpk_u32(bytes, offset + index * 4, platform).map(f32::from_bits))
        .collect()
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
fn sampler_role_for_texture(
    sampler_roles: &[MaterialSamplerRole],
    texture_index: usize,
) -> Option<MaterialSamplerRole> {
    sampler_roles
        .iter()
        .find(|role| role.texture_index == texture_index)
        .copied()
}

#[cfg(feature = "game-data")]
#[cfg(test)]
fn classify_sampler_usage(texture_usage: u32) -> Option<WeaponModelTextureKind> {
    known_sampler_names()
        .iter()
        .find(|(name, _)| physis::shpk::ShaderPackage::crc(name) == texture_usage)
        .map(|(_, logical_role)| logical_role.texture_kind())
}

#[cfg(feature = "game-data")]
#[cfg(test)]
fn classify_sampler_name(name: &str) -> Option<WeaponModelTextureKind> {
    classify_sampler_logical_role_name(name).map(MaterialSamplerLogicalRole::texture_kind)
}

#[cfg(feature = "game-data")]
fn classify_sampler_logical_role_name(name: &str) -> Option<MaterialSamplerLogicalRole> {
    known_sampler_names()
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        .map(|(_, logical_role)| *logical_role)
}

#[cfg(feature = "game-data")]
fn known_sampler_names() -> &'static [(&'static str, MaterialSamplerLogicalRole)] {
    use MaterialSamplerLogicalRole as Role;

    &[
        ("g_SamplerNormal", Role::Normal),
        ("g_NormalSampler", Role::Normal),
        ("g_SamplerNormalMap", Role::Normal),
        ("g_NormalMapSampler", Role::Normal),
        ("g_SamplerNormalMap0", Role::Normal),
        ("g_SamplerNormalMap1", Role::SecondaryNormal),
        ("g_SamplerSkinNormal", Role::SkinNormal),
        ("g_SamplerEmissive", Role::Emissive),
        ("g_EmissiveSampler", Role::Emissive),
        ("g_SamplerEmission", Role::Emissive),
        ("g_EmissionSampler", Role::Emissive),
        ("g_SamplerLight", Role::Emissive),
        ("g_LightSampler", Role::Emissive),
        ("g_SamplerIndex", Role::Index),
        ("g_IndexSampler", Role::Index),
        ("g_SamplerMask", Role::Mask),
        ("g_MaskSampler", Role::Mask),
        ("g_SamplerSkinMask", Role::SkinMask),
        ("g_SamplerMaterial", Role::MaterialMap),
        ("g_MaterialSampler", Role::MaterialMap),
        ("g_SamplerMulti", Role::MultiMap),
        ("g_MultiSampler", Role::MultiMap),
        ("g_SamplerSpecular", Role::Specular),
        ("g_SpecularSampler", Role::Specular),
        ("g_SamplerSpecularMap", Role::Specular),
        ("g_SpecularMapSampler", Role::Specular),
        ("g_SamplerSpecularMap0", Role::Specular),
        ("g_SamplerSpecularMap1", Role::SecondarySpecular),
        ("g_SamplerReflect", Role::Specular),
        ("g_ReflectSampler", Role::Specular),
        ("g_SamplerDiffuse", Role::BaseColor),
        ("g_DiffuseSampler", Role::BaseColor),
        ("g_SamplerColor", Role::BaseColor),
        ("g_ColorSampler", Role::BaseColor),
        ("g_SamplerColorMap", Role::BaseColor),
        ("g_ColorMapSampler", Role::BaseColor),
        ("g_SamplerColorMap0", Role::BaseColor),
        ("g_SamplerColorMap1", Role::SecondaryBaseColor),
        ("g_SamplerSkinDiffuse", Role::SkinDiffuse),
        ("g_SamplerAlbedo", Role::BaseColor),
        ("g_AlbedoSampler", Role::BaseColor),
        ("g_SamplerBaseColor", Role::BaseColor),
        ("g_BaseColorSampler", Role::BaseColor),
        ("g_Sampler0", Role::BaseColor),
        ("g_Sampler1", Role::SecondaryBaseColor),
        ("g_SamplerEnvMap", Role::Environment),
        ("g_SamplerWaveMap", Role::WaterWave),
        ("g_SamplerWaveMap1", Role::WaterWaveSecondary),
        ("g_SamplerWhitecapMap", Role::WaterWhitecap),
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
    #[test]
    #[cfg(feature = "game-data")]
    fn composed_iris_ring_constants_use_meddle_defaults_and_overrides() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_white_eye_color(&semantics),
            [1.0, 1.0, 1.0, 0.0]
        );
        assert_eq!(
            composed_material_iris_ring_color(&semantics),
            [1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(
            composed_material_iris_ring_emissive_intensity(&semantics),
            0.25
        );
        assert_eq!(
            composed_material_iris_ring_uv_radius(&semantics),
            [0.158, 0.174]
        );
        assert_eq!(
            composed_material_iris_ring_uv_fade_width(&semantics),
            [0.04, 0.02]
        );

        semantics.material_constants.insert(
            G_IRIS_RING_COLOR,
            ResolvedMaterialValue {
                value: vec![0.2, 0.4, 0.6],
                source: "test",
            },
        );
        semantics.material_constants.insert(
            G_IRIS_RING_EMISSIVE_INTENSITY,
            ResolvedMaterialValue {
                value: vec![2.0],
                source: "test",
            },
        );
        semantics.material_constants.insert(
            G_IRIS_RING_UV_RADIUS,
            ResolvedMaterialValue {
                value: vec![0.2, 0.3],
                source: "test",
            },
        );
        assert_eq!(
            composed_material_iris_ring_color(&semantics),
            [0.2, 0.4, 0.6, 1.0]
        );
        assert_eq!(
            composed_material_iris_ring_emissive_intensity(&semantics),
            2.0
        );
        assert_eq!(
            composed_material_iris_ring_uv_radius(&semantics),
            [0.2, 0.3]
        );
    }

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
    fn equipment_model_load_request_decodes_and_builds() {
        let item = WeaponCatalogItem {
            id: 7,
            name: "test gloves".to_string(),
            description: String::new(),
            icon: 0,
            item_ui_category: 0,
            item_search_category: 0,
            equip_slot_category: 5,
            price_mid: 0,
            price_low: 0,
            model_main: 0x0000_0000_0001_2276,
            model_sub: 0,
        };
        let request = EquipmentModelLoadRequest::from(&item)
            .with_race_id(401)
            .with_stain_ids([1, u8::MAX]);

        assert_eq!(request.equip_slot_category, 5);
        assert_eq!(request.race_id, 401);
        let model = request.primary_model();
        assert_eq!(model.set_id, 8_822);
        assert_eq!(model.variant_id, 1);
        assert!(request.secondary_model().is_none());
        assert_eq!(request.normalized_stain_ids(), [1, 0]);
    }

    #[test]
    fn runtime_staining_reuses_base_rows_and_non_color_table_assets() {
        let base_row = ColorTableRowColors {
            diffuse: [0.1, 0.2, 0.3],
            specular: [0.2, 0.3, 0.4],
            ..ColorTableRowColors::default()
        };
        let dye_row = ModelLegacyColorDyeTableRow {
            template: 42,
            diffuse: true,
            specular: false,
            emissive: false,
            gloss: false,
            specular_strength: false,
        };
        let mut material = fallback_weapon_material(0, 0, "test.mtrl".to_string(), [0.5; 3]);
        material.path = Some("chara/weapon/test.mtrl".to_string());
        material.has_color_dye_table = true;
        material.color_dye_table = Some(ModelColorDyeTable::Legacy(vec![dye_row.clone(), dye_row]));
        material.color_table_rows = Some(vec![base_row, base_row]);
        material.index_texture = Some(0);
        material.texture_indices = vec![0];

        let base = WeaponModelData {
            item_id: 1,
            item_name: "test".to_string(),
            model_main: PackedModelId::from_raw(1),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics: Vec::new(),
            loaded_paths: vec!["model.mdl".to_string(), "test.mtrl".to_string()],
            bounds: ModelBounds::default(),
            materials: vec![material],
            textures: vec![WeaponModelTexture {
                path: "index.tex".to_string(),
                kind: WeaponModelTextureKind::Index,
                texel_layout: ModelTextureTexelLayout::Standard,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![0, 0, 0, 255],
                rgba_f32: None,
            }],
            meshes: Vec::new(),
        };
        let templates = WeaponStainingTemplates::from_load_results(
            Ok(legacy_staining_fixture(42, [0.8, 0.4, 0.2])),
            Err("not needed".to_string()),
        );

        let neutral = apply_weapon_model_stains(&base, [0, 0], &templates);
        let stained = apply_weapon_model_stains(&neutral, [1, 0], &templates);
        let reset = apply_weapon_model_stains(&stained, [0, 0], &templates);
        let neutral_diffuse = color_table_baked_texture(&neutral, "#colorset-diffuse");
        let stained_diffuse = color_table_baked_texture(&stained, "#colorset-diffuse");
        let reset_diffuse = color_table_baked_texture(&reset, "#colorset-diffuse");

        assert_ne!(neutral_diffuse.rgba, stained_diffuse.rgba);
        assert_eq!(neutral_diffuse.rgba, reset_diffuse.rgba);
        assert_eq!(stained.loaded_paths, base.loaded_paths);
        assert_eq!(stained.meshes, base.meshes);
        assert_eq!(stained.textures[0], base.textures[0]);
        assert_eq!(
            stained.materials[0].color_table_rows,
            Some(vec![base_row, base_row])
        );
        assert_eq!(
            stained.materials[0]
                .staining_application
                .as_ref()
                .map(|application| application.report.rows_changed),
            Some(2)
        );
    }

    fn color_table_baked_texture<'a>(
        model: &'a WeaponModelData,
        suffix: &str,
    ) -> &'a WeaponModelTexture {
        model
            .textures
            .iter()
            .find(|texture| texture.path.ends_with(suffix))
            .unwrap_or_else(|| panic!("missing baked texture {suffix}"))
    }

    fn legacy_staining_fixture(template: u32, diffuse: [f32; 3]) -> Vec<u8> {
        const STM_MAGIC: u16 = 0x534d;
        const STM_VERSION_LEGACY: u16 = 0x0101;
        let columns = vec![
            diffuse
                .into_iter()
                .flat_map(|value| half::f16::from_f32(value).to_bits().to_le_bytes())
                .collect::<Vec<_>>(),
            vec![0; 6],
            vec![0; 6],
            half::f16::from_f32(1.0).to_bits().to_le_bytes().to_vec(),
            half::f16::from_f32(1.0).to_bits().to_le_bytes().to_vec(),
        ];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&STM_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&STM_VERSION_LEGACY.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&template.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        let mut cumulative_bytes = 0_usize;
        for column in &columns {
            cumulative_bytes += column.len();
            bytes.extend_from_slice(
                &u16::try_from(cumulative_bytes / 2)
                    .expect("fixture offset")
                    .to_le_bytes(),
            );
        }
        for column in columns {
            bytes.extend_from_slice(&column);
        }
        bytes
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
    fn installed_45068_compatibility_multiplies_colorset_diffuse() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 45068,
            item_name: "菜蔬之幻梦".to_string(),
            model_main: 4_295_032_946,
            model_sub: 0,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let material = model.materials.first().expect("45068 material");
        let base = &model.textures[material.base_color_texture.expect("active base texture")];
        let prepared =
            crate::model::prepare_material_for_draw_role(Some(material), ModelMeshDrawRole::Normal);

        assert_eq!(material.value_mode, MaterialValueMode::Compatibility);
        assert_eq!(material.vertex_movement_scale, 0.0);
        assert_eq!(material.vertex_movement_max_length, 0.0);
        assert!(prepared.unsupported_inputs.vertex_movement_parameters);
        assert!(base.path.ends_with("v01_w0114b0001_base.tex"));
        assert_eq!(base.texel_layout, ModelTextureTexelLayout::Standard);
        let colorset_diffuse = &model.textures[material
            .colorset_diffuse_texture
            .expect("colorset diffuse ramp")];
        assert!(colorset_diffuse.path.ends_with("#colorset-diffuse"));
        assert_eq!(
            colorset_diffuse.texel_layout,
            ModelTextureTexelLayout::ColorTableRampAb
        );
        assert_eq!(
            colorset_diffuse.rgba_f32.as_ref().map(Vec::len),
            Some(usize::from(colorset_diffuse.width) * usize::from(colorset_diffuse.height))
        );
        assert!(
            colorset_diffuse
                .rgba_f32
                .as_deref()
                .expect("Compatibility float diffuse payload")
                .iter()
                .all(|pixel| pixel.iter().all(|value| value.is_finite()))
        );
        assert!(material.texture_indices.iter().any(|index| {
            model.textures[*index]
                .path
                .ends_with("v01_w0114b0001_base.tex")
        }));
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_audited_tile_mip_bias_offsets_are_preserved() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        for (item_id, item_name, model_main, expected_bias, expected_scale) in [
            (47320, "改良型护锁刃魔导典", 4_295_034_596, 1.0, 1.010_056),
            (46462, "女王骑士之典", 8_590_591_710, -1.0, 1.0),
        ] {
            let request = WeaponModelLoadRequest {
                item_id,
                item_name: item_name.to_string(),
                model_main,
                model_sub: 0,
                stain_ids: [0, 0],
            };
            let model = load_weapon_model_from_resource_request(&mut resource, &request)
                .unwrap_or_else(|error| panic!("{item_id}: {error:#}"));
            let material = model
                .materials
                .iter()
                .find(|material| material.tile_mip_bias_offset == expected_bias)
                .unwrap_or_else(|| panic!("{item_id}: audited tile bias material"));
            let prepared = crate::model::prepare_material_for_draw_role(
                Some(material),
                ModelMeshDrawRole::Normal,
            );

            assert_eq!(material.tile_mip_bias_offset, expected_bias);
            assert!((material.vertex_movement_scale - expected_scale).abs() < 0.000_01);
            assert!(!prepared.unsupported_inputs.tile_mip_bias_offset);
        }
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_legacy_specular_mask_is_structured_and_supported() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 30520,
            item_name: "改良型伊修加德新型天星盘".to_string(),
            model_main: 8_593_934_389,
            model_sub: 0,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let material = model
            .materials
            .iter()
            .find(|material| material.specular_type == MaterialSpecularType::Mask)
            .expect("legacy specular mask material");
        let prepared =
            crate::model::prepare_material_for_draw_role(Some(material), ModelMeshDrawRole::Normal);

        assert_eq!(material.specular_type_raw, Some(SPECULAR_TYPE_MASK));
        assert!(!prepared.unsupported_inputs.legacy_specular_type);
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
        let mut template_paths = Vec::new();
        let templates = load_weapon_staining_templates_from_resource(
            &mut resource,
            [1, 0],
            &mut template_paths,
        );
        let stained = apply_weapon_model_stains(&unstained, [1, 0], &templates);

        assert_eq!(unstained.stain_ids, [0, 0]);
        assert_eq!(stained.stain_ids, [1, 0]);
        assert_eq!(stained.loaded_paths, unstained.loaded_paths);
        assert!(
            template_paths
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
        assert_eq!(roles[0].logical_role, MaterialSamplerLogicalRole::Normal);
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
    fn known_shader_labels_include_all_meddle_decal_color_values() {
        for (value, expected) in [
            (GET_DECAL_COLOR_OFF, "GetDecalColorOff"),
            (GET_DECAL_COLOR_ALPHA, "GetDecalColorAlpha"),
            (GET_DECAL_COLOR_RGBA, "GetDecalColorRGBA"),
        ] {
            assert_eq!(known_shader_label(value).as_deref(), Some(expected));
        }
        assert_eq!(known_shader_label(0xDEAD_BEEF), None);
    }

    #[test]
    fn known_semantic_labels_cover_audited_material_inputs() {
        for (value, expected) in [
            (CATEGORY_SPECULAR_TYPE, "CategorySpecularType"),
            (SPECULAR_TYPE_DEFAULT, "Default"),
            (SPECULAR_TYPE_MASK, "Mask"),
            (GET_VALUES_MULTI_MATERIAL, "GetValuesMultiMaterial"),
            (DRAW_DEPTH_MODE, "DrawDepthMode"),
            (ENABLE_LIGHTING, "EnableLighting"),
            (0x87D8_F48A, "ApplyVertexMovement"),
            (0xF8CA_223F, "ApplyVertexMovementOff"),
            (0xA657_DE89, "ApplyVertexMovementOn"),
            (0xDCFC_844E, "ApplyAlphaClip"),
            (0x7D50_81DF, "ApplyAlphaClipOff"),
            (0x59C4_E6DB, "ApplyAlphaClipOn"),
        ] {
            assert_eq!(known_shader_label(value).as_deref(), Some(expected));
        }
        for (value, expected) in [
            (G_TILE_MIP_BIAS_OFFSET, "g_TileMipBiasOffset"),
            (G_VERTEX_MOVEMENT_SCALE, "g_VertexMovementScale"),
            (G_VERTEX_MOVEMENT_MAX_LENGTH, "g_VertexMovementMaxLength"),
            (G_AMBIENT_OCCLUSION_MASK, "g_AmbientOcclusionMask"),
            (G_VERTEX_ALPHA_TO_ONE, "VertexAlphaToOne"),
        ] {
            assert_eq!(
                known_material_constant_name(value).as_deref(),
                Some(expected)
            );
        }
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
        assert_eq!(roles[0].logical_role, MaterialSamplerLogicalRole::Index);
        assert_eq!(roles[0].kind, WeaponModelTextureKind::Index);

        let records = parse_material_sampler_records(&bytes, &semantics);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].texture_usage_name.as_deref(), Some(sampler_name));
        assert_eq!(
            records[0].logical_role,
            Some(MaterialSamplerLogicalRole::Index)
        );
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
        assert_eq!(
            records[0].logical_role,
            Some(MaterialSamplerLogicalRole::Normal)
        );
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
        assert_eq!(records[0].logical_role, None);
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
    fn material_texture_paths_follow_duplicate_texture_offsets() {
        use physis::ReadableFile;

        let bytes = test_mtrl_with_duplicate_texture_offsets();
        let material = physis::mtrl::Material::from_existing(physis::Platform::Win32, &bytes)
            .expect("material");

        assert_eq!(
            material_texture_paths_from_offsets(&bytes, &material.texture_paths),
            vec![
                "dummy.tex".to_string(),
                "dummy.tex".to_string(),
                "chara/weapon/w2651/obj/body/b0059/texture/v01_w2651b0059_id.tex".to_string(),
            ]
        );
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
                logical_role: record.logical_role,
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
            texel_layout: ModelTextureTexelLayout::Standard,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 1,
            rgba: vec![0, 255, 0, 255],
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
        assert_eq!(tile_matrix.width, 2);
        assert_eq!(
            tile_matrix.texel_layout,
            ModelTextureTexelLayout::ColorTableTileRampAb
        );
        assert_eq!(
            textures[baked.tile_properties].texel_layout,
            ModelTextureTexelLayout::ColorTableTileRampAb
        );
        assert_eq!(
            tile_matrix.rgba_f32,
            Some(vec![[2.0, -0.5, 0.25, 1.5], [0.0; 4]])
        );
        assert_eq!(textures[baked.base_color].rgba[3], 255);
    }

    #[test]
    fn baked_diffuse_specular_emissive_sheen_and_sphere_textures_preserve_float_channels() {
        let rows = vec![
            ColorTableRowColors {
                diffuse: [6.7929688, 2.0, 0.5],
                specular: [0.25, 0.5, 0.75],
                anisotropy: 7.0,
                emissive: [61.46875, 2.0, 0.5],
                sheen_aperture: 4.0,
                sphere_index: 2.0,
                ..Default::default()
            },
            ColorTableRowColors::default(),
        ];
        let mut textures = vec![WeaponModelTexture {
            path: "index.tex".to_string(),
            kind: WeaponModelTextureKind::Index,
            texel_layout: ModelTextureTexelLayout::Standard,
            width: 1,
            height: 1,
            array_size: 1,
            array_layer_height: 1,
            rgba: vec![0, 255, 0, 255],
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

        assert_eq!(
            textures[baked.base_color].rgba_f32,
            Some(vec![[6.7929688, 2.0, 0.5, 1.0], [0.0, 0.0, 0.0, 1.0]])
        );
        assert_eq!(textures[baked.specular].rgba[3], 255);
        assert_eq!(
            textures[baked.specular].rgba_f32,
            Some(vec![[0.25, 0.5, 0.75, 7.0], [0.0, 0.0, 0.0, 0.0]])
        );
        assert_eq!(textures[baked.sheen_properties].rgba[2], 255);
        assert_eq!(
            textures[baked.sheen_properties].rgba_f32,
            Some(vec![[0.0, 0.0, 4.0, 1.0], [0.0, 0.0, 0.0, 1.0]])
        );
        assert_eq!(textures[baked.sphere_properties].rgba[0], 2);
        assert_eq!(
            textures[baked.sphere_properties].rgba_f32,
            Some(vec![[2.0 / 255.0, 0.0, 1.0, 1.0], [0.0, 0.0, 1.0, 1.0],])
        );
        let emissive = baked.emissive.expect("HDR emissive texture");
        assert_eq!(
            textures[emissive].rgba_f32,
            Some(vec![[61.46875, 2.0, 0.5, 1.0], [0.0, 0.0, 0.0, 1.0]])
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
    fn installed_45063_reuses_primary_index_for_missing_duplicate_offset_texture() {
        const PRIMARY_INDEX_PATH: &str =
            "chara/weapon/w2601/obj/body/b0059/texture/v01_w2601b0059_id.tex";
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let request = WeaponModelLoadRequest {
            item_id: 45_063,
            item_name: "夏火之幻梦".to_string(),
            model_main: 4_298_836_521,
            model_sub: 4_298_836_571,
            stain_ids: [0, 0],
        };
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let primary = model
            .materials
            .iter()
            .find(|material| {
                material
                    .path
                    .as_deref()
                    .is_some_and(|path| path.contains("/w2601/"))
            })
            .expect("45063 primary material");
        let secondary = model
            .materials
            .iter()
            .find(|material| {
                material
                    .path
                    .as_deref()
                    .is_some_and(|path| path.contains("/w2651/"))
            })
            .expect("45063 secondary material");
        let index_texture = secondary.index_texture.expect("secondary index binding");

        assert_eq!(secondary.index_texture, primary.index_texture);
        assert_eq!(model.textures[index_texture].path, PRIMARY_INDEX_PATH);
        assert_eq!(
            model.textures[index_texture].kind,
            WeaponModelTextureKind::Index
        );
        assert!(secondary.base_color_texture.is_some());
        assert!(
            model
                .loaded_paths
                .iter()
                .any(|path| path == PRIMARY_INDEX_PATH)
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
        assert_eq!(skin_material.skin_value_mode, MaterialSkinValueMode::Body);
        assert_eq!(prepared.skin_value_mode, MaterialSkinValueMode::Body);
        assert_eq!(
            prepared.texture_sampling.base_color.address_mode,
            crate::model::PreparedTextureAddressMode::Repeat
        );
        assert_eq!(
            prepared.shader_family,
            crate::model::MaterialShaderFamily::Skin
        );
        assert!(prepared.unsupported_inputs.runtime_skin_color);
        assert!(model.bounds.radius.is_finite() && model.bounds.radius > 0.0);
    }

    #[test]
    #[ignore = "requires an installed FFXIV game directory"]
    fn installed_equipment_fist_preserves_shape_vertex_targets() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let request = WeaponModelLoadRequest {
            item_id: 42_697,
            item_name: "新生王国指虎".to_string(),
            model_main: 74_357,
            model_sub: 0,
            stain_ids: [0, 0],
        };
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let model =
            load_weapon_model_from_resource_request(&mut resource, &request).expect("weapon");
        let mesh = model
            .meshes
            .iter()
            .find(|mesh| !mesh.shape_targets.is_empty())
            .expect("mesh with shape targets");

        assert!(mesh.shape_targets.iter().any(|target| {
            target.shape.name.as_deref() == Some("shp_arm") && !target.vertex_deltas.is_empty()
        }));
        assert!(mesh.shape_targets.iter().all(|target| {
            target
                .vertex_deltas
                .iter()
                .all(|delta| delta.vertex_index < mesh.vertices.len() as u32)
        }));
        let shaped = crate::model::model_mesh_vertices_with_shape_mask(mesh, Some(1));
        assert!(
            shaped
                .iter()
                .zip(&mesh.vertices)
                .any(|(shaped, base)| shaped.position != base.position
                    || shaped.normal != base.normal)
        );
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
        assert_eq!(
            reused.reference_fallback,
            Some(ModelMaterialReferenceFallback {
                kind: ModelMaterialReferenceFallbackKind::SameIndexLoadedMaterial,
                requested_name: "/mt_w3103b0001_a.mtrl".to_string(),
                source_slot: 0,
                source_material_index: 0,
                source_name: "/mt_w3004b0001_a.mtrl".to_string(),
                source_path:
                    "chara/weapon/w3004/obj/body/b0001/material/v0001/mt_w3004b0001_a.mtrl"
                        .to_string(),
            })
        );
    }

    #[test]
    fn neutral_secondary_color_table_inherits_loaded_same_index_rows() {
        let colorful_row = ColorTableRowColors {
            diffuse: [0.2, 0.4, 0.6],
            specular: [0.7, 0.8, 0.9],
            ..ColorTableRowColors::default()
        };
        let neutral_row = ColorTableRowColors {
            diffuse: [1.0; 3],
            specular: [1.0; 3],
            emissive: [0.0; 3],
            ..ColorTableRowColors::default()
        };
        let mut sources = HashMap::new();
        let mut primary_rows = Some(vec![colorful_row]);
        let mut primary_dye = None;
        assert_eq!(
            resolve_loaded_color_table_reference(
                0,
                0,
                "/mt_primary.mtrl",
                "primary.mtrl",
                &mut primary_rows,
                &mut primary_dye,
                &mut sources,
            ),
            None
        );

        let mut secondary_rows = Some(vec![neutral_row]);
        let mut secondary_dye = None;
        let fallback = resolve_loaded_color_table_reference(
            1,
            0,
            "/mt_secondary.mtrl",
            "secondary.mtrl",
            &mut secondary_rows,
            &mut secondary_dye,
            &mut sources,
        );

        assert_eq!(secondary_rows, Some(vec![colorful_row]));
        assert_eq!(
            fallback.map(|fallback| fallback.kind),
            Some(ModelMaterialReferenceFallbackKind::SameIndexLoadedColorTable)
        );
    }

    #[test]
    fn inherited_color_table_fills_only_the_missing_index_texture() {
        let mut source = fallback_weapon_material(0, 0, "/mt_primary.mtrl".to_string(), [1.0; 3]);
        source.path = Some("primary.mtrl".to_string());
        source.normal_texture = Some(0);
        source.index_texture = Some(1);
        let reference = ModelMaterialReferenceFallback {
            kind: ModelMaterialReferenceFallbackKind::SameIndexLoadedColorTable,
            requested_name: "/mt_secondary.mtrl".to_string(),
            source_slot: 0,
            source_material_index: 0,
            source_name: source.name.clone(),
            source_path: source.path.clone().expect("source path"),
        };
        let textures = vec![
            WeaponModelTexture {
                path: "normal.tex".to_string(),
                kind: WeaponModelTextureKind::Normal,
                texel_layout: ModelTextureTexelLayout::Standard,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![128, 128, 255, 255],
                rgba_f32: None,
            },
            WeaponModelTexture {
                path: "index.tex".to_string(),
                kind: WeaponModelTextureKind::Index,
                texel_layout: ModelTextureTexelLayout::Standard,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![0, 0, 0, 255],
                rgba_f32: None,
            },
        ];
        let fallback_index = loaded_color_table_reference_index_texture(
            Some(&reference),
            std::slice::from_ref(&source),
        );
        let mut set = WeaponTextureSet::default();

        apply_color_table_index_fallback(&mut set, fallback_index, &textures);

        assert_eq!(set.index, Some(1));
        assert_eq!(set.indices, vec![1]);
        assert_eq!(set.normal, None);
        assert_eq!(set.mask, None);
    }

    #[test]
    fn missing_material_reference_does_not_chain_reused_materials() {
        let mut reused_source =
            fallback_weapon_material(1, 0, "/mt_reused.mtrl".to_string(), [0.1, 0.2, 0.3]);
        reused_source.path = Some("source.mtrl".to_string());
        reused_source.reference_fallback = Some(ModelMaterialReferenceFallback {
            kind: ModelMaterialReferenceFallbackKind::SameIndexLoadedMaterial,
            requested_name: "/mt_reused.mtrl".to_string(),
            source_slot: 0,
            source_material_index: 0,
            source_name: "/mt_source.mtrl".to_string(),
            source_path: "source.mtrl".to_string(),
        });
        let missing =
            fallback_weapon_material(2, 0, "/mt_missing.mtrl".to_string(), [0.4, 0.5, 0.6]);

        let unresolved = reuse_loaded_material_for_missing_reference(missing, &[reused_source]);

        assert_eq!(unresolved.path, None);
        assert_eq!(unresolved.reference_fallback, None);
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
        assert_eq!(
            secondary.reference_fallback,
            Some(ModelMaterialReferenceFallback {
                kind: ModelMaterialReferenceFallbackKind::SameIndexLoadedMaterial,
                requested_name: "/mt_w3103b0001_a.mtrl".to_string(),
                source_slot: 0,
                source_material_index: 0,
                source_name: "/mt_w3004b0001_a.mtrl".to_string(),
                source_path:
                    "chara/weapon/w3004/obj/body/b0001/material/v0002/mt_w3004b0001_a.mtrl"
                        .to_string(),
            })
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
    fn shader_package_semantic_debug_preserves_zero_width_material_parameters() {
        let bytes = test_shpk_with_material_defaults(&[
            (G_ALPHA_THRESHOLD, &[0.35]),
            (G_AMBIENT_OCCLUSION_MASK, &[]),
        ]);

        let debug =
            shader_package_semantic_debug_from_shpk_bytes("shader/sm5/shpk/character.shpk", &bytes)
                .expect("shader package debug");

        assert_eq!(debug.material_constants.len(), 2);
        assert_eq!(debug.material_constants[1].id, G_AMBIENT_OCCLUSION_MASK);
        assert_eq!(debug.material_constants[1].byte_size, 0);
        assert_eq!(debug.material_constants[1].default_values, Some(Vec::new()));
    }

    #[test]
    fn shader_package_semantic_debug_preserves_parameters_without_defaults() {
        let bytes = test_shpk_without_material_defaults(&[(G_AMBIENT_OCCLUSION_MASK, 4)]);

        let debug =
            shader_package_semantic_debug_from_shpk_bytes("shader/sm5/shpk/character.shpk", &bytes)
                .expect("shader package debug");

        assert_eq!(debug.material_constants.len(), 1);
        assert_eq!(debug.material_constants[0].id, G_AMBIENT_OCCLUSION_MASK);
        assert_eq!(debug.material_constants[0].byte_size, 4);
        assert_eq!(debug.material_constants[0].default_values, None);
        assert!(shader_package_material_defaults(&bytes).is_empty());
    }

    #[test]
    fn shader_package_semantic_debug_reads_ps3_material_defaults_as_big_endian() {
        let bytes = test_shpk_with_semantics_for_platform(
            &[(G_ALPHA_THRESHOLD, &[0.35])],
            &[],
            &[],
            &[],
            physis::Platform::PS3,
        );

        let debug = shader_package_semantic_debug_from_bytes_for_platform(
            "shader/sm5/shpk/character.shpk",
            &bytes,
            physis::Platform::PS3,
        )
        .expect("PS3 shader package debug");

        assert_eq!(debug.material_constants.len(), 1);
        assert_eq!(debug.material_constants[0].id, G_ALPHA_THRESHOLD);
        assert_eq!(debug.material_constants[0].default_values, Some(vec![0.35]));
    }

    #[test]
    fn shader_package_semantic_debug_preserves_key_scopes_and_constant_defaults() {
        let bytes = test_shpk_with_semantics(
            &[(G_ALPHA_THRESHOLD, &[0.35, 0.75])],
            &[(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON)],
            &[(GET_VALUES, GET_VALUES_COMPATIBILITY)],
            &[(GET_DECAL_COLOR, GET_DECAL_COLOR_RGBA)],
        );

        let debug = shader_package_semantic_debug_from_shpk_bytes(
            r"Shader\SM5\ShPk\Character.ShPk",
            &bytes,
        )
        .expect("shader package debug");

        assert_eq!(debug.path, "shader/sm5/shpk/character.shpk");
        assert_eq!(debug.name, "character.shpk");
        assert_eq!(
            debug.material_keys,
            vec![ShaderPackageKeyDefaultDebug {
                id: APPLY_ALPHA_TEST,
                id_hex: hex_u32(APPLY_ALPHA_TEST),
                name: Some("ApplyAlphaTest".to_string()),
                default_value: APPLY_ALPHA_TEST_ON,
                default_value_hex: hex_u32(APPLY_ALPHA_TEST_ON),
                default_value_name: Some("ApplyAlphaTestOn".to_string()),
            }]
        );
        assert_eq!(debug.system_keys[0].id, GET_VALUES);
        assert_eq!(
            debug.system_keys[0].default_value_name.as_deref(),
            Some("GetValuesCompatibility")
        );
        assert_eq!(debug.scene_keys[0].id, GET_DECAL_COLOR);
        assert_eq!(
            debug.scene_keys[0].default_value_name.as_deref(),
            Some("GetDecalColorRGBA")
        );
        assert_eq!(
            debug.material_constants,
            vec![ShaderPackageMaterialConstantDebug {
                id: G_ALPHA_THRESHOLD,
                id_hex: hex_u32(G_ALPHA_THRESHOLD),
                name: Some("g_AlphaThreshold".to_string()),
                byte_offset: 0,
                byte_size: 8,
                default_values: Some(vec![0.35, 0.75]),
            }]
        );
    }

    #[test]
    fn shader_package_semantic_debug_from_resource_resolves_package_name() {
        let bytes =
            test_shpk_with_semantics(&[], &[(APPLY_ALPHA_TEST, APPLY_ALPHA_TEST_ON)], &[], &[]);
        let path = "shader/sm5/shpk/character.shpk";
        let expected = shader_package_semantic_debug_from_shpk_bytes(path, &bytes)
            .expect("expected shader package debug");
        let mut resource = TestShaderPackageResource {
            path: path.to_string(),
            bytes,
        };

        let actual = shader_package_semantic_debug_from_resource(&mut resource, "character.shpk")
            .expect("resource shader package debug");

        assert_eq!(actual, expected);
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
    fn composed_specular_type_preserves_default_mask_and_unknown_raw_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_specular_type(&semantics),
            (MaterialSpecularType::Default, None)
        );

        semantics.apply_shader_package_key_default(CATEGORY_SPECULAR_TYPE, SPECULAR_TYPE_DEFAULT);
        assert_eq!(
            composed_material_specular_type(&semantics),
            (MaterialSpecularType::Default, Some(SPECULAR_TYPE_DEFAULT))
        );

        semantics.apply_material_key(CATEGORY_SPECULAR_TYPE, SPECULAR_TYPE_MASK);
        assert_eq!(
            composed_material_specular_type(&semantics),
            (MaterialSpecularType::Mask, Some(SPECULAR_TYPE_MASK))
        );

        semantics.apply_material_key(CATEGORY_SPECULAR_TYPE, 0xDEAD_BEEF);
        assert_eq!(
            composed_material_specular_type(&semantics),
            (MaterialSpecularType::Unknown, Some(0xDEAD_BEEF))
        );
    }

    #[test]
    fn composed_get_values_mode_preserves_known_and_unknown_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_value_mode(&semantics),
            (MaterialValueMode::Single, None)
        );

        semantics.apply_shader_package_key_default(GET_VALUES, GET_ALPHA_MULTI_VALUES);
        assert_eq!(
            composed_material_value_mode(&semantics),
            (MaterialValueMode::AlphaMulti, Some(GET_ALPHA_MULTI_VALUES))
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
            assert_eq!(
                composed_material_value_mode(&semantics),
                (expected, Some(value))
            );
        }
    }

    #[test]
    fn composed_character_compatibility_gate_accepts_current_and_legacy_keys() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert!(!composed_material_uses_compatibility_values(&semantics));

        semantics.apply_material_key(GET_VALUES, GET_VALUES_COMPATIBILITY);
        assert!(composed_material_uses_compatibility_values(&semantics));

        semantics.apply_material_key(GET_VALUES, GET_VALUES_MULTI_MATERIAL);
        assert!(!composed_material_uses_compatibility_values(&semantics));

        semantics.apply_material_key(
            GET_VALUES_TEXTURE_TYPE,
            GET_VALUES_TEXTURE_TYPE_COMPATIBILITY,
        );
        assert!(composed_material_uses_compatibility_values(&semantics));
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
    fn composed_decal_color_mode_preserves_default_override_and_unknown_raw_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_decal_color_mode(&semantics),
            (MaterialDecalColorMode::Off, None)
        );

        semantics.apply_shader_package_key_default(GET_DECAL_COLOR, GET_DECAL_COLOR_OFF);
        assert_eq!(
            composed_material_decal_color_mode(&semantics),
            (MaterialDecalColorMode::Off, Some(GET_DECAL_COLOR_OFF))
        );

        semantics.apply_shader_package_key_default(GET_DECAL_COLOR, GET_DECAL_COLOR_ALPHA);
        assert_eq!(
            composed_material_decal_color_mode(&semantics),
            (MaterialDecalColorMode::Off, Some(GET_DECAL_COLOR_OFF))
        );

        semantics.apply_material_key(GET_DECAL_COLOR, GET_DECAL_COLOR_RGBA);
        assert_eq!(
            composed_material_decal_color_mode(&semantics),
            (MaterialDecalColorMode::Rgba, Some(GET_DECAL_COLOR_RGBA))
        );

        semantics.apply_material_key(GET_DECAL_COLOR, 0xDEAD_BEEF);
        assert_eq!(
            composed_material_decal_color_mode(&semantics),
            (MaterialDecalColorMode::Unknown, Some(0xDEAD_BEEF))
        );

        let mut alpha_semantics = ComposedMaterialSemantics::default();
        alpha_semantics.apply_shader_package_key_default(GET_DECAL_COLOR, GET_DECAL_COLOR_ALPHA);
        assert_eq!(
            composed_material_decal_color_mode(&alpha_semantics),
            (MaterialDecalColorMode::Alpha, Some(GET_DECAL_COLOR_ALPHA))
        );
    }

    #[test]
    fn composed_skin_value_mode_preserves_known_and_unknown_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_skin_value_mode(&semantics),
            MaterialSkinValueMode::None
        );

        for (value, expected) in [
            (GET_MATERIAL_VALUE_FACE, MaterialSkinValueMode::Face),
            (GET_MATERIAL_VALUE_BODY, MaterialSkinValueMode::Body),
            (GET_MATERIAL_VALUE_BODY_JJM, MaterialSkinValueMode::BodyJjm),
            (
                GET_MATERIAL_VALUE_FACE_EMISSIVE,
                MaterialSkinValueMode::FaceEmissive,
            ),
            (0xDEAD_BEEF, MaterialSkinValueMode::Unknown),
        ] {
            semantics.apply_material_key(GET_MATERIAL_VALUE, value);
            assert_eq!(composed_material_skin_value_mode(&semantics), expected);
        }
    }

    #[test]
    fn composed_character_scroll_variant_preserves_default_override_and_unknown_raw_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_character_scroll_variant(&semantics),
            (MaterialCharacterScrollVariant::None, None)
        );

        semantics.apply_shader_package_key_default(
            CHARACTER_SCROLL_VARIANT,
            CHARACTER_SCROLL_VARIANT_69EB4AE0,
        );
        assert_eq!(
            composed_material_character_scroll_variant(&semantics),
            (
                MaterialCharacterScrollVariant::Value69eb4ae0,
                Some(CHARACTER_SCROLL_VARIANT_69EB4AE0)
            )
        );

        semantics.apply_material_key(CHARACTER_SCROLL_VARIANT, CHARACTER_SCROLL_VARIANT_9A8A46F5);
        assert_eq!(
            composed_material_character_scroll_variant(&semantics),
            (
                MaterialCharacterScrollVariant::Value9a8a46f5,
                Some(CHARACTER_SCROLL_VARIANT_9A8A46F5)
            )
        );

        semantics.apply_material_key(CHARACTER_SCROLL_VARIANT, 0xDEAD_BEEF);
        assert_eq!(
            composed_material_character_scroll_variant(&semantics),
            (MaterialCharacterScrollVariant::Unknown, Some(0xDEAD_BEEF))
        );
    }

    #[test]
    fn composed_lightshaft_type_preserves_default_override_and_unknown_raw_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(
            composed_material_lightshaft_type(&semantics),
            (MaterialLightShaftType::None, None)
        );

        semantics.apply_shader_package_key_default(LIGHTSHAFT_TYPE, LIGHTSHAFT_TYPE_0);
        assert_eq!(
            composed_material_lightshaft_type(&semantics),
            (MaterialLightShaftType::Type0, Some(LIGHTSHAFT_TYPE_0))
        );

        semantics.apply_material_key(LIGHTSHAFT_TYPE, LIGHTSHAFT_TYPE_1);
        assert_eq!(
            composed_material_lightshaft_type(&semantics),
            (MaterialLightShaftType::Type1, Some(LIGHTSHAFT_TYPE_1))
        );

        semantics.apply_material_key(LIGHTSHAFT_TYPE, 0xDEAD_BEEF);
        assert_eq!(
            composed_material_lightshaft_type(&semantics),
            (MaterialLightShaftType::Unknown, Some(0xDEAD_BEEF))
        );
    }

    #[test]
    fn composed_material_alpha_params_use_resolved_material_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_ALPHA_APERTURE, &[2.5]),
            (G_ALPHA_OFFSET, &[-0.25]),
            (G_VERTEX_ALPHA_TO_ONE, &[0.01]),
            (G_SHADOW_ALPHA_THRESHOLD, &[0.35]),
        ]);

        assert_eq!(composed_material_alpha_aperture(&semantics), 2.0);
        assert_eq!(composed_material_alpha_offset(&semantics), 0.0);
        assert_eq!(composed_material_vertex_alpha_to_one(&semantics), 0.0);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 0.5);

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_alpha_aperture(&semantics), 2.5);
        assert_eq!(composed_material_alpha_offset(&semantics), -0.25);
        assert_eq!(composed_material_vertex_alpha_to_one(&semantics), 0.01);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 0.35);

        let material = test_mtrl_with_constant(G_ALPHA_APERTURE, &[4.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_aperture(&semantics), 4.0);

        let material = test_mtrl_with_constant(G_ALPHA_OFFSET, &[0.2], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_offset(&semantics), 0.2);

        let material = test_mtrl_with_constant(G_VERTEX_ALPHA_TO_ONE, &[1.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_vertex_alpha_to_one(&semantics), 1.0);

        let material = test_mtrl_with_constant(G_SHADOW_ALPHA_THRESHOLD, &[1.5], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_shadow_alpha_threshold(&semantics), 1.0);

        let material = test_mtrl_with_constant(G_ALPHA_APERTURE, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_aperture(&semantics), 2.0);

        let material = test_mtrl_with_constant(G_ALPHA_OFFSET, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_alpha_offset(&semantics), 0.0);

        let material = test_mtrl_with_constant(G_VERTEX_ALPHA_TO_ONE, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_vertex_alpha_to_one(&semantics), 0.0);

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
    fn composed_material_latent_motion_and_tile_bias_use_resolved_constants() {
        let mut semantics = ComposedMaterialSemantics::default();
        let shader_package = test_shpk_with_material_defaults(&[
            (G_TILE_MIP_BIAS_OFFSET, &[0.25]),
            (G_VERTEX_MOVEMENT_SCALE, &[0.75]),
            (G_VERTEX_MOVEMENT_MAX_LENGTH, &[3.0]),
        ]);

        assert_eq!(composed_material_tile_mip_bias_offset(&semantics), 0.0);
        assert_eq!(composed_material_vertex_movement_scale(&semantics), 1.0);
        assert_eq!(
            composed_material_vertex_movement_max_length(&semantics),
            1.0
        );

        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(composed_material_tile_mip_bias_offset(&semantics), 0.25);
        assert_eq!(composed_material_vertex_movement_scale(&semantics), 0.75);
        assert_eq!(
            composed_material_vertex_movement_max_length(&semantics),
            3.0
        );

        let material = test_mtrl_with_constant(G_TILE_MIP_BIAS_OFFSET, &[-1.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_tile_mip_bias_offset(&semantics), -1.0);

        let material = test_mtrl_with_constant(G_VERTEX_MOVEMENT_SCALE, &[1.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_vertex_movement_scale(&semantics), 1.25);

        let material = test_mtrl_with_constant(G_VERTEX_MOVEMENT_MAX_LENGTH, &[10.0], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_vertex_movement_max_length(&semantics),
            10.0
        );

        let material = test_mtrl_with_constant(G_TILE_MIP_BIAS_OFFSET, &[f32::NAN], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_tile_mip_bias_offset(&semantics), 0.0);
        let material = test_mtrl_with_constant(G_VERTEX_MOVEMENT_SCALE, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_vertex_movement_scale(&semantics), 1.0);
        let material =
            test_mtrl_with_constant(G_VERTEX_MOVEMENT_MAX_LENGTH, &[f32::NEG_INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_vertex_movement_max_length(&semantics),
            1.0
        );
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
        assert_eq!(composed_material_ambient_occlusion_mask(&semantics), None);
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
    fn composed_material_ambient_occlusion_mask_preserves_optional_finite_values() {
        let mut semantics = ComposedMaterialSemantics::default();
        assert_eq!(composed_material_ambient_occlusion_mask(&semantics), None);

        let shader_package =
            test_shpk_with_material_defaults(&[(G_AMBIENT_OCCLUSION_MASK, &[0.65])]);
        semantics.apply_shader_package_material_constants(&shader_package);
        assert_eq!(
            composed_material_ambient_occlusion_mask(&semantics),
            Some(0.65)
        );

        let material = test_mtrl_with_constant(G_AMBIENT_OCCLUSION_MASK, &[0.25], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(
            composed_material_ambient_occlusion_mask(&semantics),
            Some(0.25)
        );

        let material = test_mtrl_with_constant(G_AMBIENT_OCCLUSION_MASK, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_ambient_occlusion_mask(&semantics), None);
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
            (G_LIGHTSHAFT_ANGLE_CLIP, &[0.4]),
            (G_LIGHTSHAFT_NEAR_CLIP, &[1.25]),
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
        assert_eq!(composed_material_lightshaft_angle_clip(&semantics), 0.0);
        assert_eq!(composed_material_lightshaft_near_clip(&semantics), 0.25);

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
        assert_eq!(composed_material_lightshaft_angle_clip(&semantics), 0.4);
        assert_eq!(composed_material_lightshaft_near_clip(&semantics), 1.25);

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

        let material = test_mtrl_with_constant(G_LIGHTSHAFT_NEAR_CLIP, &[f32::INFINITY], 0);
        semantics.apply_material_constants(&material);
        assert_eq!(composed_material_lightshaft_near_clip(&semantics), 0.25);
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
            classify_sampler_name("g_Sampler1"),
            Some(WeaponModelTextureKind::SecondaryBaseColor)
        );
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
            classify_sampler_logical_role_name("g_SamplerSkinDiffuse"),
            Some(MaterialSamplerLogicalRole::SkinDiffuse)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerSkinNormal"),
            Some(WeaponModelTextureKind::Normal)
        );
        assert_eq!(
            classify_sampler_logical_role_name("g_SamplerSkinNormal"),
            Some(MaterialSamplerLogicalRole::SkinNormal)
        );
        assert_eq!(
            classify_sampler_name("g_SamplerSkinMask"),
            Some(WeaponModelTextureKind::Mask)
        );
        assert_eq!(
            classify_sampler_logical_role_name("g_SamplerSkinMask"),
            Some(MaterialSamplerLogicalRole::SkinMask)
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
    fn skin_sampler_slots_do_not_collapse_into_primary_texture_slots() {
        let textures = vec![
            test_texture("skin-diffuse.tex", WeaponModelTextureKind::BaseColor),
            test_texture("primary-diffuse.tex", WeaponModelTextureKind::BaseColor),
            test_texture("skin-normal.tex", WeaponModelTextureKind::Normal),
            test_texture("primary-normal.tex", WeaponModelTextureKind::Normal),
            test_texture("skin-mask.tex", WeaponModelTextureKind::Mask),
            test_texture("primary-mask.tex", WeaponModelTextureKind::Mask),
        ];
        let mut set = WeaponTextureSet::default();

        for (index, role) in [
            MaterialSamplerLogicalRole::SkinDiffuse,
            MaterialSamplerLogicalRole::BaseColor,
            MaterialSamplerLogicalRole::SkinNormal,
            MaterialSamplerLogicalRole::Normal,
            MaterialSamplerLogicalRole::SkinMask,
            MaterialSamplerLogicalRole::Mask,
        ]
        .into_iter()
        .enumerate()
        {
            assign_weapon_texture_slot(&mut set, index, &textures[index], Some(role));
        }

        assert_eq!(set.skin_diffuse, Some(0));
        assert_eq!(set.base_color, Some(1));
        assert_eq!(set.skin_normal, Some(2));
        assert_eq!(set.normal, Some(3));
        assert_eq!(set.skin_mask, Some(4));
        assert_eq!(set.mask, Some(5));
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
            weapon_material_alpha_mode("lightshaft.shpk", 0, &texture_set, true),
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
    fn bg_color_change_texture_alpha_is_dye_mask_not_transparency() {
        let texture_set = WeaponTextureSet {
            has_alpha: true,
            ..Default::default()
        };

        assert_eq!(
            weapon_material_alpha_mode("bgcolorchange.shpk", 0, &texture_set, false),
            WeaponMaterialAlphaMode::Opaque
        );
        assert_eq!(
            weapon_material_alpha_mode("bgcrestchange.shpk", 0, &texture_set, false),
            WeaponMaterialAlphaMode::Opaque
        );
        assert_eq!(
            weapon_material_alpha_mode(
                "bgcommon/hou/material/bgcolorchange.shpk",
                0,
                &texture_set,
                false
            ),
            WeaponMaterialAlphaMode::Opaque
        );
        // 显式 alpha test 仍按 MTRL 数据走 cutout；其它 bg 包的 alpha 行为不变。
        assert_eq!(
            weapon_material_alpha_mode("bgcolorchange.shpk", 0, &texture_set, true),
            WeaponMaterialAlphaMode::Mask
        );
        assert_eq!(
            weapon_material_alpha_mode("bg.shpk", 0, &texture_set, false),
            WeaponMaterialAlphaMode::Blend
        );
    }

    #[test]
    fn character_colorset_diffuse_multiply_is_compatibility_gated() {
        for family in [
            MaterialShaderFamily::Character,
            MaterialShaderFamily::CharacterStockings,
            MaterialShaderFamily::CharacterGlass,
            MaterialShaderFamily::CharacterTransparency,
            MaterialShaderFamily::CharacterScroll,
        ] {
            assert_eq!(
                color_table_diffuse_composition(family, true),
                ColorTableDiffuseComposition::Multiply
            );
            assert_eq!(
                color_table_diffuse_composition(family, false),
                ColorTableDiffuseComposition::Replace
            );
        }

        for family in [
            MaterialShaderFamily::Skin,
            MaterialShaderFamily::CharacterReflection,
            MaterialShaderFamily::CharacterTattoo,
            MaterialShaderFamily::CharacterOcclusion,
            MaterialShaderFamily::Bg,
        ] {
            assert_eq!(
                color_table_diffuse_composition(family, false),
                ColorTableDiffuseComposition::Multiply
            );
        }
    }

    #[test]
    fn colorset_base_selection_and_compatibility_multiply_compose_in_shader() {
        let textures = vec![
            WeaponModelTexture {
                path: "base.tex".to_string(),
                kind: WeaponModelTextureKind::BaseColor,
                texel_layout: ModelTextureTexelLayout::Standard,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![128, 64, 32, 77],
                rgba_f32: None,
            },
            WeaponModelTexture {
                path: "baked://material#colorset-diffuse".to_string(),
                kind: WeaponModelTextureKind::BaseColor,
                texel_layout: ModelTextureTexelLayout::ColorTableRampAb,
                width: 1,
                height: 1,
                array_size: 1,
                array_layer_height: 1,
                rgba: vec![64, 128, 192, 255],
                rgba_f32: Some(vec![[6.7929688, 2.0, 0.5, 1.0]]),
            },
        ];

        let replaced =
            resolve_color_table_base_texture(Some(0), 1, ColorTableDiffuseComposition::Replace);
        assert_eq!(replaced.base_color, 1);
        assert_eq!(replaced.colorset_diffuse, None);
        assert_eq!(textures.len(), 2);

        // Multiply with a real diffuse keeps the diffuse at its own resolution and
        // lets the shader compose it with the colorset ramp; no colorset-resolution
        // `#base-times-colorset` composite is baked.
        let multiplied =
            resolve_color_table_base_texture(Some(0), 1, ColorTableDiffuseComposition::Multiply);
        assert_eq!(multiplied.base_color, 0);
        assert_eq!(multiplied.colorset_diffuse, Some(1));
        assert_eq!(textures.len(), 2);
        assert_eq!(textures[0].rgba, vec![128, 64, 32, 77]);
        assert!(
            !textures
                .iter()
                .any(|texture| texture.path.ends_with("#base-times-colorset"))
        );

        // Multiply without a real diffuse keeps the ramp itself as the base.
        let ramp_base =
            resolve_color_table_base_texture(None, 1, ColorTableDiffuseComposition::Multiply);
        assert_eq!(ramp_base.base_color, 1);
        assert_eq!(ramp_base.colorset_diffuse, None);
        assert_eq!(textures.len(), 2);
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
    fn shape_remap_splits_only_the_targeted_index_occurrence() {
        let vertices = vec![
            test_vertex(0.0),
            test_vertex(1.0),
            test_vertex(2.0),
            test_vertex(3.0),
            test_vertex(10.0),
        ];
        let target = crate::mdl_geometry::MdlGeometryShapeTarget {
            info: crate::model::ModelShapeInfo {
                index: 0,
                name: Some("shape_a".to_string()),
                shape_index_mask: 1,
                shape_index_mask_hex: "0x00000001".to_string(),
                shape_mesh_index: 0,
                shape_value_count: 1,
            },
            replacements: vec![crate::mdl_geometry::MdlGeometryShapeReplacement {
                base_indices_index: 3,
                replacing_vertex_index: 4,
            }],
        };

        let (remapped_vertices, remapped_indices, shape_targets) =
            remap_mesh_vertices_with_shapes(&vertices, &[0, 1, 2, 0, 2, 3], 0, &[target])
                .expect("valid shape remap");

        assert_ne!(remapped_indices[0], remapped_indices[3]);
        assert_eq!(
            remapped_vertices[remapped_indices[0] as usize].position[0],
            0.0
        );
        assert_eq!(
            remapped_vertices[remapped_indices[3] as usize].position[0],
            0.0
        );
        assert_eq!(shape_targets.len(), 1);
        assert_eq!(shape_targets[0].vertex_deltas.len(), 1);
        assert_eq!(
            shape_targets[0].vertex_deltas[0].vertex_index,
            remapped_indices[3]
        );
        assert_eq!(shape_targets[0].vertex_deltas[0].position, [10.0, 0.0, 0.0]);
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
            texel_layout: ModelTextureTexelLayout::Standard,
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

    fn test_mtrl_with_duplicate_texture_offsets() -> Vec<u8> {
        let mut strings = Vec::new();
        let dummy_offset = strings.len() as u16;
        strings.extend_from_slice(b"dummy.tex\0");
        let index_offset = strings.len() as u16;
        strings.extend_from_slice(
            b"chara/weapon/w2651/obj/body/b0059/texture/v01_w2651b0059_id.tex\0",
        );
        let uv_name_offset = strings.len() as u16;
        strings.extend_from_slice(b"map1\0");
        let color_name_offset = strings.len() as u16;
        strings.extend_from_slice(b"colorSet1\0");
        let shader_package_name_offset = strings.len() as u16;
        strings.extend_from_slice(b"characterlegacy.shpk\0");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x0103_0000_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(strings.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&shader_package_name_offset.to_le_bytes());
        bytes.push(3);
        bytes.push(1);
        bytes.push(1);
        bytes.push(0);

        for texture_offset in [dummy_offset, dummy_offset, index_offset] {
            bytes.extend_from_slice(&u32::from(texture_offset).to_le_bytes());
        }
        bytes.extend_from_slice(&uv_name_offset.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&color_name_offset.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&strings);

        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&3_u16.to_le_bytes());
        bytes.extend_from_slice(&0x0d_u32.to_le_bytes());
        for (sampler, texture_index) in [
            ("g_SamplerNormal", 0_u8),
            ("g_SamplerMask", 1_u8),
            ("g_SamplerIndex", 2_u8),
        ] {
            bytes.extend_from_slice(&physis::shpk::ShaderPackage::crc(sampler).to_le_bytes());
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.push(texture_index);
            bytes.extend_from_slice(&[0; 3]);
        }

        let file_size = bytes.len() as u16;
        bytes[4..6].copy_from_slice(&file_size.to_le_bytes());
        bytes
    }

    #[derive(Clone)]
    struct TestShaderPackageResource {
        path: String,
        bytes: Vec<u8>,
    }

    impl physis::resource::Resource for TestShaderPackageResource {
        fn read(&mut self, path: &str) -> Option<Vec<u8>> {
            (path == self.path).then(|| self.bytes.clone())
        }

        fn exists(&mut self, path: &str) -> bool {
            path == self.path
        }
    }

    fn test_shpk_with_material_defaults(parameters: &[(u32, &[f32])]) -> Vec<u8> {
        test_shpk_with_semantics(parameters, &[], &[], &[])
    }

    fn test_shpk_with_semantics(
        parameters: &[(u32, &[f32])],
        material_keys: &[(u32, u32)],
        system_keys: &[(u32, u32)],
        scene_keys: &[(u32, u32)],
    ) -> Vec<u8> {
        test_shpk_with_semantics_for_platform(
            parameters,
            material_keys,
            system_keys,
            scene_keys,
            physis::Platform::Win32,
        )
    }

    fn test_shpk_with_semantics_for_platform(
        parameters: &[(u32, &[f32])],
        material_keys: &[(u32, u32)],
        system_keys: &[(u32, u32)],
        scene_keys: &[(u32, u32)],
        platform: physis::Platform,
    ) -> Vec<u8> {
        let defaults_size = parameters
            .iter()
            .map(|(_, values)| values.len() * 4)
            .sum::<usize>() as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"ShPk");
        bytes.extend_from_slice(&test_shpk_u32(0x0C01, platform));
        bytes.extend_from_slice(b"DX11");
        for value in [0, 0, 0, 0, 0, defaults_size] {
            bytes.extend_from_slice(&test_shpk_u32(value, platform));
        }
        bytes.extend_from_slice(&test_shpk_u16(parameters.len() as u16, platform));
        bytes.extend_from_slice(&test_shpk_u16(1, platform));
        bytes.extend_from_slice(&test_shpk_u32(0, platform));
        bytes.extend_from_slice(&test_shpk_u16(0, platform));
        bytes.extend_from_slice(&test_shpk_u16(0, platform));
        bytes.extend_from_slice(&test_shpk_u32(0, platform));
        bytes.extend_from_slice(&test_shpk_u32(system_keys.len() as u32, platform));
        bytes.extend_from_slice(&test_shpk_u32(scene_keys.len() as u32, platform));
        bytes.extend_from_slice(&test_shpk_u32(material_keys.len() as u32, platform));
        bytes.extend_from_slice(&test_shpk_u32(0, platform));
        bytes.extend_from_slice(&test_shpk_u32(0, platform));

        let mut byte_offset = 0_u16;
        for (id, values) in parameters {
            let byte_size = (values.len() * 4) as u16;
            bytes.extend_from_slice(&test_shpk_u32(*id, platform));
            bytes.extend_from_slice(&test_shpk_u16(byte_offset, platform));
            bytes.extend_from_slice(&test_shpk_u16(byte_size, platform));
            byte_offset += byte_size;
        }

        for (_, values) in parameters {
            for value in *values {
                bytes.extend_from_slice(&test_shpk_u32(value.to_bits(), platform));
            }
        }

        for (id, default_value) in system_keys.iter().chain(scene_keys).chain(material_keys) {
            bytes.extend_from_slice(&test_shpk_u32(*id, platform));
            bytes.extend_from_slice(&test_shpk_u32(*default_value, platform));
        }
        bytes.extend_from_slice(&test_shpk_u32(0, platform));
        bytes.extend_from_slice(&test_shpk_u32(0, platform));

        bytes
    }

    fn test_shpk_without_material_defaults(parameters: &[(u32, u16)]) -> Vec<u8> {
        let defaults = parameters
            .iter()
            .map(|(_, byte_size)| vec![0.0; usize::from(*byte_size) / 4])
            .collect::<Vec<_>>();
        let parameter_values = parameters
            .iter()
            .zip(&defaults)
            .map(|((id, _), values)| (*id, values.as_slice()))
            .collect::<Vec<_>>();
        let mut bytes = test_shpk_with_semantics(&parameter_values, &[], &[], &[]);
        let defaults_offset = 72 + parameters.len() * 8;
        let defaults_size = parameters
            .iter()
            .map(|(_, byte_size)| usize::from(*byte_size))
            .sum::<usize>();
        bytes.drain(defaults_offset..defaults_offset + defaults_size);
        bytes[38..40].copy_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    fn test_shpk_u16(value: u16, platform: physis::Platform) -> [u8; 2] {
        match platform {
            physis::Platform::PS3 => value.to_be_bytes(),
            _ => value.to_le_bytes(),
        }
    }

    fn test_shpk_u32(value: u32, platform: physis::Platform) -> [u8; 4] {
        match platform {
            physis::Platform::PS3 => value.to_be_bytes(),
            _ => value.to_le_bytes(),
        }
    }
}

#[cfg(all(test, feature = "game-data"))]
mod furniture_loader_tests {
    use super::*;
    use crate::furniture::synthetic_sgb;

    #[derive(Default)]
    struct MockAsyncResource {
        files: HashMap<String, Vec<u8>>,
    }

    impl MockAsyncResource {
        fn with_sgb(mut self, path: &str, entries: &[&str]) -> Self {
            self.files.insert(path.to_string(), synthetic_sgb(entries));
            self
        }
    }

    impl AsyncGameResource for MockAsyncResource {
        type Error = String;
        type ReadFuture<'a> = std::future::Ready<Result<Vec<u8>, String>>;

        fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a> {
            std::future::ready(
                self.files
                    .get(path)
                    .cloned()
                    .ok_or_else(|| format!("not found: {path}")),
            )
        }

        fn platform(&self) -> physis::Platform {
            physis::Platform::Win32
        }
    }

    fn furniture_request() -> FurnitureModelLoadRequest {
        FurnitureModelLoadRequest {
            item_id: 19770,
            item_name: "测试木桌".to_string(),
            kind: FurnitureModelKind::Indoor,
            model_key: 1,
        }
    }

    #[test]
    fn furniture_request_builds_sgb_path_and_loads_from_catalog_item() {
        let request = furniture_request();
        assert_eq!(
            request.sgb_path(),
            "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001.sgb"
        );

        let item = FurnitureCatalogItem {
            id: 9710,
            kind: FurnitureModelKind::Outdoor,
            name: "测试庭具石灯".to_string(),
            icon: 59002,
            model_key: 1234,
        };
        let request = FurnitureModelLoadRequest::from(&item);
        assert_eq!(
            request,
            FurnitureModelLoadRequest {
                item_id: 9710,
                item_name: "测试庭具石灯".to_string(),
                kind: FurnitureModelKind::Outdoor,
                model_key: 1234,
            }
        );
        assert_eq!(
            request.sgb_path(),
            "bgcommon/hou/outdoor/general/1234/asset/gar_b0_m1234.sgb"
        );
    }

    #[test]
    fn sgb_collection_merges_related_sgbs_and_guards_cycles() {
        let root = "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001.sgb";
        let related_a = "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001_a.sgb";
        let related_b = "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001_b.sgb";
        let mut resource = MockAsyncResource::default()
            .with_sgb(
                root,
                &[
                    "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
                    "bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl",
                    related_a,
                    related_b,
                ],
            )
            // related_a 回指 root 并重复引用 related_b，验证 scanned set 防环与去重。
            .with_sgb(
                related_a,
                &[
                    "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001_a.mdl",
                    root,
                    related_b,
                ],
            )
            .with_sgb(
                related_b,
                &[
                    "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001_a.mdl",
                    "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001_b.mdl",
                ],
            );

        let assets = futures_executor::block_on(collect_furniture_sgb_assets_from_async_resource(
            &mut resource,
            root,
        ))
        .expect("sgb assets should load");

        assert_eq!(
            assets.models,
            [
                "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
                "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001_a.mdl",
                "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001_b.mdl",
            ]
        );
        assert_eq!(
            assets.files,
            ["bgcommon/hou/indoor/general/0001/material/v0001/mt_fun_b0_m0001_a.mtrl"]
        );
        assert_eq!(assets.scanned_sgbs, [root, related_a, related_b]);
    }

    #[test]
    fn sgb_collection_skips_missing_related_but_not_missing_root() {
        let root = "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001.sgb";
        let mut resource = MockAsyncResource::default().with_sgb(
            root,
            &[
                "bgcommon/hou/indoor/general/0001/model/fun_b0_m0001.mdl",
                "bgcommon/hou/indoor/general/0001/asset/missing.sgb",
            ],
        );

        let assets = futures_executor::block_on(collect_furniture_sgb_assets_from_async_resource(
            &mut resource,
            root,
        ))
        .expect("missing related sgb should be skipped");
        assert_eq!(assets.scanned_sgbs, [root]);
        assert_eq!(assets.models.len(), 1);

        let mut empty = MockAsyncResource::default();
        let error = futures_executor::block_on(collect_furniture_sgb_assets_from_async_resource(
            &mut empty, root,
        ))
        .expect_err("missing root sgb must fail");
        assert!(error.contains(root));
    }

    #[test]
    fn furniture_load_reports_sgb_without_models() {
        let root = "bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001.sgb";
        let mut resource = MockAsyncResource::default().with_sgb(root, &[]);

        let error = futures_executor::block_on(load_furniture_model_from_async_resource(
            &mut resource,
            &furniture_request(),
        ))
        .expect_err("sgb without mdl references must fail");
        assert!(format!("{error:#}").contains("references no MDL"));
    }

    #[test]
    fn furniture_load_reports_missing_sgb() {
        let mut resource = MockAsyncResource::default();
        let error = futures_executor::block_on(load_furniture_model_from_async_resource(
            &mut resource,
            &furniture_request(),
        ))
        .expect_err("missing sgb must fail");
        assert!(
            format!("{error:#}")
                .contains("bgcommon/hou/indoor/general/0001/asset/fun_b0_m0001.sgb")
        );
    }
}

#[cfg(all(test, feature = "game-data"))]
mod chara_loader_tests {
    use super::*;
    use crate::chara_models::CharaModelType;
    use crate::mdl_geometry::tests::fixture_mdl_with_normal_and_glass_mesh;
    use crate::model::weapon_model_mesh_component_index;

    #[derive(Default)]
    struct MockAsyncResource {
        files: HashMap<String, Vec<u8>>,
    }

    impl MockAsyncResource {
        fn with_bytes(mut self, path: &str, bytes: Vec<u8>) -> Self {
            self.files.insert(path.to_string(), bytes);
            self
        }
    }

    impl AsyncGameResource for MockAsyncResource {
        type Error = String;
        type ReadFuture<'a> = std::future::Ready<Result<Vec<u8>, String>>;

        fn read<'a>(&'a mut self, path: &'a str) -> Self::ReadFuture<'a> {
            std::future::ready(
                self.files
                    .get(path)
                    .cloned()
                    .ok_or_else(|| format!("not found: {path}")),
            )
        }

        fn platform(&self) -> physis::Platform {
            physis::Platform::Win32
        }
    }

    fn monster_request() -> CharaModelLoadRequest {
        CharaModelLoadRequest {
            item_id: 100,
            item_name: "爆弹仔".to_string(),
            kind: CharaModelKind::Minion,
            model: PackedCharaModelId {
                model_id: 8003,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            },
        }
    }

    fn demihuman_request() -> CharaModelLoadRequest {
        CharaModelLoadRequest {
            item_id: 200,
            item_name: "专属陆行鸟".to_string(),
            kind: CharaModelKind::Mount,
            model: PackedCharaModelId {
                model_id: 1,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Demihuman,
            },
        }
    }

    #[test]
    fn chara_request_builds_from_catalog_item() {
        let item = CharaCatalogItem {
            id: 100,
            kind: CharaModelKind::Minion,
            name: "爆弹仔".to_string(),
            icon: 2598,
            model: PackedCharaModelId {
                model_id: 8003,
                base_id: 1,
                variant_id: 1,
                chara_type: CharaModelType::Monster,
            },
        };
        assert_eq!(CharaModelLoadRequest::from(&item), monster_request());
    }

    #[test]
    fn monster_load_merges_nothing_but_succeeds_with_single_mdl() {
        let mdl = "chara/monster/m8003/obj/body/b0001/model/m8003b0001.mdl";
        let mut resource =
            MockAsyncResource::default().with_bytes(mdl, fixture_mdl_with_normal_and_glass_mesh());

        let data = futures_executor::block_on(load_chara_model_from_async_resource(
            &mut resource,
            &monster_request(),
        ))
        .expect("monster mdl should load");

        assert_eq!(data.meshes.len(), 2);
        assert_eq!(data.loaded_paths, [mdl.to_string()]);
        assert_eq!(data.model_main.model_id, 8003);
        assert_eq!(data.model_main.body_id, 1);
        assert_eq!(data.model_main.variant_id, 1);
        assert!(data.model_sub.is_none());
        assert!(data.load_diagnostics.is_empty());
    }

    #[test]
    fn monster_load_reports_missing_mdl() {
        let mut resource = MockAsyncResource::default();
        let error = futures_executor::block_on(load_chara_model_from_async_resource(
            &mut resource,
            &monster_request(),
        ))
        .expect_err("missing monster mdl must fail");
        assert!(format!("{error:#}").contains("m8003b0001.mdl"));
    }

    #[test]
    fn demihuman_load_merges_existing_slots_and_skips_missing() {
        let met = "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_met.mdl";
        let top = "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_top.mdl";
        let mut resource = MockAsyncResource::default()
            .with_bytes(met, fixture_mdl_with_normal_and_glass_mesh())
            .with_bytes(top, fixture_mdl_with_normal_and_glass_mesh());

        let data = futures_executor::block_on(load_chara_model_from_async_resource(
            &mut resource,
            &demihuman_request(),
        ))
        .expect("existing demihuman slots should load");

        // 两个槽位各 2 个网格，合并为一个模型，component 按 MDL 序号区分。
        assert_eq!(data.meshes.len(), 4);
        assert_eq!(
            data.loaded_paths,
            [met.to_string(), top.to_string()],
            "glv/dwn/sho 缺失应静默跳过"
        );
        assert!(data.load_diagnostics.is_empty());
        let components = data
            .meshes
            .iter()
            .map(|mesh| weapon_model_mesh_component_index(&data, mesh))
            .collect::<Vec<_>>();
        assert_eq!(components, [0, 0, 1, 1]);
    }

    #[test]
    fn demihuman_load_records_parse_errors_but_not_absent_slots() {
        let met = "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_met.mdl";
        let top = "chara/demihuman/d0001/obj/equipment/e0001/model/d0001e0001_top.mdl";
        let mut resource = MockAsyncResource::default()
            .with_bytes(met, b"not an mdl".to_vec())
            .with_bytes(top, fixture_mdl_with_normal_and_glass_mesh());

        let data = futures_executor::block_on(load_chara_model_from_async_resource(
            &mut resource,
            &demihuman_request(),
        ))
        .expect("one valid slot is enough to load");

        assert_eq!(data.meshes.len(), 2);
        assert_eq!(data.load_diagnostics.len(), 1);
        assert_eq!(
            data.load_diagnostics[0].role,
            WeaponModelLoadRole::Secondary
        );
        assert!(
            data.load_diagnostics[0]
                .candidates
                .iter()
                .any(|candidate| candidate.path == met
                    && candidate.status == WeaponModelLoadCandidateStatus::ParseError)
        );
    }

    #[test]
    fn demihuman_load_reports_when_no_slot_exists() {
        let mut resource = MockAsyncResource::default();
        let error = futures_executor::block_on(load_chara_model_from_async_resource(
            &mut resource,
            &demihuman_request(),
        ))
        .expect_err("demihuman without any slot mdl must fail");
        assert!(format!("{error:#}").contains("no renderable model meshes"));
    }
}

#[cfg(test)]
#[cfg(feature = "game-data")]
mod dressed_character_tests {
    use super::*;

    fn mesh(path: &str, material_name: &str, mask: u32, names: &[&str]) -> WeaponModelMesh {
        ModelMesh {
            path: path.to_string(),
            part_index: 0,
            mesh_category: None,
            submesh: Some(ModelSubmeshInfo {
                index: 0,
                table_index: 0,
                attribute_index_mask: mask,
                attribute_index_mask_hex: format!("0x{mask:08X}"),
                attribute_names: names.iter().map(|name| name.to_string()).collect(),
                bone_start_index: 0,
                bone_count: 0,
            }),
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: material_name.to_string(),
            color: [0.0; 3],
            bone_table: None,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn piece(
        category: u32,
        mesh_start: usize,
        mesh_end: usize,
        imc_mask: Option<u16>,
        eqp_raw: u64,
    ) -> DressedPieceLoad {
        DressedPieceLoad {
            item_id: category,
            equip_slot_category: category,
            is_accessory: category >= 9,
            mesh_start,
            mesh_end,
            material_start: 0,
            material_end: 0,
            imc_mask,
            eqp: Some(EquipmentParameterEntry { raw: eqp_raw }),
        }
    }

    fn kept_paths(meshes: &[WeaponModelMesh]) -> Vec<&str> {
        meshes.iter().map(|mesh| mesh.path.as_str()).collect()
    }

    /// 指定 race 的捏脸夹具（race byte：1 中原、2 精灵、3 拉拉、4 猫魅、
    /// 5 鲁加、6 敖龙、7 硌狮、8 维埃拉）。
    fn customize(race: u8) -> CharacterCustomize {
        CharacterCustomize {
            race,
            ..Default::default()
        }
    }

    const TOP_CLOTH: &str = "chara/equipment/e0001/model/c1301e0001_top.mdl#part-0-submesh-0";
    const TOP_SKIN: &str = "chara/equipment/e0001/model/c1301e0001_top.mdl#part-0-submesh-1";
    const DWN_CLOTH: &str = "chara/equipment/e0001/model/c1301e0001_dwn.mdl#part-0-submesh-0";
    const DWN_SKIN: &str = "chara/equipment/e0001/model/c1301e0001_dwn.mdl#part-0-submesh-1";
    const BARE_GLV: &str = "chara/equipment/e0000/model/c1301e0000_glv.mdl#part-0-submesh-0";
    const BARE_SHO: &str = "chara/equipment/e0000/model/c1301e0000_sho.mdl#part-0-submesh-0";
    const SMALL_SHO: &str = "chara/equipment/e0001/model/c1301e0001_sho.mdl#part-0-submesh-0";
    const HAIR: &str = "chara/human/c1301/obj/hair/h0001/model/c1301h0001_hir.mdl#part-0-submesh-0";
    const FACE: &str = "chara/human/c1301/obj/face/f0001/model/c1301f0001_fac.mdl#part-0-submesh-0";
    const TAIL: &str = "chara/human/c1301/obj/tail/t0001/model/c1301t0001_til.mdl";
    const ZEAR: &str = "chara/human/c1801/obj/zear/z0001/model/c1801z0001_zer.mdl";
    const EAR_GEAR: &str = "chara/accessory/a0043/model/c1301a0043_ear.mdl";
    const GEAR_TOP: &str = "chara/equipment/e0908/model/c1301e0908_top.mdl#part-0-submesh-0";
    const SKIN_MAT: &str = "mt_c1301b0001_a.mtrl";
    const CLOTH_MAT: &str = "mt_c1301e0001_top_a.mtrl";

    #[test]
    fn visibility_removes_top_cloth_and_flagged_skin() {
        use EquipmentParameterEntry as E;
        let mut meshes = vec![
            mesh(TOP_CLOTH, CLOTH_MAT, 0x1, &["atr_nek"]),
            mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_ude"]),
            mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_nek"]),
            mesh(GEAR_TOP, "mt_c1301e0908_top_a.mtrl", 0, &[]),
        ];
        let eqp = (1 << E::BODY_HIDE_SHORT_GLOVES) | (1 << E::BODY_HIDE_GORGET);
        let pieces = [piece(4, 3, 4, None, eqp)];
        let hidden = apply_dressed_visibility(&mut meshes, 3, &pieces, &customize(6));
        assert_eq!(kept_paths(&meshes), [GEAR_TOP]);
        assert_eq!(
            hidden,
            ["top:cloth", "top:skin:atr_ude", "top:skin:atr_nek"]
        );

        // 无 top 装备：小衣布料与皮肤全部保留。
        let mut meshes = vec![
            mesh(TOP_CLOTH, CLOTH_MAT, 0x1, &["atr_nek"]),
            mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_ude"]),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &[], &customize(6));
        assert!(hidden.is_empty());
        assert_eq!(meshes.len(), 2);
    }

    #[test]
    fn visibility_removes_bare_limbs_for_glv_and_sho() {
        let mut meshes = vec![
            mesh(BARE_GLV, SKIN_MAT, 0, &[]),
            mesh(SMALL_SHO, "mt_c1301e0001_sho_a.mtrl", 0, &[]),
            mesh(BARE_SHO, SKIN_MAT, 0, &[]),
        ];
        let pieces = [piece(5, 3, 3, None, 0), piece(8, 3, 3, None, 0)];
        let hidden = apply_dressed_visibility(&mut meshes, 3, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["glv:body", "sho:body"]);

        // 仅 glv 装备：只去裸肤手，小衣/裸肤足保留。
        let mut meshes = vec![
            mesh(BARE_GLV, SKIN_MAT, 0, &[]),
            mesh(SMALL_SHO, "mt_c1301e0001_sho_a.mtrl", 0, &[]),
            mesh(BARE_SHO, SKIN_MAT, 0, &[]),
        ];
        let pieces = [piece(5, 3, 3, None, 0)];
        let hidden = apply_dressed_visibility(&mut meshes, 3, &pieces, &customize(6));
        assert_eq!(kept_paths(&meshes), [SMALL_SHO, BARE_SHO]);
        assert_eq!(hidden, ["glv:body"]);
    }

    #[test]
    fn visibility_gear_imc_mask_is_mdl_local_and_name_collision_safe() {
        use EquipmentParameterEntry as E;
        // 身体与装备的 attribute 名冲突（atr_ude 两边都有）：身体侧按 EQP 名
        // 集合丢弃，装备侧按 IMC 位（MDL 本地表序）裁剪，同名不受影响。
        let mut meshes = vec![
            mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_ude"]),
            mesh(
                "chara/equipment/e0908/model/c1301e0908_glv.mdl#part-0-submesh-0",
                "mt_c1301e0908_glv_a.mtrl",
                0x1,
                &["atr_ude"],
            ),
            mesh(
                "chara/equipment/e0908/model/c1301e0908_glv.mdl#part-0-submesh-1",
                "mt_c1301e0908_glv_a.mtrl",
                0x2,
                &["atr_ude", "atr_lpd"],
            ),
        ];
        let pieces = [piece(5, 1, 3, Some(0b001), 1 << E::HAND_HIDE_FOREARM)];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert_eq!(
            kept_paths(&meshes),
            ["chara/equipment/e0908/model/c1301e0908_glv.mdl#part-0-submesh-0"]
        );
        assert_eq!(hidden, ["top:skin:atr_ude"]);
    }

    #[test]
    fn visibility_hand_hiding_cross_slot_override() {
        use EquipmentParameterEntry as E;
        // 常规：手部遮蔽读 glv 条目（HandHideForearm → atr_ude）。
        let mut meshes = vec![mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_ude"])];
        let pieces = [
            piece(4, 1, 1, None, 1 << E::BODY_SHOW_HAND),
            piece(5, 1, 1, None, 1 << E::HAND_HIDE_FOREARM),
        ];
        apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());

        // top 条目 BodyShowHand 关闭：改读 top 条目自身（无手部位）→ 不遮蔽。
        let mut meshes = vec![mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_ude"])];
        let pieces = [
            piece(4, 1, 1, None, 0),
            piece(5, 1, 1, None, 1 << E::HAND_HIDE_FOREARM),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert_eq!(meshes.len(), 1);
        assert!(hidden.is_empty());

        // BodyShowHand 关闭且 top 条目带 HandHideForearm：遮蔽来自 top 条目。
        let mut meshes = vec![mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_ude"])];
        let pieces = [
            piece(4, 1, 1, None, 1 << E::HAND_HIDE_FOREARM),
            piece(5, 1, 1, None, 0),
        ];
        apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());
    }

    #[test]
    fn visibility_hair_rules_follow_met_eqp() {
        use EquipmentParameterEntry as E;
        let hair_pair = || {
            vec![
                mesh(HAIR, "mt_c1301h0001_a.mtrl", 0x1, &["atr_top"]),
                mesh(HAIR, "mt_c1301h0001_a.mtrl", 0x1, &["atr_kam"]),
            ]
        };
        // HeadHideScalp：只去 atr_top 子网格。
        let mut meshes = hair_pair();
        let pieces = [piece(3, 2, 2, None, 1 << E::HEAD_HIDE_SCALP)];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &pieces, &customize(6));
        assert_eq!(meshes.len(), 1);
        assert_eq!(hidden, ["hair:atr_top"]);

        // HeadHideHair：全部头发网格。
        let mut meshes = hair_pair();
        let pieces = [piece(3, 2, 2, None, 1 << E::HEAD_HIDE_HAIR)];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["hair:all"]);

        // HeadShowHairOverride 覆盖 HeadHideHair。
        let mut meshes = hair_pair();
        let pieces = [piece(
            3,
            2,
            2,
            None,
            (1 << E::HEAD_HIDE_HAIR) | (1 << E::HEAD_SHOW_HAIR_OVERRIDE),
        )];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &pieces, &customize(6));
        assert_eq!(meshes.len(), 2);
        assert!(hidden.is_empty());

        // HeadHideNeck → top 皮肤 atr_nek。
        let mut meshes = vec![mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_nek"])];
        let pieces = [piece(3, 1, 1, None, 1 << E::HEAD_HIDE_NECK)];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["top:skin:atr_nek"]);
    }

    #[test]
    fn visibility_dwn_leg_hiding_and_pants_tuck() {
        use EquipmentParameterEntry as E;
        let gear_dwn_leg = "chara/equipment/e0908/model/c1301e0908_dwn.mdl#part-0-submesh-0";
        let gear_dwn_hip = "chara/equipment/e0908/model/c1301e0908_dwn.mdl#part-0-submesh-1";
        let mut meshes = vec![
            mesh(DWN_CLOTH, "mt_c1301e0001_dwn_a.mtrl", 0x1, &["atr_hiz"]),
            mesh(DWN_SKIN, SKIN_MAT, 0x1, &["atr_hiz"]),
            mesh(DWN_SKIN, SKIN_MAT, 0x1, &["atr_sne"]),
            mesh(gear_dwn_leg, "mt_c1301e0908_dwn_a.mtrl", 0x1, &["atr_leg"]),
            mesh(gear_dwn_hip, "mt_c1301e0908_dwn_a.mtrl", 0x1, &["atr_hip"]),
        ];
        let pieces = [
            piece(7, 3, 5, None, 1 << E::LEG_HIDE_KNEE_PADS),
            piece(8, 5, 5, None, 1 << E::FOOT_HIDE_ANKLE),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 3, &pieces, &customize(6));
        assert_eq!(kept_paths(&meshes), [gear_dwn_hip]);
        assert_eq!(
            hidden,
            [
                "dwn:cloth",
                "dwn:skin:atr_hiz",
                "dwn:skin:atr_sne",
                "dwn-gear:atr_leg"
            ]
        );

        // BodyShowLeg 关闭：腿部遮蔽改读 top 条目（dwn/sho 条目忽略）。
        let mut meshes = vec![mesh(DWN_SKIN, SKIN_MAT, 0x1, &["atr_hiz"])];
        let pieces = [
            piece(4, 1, 1, None, 1 << E::LEG_HIDE_KNEE_PADS),
            piece(7, 1, 1, None, 0),
        ];
        apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());

        // 对照：BodyShowLeg 开启时 top 条目的 LEG 位不生效。
        let mut meshes = vec![mesh(DWN_SKIN, SKIN_MAT, 0x1, &["atr_hiz"])];
        let pieces = [
            piece(
                4,
                1,
                1,
                None,
                (1 << E::BODY_SHOW_LEG) | (1 << E::LEG_HIDE_KNEE_PADS),
            ),
            piece(7, 1, 1, None, 0),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert_eq!(meshes.len(), 1);
        assert!(hidden.is_empty());
    }

    #[test]
    fn visibility_earring_piece_gated_by_race_group_bit() {
        use EquipmentParameterEntry as E;
        // 耳饰件（slot 9 饰品）：头部条目清角色种族组的耳饰位 → 整件丢弃。
        let meshes = || {
            vec![
                mesh(EAR_GEAR, "mt_c1301a0043_ear_a.mtrl", 0, &[]),
                mesh(EAR_GEAR, "mt_c1301a0043_ear_a.mtrl", 0, &[]),
            ]
        };
        // 中原（耳饰位 46）：met 清 46 → 丢弃。
        let mut m = meshes();
        let pieces = [piece(3, 2, 2, None, 0), piece(9, 0, 2, None, 0)];
        let hidden = apply_dressed_visibility(&mut m, 2, &pieces, &customize(1));
        assert!(m.is_empty());
        assert_eq!(hidden, ["ear:gear"]);

        // 敖龙（耳饰位 49）：met 清 46 但置 49 → 保留（种族组区分）。
        let mut m = meshes();
        let pieces = [
            piece(3, 2, 2, None, 1 << E::HEAD_SHOW_EARRINGS_AURA),
            piece(9, 0, 2, None, 0),
        ];
        let hidden = apply_dressed_visibility(&mut m, 2, &pieces, &customize(6));
        assert_eq!(m.len(), 2);
        assert!(hidden.is_empty());

        // 无 met：耳饰恒显示。
        let mut m = meshes();
        let pieces = [piece(9, 0, 2, None, 0)];
        let hidden = apply_dressed_visibility(&mut m, 2, &pieces, &customize(1));
        assert_eq!(m.len(), 2);
        assert!(hidden.is_empty());

        // BodyShowHead 关闭：耳饰位改读 top 条目（top 清 46 → 中原耳饰隐藏，
        // 即使 met 条目置位）。
        let mut m = meshes();
        let pieces = [
            piece(3, 2, 2, None, 1 << E::HEAD_SHOW_EARRINGS_HYUR_ROE),
            piece(4, 2, 2, None, 1 << E::BODY_SHOW_LEG),
            piece(9, 0, 2, None, 0),
        ];
        let hidden = apply_dressed_visibility(&mut m, 2, &pieces, &customize(1));
        assert!(m.is_empty());
        assert_eq!(hidden, ["ear:gear"]);
    }

    #[test]
    fn visibility_ear_geometry_by_race_mechanism() {
        use EquipmentParameterEntry as E;
        // 人族（中原 race 1，bit 50）：脸部 atr_mim 子网格隐藏。
        let mut meshes = vec![
            mesh(FACE, "mt_c1301f0001_fac_a.mtrl", 0x1, &["atr_mim"]),
            mesh(FACE, "mt_c1301f0001_fac_a.mtrl", 0x1, &["atr_kao"]),
        ];
        let pieces = [piece(3, 2, 2, None, 0)];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &pieces, &customize(1));
        assert_eq!(meshes.len(), 1);
        assert_eq!(hidden, ["face:atr_mim"]);

        // 敖龙（bit 52）：脸部 atr_hrn 子网格隐藏；含 atr_hrn 的组合网格同隐。
        let mut meshes = vec![
            mesh(FACE, "mt_c1301f0001_fac_a.mtrl", 0x1, &["atr_hrn"]),
            mesh(
                FACE,
                "mt_c1301f0001_fac_a.mtrl",
                0x3,
                &["atr_hrn", "atr_fv_c"],
            ),
            mesh(FACE, "mt_c1301f0001_fac_a.mtrl", 0x1, &["atr_kao"]),
        ];
        let pieces = [piece(3, 2, 2, None, 1 << E::HEAD_SHOW_EAR_HUMAN)];
        let hidden = apply_dressed_visibility(&mut meshes, 3, &pieces, &customize(6));
        assert_eq!(meshes.len(), 1);
        assert_eq!(hidden, ["face:atr_hrn"]);

        // 维埃拉（bit 53）：zear 部件整网格隐藏，脸部不动。
        let mut meshes = vec![
            mesh(ZEAR, "mt_c1801z0001_zer_a.mtrl", 0, &[]),
            mesh(FACE, "mt_c1801f0001_fac_a.mtrl", 0, &[]),
        ];
        let pieces = [piece(3, 2, 2, None, 0)];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &pieces, &customize(8));
        assert_eq!(kept_paths(&meshes), [FACE]);
        assert_eq!(hidden, ["zear:all"]);

        // 猫魅（bit 51）：耳朵无 attribute 隔离 → 不隐藏，记诊断。
        let mut meshes = vec![mesh(FACE, "mt_c0801f0001_fac_a.mtrl", 0, &[])];
        let pieces = [piece(3, 1, 1, None, 0)];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(4));
        assert_eq!(meshes.len(), 1);
        assert_eq!(hidden, ["face:ear-miqo-unsupported"]);

        // 对应耳位置位 → 全部保留。
        let mut meshes = vec![
            mesh(FACE, "mt_c1301f0001_fac_a.mtrl", 0x1, &["atr_hrn"]),
            mesh(ZEAR, "mt_c1801z0001_zer_a.mtrl", 0, &[]),
        ];
        let pieces = [piece(
            3,
            2,
            2,
            None,
            (1 << E::HEAD_SHOW_EAR_AURA) | (1 << E::HEAD_SHOW_EAR_VIERA),
        )];
        let hidden = apply_dressed_visibility(&mut meshes, 2, &pieces, &customize(6));
        assert_eq!(meshes.len(), 2);
        assert!(hidden.is_empty());
    }

    #[test]
    fn visibility_body_show_head_clear_hides_face_and_drives_head_rules() {
        use EquipmentParameterEntry as E;
        // 全身套装（top 清 BodyShowHead）：脸部网格整体丢弃，头发/颈/耳规则
        // 改读 top 条目（41+42 → 全发、44 → atr_nek）。
        let mut meshes = vec![
            mesh(FACE, "mt_c1301f0001_fac_a.mtrl", 0x1, &["atr_kao"]),
            mesh(HAIR, "mt_c1301h0001_a.mtrl", 0x1, &["atr_top"]),
            mesh(TOP_SKIN, SKIN_MAT, 0x1, &["atr_nek"]),
        ];
        let top_eqp =
            (1 << E::HEAD_HIDE_SCALP) | (1 << E::HEAD_HIDE_HAIR) | (1 << E::HEAD_HIDE_NECK);
        let pieces = [piece(4, 3, 3, None, top_eqp)];
        let hidden = apply_dressed_visibility(&mut meshes, 3, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["face:all", "hair:all", "top:skin:atr_nek"]);

        // top 清 BodyShowHead 时 met 条目的头发位不再生效（全发仍按 top
        // 条目的 41+42 隐藏——top 条目 41/42 未置位时头发保留）。
        let mut meshes = vec![mesh(HAIR, "mt_c1301h0001_a.mtrl", 0x1, &["atr_top"])];
        let pieces = [
            piece(4, 1, 1, None, 0), // BodyShowHead 清、头发位全清
            piece(3, 1, 1, None, 1 << E::HEAD_HIDE_HAIR),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert_eq!(meshes.len(), 1, "BodyShowHead 清时头发规则读 top 条目");
        assert!(hidden.is_empty());

        // 对照：BodyShowHead 置位时头发规则读 met 条目。
        let mut meshes = vec![mesh(HAIR, "mt_c1301h0001_a.mtrl", 0x1, &["atr_top"])];
        let pieces = [
            piece(4, 1, 1, None, 1 << E::BODY_SHOW_HEAD),
            piece(3, 1, 1, None, 1 << E::HEAD_HIDE_HAIR),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["hair:all"]);
    }

    #[test]
    fn visibility_tail_gated_by_top_or_leg_entry() {
        use EquipmentParameterEntry as E;
        let tail_mesh = || mesh(TAIL, "mt_c1301t0001_a.mtrl", 0, &[]);
        // 猫魅/敖龙 + top 清 ShowTail(13) → 尾丢弃（top 置 BodyShowHead/
        // BodyShowLeg 以隔离头部/腿部跨槽规则）。
        let mut meshes = vec![tail_mesh()];
        let pieces = [piece(
            4,
            1,
            1,
            None,
            (1 << E::LEG_SHOW_TAIL) | (1 << E::BODY_SHOW_HEAD) | (1 << E::BODY_SHOW_LEG),
        )];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(4));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["tail:all"]);

        // top 置 13 但 dwn 清 LegShowTail(22) → 任一关闭即藏。
        let mut meshes = vec![tail_mesh()];
        let pieces = [
            piece(
                4,
                1,
                1,
                None,
                (1 << E::BODY_SHOW_TAIL) | (1 << E::BODY_SHOW_LEG) | (1 << E::BODY_SHOW_HEAD),
            ),
            piece(7, 1, 1, None, 0),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["tail:all"]);

        // 两件都置位 → 保留。
        let mut meshes = vec![tail_mesh()];
        let pieces = [
            piece(
                4,
                1,
                1,
                None,
                (1 << E::BODY_SHOW_TAIL) | (1 << E::BODY_SHOW_LEG) | (1 << E::BODY_SHOW_HEAD),
            ),
            piece(7, 1, 1, None, 1 << E::LEG_SHOW_TAIL),
        ];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert_eq!(meshes.len(), 1);
        assert!(hidden.is_empty());

        // BodyShowLeg 关闭：LegShowTail 改读 top 条目（top 置 13 但清 22 → 藏）。
        let mut meshes = vec![tail_mesh()];
        let pieces = [piece(
            4,
            1,
            1,
            None,
            (1 << E::BODY_SHOW_TAIL) | (1 << E::BODY_SHOW_HEAD),
        )];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(6));
        assert!(meshes.is_empty());
        assert_eq!(hidden, ["tail:all"]);

        // 无尾种族（中原）：位全清也不动（无尾网格部件，规则空转）。
        let mut meshes = vec![tail_mesh()];
        let pieces = [piece(4, 1, 1, None, 0)];
        let hidden = apply_dressed_visibility(&mut meshes, 1, &pieces, &customize(1));
        assert_eq!(meshes.len(), 1);
        assert!(hidden.is_empty());
    }

    /// 双 stain 行的 legacy STM 夹具（diffuse 列满表：行 1/2 不同 diffuse，
    /// 其余行零；其余列空 = 广播默认值。列长只能是 0/单元素/满表，见
    /// `staining::decode_array`）。
    fn staining_fixture(template: u32, diffuse_a: [f32; 3], diffuse_b: [f32; 3]) -> Vec<u8> {
        const STM_MAGIC: u16 = 0x534d;
        const STM_VERSION_LEGACY: u16 = 0x0101;
        const STAINS: usize = 254;
        let color_row = |color: [f32; 3]| {
            color
                .into_iter()
                .flat_map(|value| half::f16::from_f32(value).to_bits().to_le_bytes())
                .collect::<Vec<_>>()
        };
        let mut diffuse_column = [color_row(diffuse_a), color_row(diffuse_b)].concat();
        diffuse_column.resize(STAINS * 6, 0);
        let columns = vec![
            diffuse_column,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&STM_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&STM_VERSION_LEGACY.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&template.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        let mut cumulative_bytes = 0_usize;
        for column in &columns {
            cumulative_bytes += column.len();
            bytes.extend_from_slice(
                &u16::try_from(cumulative_bytes / 2)
                    .expect("fixture offset")
                    .to_le_bytes(),
            );
        }
        for column in columns {
            bytes.extend_from_slice(&column);
        }
        bytes
    }

    fn dyeable_material(name: &str) -> WeaponModelMaterial {
        let base_row = ColorTableRowColors {
            diffuse: [0.1, 0.2, 0.3],
            specular: [0.2, 0.3, 0.4],
            ..ColorTableRowColors::default()
        };
        let dye_row = ModelLegacyColorDyeTableRow {
            template: 42,
            diffuse: true,
            specular: false,
            emissive: false,
            gloss: false,
            specular_strength: false,
        };
        let mut material = fallback_weapon_material(0, 0, name.to_string(), [0.5; 3]);
        material.path = Some(format!("chara/equipment/{name}"));
        material.has_color_dye_table = true;
        material.color_dye_table = Some(ModelColorDyeTable::Legacy(vec![dye_row.clone(), dye_row]));
        material.color_table_rows = Some(vec![base_row, base_row]);
        material.index_texture = Some(0);
        material.texture_indices = vec![0];
        material
    }

    /// 按材质路径片段找重烘焙的 colorset-diffuse 贴图内容（切片重染色与整
    /// 模型重染色的贴图索引不同，按内容比较）。
    fn baked_colorset_diffuse_rgba<'a>(
        model: &'a WeaponModelData,
        material_fragment: &str,
    ) -> &'a [u8] {
        &model
            .textures
            .iter()
            .find(|texture| {
                texture.path.contains(material_fragment)
                    && texture.path.ends_with("#colorset-diffuse")
            })
            .expect("baked colorset diffuse texture")
            .rgba
    }

    #[test]
    fn apply_dressed_piece_stains_restains_only_target_slice() {
        let base = DressedCharacterData {
            model: WeaponModelData {
                item_id: 0,
                item_name: "dressed".to_string(),
                model_main: PackedModelId::from_raw(101),
                model_sub: None,
                stain_ids: [0, 0],
                load_diagnostics: Vec::new(),
                loaded_paths: Vec::new(),
                bounds: ModelBounds::default(),
                materials: vec![
                    dyeable_material("body.mtrl"),
                    dyeable_material("piece-a.mtrl"),
                    dyeable_material("piece-b.mtrl"),
                ],
                textures: vec![WeaponModelTexture {
                    path: "index.tex".to_string(),
                    kind: WeaponModelTextureKind::Index,
                    texel_layout: ModelTextureTexelLayout::Standard,
                    width: 1,
                    height: 1,
                    array_size: 1,
                    array_layer_height: 1,
                    rgba: vec![0, 0, 0, 255],
                    rgba_f32: None,
                }],
                meshes: Vec::new(),
            },
            equipment_material_ranges: vec![
                EquipmentMaterialRange {
                    item_id: 10,
                    equip_slot_category: 4,
                    material_start: 1,
                    material_end: 2,
                },
                EquipmentMaterialRange {
                    item_id: 20,
                    equip_slot_category: 5,
                    material_start: 2,
                    material_end: 3,
                },
            ],
            hidden_body_attributes: Vec::new(),
        };
        let templates = WeaponStainingTemplates::from_load_results(
            Ok(staining_fixture(42, [0.8, 0.4, 0.2], [0.1, 0.9, 0.3])),
            Err("not needed".to_string()),
        );

        let stained_b = apply_dressed_piece_stains(&base, 20, [1, 0], &templates);
        // 其余切片不动：无染色记录、材质原样。
        assert!(stained_b.materials[0].staining_application.is_none());
        assert!(stained_b.materials[1].staining_application.is_none());
        assert_eq!(stained_b.materials[0], base.model.materials[0]);
        assert_eq!(stained_b.materials[1], base.model.materials[1]);
        assert_eq!(
            stained_b.materials[2]
                .staining_application
                .as_ref()
                .map(|application| application.report.rows_changed),
            Some(2)
        );
        // 目标切片染色结果与整模型染色一致（同基准色表 + 同模板）。
        let whole = apply_weapon_model_stains(&base.model, [1, 0], &templates);
        assert_eq!(
            stained_b.materials[2].staining_application,
            whole.materials[2].staining_application
        );
        assert_eq!(
            baked_colorset_diffuse_rgba(&stained_b, "piece-b"),
            baked_colorset_diffuse_rgba(&whole, "piece-b")
        );

        // 同切片换色：只影响目标切片，且两行 stain 产出不同颜色。
        let stained_b2 = apply_dressed_piece_stains(&base, 20, [2, 0], &templates);
        assert_eq!(stained_b2.materials[0], base.model.materials[0]);
        assert_eq!(stained_b2.materials[1], base.model.materials[1]);
        assert_ne!(
            baked_colorset_diffuse_rgba(&stained_b, "piece-b"),
            baked_colorset_diffuse_rgba(&stained_b2, "piece-b")
        );

        // 清除染色：恢复基准色表（重烘焙贴图与未染色整模型一致）。
        let reset = apply_dressed_piece_stains(&base, 20, [0, 0], &templates);
        let neutral = apply_weapon_model_stains(&base.model, [0, 0], &templates);
        assert!(reset.materials[2].staining_application.is_none());
        assert_eq!(
            baked_colorset_diffuse_rgba(&reset, "piece-b"),
            baked_colorset_diffuse_rgba(&neutral, "piece-b")
        );

        // 未知 item_id：原样返回。
        let untouched = apply_dressed_piece_stains(&base, 99, [1, 0], &templates);
        assert_eq!(untouched, base.model);
    }

    /// 真实数据集成探针：敖龙女默认身体 + e0908 制敌五件（总冠军制敌系列，
    /// ilvl 790；model_main 低 16 位 = set 908、次段 = IMC 子集 2），glv 染色。
    #[test]
    #[ignore = "loads a dressed character from the installed game; requires XIV_GAME_DIR"]
    fn load_dressed_au_ra_with_e0908_set_from_installed_game() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        // 敖龙女有效取值：tribe 11（晨曦）/12（暮晖），head 0-3（→ f0001-f0004），
        // hair 须命中 character-make.json 的 hairOptions（51 不在其中）。
        let customize = CharacterCustomize {
            race: 6,
            gender: 1,
            age: 1,
            height: 50,
            tribe: 11,
            head: 1,
            hair: 1,
            ..Default::default()
        };
        assert_eq!(customize.race_code(), 1401);
        let naked = load_character_assembly_from_resource(
            &mut resource,
            &CharacterAssemblyLoadRequest::new(customize, "naked"),
        )
        .expect("naked assembly");

        let piece =
            |item_id: u32, name: &str, category: u32, stain_ids: [u8; 2]| DressedEquipmentPiece {
                item_id,
                item_name: name.to_string(),
                model_main: 0x2_038C,
                model_sub: 0,
                equip_slot_category: category,
                stain_ids,
            };
        let equipment = vec![
            piece(49685, "总冠军制敌头甲", 3, [0, 0]),
            piece(49686, "总冠军制敌上衣", 4, [0, 0]),
            piece(49687, "总冠军制敌手套", 5, [1, 0]),
            piece(49688, "总冠军制敌马裤", 7, [0, 0]),
            piece(49689, "总冠军制敌矮靴", 8, [0, 0]),
        ];
        let request =
            DressedCharacterLoadRequest::new(customize, "dressed-au-ra").with_equipment(equipment);
        let (data, skeleton) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &request)
                .expect("dressed load");
        eprintln!(
            "dressed: meshes={} (naked {}) materials={} textures={} diagnostics={} hidden={:?}",
            data.model.meshes.len(),
            naked.meshes.len(),
            data.model.materials.len(),
            data.model.textures.len(),
            data.model.load_diagnostics.len(),
            data.hidden_body_attributes
        );

        assert!(data.model.meshes.len() > naked.meshes.len());
        assert!(skeleton.is_some());
        // 材质区间不相交且在界内（按槽位序单调）。
        let ranges = &data.equipment_material_ranges;
        assert_eq!(ranges.len(), 5);
        let mut end = 0;
        for range in ranges {
            assert!(range.material_start >= end && range.material_end > range.material_start);
            end = range.material_end;
        }
        assert!(end <= data.model.materials.len());
        // 骨骼名并集在渲染器关节表上限内。
        let joints: HashSet<&str> = data
            .model
            .meshes
            .iter()
            .flat_map(|mesh| mesh.bone_table.iter())
            .flat_map(|table| table.bone_names.iter().flatten())
            .map(|name| name.as_str())
            .collect();
        assert!(joints.len() <= 256, "joint union {}", joints.len());
        // top/dwn 装备 → 小衣布料遮蔽。
        assert!(
            data.hidden_body_attributes
                .iter()
                .any(|entry| entry == "top:cloth")
        );
        assert!(
            data.hidden_body_attributes
                .iter()
                .any(|entry| entry == "dwn:cloth")
        );
        // 染色的 glv 件：材质切片内有染色落地。
        let glv = ranges
            .iter()
            .find(|range| range.item_id == 49687)
            .expect("glv range");
        assert!(
            data.model.materials[glv.material_start..glv.material_end]
                .iter()
                .any(|material| material.staining_application.is_some())
        );
    }

    /// 头部 EQP 规则测试集的真实数据探针：EQP 条目位 + 脸部/尾/兔耳 MDL
    /// attribute 表（规则实现的地面真值）。
    ///
    /// 测试集（assets/collection-catalog.json 查证）：
    /// - 暗黑之面（16091，set 393，slot 3）：全盔——41/42/44 置位，46-53 全清。
    /// - C1战术兜帽（41544，set 836，slot 3）：开面帽——耳饰位仅 48（猫魅/硌狮/
    ///   维埃拉组）清；耳位 51（猫魅）清，50/52/53 置。
    /// - 黯云制敌头盔（44610，set 871，slot 3）：开面盔——41/42 清、44 置，
    ///   耳饰/耳位（46-53）全清。
    /// - 波奇服/波奇头套（15479/15478，set 6023）：BodyShowHead(10)/ShowTail(13)/
    ///   LegShowTail(22) 全清，41/42 置位，46-53 全清。
    /// - 先锋御敌战甲（42398，set 846，slot 4）：ShowTail(13) 清。
    /// - 维埃拉束膝裤（33945，set 744，slot 7）：LegShowTail(22) 清。
    #[test]
    #[ignore = "probes EQP head-rule sets from the installed game; requires XIV_GAME_DIR"]
    fn probe_eqp_head_rule_sets_from_installed_game() {
        use physis::resource::Resource;
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let bytes = resource
            .read(EQUIPMENT_PARAMETER_PATH)
            .expect("read equipmentparameter.eqp");
        let table = EquipmentParameterTable::from_bytes(&bytes);
        use EquipmentParameterEntry as E;

        let entry = |set_id: u16| table.entry(set_id).expect("EQP entry");
        // 暗黑之面：全盔（遮头皮+全发+颈），耳饰/耳全族关闭。
        let helm = entry(393);
        assert!(helm.flag(E::HEAD_HIDE_SCALP) && helm.flag(E::HEAD_HIDE_HAIR));
        assert!(helm.flag(E::HEAD_HIDE_NECK));
        for bit in 46..=53 {
            assert!(!helm.flag(bit), "set 393 bit {bit} must be clear");
        }
        // C1战术兜帽：耳饰仅猫魅/硌狮/维埃拉组关闭（48），耳位仅猫魅（51）清。
        let hood = entry(836);
        assert!(hood.flag(E::HEAD_SHOW_EARRINGS_HYUR_ROE));
        assert!(hood.flag(E::HEAD_SHOW_EARRINGS_LALA_ELEZEN));
        assert!(!hood.flag(E::HEAD_SHOW_EARRINGS_MIQO_HROTH_VIERA));
        assert!(hood.flag(E::HEAD_SHOW_EARRINGS_AURA));
        assert!(hood.flag(E::HEAD_SHOW_EAR_HUMAN));
        assert!(!hood.flag(E::HEAD_SHOW_EAR_MIQO));
        assert!(hood.flag(E::HEAD_SHOW_EAR_AURA));
        assert!(hood.flag(E::HEAD_SHOW_EAR_VIERA));
        // 黯云制敌头盔：开面（41/42 清）但耳饰/耳全族关闭。
        let kabuto = entry(871);
        assert!(!kabuto.flag(E::HEAD_HIDE_SCALP) && !kabuto.flag(E::HEAD_HIDE_HAIR));
        for bit in 46..=53 {
            assert!(!kabuto.flag(bit), "set 871 bit {bit} must be clear");
        }
        // 波奇套装：met 驱动遮头（BodyShowHead 置位——头套件自身条目驱动），
        // 全发 + 遮尾（13/22 双清）。
        let pig = entry(6023);
        assert!(pig.flag(E::BODY_SHOW_HEAD));
        assert!(pig.flag(E::HEAD_HIDE_SCALP) && pig.flag(E::HEAD_HIDE_HAIR));
        assert!(!pig.flag(E::BODY_SHOW_TAIL));
        assert!(!pig.flag(E::LEG_SHOW_TAIL));
        for bit in 46..=53 {
            assert!(!pig.flag(bit), "set 6023 bit {bit} must be clear");
        }
        // 幽灵套装：全身套装遮头（BodyShowHead 清）+ 全发 + 耳饰/耳全族关闭；
        // BodyShowLeg 清 → 腿部规则（含 LegShowTail 22 清 → 遮尾）也读本条目。
        let ghost = entry(137);
        assert!(!ghost.flag(E::BODY_SHOW_HEAD));
        assert!(!ghost.flag(E::BODY_SHOW_LEG));
        assert!(ghost.flag(E::HEAD_HIDE_SCALP) && ghost.flag(E::HEAD_HIDE_HAIR));
        assert!(!ghost.flag(E::LEG_SHOW_TAIL));
        for bit in 46..=53 {
            assert!(!ghost.flag(bit), "set 137 bit {bit} must be clear");
        }
        // 先锋御敌战甲：top 遮尾（13 清）；维埃拉束膝裤：dwn 遮尾（22 清）。
        assert!(!entry(846).flag(E::BODY_SHOW_TAIL));
        assert!(!entry(744).flag(E::LEG_SHOW_TAIL));
        // 小衣参照：全部显示。
        let small = entry(1);
        for bit in 46..=53 {
            assert!(small.flag(bit), "set 1 bit {bit} must be set");
        }

        // 测试集的模型文件存在性（c1401 敖龙女 + c0101 回退链）。
        for set_id in [137u16, 6023, 846, 744] {
            let mut row = String::new();
            for slot in ["met", "top", "glv", "dwn", "sho"] {
                for race in [1401u16, 101] {
                    let path = format!(
                        "chara/equipment/e{set_id:04}/model/c{race:04}e{set_id:04}_{slot}.mdl"
                    );
                    if resource.exists(&path) {
                        row.push_str(&format!(" c{race:04}_{slot}"));
                    }
                }
            }
            eprintln!("set {set_id} models:{row}");
        }

        // 脸部 MDL attribute 表：人族（中原/精灵/拉拉/鲁加）含 atr_mim，敖龙含
        // atr_hrn，猫魅/硌狮/维埃拉两者皆无（猫魅耳在脸部基础网格内，不可隔离）。
        let mut face_attribute_names = |race_code: u16, face: u16| -> Vec<String> {
            let path = format!(
                "chara/human/c{race_code:04}/obj/face/f{face:04}/model/c{race_code:04}f{face:04}_fac.mdl"
            );
            let bytes = resource.read(&path).expect("read face mdl");
            let meshes = meshes_from_mdl_bytes(&path, &bytes).expect("parse face mdl");
            meshes
                .iter()
                .flat_map(|mesh| {
                    mesh.submesh
                        .iter()
                        .flat_map(|submesh| submesh.attribute_names.iter().cloned())
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        for race_code in [101u16, 201, 501, 601, 901, 1001, 1101, 1201] {
            let names = face_attribute_names(race_code, 1);
            assert!(
                names.iter().any(|name| name == "atr_mim"),
                "c{race_code:04} face must carry atr_mim: {names:?}"
            );
        }
        for race_code in [701u16, 801, 1501, 1701, 1801] {
            let names = face_attribute_names(race_code, 1);
            assert!(
                !names
                    .iter()
                    .any(|name| name == "atr_mim" || name == "atr_hrn"),
                "c{race_code:04} face must not carry ear attributes: {names:?}"
            );
        }
        // 硌狮女（脸文件从 f0005 起）。
        let names = face_attribute_names(1601, 5);
        assert!(
            !names
                .iter()
                .any(|name| name == "atr_mim" || name == "atr_hrn")
        );
        for race_code in [1301u16, 1401] {
            let names = face_attribute_names(race_code, 1);
            assert!(
                names.iter().any(|name| name == "atr_hrn"),
                "c{race_code:04} face must carry atr_hrn: {names:?}"
            );
        }
    }

    /// 头部/尾部 EQP 规则的真实数据着装探针：合并版隐藏标签与场景版遮蔽计划
    /// 一致驱动（猫魅女 + 全盔 ⇒ 耳饰件隐藏 + 头发全隐 + 猫耳诊断；敖龙女 +
    /// 开面盔 ⇒ 角隐藏；敖龙女 + 波奇套装 ⇒ 脸/发/尾/耳饰全隐）。
    #[test]
    #[ignore = "drives dressed EQP visibility tags from the installed game; requires XIV_GAME_DIR"]
    fn dressed_eqp_visibility_tags_from_installed_game() {
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);
        let miqo_female = CharacterCustomize {
            race: 4,
            gender: 1,
            age: 1,
            height: 50,
            tribe: 5,
            head: 1,
            hair: 1,
            ..Default::default()
        };
        assert_eq!(miqo_female.race_code(), 801);
        let au_ra_female = CharacterCustomize {
            race: 6,
            gender: 1,
            age: 1,
            height: 50,
            tribe: 11,
            head: 1,
            hair: 1,
            ..Default::default()
        };
        let piece =
            |item_id: u32, name: &str, category: u32, model_main: u64| DressedEquipmentPiece {
                item_id,
                item_name: name.to_string(),
                model_main,
                model_sub: 0,
                equip_slot_category: category,
                stain_ids: [0, 0],
            };
        // 暗黑之面（set 393）+ 亚拉戈高位咏咒耳坠（7215，set 43）。
        let full_helm = piece(16091, "暗黑之面", 3, 0x1_0189);
        let earring = piece(7215, "亚拉戈高位咏咒耳坠", 9, 0x1_002B);

        // 猫魅女 + 全盔 + 耳坠：耳饰件隐藏（48 清）、头发全隐、猫耳诊断。
        let request = DressedCharacterLoadRequest::new(miqo_female, "miqo-full-helm")
            .with_equipment(vec![full_helm.clone(), earring.clone()]);
        let (merged, _) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &request)
                .expect("merged miqo load");
        for expected in ["hair:all", "ear:gear", "face:ear-miqo-unsupported"] {
            assert!(
                merged
                    .hidden_body_attributes
                    .iter()
                    .any(|entry| entry == expected),
                "merged must record {expected}: {:?}",
                merged.hidden_body_attributes
            );
        }
        let scene_request = DressedCharacterLoadRequest::new(miqo_female, "miqo-full-helm-scene")
            .with_equipment(vec![full_helm.clone(), earring.clone()]);
        let scene = load_dressed_character_scene_from_resource(&mut resource, &scene_request)
            .expect("scene miqo load");
        let plan = plan_dressed_concealment(&miqo_female, &scene.body, &scene.pieces);
        assert!(
            plan.hidden_pieces.contains(&(9, 7215)),
            "scene plan must hide the earring piece: {:?}",
            plan.hidden_pieces
        );
        assert!(plan.hidden_notes.contains(&"hair:all".to_string()));

        // 敖龙女 + 黯云制敌头盔（871）：角隐藏（face:atr_hrn）、头发保留。
        let kabuto = piece(44610, "黯云制敌头盔", 3, 0x2_0367);
        let request = DressedCharacterLoadRequest::new(au_ra_female, "au-ra-kabuto")
            .with_equipment(vec![kabuto]);
        let (merged, _) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &request)
                .expect("merged au-ra load");
        assert!(
            merged
                .hidden_body_attributes
                .iter()
                .any(|entry| entry == "face:atr_hrn"),
            "merged must hide au-ra horns: {:?}",
            merged.hidden_body_attributes
        );
        assert!(
            !merged
                .hidden_body_attributes
                .iter()
                .any(|entry| entry == "hair:all"),
            "kabuto keeps hair: {:?}",
            merged.hidden_body_attributes
        );

        // 敖龙女 + 幽灵套装（top 语义加载）：全身套装（BodyShowHead 清）→
        // 脸/发/角/尾全隐（尾：BodyShowLeg 清 → LegShowTail 读 top 条目，22 清）。
        let ghost_top = piece(6107, "尖啸幽灵套装", 4, 0x1_0089);
        let request = DressedCharacterLoadRequest::new(au_ra_female, "au-ra-ghost")
            .with_equipment(vec![ghost_top]);
        let (merged, _) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &request)
                .expect("merged ghost load");
        for expected in ["face:all", "hair:all", "tail:all"] {
            assert!(
                merged
                    .hidden_body_attributes
                    .iter()
                    .any(|entry| entry == expected),
                "mascot suit must record {expected}: {:?}",
                merged.hidden_body_attributes
            );
        }

        // 猫魅女 + 先锋御敌战甲（846，top ShowTail 清）：尾隐藏。
        let vanguard_top = piece(42398, "先锋御敌战甲", 4, 0x1_034E);
        let request = DressedCharacterLoadRequest::new(miqo_female, "miqo-vanguard")
            .with_equipment(vec![vanguard_top]);
        let (merged, _) =
            load_dressed_character_with_skeleton_from_resource(&mut resource, &request)
                .expect("merged vanguard load");
        assert!(
            merged
                .hidden_body_attributes
                .iter()
                .any(|entry| entry == "tail:all"),
            "vanguard top must hide the tail: {:?}",
            merged.hidden_body_attributes
        );
    }

    /// 女仆装（e6016）种族变体与 EQDP 地面真值探针：
    /// 1. 全部 18 族 × 5 槽的 `c{race}e6016_{slot}.mdl` 存在性矩阵；
    /// 2. 各族 EQDP（`chara/xls/charadb/equipmentdeformerparameter/c{race}.eqdp`）
    ///    set 6016 条目的 HasMaterial/HasModel 位（xivModdingFramework `Eqp.cs`
    ///    布局：u16 条目，槽序 [met,top,glv,dwn,sho] 各 2 位，bit0 材质 bit1 模型）；
    /// 3. 敖龙女着装女仆装五件实际命中的模型路径。
    #[test]
    #[ignore = "probes e6016 race variants and EQDP from the installed game; requires XIV_GAME_DIR"]
    fn probe_e6016_race_variants_and_eqdp() {
        use physis::resource::Resource;
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);

        let races: [u16; 18] = [
            101, 201, 301, 401, 501, 601, 701, 801, 901, 1001, 1101, 1201, 1301, 1401, 1501, 1601,
            1701, 1801,
        ];
        let slots = ["met", "top", "glv", "dwn", "sho"];

        eprintln!("== e6016 MDL existence (race x slot) ==");
        for race in races {
            let mut row = String::new();
            for slot in slots {
                let path = format!("chara/equipment/e6016/model/c{race:04}e6016_{slot}.mdl");
                row.push_str(&format!(
                    " {slot}={}",
                    if resource.exists(&path) { "1" } else { "0" }
                ));
            }
            eprintln!("c{race:04}:{row}");
        }

        let eqdp_entry = |bytes: &[u8], set_id: u16| -> Option<u16> {
            let block_size = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
            let block_count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
            let header_entry_offset = 6 + 2 * (usize::from(set_id) / block_size);
            let base_data_offset =
                u16::from_le_bytes([bytes[header_entry_offset], bytes[header_entry_offset + 1]]);
            if base_data_offset == 0xFFFF {
                return None;
            }
            let full_header = 6 + 2 * block_count;
            let offset = full_header
                + usize::from(base_data_offset) * 2
                + (usize::from(set_id) % block_size) * 2;
            Some(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
        };

        eprintln!("== EQDP set 6016 (M=HasMaterial m=HasModel, slot order met/top/glv/dwn/sho) ==");
        for race in races {
            let path = format!("chara/xls/charadb/equipmentdeformerparameter/c{race:04}.eqdp");
            let Some(bytes) = resource.read(&path) else {
                eprintln!("c{race:04}: eqdp missing");
                continue;
            };
            match eqdp_entry(&bytes, 6016) {
                None => eprintln!("c{race:04}: block omitted (all zero)"),
                Some(raw) => {
                    let mut row = String::new();
                    for (idx, slot) in slots.iter().enumerate() {
                        let has_material = (raw >> (idx * 2)) & 1 != 0;
                        let has_model = (raw >> (idx * 2 + 1)) & 1 != 0;
                        row.push_str(&format!(
                            " {slot}=M{}m{}",
                            u8::from(has_material),
                            u8::from(has_model)
                        ));
                    }
                    eprintln!("c{race:04}: raw=0x{raw:04x}{row}");
                }
            }
        }

        // 一致性校验：候选链（骨变形树祖先）上首个文件存在者 == 首个 EQDP
        // HasModel=1 者。覆盖多个年代的装备套装与饰品套装 × 18 族 × 全槽位。
        let eqdp_has_model = |bytes: &[u8], set_id: u16, slot_idx: usize| -> Option<bool> {
            eqdp_entry(bytes, set_id).map(|raw| (raw >> (slot_idx * 2 + 1)) & 1 != 0)
        };
        let mut mismatches = 0usize;
        for (set_id, is_accessory) in [
            (1u16, false),
            (908, false),
            (6016, false),
            (55, true),
            (98, true),
        ] {
            let (root, type_prefix, eqdp_root, slot_list): (&str, char, &str, &[&str]) =
                if is_accessory {
                    (
                        "chara/accessory/a",
                        'a',
                        "chara/xls/charadb/accessorydeformerparameter",
                        &["ear", "nek", "wrs", "rir", "ril"],
                    )
                } else {
                    (
                        "chara/equipment/e",
                        'e',
                        "chara/xls/charadb/equipmentdeformerparameter",
                        &slots,
                    )
                };
            let eqdp_by_race: HashMap<u16, Option<Vec<u8>>> = races
                .iter()
                .map(|race| {
                    let bytes = resource.read(&format!("{eqdp_root}/c{race:04}.eqdp"));
                    (*race, bytes)
                })
                .collect();
            for (slot_idx, slot) in slot_list.iter().enumerate() {
                for race in races {
                    let chain = crate::model::equipment_model_race_candidates(race);
                    let file_pick = chain.iter().copied().find(|candidate| {
                        resource.exists(&format!(
                            "{root}{set_id:04}/model/c{candidate:04}{type_prefix}{set_id:04}_{slot}.mdl"
                        ))
                    });
                    let eqdp_pick = chain.iter().copied().find(|candidate| {
                        eqdp_by_race
                            .get(candidate)
                            .and_then(|bytes| bytes.as_ref())
                            .and_then(|bytes| eqdp_has_model(bytes, set_id, slot_idx))
                            .unwrap_or(false)
                    });
                    if file_pick != eqdp_pick {
                        mismatches += 1;
                        eprintln!(
                            "MISMATCH {type_prefix}{set_id:04} {slot} c{race:04}: file={file_pick:?} eqdp={eqdp_pick:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(mismatches, 0, "chain existence probing must match EQDP");

        // 女仆装（e6016）关键断言：敖龙女 top 经链回退到中原女 c0201（而不是
        // 旧实现的 c0101 中原男）；met 用自有模型。
        let top_pick = crate::model::equipment_model_race_candidates(1401)
            .into_iter()
            .find(|race| {
                resource.exists(&format!(
                    "chara/equipment/e6016/model/c{race:04}e6016_top.mdl"
                ))
            });
        assert_eq!(top_pick, Some(201));

        // 小衣 e0001 的 EQDP HasModel（对照 `smallclothes_model_race_candidates`）。
        eprintln!("== EQDP set 1 top HasModel ==");
        for race in [1601u16, 1001, 201, 1701, 701, 1801, 801, 1501, 901, 101] {
            let path = format!("chara/xls/charadb/equipmentdeformerparameter/c{race:04}.eqdp");
            let Some(bytes) = resource.read(&path) else {
                continue;
            };
            let top = eqdp_has_model(&bytes, 1, 1).unwrap_or(false);
            eprintln!("c{race:04} e0001 top HasModel={}", u8::from(top));
        }

        // 敖龙女 + 女仆装五件（14972-14976，model_main 0x11780 = set 6016 + IMC 子集 1）。
        let customize = CharacterCustomize {
            race: 6,
            gender: 1,
            age: 1,
            height: 50,
            tribe: 11,
            head: 1,
            hair: 1,
            ..Default::default()
        };
        let piece = |item_id: u32, category: u32| DressedEquipmentPiece {
            item_id,
            item_name: format!("maid-{item_id}"),
            model_main: 0x1_1780,
            model_sub: 0,
            equip_slot_category: category,
            stain_ids: [0, 0],
        };
        let equipment = vec![
            piece(14972, 3),
            piece(14973, 4),
            piece(14974, 5),
            piece(14975, 7),
            piece(14976, 8),
        ];
        let request = DressedCharacterLoadRequest::new(customize, "dressed-au-ra-maid-probe")
            .with_equipment(equipment);
        let (data, _) = load_dressed_character_with_skeleton_from_resource(&mut resource, &request)
            .expect("dressed maid load");
        eprintln!("== c1401 maid loaded paths (e6016) ==");
        for path in &data.model.loaded_paths {
            if path.contains("e6016") {
                eprintln!("{path}");
            }
        }
        eprintln!("== c1401 maid mesh paths ==");
        for mesh in &data.model.meshes {
            eprintln!("{}", mesh.path);
        }

        // human.pbd 种族骨变形树：item = (body_id, link_index)，link =
        // (parent, first_child, next_sibling, deformer_index)（physis pbd.rs /
        // Meddle PbdFile 布局）。
        eprintln!("== human.pbd deform tree ==");
        let Some(pbd) = resource.read("chara/xls/boneDeformer/human.pbd") else {
            panic!("human.pbd missing");
        };
        let pbd = pbd.as_slice();
        let count = i32::from_le_bytes(pbd[0..4].try_into().unwrap()) as usize;
        let mut items = Vec::new();
        for i in 0..count {
            let base = 4 + i * 12;
            let body_id = u16::from_le_bytes(pbd[base..base + 2].try_into().unwrap());
            let link_index = i16::from_le_bytes(pbd[base + 2..base + 4].try_into().unwrap());
            items.push((body_id, link_index));
        }
        let links_base = 4 + count * 12;
        let mut links = Vec::new();
        for i in 0..count {
            let base = links_base + i * 8;
            let parent = i16::from_le_bytes(pbd[base..base + 2].try_into().unwrap());
            let deformer_index = u16::from_le_bytes(pbd[base + 6..base + 8].try_into().unwrap());
            links.push((parent, deformer_index));
        }
        for (body_id, link_index) in &items {
            let mut chain = vec![*body_id];
            let mut link = *link_index;
            while link >= 0 {
                let (parent, deformer_index) = links[link as usize];
                if parent < 0 {
                    break;
                }
                let parent_body = items[links[parent as usize].1 as usize].0;
                chain.push(parent_body);
                let _ = deformer_index;
                link = parent;
            }
            eprintln!("c{body_id:04} deform chain to root: {chain:04?}");
        }
    }

    /// 小衣（e0001）与裸肤（e0000）回退语义探针：
    /// 1. 18 族 × 5 槽的 `c{race}e0001/e0000_{slot}.mdl` 文件存在性矩阵与各族
    ///    EQDP set 1 / set 0 条目的 HasModel 位（布局同 e6016 探针）；
    /// 2. 骨变形树祖先链（[`crate::model::equipment_model_race_candidates`]）上
    ///    "首个文件存在者"与"首个 EQDP HasModel=1 者"逐族逐槽对照（set 1 硬
    ///    断言，set 0 仅报告——set 0 是否走 EQDP 由本探针判定）；
    /// 3. 逐族打印 e0001/e0000 各槽的有效来源族，对照
    ///    [`crate::chara_assemble::smallclothes_model_race_candidates`] /
    ///    [`crate::chara_assemble::bare_limb_model_race_candidates`] 的当前回退。
    #[test]
    #[ignore = "probes smallclothes/bare-limb race fallback from the installed game; requires XIV_GAME_DIR"]
    fn probe_smallclothes_bare_limb_race_fallback_eqdp() {
        use physis::resource::Resource;
        let game_dir =
            std::env::var("XIV_GAME_DIR").unwrap_or_else(|_| r"E:\_ff14\game".to_string());
        let mut resource = physis::resource::SqPackResource::from_existing(&game_dir);

        let races: [u16; 18] = [
            101, 201, 301, 401, 501, 601, 701, 801, 901, 1001, 1101, 1201, 1301, 1401, 1501, 1601,
            1701, 1801,
        ];
        let slots = ["met", "top", "glv", "dwn", "sho"];

        let eqdp_entry = |bytes: &[u8], set_id: u16| -> Option<u16> {
            let block_size = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
            let block_count = usize::from(u16::from_le_bytes([bytes[4], bytes[5]]));
            let header_entry_offset = 6 + 2 * (usize::from(set_id) / block_size);
            let base_data_offset =
                u16::from_le_bytes([bytes[header_entry_offset], bytes[header_entry_offset + 1]]);
            if base_data_offset == 0xFFFF {
                return None;
            }
            let full_header = 6 + 2 * block_count;
            let offset = full_header
                + usize::from(base_data_offset) * 2
                + (usize::from(set_id) % block_size) * 2;
            Some(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]))
        };
        let eqdp_has_model = |bytes: &[u8], set_id: u16, slot_idx: usize| -> Option<bool> {
            eqdp_entry(bytes, set_id).map(|raw| (raw >> (slot_idx * 2 + 1)) & 1 != 0)
        };
        let eqdp_by_race: HashMap<u16, Option<Vec<u8>>> = races
            .iter()
            .map(|race| {
                let bytes = resource.read(&format!(
                    "chara/xls/charadb/equipmentdeformerparameter/c{race:04}.eqdp"
                ));
                (*race, bytes)
            })
            .collect();

        for set_id in [1u16, 0] {
            eprintln!("== e{set_id:04} MDL existence (f) vs EQDP HasModel (e), race x slot ==");
            for race in races {
                let mut row = String::new();
                for (idx, slot) in slots.iter().enumerate() {
                    let path = format!(
                        "chara/equipment/e{set_id:04}/model/c{race:04}e{set_id:04}_{slot}.mdl"
                    );
                    let file = resource.exists(&path);
                    let eqdp = eqdp_by_race
                        .get(&race)
                        .and_then(|bytes| bytes.as_ref())
                        .and_then(|bytes| eqdp_has_model(bytes, set_id, idx))
                        .unwrap_or(false);
                    row.push_str(&format!(" {slot}=f{}e{}", u8::from(file), u8::from(eqdp)));
                }
                eprintln!("c{race:04}:{row}");
            }
        }

        // 链一致性：骨变形树祖先链上首个文件存在者 == 首个 EQDP HasModel=1 者。
        let mut mismatches = 0usize;
        for set_id in [1u16, 0] {
            for (slot_idx, slot) in slots.iter().enumerate() {
                for race in races {
                    let chain = crate::model::equipment_model_race_candidates(race);
                    let file_pick = chain.iter().copied().find(|candidate| {
                        resource.exists(&format!(
                            "chara/equipment/e{set_id:04}/model/c{candidate:04}e{set_id:04}_{slot}.mdl"
                        ))
                    });
                    let eqdp_pick = chain.iter().copied().find(|candidate| {
                        eqdp_by_race
                            .get(candidate)
                            .and_then(|bytes| bytes.as_ref())
                            .and_then(|bytes| eqdp_has_model(bytes, set_id, slot_idx))
                            .unwrap_or(false)
                    });
                    if file_pick != eqdp_pick {
                        mismatches += 1;
                        eprintln!(
                            "MISMATCH e{set_id:04} {slot} c{race:04}: file={file_pick:?} eqdp={eqdp_pick:?}"
                        );
                    }
                }
            }
        }
        assert_eq!(mismatches, 0, "chain existence probing must match EQDP");

        // 逐族有效来源（链上首个文件存在者）对照当前回退表首个回退族。
        eprintln!("== effective e0001/e0000 source race vs current tables ==");
        for race in races {
            let chain = crate::model::equipment_model_race_candidates(race);
            let mut pick = |set_id: u16, slot: &str| {
                chain.iter().copied().find(|candidate| {
                    resource.exists(&format!(
                        "chara/equipment/e{set_id:04}/model/c{candidate:04}e{set_id:04}_{slot}.mdl"
                    ))
                })
            };
            let smallclothes = crate::chara_assemble::smallclothes_model_race_candidates(race);
            let bare_limb = crate::chara_assemble::bare_limb_model_race_candidates(race);
            eprintln!(
                "c{race:04}: e0001 top={:?} dwn={:?} sho={:?} | e0000 glv={:?} sho={:?} | smallclothes={smallclothes:04?} bare_limb={bare_limb:04?}",
                pick(1, "top"),
                pick(1, "dwn"),
                pick(1, "sho"),
                pick(0, "glv"),
                pick(0, "sho"),
            );
        }
    }
}

#[cfg(all(test, feature = "game-data"))]
mod dressed_scene_tests {
    use super::*;

    fn body_mesh(path: &str, material_name: &str) -> WeaponModelMesh {
        WeaponModelMesh {
            path: path.to_string(),
            part_index: 0,
            mesh_category: None,
            submesh: None,
            shape_influences: Vec::new(),
            shape_targets: Vec::new(),
            material_index: 0,
            material_slot: 0,
            material_name: material_name.to_string(),
            color: [1.0, 1.0, 1.0],
            bone_table: None,
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn mesh_with_attributes(path: &str, material_name: &str, names: &[&str]) -> WeaponModelMesh {
        let mut mesh = body_mesh(path, material_name);
        mesh.submesh = Some(ModelSubmeshInfo {
            index: 0,
            table_index: 0,
            attribute_index_mask: if names.is_empty() {
                0
            } else {
                (1 << names.len()) - 1
            },
            attribute_index_mask_hex: String::new(),
            attribute_names: names.iter().map(|name| name.to_string()).collect(),
            bone_start_index: 0,
            bone_count: 0,
        });
        mesh
    }

    fn piece_model(meshes: Vec<WeaponModelMesh>) -> WeaponModelData {
        WeaponModelData {
            item_id: 1,
            item_name: "piece".to_string(),
            model_main: PackedModelId::from_raw(0),
            model_sub: None,
            stain_ids: [0, 0],
            load_diagnostics: Vec::new(),
            loaded_paths: Vec::new(),
            bounds: ModelBounds::default(),
            materials: Vec::new(),
            textures: Vec::new(),
            meshes,
        }
    }

    fn piece(
        equip_slot_category: u32,
        eqp_raw: u64,
        imc_mask: Option<u16>,
        meshes: Vec<WeaponModelMesh>,
    ) -> DressedPieceModel {
        DressedPieceModel {
            item_id: u32::from(equip_slot_category),
            item_name: format!("piece-{equip_slot_category}"),
            equip_slot_category,
            is_accessory: equip_slot_category >= 9,
            stain_ids: [0, 0],
            imc_mask,
            eqp: (eqp_raw != 0).then(|| EquipmentParameterEntry { raw: eqp_raw }),
            attach: None,
            model: std::rc::Rc::new(piece_model(meshes)),
        }
    }

    fn eqp_bits(bits: &[u8]) -> u64 {
        bits.iter().fold(0_u64, |raw, bit| raw | (1 << bit))
    }

    /// 指定 race 的捏脸夹具（race byte：1 中原、4 猫魅、6 敖龙、8 维埃拉）。
    fn customize(race: u8) -> CharacterCustomize {
        CharacterCustomize {
            race,
            ..Default::default()
        }
    }

    #[test]
    fn concealment_plan_hides_body_regions_and_gates_piece_attributes() {
        let body = piece_model(vec![
            body_mesh("chara/equipment/e0001_top/top_cloth.mdl", "a0001"),
            mesh_with_attributes(
                "chara/equipment/e0001_top/top_skin.mdl",
                "b0001",
                &["atr_nek", "atr_ude"],
            ),
            mesh_with_attributes(
                "chara/equipment/e0001_dwn/dwn_skin.mdl",
                "b0001",
                &["atr_hiz"],
            ),
            body_mesh("chara/equipment/e0001_dwn/dwn_cloth.mdl", "a0001"),
            body_mesh("chara/equipment/e0000_sho/sho.mdl", "a0001"),
            mesh_with_attributes(
                "chara/human/c0101/obj/hair/h0001/h0001.mdl",
                "b0001",
                &["atr_top"],
            ),
        ]);
        // top：遮颈（atr_nek）；dwn 带 atr_leg 变体位；sho 触发裤脚塞靴。
        let pieces = vec![
            piece(
                4,
                eqp_bits(&[EquipmentParameterEntry::BODY_HIDE_GORGET]),
                None,
                Vec::new(),
            ),
            piece(
                7,
                0,
                None,
                vec![mesh_with_attributes(
                    "chara/equipment/d0001e0007/dwn.mdl",
                    "a0001",
                    &["atr_leg", "atr_knk"],
                )],
            ),
            piece(8, 0, None, Vec::new()),
        ];

        let plan = plan_dressed_concealment(&customize(6), &body, &pieces);

        // 整网格隐藏：top 布料(0)、dwn 布料(3)、裸肤足(4)。
        assert_eq!(plan.body_hidden_meshes, vec![0, 3, 4]);
        // 身体 attribute 隐藏：top 皮肤的 atr_nek（BODY_HIDE_GORGET）。
        assert_eq!(plan.body_hidden_attributes, vec!["atr_nek".to_string()]);
        // dwn 件启用名单：IMC 缺失全启用，但 sho 在身去除 atr_leg。
        let dwn_enabled = plan
            .piece_enabled_attributes
            .iter()
            .find(|((slot, _), _)| *slot == 7)
            .map(|(_, names)| names.clone())
            .unwrap();
        assert_eq!(dwn_enabled, vec!["atr_knk".to_string()]);
        assert!(plan.hidden_notes.contains(&"top:cloth".to_string()));
        assert!(plan.hidden_notes.contains(&"top:skin:atr_nek".to_string()));
        assert!(plan.hidden_notes.contains(&"dwn-gear:atr_leg".to_string()));
    }

    #[test]
    fn concealment_plan_without_equipment_hides_nothing() {
        let body = piece_model(vec![
            body_mesh("chara/equipment/e0001_top/top_cloth.mdl", "a0001"),
            mesh_with_attributes(
                "chara/human/c0101/obj/hair/h0001/h0001.mdl",
                "b0001",
                &["atr_top"],
            ),
        ]);

        let plan = plan_dressed_concealment(&customize(6), &body, &[]);

        assert!(plan.body_hidden_meshes.is_empty());
        assert!(plan.body_hidden_attributes.is_empty());
        assert!(plan.piece_enabled_attributes.is_empty());
        assert!(plan.hidden_notes.is_empty());
    }

    #[test]
    fn concealment_plan_maps_imc_variant_bits_to_enabled_names() {
        // IMC 位 0 关、位 1 开：atr_arm 隐藏、atr_knk 启用。
        let piece = piece(
            4,
            0,
            Some(0b10),
            vec![mesh_with_attributes(
                "chara/equipment/d0001e0004/top.mdl",
                "a0001",
                &["atr_arm", "atr_knk"],
            )],
        );

        let plan = plan_dressed_concealment(
            &customize(6),
            &piece_model(Vec::new()),
            std::slice::from_ref(&piece),
        );

        assert_eq!(
            plan.piece_enabled_attributes,
            vec![((4, 4), vec!["atr_knk".to_string()])]
        );
    }

    #[test]
    fn concealment_plan_hides_all_hair_when_head_piece_demands_it() {
        let body = piece_model(vec![
            body_mesh("chara/human/c0101/obj/hair/h0001/h0001.mdl", "b0001"),
            mesh_with_attributes(
                "chara/human/c0101/obj/hair/h0001/bang.mdl",
                "b0001",
                &["atr_top"],
            ),
        ]);
        let met = piece(
            3,
            eqp_bits(&[EquipmentParameterEntry::HEAD_HIDE_HAIR]),
            None,
            Vec::new(),
        );

        let plan = plan_dressed_concealment(&customize(6), &body, std::slice::from_ref(&met));

        assert_eq!(plan.body_hidden_meshes, vec![0, 1]);
        assert!(plan.hidden_notes.contains(&"hair:all".to_string()));
    }

    #[test]
    fn concealment_plan_gates_earring_piece_by_race_group() {
        use EquipmentParameterEntry as E;
        let earring_meshes = || {
            vec![body_mesh(
                "chara/accessory/a0043/model/c1301a0043_ear.mdl",
                "a0001",
            )]
        };
        // 敖龙（耳饰位 49）：met 清 49 → 耳饰件整件隐藏。
        let met = piece(
            3,
            eqp_bits(&[E::HEAD_SHOW_EARRINGS_HYUR_ROE]),
            None,
            Vec::new(),
        );
        let earring = piece(9, 0, None, earring_meshes());
        let plan =
            plan_dressed_concealment(&customize(6), &piece_model(Vec::new()), &[met, earring]);
        assert_eq!(plan.hidden_pieces, vec![(9, 9)]);
        assert!(plan.hidden_notes.contains(&"ear:gear".to_string()));

        // 中原（耳饰位 46）：同一 met（置 46）→ 显示。
        let met = piece(
            3,
            eqp_bits(&[E::HEAD_SHOW_EARRINGS_HYUR_ROE]),
            None,
            Vec::new(),
        );
        let earring = piece(9, 0, None, earring_meshes());
        let plan =
            plan_dressed_concealment(&customize(1), &piece_model(Vec::new()), &[met, earring]);
        assert!(plan.hidden_pieces.is_empty());

        // 无 met（BodyShowHead 未关）→ 显示。
        let earring = piece(9, 0, None, earring_meshes());
        let plan = plan_dressed_concealment(&customize(6), &piece_model(Vec::new()), &[earring]);
        assert!(plan.hidden_pieces.is_empty());
    }

    #[test]
    fn concealment_plan_ear_geometry_by_race_mechanism() {
        use EquipmentParameterEntry as E;
        // 敖龙角：脸部 atr_hrn 进入隐藏名单；其余脸部名保留。
        let body = piece_model(vec![
            mesh_with_attributes(
                "chara/human/c1401/obj/face/f0001/fac.mdl",
                "b0001",
                &["atr_kao", "atr_hrn"],
            ),
            mesh_with_attributes(
                "chara/human/c1401/obj/face/f0001/horn.mdl",
                "b0001",
                &["atr_hrn"],
            ),
        ]);
        let met = piece(3, eqp_bits(&[E::HEAD_HIDE_SCALP]), None, Vec::new());
        let plan = plan_dressed_concealment(&customize(6), &body, std::slice::from_ref(&met));
        assert_eq!(plan.body_hidden_attributes, vec!["atr_hrn".to_string()]);
        assert!(plan.body_hidden_meshes.is_empty());
        assert!(plan.hidden_notes.contains(&"face:atr_hrn".to_string()));

        // 维埃拉耳：zear 网格整网格隐藏。
        let body = piece_model(vec![body_mesh(
            "chara/human/c1801/obj/zear/z0001/model/c1801z0001_zer.mdl",
            "a0001",
        )]);
        let met = piece(3, eqp_bits(&[E::HEAD_HIDE_SCALP]), None, Vec::new());
        let plan = plan_dressed_concealment(&customize(8), &body, std::slice::from_ref(&met));
        assert_eq!(plan.body_hidden_meshes, vec![0]);
        assert!(plan.hidden_notes.contains(&"zear:all".to_string()));

        // 猫魅耳：无 attribute 隔离 → 仅诊断。
        let body = piece_model(vec![body_mesh(
            "chara/human/c0801/obj/face/f0001/fac.mdl",
            "b0001",
        )]);
        let met = piece(3, eqp_bits(&[E::HEAD_HIDE_SCALP]), None, Vec::new());
        let plan = plan_dressed_concealment(&customize(4), &body, std::slice::from_ref(&met));
        assert!(plan.body_hidden_meshes.is_empty());
        assert!(plan.body_hidden_attributes.is_empty());
        assert!(
            plan.hidden_notes
                .contains(&"face:ear-miqo-unsupported".to_string())
        );
    }

    #[test]
    fn concealment_plan_body_show_head_clear_hides_face_and_drives_head_rules() {
        use EquipmentParameterEntry as E;
        // 全身套装（top 清 BodyShowHead + 置 41/42）：脸部/头发整体隐藏。
        let body = piece_model(vec![
            mesh_with_attributes(
                "chara/human/c1401/obj/face/f0001/fac.mdl",
                "b0001",
                &["atr_kao"],
            ),
            body_mesh("chara/human/c1401/obj/hair/h0001/h0001.mdl", "a0001"),
        ]);
        let top = piece(
            4,
            eqp_bits(&[E::HEAD_HIDE_SCALP, E::HEAD_HIDE_HAIR]),
            None,
            Vec::new(),
        );
        let plan = plan_dressed_concealment(&customize(6), &body, std::slice::from_ref(&top));
        assert_eq!(plan.body_hidden_meshes, vec![0, 1]);
        assert!(plan.hidden_notes.contains(&"face:all".to_string()));
        assert!(plan.hidden_notes.contains(&"hair:all".to_string()));

        // top 清 BodyShowHead 且头发位全清：met 的 HEAD_HIDE_HAIR 不再生效。
        let body = piece_model(vec![body_mesh(
            "chara/human/c1401/obj/hair/h0001/h0001.mdl",
            "a0001",
        )]);
        let top = piece(4, eqp_bits(&[E::BODY_SHOW_LEG]), None, Vec::new());
        let met = piece(3, eqp_bits(&[E::HEAD_HIDE_HAIR]), None, Vec::new());
        let plan = plan_dressed_concealment(&customize(6), &body, &[top, met]);
        assert!(plan.body_hidden_meshes.is_empty());
    }

    #[test]
    fn concealment_plan_tail_gated_by_top_or_leg_entry() {
        use EquipmentParameterEntry as E;
        let tail_body = || {
            piece_model(vec![body_mesh(
                "chara/human/c0801/obj/tail/t0001/model/c0801t0001_til.mdl",
                "b0001",
            )])
        };
        // top 清 ShowTail → 尾隐藏。
        let top = piece(
            4,
            eqp_bits(&[E::BODY_SHOW_LEG, E::LEG_SHOW_TAIL]),
            None,
            Vec::new(),
        );
        let plan =
            plan_dressed_concealment(&customize(4), &tail_body(), std::slice::from_ref(&top));
        assert_eq!(plan.body_hidden_meshes, vec![0]);
        assert!(plan.hidden_notes.contains(&"tail:all".to_string()));

        // top 置 13、dwn 清 22 → 任一关闭即藏。
        let top = piece(
            4,
            eqp_bits(&[E::BODY_SHOW_TAIL, E::BODY_SHOW_LEG]),
            None,
            Vec::new(),
        );
        let dwn = piece(7, eqp_bits(&[E::LEG_SHOW_FOOT]), None, Vec::new());
        let plan = plan_dressed_concealment(&customize(4), &tail_body(), &[top, dwn]);
        assert_eq!(plan.body_hidden_meshes, vec![0]);

        // 两件都置位 → 保留；无尾种族（中原）位全清也不藏。
        let top = piece(
            4,
            eqp_bits(&[E::BODY_SHOW_TAIL, E::BODY_SHOW_LEG]),
            None,
            Vec::new(),
        );
        let dwn = piece(7, eqp_bits(&[E::LEG_SHOW_TAIL]), None, Vec::new());
        let plan = plan_dressed_concealment(&customize(4), &tail_body(), &[top, dwn]);
        assert!(plan.body_hidden_meshes.is_empty());
        let top = piece(4, eqp_bits(&[]), None, Vec::new());
        let plan =
            plan_dressed_concealment(&customize(1), &tail_body(), std::slice::from_ref(&top));
        assert!(plan.body_hidden_meshes.is_empty());
    }

    #[test]
    fn concealment_plan_skips_weapon_pieces() {
        // 武器件不参与遮蔽：不进 piece_enabled_attributes（渲染层全显示），
        // 也不影响身体隐藏（武器槽位无任何 EQP 遮蔽位）。
        let body = piece_model(vec![body_mesh(
            "chara/equipment/e0001_top/top_cloth.mdl",
            "a0001",
        )]);
        let mut weapon = piece(
            1,
            0,
            None,
            vec![mesh_with_attributes(
                "chara/weapon/w0001/obj/body/b0001/model/w0001b0001.mdl",
                "a0001",
                &["atr_wp"],
            )],
        );
        weapon.attach = Some(WeaponAttachInfo::default());

        let plan = plan_dressed_concealment(&customize(6), &body, std::slice::from_ref(&weapon));

        assert!(plan.body_hidden_meshes.is_empty());
        assert!(plan.body_hidden_attributes.is_empty());
        assert!(plan.piece_enabled_attributes.is_empty());
        assert!(plan.hidden_notes.is_empty());
    }

    #[test]
    fn weapon_attach_bake_forces_single_joint_and_reports_articulation() {
        let skinned_vertex = |first: u8, second: u8, w2: f32| WeaponModelVertex {
            position: [0.0; 3],
            blend_weights: Some(ModelBlendWeights {
                count: 2,
                values: [1.0 - w2, w2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            }),
            blend_indices: Some(ModelBlendIndices {
                count: 2,
                values: [first, second, 0, 0, 0, 0, 0, 0],
            }),
            normal: [0.0, 1.0, 0.0],
            uv0: [0.0; 2],
            uv1: [0.0; 2],
            uv2: [0.0; 2],
            uv3: [0.0; 2],
            bitangent: [0.0; 4],
            normal1: None,
            bitangent1: None,
            color: [0.0; 4],
            color1: None,
            flow0: None,
            flow1: None,
        };
        let mut meshes = vec![
            // 刚性网格：全部顶点只引用骨 0（即使 bone table 有多根骨）。
            body_mesh("w_rigid", "a"),
            // 内部可动网格：顶点蒙皮到骨 0+1（单骨化丢失内部动画）。
            {
                let mut mesh = body_mesh("w_articulated", "a");
                mesh.vertices = vec![skinned_vertex(0, 1, 0.25), skinned_vertex(0, 0, 0.0)];
                mesh
            },
        ];

        let articulated = bake_weapon_attach(&mut meshes, WEAPON_ATTACH_BONE_MAIN_HAND);

        assert_eq!(articulated, 1, "only the multi-bone mesh is reported");
        for mesh in &meshes {
            let bone_table = mesh.bone_table.as_ref().expect("bone table baked");
            assert_eq!(bone_table.bone_count, 1);
            assert_eq!(
                bone_table.bone_names,
                vec![Some(WEAPON_ATTACH_BONE_MAIN_HAND.to_string())]
            );
            for vertex in &mesh.vertices {
                let weights = vertex.blend_weights.expect("weights forced");
                let indices = vertex.blend_indices.expect("indices forced");
                assert_eq!(weights.count, 1);
                assert_eq!(weights.values[0], 1.0);
                assert!(weights.values[1..].iter().all(|weight| *weight == 0.0));
                assert_eq!(indices.count, 1);
                assert!(indices.values.iter().all(|index| *index == 0));
            }
        }
    }
}
