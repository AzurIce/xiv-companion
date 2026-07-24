#![cfg(feature = "game-data")]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    ffi::{c_char, c_void},
    fs,
    path::PathBuf,
    ptr,
    sync::{
        OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

#[cfg(windows)]
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use physis::{
    ReadableFile,
    resource::{Resource, SqPackResource},
};
use serde::Serialize;
use xiv_companion::{
    MaterialShaderFamily, PackedModelId, WeaponCatalogItem,
    game_data::{export_weapon_catalog_from_resource, game_version, normalize_game_dir},
    material_shader_family, mdl_metadata_from_mdl_bytes, weapon_material_candidate_paths,
    weapon_model_candidate_paths,
};
use xiv_companion_data::{
    MaterialSamplerLogicalRole, ModelTextureKind, ShaderPackageKeyDefaultDebug,
    ShaderPackageMaterialConstantDebug, ShaderPackageSamplerResourceDebug,
    ShaderPackageSemanticDebug, material_debug_info_from_mtrl_bytes,
    shader_package_semantic_debug_from_resource,
};

const MAX_SEMANTIC_REPRESENTATIVES: usize = 3;
const MAX_RAMP_F16: f32 = 65_504.0;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFamilyAudit {
    game_dir: String,
    catalog_items: usize,
    unique_models: usize,
    scanned_models: usize,
    scanned_materials: usize,
    unique_material_resources: usize,
    unique_shader_packages: usize,
    family_counts: BTreeMap<String, usize>,
    lod0_mesh_range_model_counts: BTreeMap<String, usize>,
    lod0_mesh_range_mesh_counts: BTreeMap<String, usize>,
    sampler_coverage: Vec<WeaponMaterialSamplerCoverage>,
    material_key_coverage: Vec<WeaponMaterialKeyCoverage>,
    material_constant_coverage: Vec<WeaponMaterialConstantCoverage>,
    unknown_key_category_count: usize,
    unknown_key_value_count: usize,
    unknown_constant_id_count: usize,
    unknown_sampler_role_count: usize,
    unresolved_sampler_name_count: usize,
    color_table_diffuse: WeaponColorTableScalarCoverage,
    color_table_specular: WeaponColorTableScalarCoverage,
    color_table_emissive: WeaponColorTableScalarCoverage,
    color_table_metalness: WeaponColorTableScalarCoverage,
    color_table_roughness: WeaponColorTableScalarCoverage,
    color_table_gloss_strength: WeaponColorTableScalarCoverage,
    color_table_specular_strength: WeaponColorTableScalarCoverage,
    color_table_anisotropy: WeaponColorTableScalarCoverage,
    color_table_sheen_rate: WeaponColorTableScalarCoverage,
    color_table_sheen_aptitude: WeaponColorTableScalarCoverage,
    color_table_sphere_mask: WeaponColorTableScalarCoverage,
    unknown_constant_dxbc: Vec<UnknownConstantDxbcPackageAudit>,
    alpha_shaping_dxbc: Vec<AlphaShapingDxbcPackageAudit>,
    vertex_texcoord4_dxbc: Vec<VertexTexcoord4DxbcPackageAudit>,
    tile_mip_dxbc: Vec<TileMipDxbcPackageAudit>,
    tile_blend_dxbc: Vec<TileBlendDxbcPackageAudit>,
    texture_mip_dxbc: Vec<TextureMipDxbcPackageAudit>,
    material_strength_dxbc: Vec<MaterialStrengthDxbcPackageAudit>,
    candidates: Vec<WeaponShaderFamilyCandidate>,
    unclassified_materials: Vec<WeaponShaderFamilyCandidate>,
    resource_collisions: Vec<WeaponMaterialResourceCollision>,
    unresolved_material_references: Vec<WeaponUnresolvedMaterialReference>,
    shape_models: Vec<WeaponShapeModel>,
    semantic_failures: Vec<String>,
    failures: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TileMipDxbcPackageAudit {
    shader_package_name: String,
    pixel_shader_count: usize,
    declared_shader_count: usize,
    consumer_shader_count: usize,
    use_count: usize,
    formula_count: usize,
    uses_per_shader: BTreeMap<usize, usize>,
    consumer_sets: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TextureMipDxbcPackageAudit {
    shader_package_name: String,
    pixel_shader_count: usize,
    declared_shader_count: usize,
    consumer_shader_count: usize,
    sum_count: usize,
    consumer_sets: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnknownConstantDxbcPackageAudit {
    shader_package_name: String,
    byte_offset: u16,
    dxbc_operand: String,
    pixel_shader_count: usize,
    consumer_shader_count: usize,
    use_count: usize,
    vertex_alpha_remap_count: usize,
    immediate_alpha_product_count: usize,
    alpha_threshold_test_count: usize,
    instruction_patterns: BTreeMap<String, usize>,
    representative_uses: Vec<String>,
}

/// Evidence-only audit for the view-dependent alpha path.  This deliberately
/// records the DXBC sequence rather than claiming that the preview renderer's
/// normal/view inputs are equivalent to the game's inputs.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AlphaShapingDxbcPackageAudit {
    shader_package_name: String,
    pixel_shader_count: usize,
    aperture_use_count: usize,
    offset_use_count: usize,
    formula_count: usize,
    alpha_composition_count: usize,
    unclassified_formula_count: usize,
    unclassified_downstream_use_count: usize,
    unclassified_dead_count: usize,
    unclassified_first_use_opcodes: BTreeMap<String, usize>,
    shaping_dot_count: usize,
    view_from_v6_count: usize,
    non_view_dot_producer_opcodes: BTreeMap<String, usize>,
    scaled_alpha_count: usize,
    shaping_scale_operands: BTreeMap<String, usize>,
    shaping_base_operands: BTreeMap<String, usize>,
    shaping_scale_root_sources: BTreeMap<String, usize>,
    shaping_base_root_sources: BTreeMap<String, usize>,
    offset_sign_gate_count: usize,
    alpha_less_than_one_gate_count: usize,
}

/// Vertex-stage evidence for the interpolant consumed as `v6` by the alpha
/// shaping pixel shaders.  The report intentionally stops at the VS output;
/// a matching coordinate-space formula still needs to be demonstrated before
/// it can be substituted with the preview camera/world-position vector.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VertexTexcoord4DxbcPackageAudit {
    shader_package_name: String,
    vertex_shader_count: usize,
    texcoord4_output_count: usize,
    texcoord4_registers: BTreeMap<String, usize>,
    texcoord4_write_count: usize,
    texcoord4_write_opcodes: BTreeMap<String, usize>,
    representative_writes: Vec<String>,
    texcoord4_w_pixel_shader_count: usize,
    texcoord4_w_output_reach_counts: BTreeMap<String, usize>,
    texcoord4_w_output_representatives: BTreeMap<String, Vec<usize>>,
    legacy_gbuffer1_w_producer_pixel_shader_count: usize,
    legacy_gbuffer1_w_producer_pass_pair_count: usize,
    legacy_gbuffer1_w_producer_vertex_shader_count: usize,
    legacy_gbuffer1_w_producer_vertex_height_clamp_count: usize,
    legacy_gbuffer1_w_producer_vertex_shaders: Vec<usize>,
    legacy_gbuffer1_w_producer_vertex_height_clamp_shaders: Vec<usize>,
    legacy_gbuffer1_w_producer_vertex_other_writes: Vec<String>,
    legacy_gbuffer1_w_producer_vertex_wetness_reflection_count: usize,
    legacy_gbuffer1_w_producer_vertex_wetness_reflection_unclassified_shaders: Vec<usize>,
    /// Per-output write coverage for the 288 Legacy PS that carry
    /// TEXCOORD4.w into the GBuffer1 producer attachment.
    legacy_gbuffer1_producer_o1_write_counts: BTreeMap<String, usize>,
    legacy_gbuffer1_producer_o1_write_opcodes: BTreeMap<String, usize>,
    /// `o1.x` is not a single material scalar. Group the producer pixel
    /// shaders by their final write opcode, then retain the SHPK node/pass and
    /// material-key coverage for each group so pass-specific semantics are not
    /// flattened into one renderer input.
    legacy_gbuffer1_producer_o1_x_pixel_shader_counts: BTreeMap<String, usize>,
    legacy_gbuffer1_producer_o1_x_representative_pixel_shaders: BTreeMap<String, Vec<usize>>,
    legacy_gbuffer1_producer_o1_x_pass_pair_counts: BTreeMap<String, usize>,
    legacy_gbuffer1_producer_o1_x_node_counts: BTreeMap<String, usize>,
    legacy_gbuffer1_producer_o1_x_pass_ids: BTreeMap<String, BTreeMap<String, usize>>,
    legacy_gbuffer1_producer_o1_x_material_key_sets: BTreeMap<String, BTreeMap<String, usize>>,
    /// ColorTable source lane to producer-output reachability.  The resource
    /// lane is kept physical so shader swizzles cannot hide a packed W value.
    legacy_gbuffer1_producer_table_o1_reach_counts: BTreeMap<String, usize>,
    legacy_gbuffer1_producer_table_o1_representatives: Vec<String>,
    alpha_pixel_shader_count: usize,
    alpha_vertex_shader_count: usize,
    alpha_vertex_texcoord4_registers: BTreeMap<String, usize>,
    alpha_vertex_projection_link_count: usize,
    alpha_vertex_representative_writes: Vec<String>,
    alpha_vertex_representative_traces: Vec<String>,
    gloss_o0_pixel_shader_count: usize,
    gloss_o0_pass_pair_count: usize,
    gloss_o0_vertex_shader_count: usize,
    gloss_o0_vertex_texcoord4_registers: BTreeMap<String, usize>,
    gloss_o0_vertex_projection_link_count: usize,
    gloss_o0_vertex_w_write_count: usize,
    gloss_o0_vertex_w_height_clamp_count: usize,
    gloss_o0_vertex_w_write_opcodes: BTreeMap<String, usize>,
    gloss_o0_vertex_w_root_source_sets: BTreeMap<String, usize>,
    gloss_o0_vertex_scalar_parameters: BTreeMap<String, usize>,
    gloss_o0_vertex_representative_writes: Vec<String>,
    gloss_o0_vertex_representative_traces: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TileBlendDxbcPackageAudit {
    shader_package_name: String,
    pixel_shader_count: usize,
    orb_neutral_pair_count: usize,
    orb_blend_pair_count: usize,
    ordered_ab_blend_pair_count: usize,
    normal_blend_pair_count: usize,
    shaping_table_sample_count: usize,
    shaping_anisotropy_a_sample_count: usize,
    index_texture_sample_count: usize,
    inverted_index_weight_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialStrengthDxbcPackageAudit {
    shader_package_name: String,
    pixel_shader_count: usize,
    roughness_sample_count: usize,
    roughness_pixel_shader_count: usize,
    roughness_consumer_sample_count: usize,
    roughness_consumer_opcodes: BTreeMap<String, usize>,
    roughness_o1_y_reach_count: usize,
    roughness_consumer_representatives: Vec<String>,
    gloss_sample_count: usize,
    gloss_pixel_shader_count: usize,
    gloss_consumer_sample_count: usize,
    gloss_consumer_opcodes: BTreeMap<String, usize>,
    gloss_o1_y_reach_count: usize,
    gloss_o0_rgb_reach_count: usize,
    gloss_power_chain_count: usize,
    gloss_power_o0_rgb_reach_count: usize,
    gloss_camera_reflection_power_chain_count: usize,
    gloss_camera_reflection_lobe_count: usize,
    gloss_camera_reflection_lobe_unclassified_pixel_shaders: Vec<usize>,
    gloss_cube_lod_sample_count: usize,
    gloss_cube_sample_hdr_decode_count: usize,
    gloss_cube_sample_o0_rgb_reach_count: usize,
    gloss_cube_current_location_sample_count: usize,
    gloss_cube_previous_location_sample_count: usize,
    gloss_ambient_location_interpolation_count: usize,
    gloss_ambient_reflection_scale_offset_count: usize,
    gloss_ambient_bake_light_composition_count: usize,
    gloss_ambient_bake_light_unclassified_pixel_shaders: Vec<usize>,
    gloss_environment_specular_strength_join_count: usize,
    gloss_environment_specular_strength_unjoined_pixel_shaders: Vec<usize>,
    gloss_cube_specular_strength_pixel_shader_count: usize,
    gloss_non_cube_specular_strength_pixel_shader_count: usize,
    gloss_texcoord4_w_environment_blend_count: usize,
    gloss_gbuffer1_w_environment_blend_count: usize,
    gloss_environment_blend_unclassified_pixel_shaders: Vec<usize>,
    gloss_consumer_opcode_sequences: BTreeMap<String, usize>,
    gloss_consumer_classes: Vec<MaterialStrengthDxbcGlossConsumerClassAudit>,
    gloss_consumer_representatives: Vec<String>,
    gloss_node_count: usize,
    gloss_material_key_sets: BTreeMap<String, usize>,
    specular_strength_sample_count: usize,
    specular_strength_pixel_shader_count: usize,
    specular_strength_without_gloss_pixel_shader_count: usize,
    specular_strength_consumer_sample_count: usize,
    specular_strength_consumer_opcodes: BTreeMap<String, usize>,
    specular_strength_composition_classes: Vec<MaterialStrengthDxbcSpecularClassAudit>,
    specular_strength_terminal_unclassified_pixel_shaders: Vec<usize>,
    /// Deferred Legacy passes read material intermediates from GBuffer1 rather
    /// than resampling every ColorTable lane. Keep the physical resource lane
    /// and its first consumers separate because DXBC output swizzles can pack
    /// multiple lanes into one temporary register.
    gbuffer1_sample_count: usize,
    gbuffer1_pixel_shader_count: usize,
    gbuffer1_lane_sample_counts: BTreeMap<String, usize>,
    gbuffer1_lane_consumer_opcodes: BTreeMap<String, BTreeMap<String, usize>>,
    gbuffer1_lane_o0_rgb_reach_counts: BTreeMap<String, usize>,
    /// Per-PS direct-consumer shape for physical GBuffer1.X, plus the sampled
    /// resources whose values subsequently join the X taint. This separates
    /// the deferred material multiplier from the independent W environment
    /// control without assigning an unsupported public material name.
    gbuffer1_x_consumer_signatures: BTreeMap<String, usize>,
    gbuffer1_x_resource_join_counts: BTreeMap<String, usize>,
    gbuffer1_x_terminal_multiplier_count: usize,
    gbuffer1_x_terminal_multiplier_o0_rgb_reach_count: usize,
    gbuffer1_x_terminal_multiplier_resource_counts: BTreeMap<String, usize>,
    gbuffer1_x_post_multiplier_consumer_signatures: BTreeMap<String, usize>,
    gbuffer1_x_post_multiplier_resource_counts: BTreeMap<String, usize>,
    gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders: Vec<usize>,
    gbuffer1_x_node_count: usize,
    gbuffer1_x_pass_ids: BTreeMap<String, usize>,
    gbuffer1_x_material_key_sets: BTreeMap<String, usize>,
    gbuffer1_x_representative_pixel_shaders: Vec<usize>,
    gbuffer1_consumer_representatives: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialStrengthDxbcGlossConsumerClassAudit {
    consumer_signature: String,
    opcode_sequence: String,
    literal_constants: Vec<String>,
    sample_count: usize,
    o1_y_reach_count: usize,
    o0_rgb_reach_count: usize,
    power_chain_count: usize,
    power_o0_rgb_reach_count: usize,
    camera_reflection_power_chain_count: usize,
    camera_reflection_lobe_count: usize,
    cube_lod_sample_count: usize,
    cube_sample_hdr_decode_count: usize,
    cube_sample_o0_rgb_reach_count: usize,
    cube_current_location_sample_count: usize,
    cube_previous_location_sample_count: usize,
    ambient_location_interpolation_count: usize,
    ambient_reflection_scale_offset_count: usize,
    ambient_bake_light_composition_count: usize,
    environment_specular_strength_join_count: usize,
    texcoord4_w_environment_blend_count: usize,
    gbuffer1_w_environment_blend_count: usize,
    pixel_shader_count: usize,
    node_count: usize,
    pass_ids: BTreeMap<String, usize>,
    material_key_sets: BTreeMap<String, usize>,
    representative_pixel_shaders: Vec<usize>,
    representative_trace: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterialStrengthDxbcSpecularClassAudit {
    class_name: String,
    pixel_shader_count: usize,
    product_o0_rgb_reach_count: usize,
    product_other_resource_counts: BTreeMap<String, usize>,
    first_post_product_consumer_opcodes: BTreeMap<String, usize>,
    fifth_root_shaping_count: usize,
    terminal_rgb_multiplier_count: usize,
    terminal_rgb_multiplier_o0_reach_count: usize,
    terminal_rgb_multiplier_resource_counts: BTreeMap<String, usize>,
    post_terminal_multiplier_opcodes: BTreeMap<String, usize>,
    post_terminal_multiplier_resource_counts: BTreeMap<String, usize>,
    terminal_rgb_multiplier_unclassified_pixel_shaders: Vec<usize>,
    dynamic_emissive_o0_rgb_reach_count: usize,
    dynamic_emissive_table_join_o0_rgb_reach_count: usize,
    dynamic_emissive_luminance_scale_o0_rgb_reach_count: usize,
    dynamic_emissive_luminance_scale_composition_opcodes: BTreeMap<String, usize>,
    dynamic_emissive_luminance_scale_texture_resource_counts: BTreeMap<String, usize>,
    dynamic_emissive_luminance_scale_constant_buffer_vector_counts: BTreeMap<String, usize>,
    dynamic_emissive_luminance_source_o0_rgb_reach_count: usize,
    dynamic_emissive_luminance_source_composition_opcodes: BTreeMap<String, usize>,
    dynamic_emissive_luminance_scale_unclassified_pixel_shaders: Vec<usize>,
    instance_mul_color_o0_rgb_reach_count: usize,
    instance_env_parameter_o0_rgb_reach_count: usize,
    instance_camera_diffuse_specular_o0_rgb_reach_count: usize,
    instance_camera_rim_o0_rgb_reach_count: usize,
    node_count: usize,
    pass_ids: BTreeMap<String, usize>,
    material_key_sets: BTreeMap<String, usize>,
    representative_pixel_shaders: Vec<usize>,
    representative_trace: String,
}

#[cfg(windows)]
#[derive(Default)]
struct MaterialStrengthDxbcSpecularClassAccumulator {
    pixel_shaders: BTreeSet<usize>,
    product_o0_rgb_reach_count: usize,
    product_other_resource_counts: BTreeMap<String, usize>,
    first_post_product_consumer_opcodes: BTreeMap<String, usize>,
    fifth_root_shaping_count: usize,
    terminal_rgb_multiplier_count: usize,
    terminal_rgb_multiplier_o0_reach_count: usize,
    terminal_rgb_multiplier_resource_counts: BTreeMap<String, usize>,
    post_terminal_multiplier_opcodes: BTreeMap<String, usize>,
    post_terminal_multiplier_resource_counts: BTreeMap<String, usize>,
    terminal_rgb_multiplier_unclassified_pixel_shaders: BTreeSet<usize>,
    dynamic_emissive_o0_rgb_reach_count: usize,
    dynamic_emissive_table_join_o0_rgb_reach_count: usize,
    dynamic_emissive_luminance_scale_o0_rgb_reach_count: usize,
    dynamic_emissive_luminance_scale_composition_opcodes: BTreeMap<String, usize>,
    dynamic_emissive_luminance_scale_texture_resource_counts: BTreeMap<String, usize>,
    dynamic_emissive_luminance_scale_constant_buffer_vector_counts: BTreeMap<String, usize>,
    dynamic_emissive_luminance_source_o0_rgb_reach_count: usize,
    dynamic_emissive_luminance_source_composition_opcodes: BTreeMap<String, usize>,
    dynamic_emissive_luminance_scale_unclassified_pixel_shaders: BTreeSet<usize>,
    instance_mul_color_o0_rgb_reach_count: usize,
    instance_env_parameter_o0_rgb_reach_count: usize,
    instance_camera_diffuse_specular_o0_rgb_reach_count: usize,
    instance_camera_rim_o0_rgb_reach_count: usize,
    node_count: usize,
    pass_ids: BTreeMap<String, usize>,
    material_key_sets: BTreeMap<String, usize>,
    representative_trace: String,
}

#[cfg(windows)]
struct MaterialStrengthDxbcSpecularShaderTrace {
    sampled_resources: BTreeSet<String>,
    product_o0_rgb_reaches: bool,
    product_other_resources: BTreeSet<String>,
    first_post_product_consumer_opcode: Option<String>,
    has_fifth_root_shaping: bool,
    terminal_rgb_multiplier_o0_reaches: Option<bool>,
    terminal_rgb_multiplier_resources: BTreeSet<String>,
    post_terminal_multiplier_opcode: Option<String>,
    post_terminal_multiplier_resources: BTreeSet<String>,
    dynamic_emissive_o0_rgb_reaches: bool,
    dynamic_emissive_table_join_o0_rgb_reaches: bool,
    dynamic_emissive_luminance_scale: Option<DxbcDynamicEmissiveLuminanceScaleTrace>,
    instance_mul_color_o0_rgb_reaches: bool,
    instance_env_parameter_o0_rgb_reaches: bool,
    instance_camera_diffuse_specular_o0_rgb_reaches: bool,
    instance_camera_rim_o0_rgb_reaches: bool,
    representative_trace: String,
}

#[cfg(windows)]
struct DxbcDynamicEmissiveLuminanceScaleTrace {
    composition_opcode: String,
    texture_resources: BTreeSet<String>,
    constant_buffer_vectors: BTreeSet<String>,
    source_o0_rgb_composition_opcode: Option<String>,
}

#[cfg(windows)]
#[derive(Default)]
struct MaterialStrengthDxbcGlossConsumerClassAccumulator {
    opcode_sequence: String,
    literal_constants: Vec<String>,
    sample_count: usize,
    o1_y_reach_count: usize,
    o0_rgb_reach_count: usize,
    power_chain_count: usize,
    power_o0_rgb_reach_count: usize,
    camera_reflection_power_chain_count: usize,
    camera_reflection_lobe_count: usize,
    cube_lod_sample_count: usize,
    cube_sample_hdr_decode_count: usize,
    cube_sample_o0_rgb_reach_count: usize,
    cube_current_location_sample_count: usize,
    cube_previous_location_sample_count: usize,
    ambient_location_interpolation_count: usize,
    ambient_reflection_scale_offset_count: usize,
    ambient_bake_light_composition_count: usize,
    environment_specular_strength_join_count: usize,
    texcoord4_w_environment_blend_count: usize,
    gbuffer1_w_environment_blend_count: usize,
    pixel_shaders: BTreeSet<usize>,
    node_count: usize,
    pass_ids: BTreeMap<String, usize>,
    material_key_sets: BTreeMap<String, usize>,
    representative_trace: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialSamplerCoverage {
    shader_package_name: String,
    texture_usage: u32,
    texture_usage_hex: String,
    texture_usage_name: Option<String>,
    logical_role: Option<MaterialSamplerLogicalRole>,
    texture_kind: Option<ModelTextureKind>,
    flags: u32,
    flags_hex: String,
    material_resource_count: usize,
    material_reference_count: usize,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
enum WeaponShaderKeyScope {
    Material,
    System,
    Scene,
    MaterialOverrideOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponSemanticRepresentative {
    item_reference_count: usize,
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    material_path: String,
    shader_flags: u32,
    shader_flags_hex: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialKeyValueCount {
    value: u32,
    value_hex: String,
    value_name: Option<String>,
    material_resource_count: usize,
    material_reference_count: usize,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialKeyCoverage {
    shader_package_name: String,
    scope: WeaponShaderKeyScope,
    category: u32,
    category_hex: String,
    category_name: Option<String>,
    default_value: Option<u32>,
    default_value_hex: Option<String>,
    default_value_name: Option<String>,
    material_resource_count: usize,
    material_reference_count: usize,
    material_override_resource_count: usize,
    material_override_reference_count: usize,
    non_default_override_resource_count: usize,
    non_default_override_reference_count: usize,
    observed_values: Vec<WeaponMaterialKeyValueCount>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialConstantValueCount {
    values: Vec<Option<f32>>,
    raw_values_hex: Vec<String>,
    material_resource_count: usize,
    material_reference_count: usize,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFlagCount {
    shader_flags: u32,
    shader_flags_hex: String,
    material_resource_count: usize,
    material_reference_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialConstantCoverage {
    shader_package_name: String,
    id: u32,
    id_hex: String,
    name: Option<String>,
    package_byte_offset: Option<u16>,
    package_byte_size: Option<u16>,
    default_values: Option<Vec<Option<f32>>>,
    default_raw_values_hex: Option<Vec<String>>,
    material_resource_count: usize,
    material_reference_count: usize,
    material_override_resource_count: usize,
    material_override_reference_count: usize,
    non_default_override_resource_count: usize,
    non_default_override_reference_count: usize,
    malformed_override_resource_count: usize,
    malformed_override_reference_count: usize,
    non_finite_resource_count: usize,
    non_finite_reference_count: usize,
    unresolved_value_resource_count: usize,
    unresolved_value_reference_count: usize,
    value_width_resource_counts: BTreeMap<usize, usize>,
    malformed_override_value_size_resource_counts: BTreeMap<u16, usize>,
    observed_values: Vec<WeaponMaterialConstantValueCount>,
    shader_flag_counts: Vec<WeaponShaderFlagCount>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponColorTableScalarCoverage {
    material_resource_count: usize,
    material_reference_count: usize,
    nonzero_material_resource_count: usize,
    nonzero_material_reference_count: usize,
    minimum: Option<f32>,
    maximum: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default)]
struct ObservedColorTableScalars<'a> {
    diffuse: &'a [f32],
    specular: &'a [f32],
    emissive: &'a [f32],
    metalness: &'a [f32],
    roughness: &'a [f32],
    gloss_strength: &'a [f32],
    specular_strength: &'a [f32],
    anisotropy: &'a [f32],
    sheen_rate: &'a [f32],
    sheen_aptitude: &'a [f32],
    sphere_mask: &'a [f32],
}

fn invalid_color_table_float_ramp_value(
    rows: &[xiv_companion_data::MaterialColorTableRowDebug],
) -> Option<(usize, &'static str, f32)> {
    rows.iter().find_map(|row| {
        row.diffuse_color
            .iter()
            .flatten()
            .map(|value| ("Diffuse", *value))
            .chain(
                row.specular_color
                    .iter()
                    .flatten()
                    .map(|value| ("Specular", *value)),
            )
            .chain(
                row.emissive_color
                    .iter()
                    .flatten()
                    .map(|value| ("Emissive", *value)),
            )
            .chain(row.metalness.iter().map(|value| ("Metalness", *value)))
            .chain(row.roughness.iter().map(|value| ("Roughness", *value)))
            .chain(
                row.gloss_strength
                    .iter()
                    .map(|value| ("GlossStrength", *value)),
            )
            .chain(
                row.specular_strength
                    .iter()
                    .map(|value| ("SpecularStrength", *value)),
            )
            .chain(row.anisotropy.iter().map(|value| ("Anisotropy", *value)))
            .chain(row.sheen_rate.iter().map(|value| ("SheenRate", *value)))
            .chain(row.sheen_tint.iter().map(|value| ("SheenTint", *value)))
            .chain(
                row.sheen_aperture
                    .iter()
                    .map(|value| ("SheenAptitude", *value)),
            )
            .chain(row.sphere_mask.iter().map(|value| ("SphereMask", *value)))
            .find(|(_, value)| !value.is_finite() || value.abs() > MAX_RAMP_F16)
            .map(|(field, value)| (row.index, field, value))
    })
}

fn observe_color_table_scalar(
    coverage: &mut WeaponColorTableScalarCoverage,
    values: &[f32],
    unique_resource: bool,
    references: usize,
) {
    if values.is_empty() {
        return;
    }
    coverage.material_reference_count += references;
    if unique_resource {
        coverage.material_resource_count += 1;
    }
    if values.iter().any(|value| value.abs() > 1.0e-6) {
        coverage.nonzero_material_reference_count += references;
        if unique_resource {
            coverage.nonzero_material_resource_count += 1;
        }
    }
    for value in values {
        coverage.minimum = Some(
            coverage
                .minimum
                .map_or(*value, |minimum| minimum.min(*value)),
        );
        coverage.maximum = Some(
            coverage
                .maximum
                .map_or(*value, |maximum| maximum.max(*value)),
        );
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SemanticCount {
    resources: usize,
    references: usize,
}

impl SemanticCount {
    fn observe(&mut self, unique_resource: bool, references: usize) {
        self.references += references;
        if unique_resource {
            self.resources += 1;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialKeyCoverageId {
    shader_package_name: String,
    scope: WeaponShaderKeyScope,
    category: u32,
}

#[derive(Debug)]
struct MaterialKeyValueAccumulator {
    value_name: Option<String>,
    count: SemanticCount,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Debug)]
struct MaterialKeyCoverageAccumulator {
    category_name: Option<String>,
    default_value: Option<u32>,
    default_value_name: Option<String>,
    count: SemanticCount,
    override_count: SemanticCount,
    non_default_override_count: SemanticCount,
    observed_values: BTreeMap<u32, MaterialKeyValueAccumulator>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialConstantCoverageId {
    shader_package_name: String,
    id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialSamplerCoverageId {
    shader_package_name: String,
    texture_usage: u32,
    flags: u32,
}

#[derive(Debug)]
struct MaterialSamplerCoverageAccumulator {
    texture_usage_name: Option<String>,
    logical_role: Option<MaterialSamplerLogicalRole>,
    texture_kind: Option<ModelTextureKind>,
    count: SemanticCount,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MaterialConstantValueKey(Vec<u32>);

#[derive(Debug)]
struct MaterialConstantValueAccumulator {
    values: Vec<f32>,
    count: SemanticCount,
    representatives: Vec<WeaponSemanticRepresentative>,
}

impl MaterialConstantValueAccumulator {
    fn observe(&mut self, unique_resource: bool, representative: &WeaponSemanticRepresentative) {
        self.count
            .observe(unique_resource, representative.item_reference_count);
        add_semantic_representative(&mut self.representatives, representative);
    }
}

#[derive(Debug)]
struct MaterialConstantCoverageAccumulator {
    name: Option<String>,
    package_byte_offset: Option<u16>,
    package_byte_size: Option<u16>,
    default_values: Option<Vec<f32>>,
    count: SemanticCount,
    override_count: SemanticCount,
    non_default_override_count: SemanticCount,
    malformed_override_count: SemanticCount,
    non_finite_count: SemanticCount,
    unresolved_value_count: SemanticCount,
    value_width_resource_counts: BTreeMap<usize, usize>,
    malformed_override_value_size_resource_counts: BTreeMap<u16, usize>,
    observed_values: BTreeMap<MaterialConstantValueKey, MaterialConstantValueAccumulator>,
    shader_flag_counts: BTreeMap<u32, SemanticCount>,
    representatives: Vec<WeaponSemanticRepresentative>,
}

#[derive(Clone, Debug)]
struct ObservedMaterialKey {
    category: u32,
    category_name: Option<String>,
    value: u32,
    value_name: Option<String>,
}

#[derive(Clone, Debug)]
struct ObservedMaterialConstant {
    id: u32,
    name: Option<String>,
    values: Vec<f32>,
    value_size: u16,
    malformed: bool,
    resolved: bool,
}

#[derive(Debug)]
struct ObservedMaterialConstantGroup {
    name: Option<String>,
    effective_values: Option<Vec<f32>>,
    malformed_value_sizes: BTreeSet<u16>,
    non_finite: bool,
}

#[derive(Clone, Debug)]
struct ObservedMaterialSampler {
    texture_usage: u32,
    texture_usage_name: Option<String>,
    logical_role: Option<MaterialSamplerLogicalRole>,
    texture_kind: Option<ModelTextureKind>,
    flags: u32,
}

#[derive(Default)]
struct MaterialSemanticCoverageBuilder {
    material_resources: HashSet<String>,
    shader_packages: HashSet<String>,
    shader_package_cache: HashMap<String, Option<ShaderPackageSemanticDebug>>,
    sampler_coverage: BTreeMap<MaterialSamplerCoverageId, MaterialSamplerCoverageAccumulator>,
    key_coverage: BTreeMap<MaterialKeyCoverageId, MaterialKeyCoverageAccumulator>,
    constant_coverage: BTreeMap<MaterialConstantCoverageId, MaterialConstantCoverageAccumulator>,
    color_table_diffuse: WeaponColorTableScalarCoverage,
    color_table_specular: WeaponColorTableScalarCoverage,
    color_table_emissive: WeaponColorTableScalarCoverage,
    color_table_metalness: WeaponColorTableScalarCoverage,
    color_table_roughness: WeaponColorTableScalarCoverage,
    color_table_gloss_strength: WeaponColorTableScalarCoverage,
    color_table_specular_strength: WeaponColorTableScalarCoverage,
    color_table_anisotropy: WeaponColorTableScalarCoverage,
    color_table_sheen_rate: WeaponColorTableScalarCoverage,
    color_table_sheen_aptitude: WeaponColorTableScalarCoverage,
    color_table_sphere_mask: WeaponColorTableScalarCoverage,
    failures: Vec<String>,
}

struct MaterialSemanticCoverageResult {
    unique_material_resources: usize,
    unique_shader_packages: usize,
    sampler_coverage: Vec<WeaponMaterialSamplerCoverage>,
    material_key_coverage: Vec<WeaponMaterialKeyCoverage>,
    material_constant_coverage: Vec<WeaponMaterialConstantCoverage>,
    unknown_key_category_count: usize,
    unknown_key_value_count: usize,
    unknown_constant_id_count: usize,
    unknown_sampler_role_count: usize,
    unresolved_sampler_name_count: usize,
    color_table_diffuse: WeaponColorTableScalarCoverage,
    color_table_specular: WeaponColorTableScalarCoverage,
    color_table_emissive: WeaponColorTableScalarCoverage,
    color_table_metalness: WeaponColorTableScalarCoverage,
    color_table_roughness: WeaponColorTableScalarCoverage,
    color_table_gloss_strength: WeaponColorTableScalarCoverage,
    color_table_specular_strength: WeaponColorTableScalarCoverage,
    color_table_anisotropy: WeaponColorTableScalarCoverage,
    color_table_sheen_rate: WeaponColorTableScalarCoverage,
    color_table_sheen_aptitude: WeaponColorTableScalarCoverage,
    color_table_sphere_mask: WeaponColorTableScalarCoverage,
    failures: Vec<String>,
}

impl MaterialSemanticCoverageBuilder {
    #[allow(clippy::too_many_arguments)]
    fn record_material<R: Resource>(
        &mut self,
        resource: &mut R,
        model: PackedModelId,
        items: &[&WeaponCatalogItem],
        model_path: &str,
        material_name: &str,
        material_path: &str,
        shader_package_name: &str,
        material_bytes: &[u8],
    ) {
        let debug = match material_debug_info_from_mtrl_bytes(material_path, material_bytes) {
            Ok(debug) => debug,
            Err(error) => {
                self.failures.push(format!(
                    "{} material semantic debug ({}) failed: {error:#}",
                    material_path,
                    item_label(items)
                ));
                return;
            }
        };

        self.shader_packages.insert(shader_package_name.to_string());
        if !self.shader_package_cache.contains_key(shader_package_name) {
            let package =
                match shader_package_semantic_debug_from_resource(resource, shader_package_name) {
                    Ok(package) => Some(package),
                    Err(error) => {
                        self.failures.push(format!(
                            "shader package {} ({}) failed: {error:#}",
                            shader_package_name,
                            item_label(items)
                        ));
                        None
                    }
                };
            self.shader_package_cache
                .insert(shader_package_name.to_string(), package);
        }
        let shader_package = self
            .shader_package_cache
            .get(shader_package_name)
            .and_then(Clone::clone);

        if let Some(color_table) = debug.color_table.as_ref() {
            if let Some((row, field, value)) =
                invalid_color_table_float_ramp_value(&color_table.rows)
            {
                self.failures.push(format!(
                    "{material_path} ColorTable row {row} {field}={value:?} cannot be represented by Rgba16Float"
                ));
            }
        }

        let keys = debug
            .summary
            .shader_keys
            .iter()
            .map(|key| ObservedMaterialKey {
                category: key.category,
                category_name: key.category_name.clone(),
                value: key.value,
                value_name: key.value_name.clone(),
            })
            .collect::<Vec<_>>();
        let constant_names = debug
            .summary
            .constants
            .iter()
            .map(|constant| (constant.id, constant.name.clone()))
            .collect::<HashMap<_, _>>();
        let constants = debug
            .constants
            .iter()
            .map(|constant| {
                let expected_count = usize::from(constant.value_size) / 4;
                ObservedMaterialConstant {
                    id: constant.id,
                    name: constant_names.get(&constant.id).cloned().flatten(),
                    values: constant.values.clone(),
                    value_size: constant.value_size,
                    malformed: constant.value_size < 4
                        || usize::from(constant.value_size) % 4 != 0
                        || constant.values.len() != expected_count,
                    resolved: constant.value_size >= 4 && constant.values.len() == expected_count,
                }
            })
            .collect::<Vec<_>>();
        let samplers = debug
            .samplers
            .iter()
            .map(|sampler| ObservedMaterialSampler {
                texture_usage: sampler.texture_usage,
                texture_usage_name: sampler.texture_usage_name.clone(),
                logical_role: sampler.logical_role,
                texture_kind: sampler.kind,
                flags: sampler.flags,
            })
            .collect::<Vec<_>>();
        let color_table_channel =
            |channel: fn(&xiv_companion_data::MaterialColorTableRowDebug) -> Option<[f32; 3]>| {
                debug
                    .color_table
                    .as_ref()
                    .into_iter()
                    .flat_map(|table| {
                        table
                            .rows
                            .iter()
                            .flat_map(move |row| channel(row).into_iter().flatten())
                    })
                    .filter(|value| value.is_finite())
                    .collect::<Vec<_>>()
            };
        let diffuse = color_table_channel(|row| row.diffuse_color);
        let specular = color_table_channel(|row| row.specular_color);
        let emissive = color_table_channel(|row| row.emissive_color);
        let color_table_scalar =
            |channel: fn(&xiv_companion_data::MaterialColorTableRowDebug) -> Option<f32>| {
                debug
                    .color_table
                    .as_ref()
                    .into_iter()
                    .flat_map(|table| table.rows.iter().filter_map(channel))
                    .filter(|value| value.is_finite())
                    .collect::<Vec<_>>()
            };
        let metalness = color_table_scalar(|row| row.metalness);
        let roughness = color_table_scalar(|row| row.roughness);
        let gloss_strength = color_table_scalar(|row| row.gloss_strength);
        let specular_strength = color_table_scalar(|row| row.specular_strength);
        let anisotropy = debug
            .color_table
            .as_ref()
            .into_iter()
            .flat_map(|table| table.rows.iter().filter_map(|row| row.anisotropy))
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        let sheen_rate = debug
            .color_table
            .as_ref()
            .into_iter()
            .flat_map(|table| table.rows.iter().filter_map(|row| row.sheen_rate))
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        let sheen_aptitude = debug
            .color_table
            .as_ref()
            .into_iter()
            .flat_map(|table| table.rows.iter().filter_map(|row| row.sheen_aperture))
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        let sphere_mask = debug
            .color_table
            .as_ref()
            .into_iter()
            .flat_map(|table| table.rows.iter().filter_map(|row| row.sphere_mask))
            .filter(|value| value.is_finite())
            .collect::<Vec<_>>();
        let representative = WeaponSemanticRepresentative {
            item_reference_count: items.len(),
            item_ids: items.iter().take(3).map(|item| item.id).collect(),
            item_names: items.iter().take(3).map(|item| item.name.clone()).collect(),
            model,
            model_path: model_path.to_string(),
            material_name: material_name.to_string(),
            material_path: material_path.to_string(),
            shader_flags: debug.shader_flags,
            shader_flags_hex: hex_u32(debug.shader_flags),
        };

        self.observe_material(
            shader_package_name,
            shader_package.as_ref(),
            material_path,
            debug.shader_flags,
            &keys,
            &constants,
            &samplers,
            ObservedColorTableScalars {
                diffuse: &diffuse,
                specular: &specular,
                emissive: &emissive,
                metalness: &metalness,
                roughness: &roughness,
                gloss_strength: &gloss_strength,
                specular_strength: &specular_strength,
                anisotropy: &anisotropy,
                sheen_rate: &sheen_rate,
                sheen_aptitude: &sheen_aptitude,
                sphere_mask: &sphere_mask,
            },
            &representative,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_material(
        &mut self,
        shader_package_name: &str,
        shader_package: Option<&ShaderPackageSemanticDebug>,
        material_path: &str,
        shader_flags: u32,
        material_keys: &[ObservedMaterialKey],
        material_constants: &[ObservedMaterialConstant],
        material_samplers: &[ObservedMaterialSampler],
        color_table: ObservedColorTableScalars<'_>,
        representative: &WeaponSemanticRepresentative,
    ) {
        let unique_resource = self.material_resources.insert(material_path.to_string());
        self.shader_packages.insert(shader_package_name.to_string());
        observe_color_table_scalar(
            &mut self.color_table_diffuse,
            color_table.diffuse,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_specular,
            color_table.specular,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_emissive,
            color_table.emissive,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_metalness,
            color_table.metalness,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_roughness,
            color_table.roughness,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_gloss_strength,
            color_table.gloss_strength,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_specular_strength,
            color_table.specular_strength,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_anisotropy,
            color_table.anisotropy,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_sheen_rate,
            color_table.sheen_rate,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_sheen_aptitude,
            color_table.sheen_aptitude,
            unique_resource,
            representative.item_reference_count,
        );
        observe_color_table_scalar(
            &mut self.color_table_sphere_mask,
            color_table.sphere_mask,
            unique_resource,
            representative.item_reference_count,
        );
        let mut observed_sampler_ids = BTreeSet::new();
        for sampler in material_samplers {
            let package_resource = shader_package.and_then(|package| {
                package
                    .sampler_resources
                    .iter()
                    .find(|resource| resource.crc == sampler.texture_usage)
            });
            let id = MaterialSamplerCoverageId {
                shader_package_name: shader_package_name.to_string(),
                texture_usage: sampler.texture_usage,
                flags: sampler.flags,
            };
            if !observed_sampler_ids.insert(id.clone()) {
                continue;
            }
            let coverage = self.sampler_coverage.entry(id).or_insert_with(|| {
                MaterialSamplerCoverageAccumulator {
                    texture_usage_name: package_resource
                        .map(|resource| resource.name.clone())
                        .or_else(|| sampler.texture_usage_name.clone()),
                    logical_role: package_resource
                        .and_then(|resource| resource.logical_role)
                        .or(sampler.logical_role),
                    texture_kind: package_resource
                        .and_then(|resource| resource.kind)
                        .or(sampler.texture_kind),
                    count: SemanticCount::default(),
                    representatives: Vec::new(),
                }
            });
            coverage
                .count
                .observe(unique_resource, representative.item_reference_count);
            add_semantic_representative(&mut coverage.representatives, representative);
        }
        let key_overrides = material_keys
            .iter()
            .map(|key| (key.category, key))
            .collect::<HashMap<_, _>>();
        let mut constant_overrides = BTreeMap::<u32, ObservedMaterialConstantGroup>::new();
        for constant in material_constants {
            let group = constant_overrides.entry(constant.id).or_insert_with(|| {
                ObservedMaterialConstantGroup {
                    name: constant.name.clone(),
                    effective_values: None,
                    malformed_value_sizes: BTreeSet::new(),
                    non_finite: false,
                }
            });
            if group.name.is_none() {
                group.name = constant.name.clone();
            }
            if constant.malformed {
                group.malformed_value_sizes.insert(constant.value_size);
            }
            group.non_finite |= constant.values.iter().any(|value| !value.is_finite());
            if constant.resolved {
                group.effective_values = Some(constant.values.clone());
            }
        }
        let mut package_key_categories = HashSet::new();
        let mut package_constant_ids = HashSet::new();

        if let Some(shader_package) = shader_package {
            for key in &shader_package.material_keys {
                package_key_categories.insert(key.id);
                let override_key = key_overrides.get(&key.id).copied();
                self.observe_key(
                    shader_package_name,
                    WeaponShaderKeyScope::Material,
                    key.id,
                    key.name.clone(),
                    Some(key.default_value),
                    key.default_value_name.clone(),
                    override_key.map_or(key.default_value, |value| value.value),
                    override_key.map_or_else(
                        || key.default_value_name.clone(),
                        |value| value.value_name.clone(),
                    ),
                    override_key.map(|value| value.value),
                    unique_resource,
                    representative,
                );
            }
            for key in &shader_package.system_keys {
                package_key_categories.insert(key.id);
                let override_key = key_overrides.get(&key.id).copied();
                self.observe_key(
                    shader_package_name,
                    WeaponShaderKeyScope::System,
                    key.id,
                    key.name.clone(),
                    Some(key.default_value),
                    key.default_value_name.clone(),
                    override_key.map_or(key.default_value, |value| value.value),
                    override_key.map_or_else(
                        || key.default_value_name.clone(),
                        |value| value.value_name.clone(),
                    ),
                    override_key.map(|value| value.value),
                    unique_resource,
                    representative,
                );
            }
            for key in &shader_package.scene_keys {
                package_key_categories.insert(key.id);
                let override_key = key_overrides.get(&key.id).copied();
                self.observe_key(
                    shader_package_name,
                    WeaponShaderKeyScope::Scene,
                    key.id,
                    key.name.clone(),
                    Some(key.default_value),
                    key.default_value_name.clone(),
                    override_key.map_or(key.default_value, |value| value.value),
                    override_key.map_or_else(
                        || key.default_value_name.clone(),
                        |value| value.value_name.clone(),
                    ),
                    override_key.map(|value| value.value),
                    unique_resource,
                    representative,
                );
            }

            for constant in &shader_package.material_constants {
                package_constant_ids.insert(constant.id);
                self.observe_constant(
                    shader_package_name,
                    constant.id,
                    constant.name.clone(),
                    Some(constant.byte_offset),
                    Some(constant.byte_size),
                    constant.default_values.as_deref(),
                    constant_overrides.get(&constant.id),
                    shader_flags,
                    unique_resource,
                    representative,
                );
            }
        }

        for key in material_keys
            .iter()
            .filter(|key| !package_key_categories.contains(&key.category))
        {
            self.observe_key(
                shader_package_name,
                WeaponShaderKeyScope::MaterialOverrideOnly,
                key.category,
                key.category_name.clone(),
                None,
                None,
                key.value,
                key.value_name.clone(),
                Some(key.value),
                unique_resource,
                representative,
            );
        }

        for (id, constant) in constant_overrides
            .iter()
            .filter(|(id, _)| !package_constant_ids.contains(id))
        {
            self.observe_constant(
                shader_package_name,
                *id,
                constant.name.clone(),
                None,
                None,
                None,
                Some(constant),
                shader_flags,
                unique_resource,
                representative,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_key(
        &mut self,
        shader_package_name: &str,
        scope: WeaponShaderKeyScope,
        category: u32,
        category_name: Option<String>,
        default_value: Option<u32>,
        default_value_name: Option<String>,
        effective_value: u32,
        effective_value_name: Option<String>,
        override_value: Option<u32>,
        unique_resource: bool,
        representative: &WeaponSemanticRepresentative,
    ) {
        let id = MaterialKeyCoverageId {
            shader_package_name: shader_package_name.to_string(),
            scope,
            category,
        };
        let coverage =
            self.key_coverage
                .entry(id)
                .or_insert_with(|| MaterialKeyCoverageAccumulator {
                    category_name: category_name.clone(),
                    default_value,
                    default_value_name: default_value_name.clone(),
                    count: SemanticCount::default(),
                    override_count: SemanticCount::default(),
                    non_default_override_count: SemanticCount::default(),
                    observed_values: BTreeMap::new(),
                    representatives: Vec::new(),
                });
        if coverage.category_name.is_none() {
            coverage.category_name = category_name;
        }
        if coverage.default_value_name.is_none() {
            coverage.default_value_name = default_value_name;
        }
        coverage
            .count
            .observe(unique_resource, representative.item_reference_count);
        if let Some(override_value) = override_value {
            coverage
                .override_count
                .observe(unique_resource, representative.item_reference_count);
            if default_value != Some(override_value) {
                coverage
                    .non_default_override_count
                    .observe(unique_resource, representative.item_reference_count);
            }
        }
        let value = coverage
            .observed_values
            .entry(effective_value)
            .or_insert_with(|| MaterialKeyValueAccumulator {
                value_name: effective_value_name.clone(),
                count: SemanticCount::default(),
                representatives: Vec::new(),
            });
        if value.value_name.is_none() {
            value.value_name = effective_value_name;
        }
        value
            .count
            .observe(unique_resource, representative.item_reference_count);
        add_semantic_representative(&mut value.representatives, representative);
        add_semantic_representative(&mut coverage.representatives, representative);
    }

    #[allow(clippy::too_many_arguments)]
    fn observe_constant(
        &mut self,
        shader_package_name: &str,
        id: u32,
        name: Option<String>,
        package_byte_offset: Option<u16>,
        package_byte_size: Option<u16>,
        default_values: Option<&[f32]>,
        override_constant: Option<&ObservedMaterialConstantGroup>,
        shader_flags: u32,
        unique_resource: bool,
        representative: &WeaponSemanticRepresentative,
    ) {
        let coverage_id = MaterialConstantCoverageId {
            shader_package_name: shader_package_name.to_string(),
            id,
        };
        let coverage = self
            .constant_coverage
            .entry(coverage_id)
            .or_insert_with(|| MaterialConstantCoverageAccumulator {
                name: name.clone(),
                package_byte_offset,
                package_byte_size,
                default_values: default_values.map(<[f32]>::to_vec),
                count: SemanticCount::default(),
                override_count: SemanticCount::default(),
                non_default_override_count: SemanticCount::default(),
                malformed_override_count: SemanticCount::default(),
                non_finite_count: SemanticCount::default(),
                unresolved_value_count: SemanticCount::default(),
                value_width_resource_counts: BTreeMap::new(),
                malformed_override_value_size_resource_counts: BTreeMap::new(),
                observed_values: BTreeMap::new(),
                shader_flag_counts: BTreeMap::new(),
                representatives: Vec::new(),
            });
        if coverage.name.is_none() {
            coverage.name = name;
        }
        if coverage.package_byte_offset.is_none() {
            coverage.package_byte_offset = package_byte_offset;
        }
        if coverage.package_byte_size.is_none() {
            coverage.package_byte_size = package_byte_size;
        }
        let reference_count = representative.item_reference_count;
        coverage.count.observe(unique_resource, reference_count);
        let override_values = override_constant
            .and_then(|constant| constant.effective_values.as_ref().map(Vec::as_slice));
        let effective_values = override_values.or(default_values);
        if let Some(override_constant) = override_constant {
            coverage
                .override_count
                .observe(unique_resource, reference_count);
            if override_values.is_some_and(|values| {
                default_values.is_none_or(|default| !same_f32_bits(default, values))
            }) {
                coverage
                    .non_default_override_count
                    .observe(unique_resource, reference_count);
            }
            if !override_constant.malformed_value_sizes.is_empty() {
                coverage
                    .malformed_override_count
                    .observe(unique_resource, reference_count);
                if unique_resource {
                    for value_size in &override_constant.malformed_value_sizes {
                        *coverage
                            .malformed_override_value_size_resource_counts
                            .entry(*value_size)
                            .or_default() += 1;
                    }
                }
            }
        }
        if override_constant.is_some_and(|constant| constant.non_finite)
            || effective_values.is_some_and(|values| values.iter().any(|value| !value.is_finite()))
        {
            coverage
                .non_finite_count
                .observe(unique_resource, reference_count);
        }
        if let Some(effective_values) = effective_values {
            if unique_resource {
                *coverage
                    .value_width_resource_counts
                    .entry(effective_values.len())
                    .or_default() += 1;
            }
            let value_key = MaterialConstantValueKey(
                effective_values
                    .iter()
                    .map(|value| value.to_bits())
                    .collect(),
            );
            coverage
                .observed_values
                .entry(value_key)
                .or_insert_with(|| MaterialConstantValueAccumulator {
                    values: effective_values.to_vec(),
                    count: SemanticCount::default(),
                    representatives: Vec::new(),
                })
                .observe(unique_resource, representative);
        } else {
            coverage
                .unresolved_value_count
                .observe(unique_resource, reference_count);
        }
        coverage
            .shader_flag_counts
            .entry(shader_flags)
            .or_default()
            .observe(unique_resource, reference_count);
        add_semantic_representative(&mut coverage.representatives, representative);
    }

    fn finish(self) -> MaterialSemanticCoverageResult {
        let sampler_coverage = self
            .sampler_coverage
            .into_iter()
            .map(|(id, coverage)| WeaponMaterialSamplerCoverage {
                shader_package_name: id.shader_package_name,
                texture_usage: id.texture_usage,
                texture_usage_hex: hex_u32(id.texture_usage),
                texture_usage_name: coverage.texture_usage_name,
                logical_role: coverage.logical_role,
                texture_kind: coverage.texture_kind,
                flags: id.flags,
                flags_hex: hex_u32(id.flags),
                material_resource_count: coverage.count.resources,
                material_reference_count: coverage.count.references,
                representatives: coverage.representatives,
            })
            .collect::<Vec<_>>();
        let material_key_coverage = self
            .key_coverage
            .into_iter()
            .map(|(id, coverage)| WeaponMaterialKeyCoverage {
                shader_package_name: id.shader_package_name,
                scope: id.scope,
                category: id.category,
                category_hex: hex_u32(id.category),
                category_name: coverage.category_name,
                default_value: coverage.default_value,
                default_value_hex: coverage.default_value.map(hex_u32),
                default_value_name: coverage.default_value_name,
                material_resource_count: coverage.count.resources,
                material_reference_count: coverage.count.references,
                material_override_resource_count: coverage.override_count.resources,
                material_override_reference_count: coverage.override_count.references,
                non_default_override_resource_count: coverage.non_default_override_count.resources,
                non_default_override_reference_count: coverage
                    .non_default_override_count
                    .references,
                observed_values: coverage
                    .observed_values
                    .into_iter()
                    .map(|(value, count)| WeaponMaterialKeyValueCount {
                        value,
                        value_hex: hex_u32(value),
                        value_name: count.value_name,
                        material_resource_count: count.count.resources,
                        material_reference_count: count.count.references,
                        representatives: count.representatives,
                    })
                    .collect(),
                representatives: coverage.representatives,
            })
            .collect::<Vec<_>>();
        let material_constant_coverage = self
            .constant_coverage
            .into_iter()
            .map(|(id, coverage)| WeaponMaterialConstantCoverage {
                shader_package_name: id.shader_package_name,
                id: id.id,
                id_hex: hex_u32(id.id),
                name: coverage.name,
                package_byte_offset: coverage.package_byte_offset,
                package_byte_size: coverage.package_byte_size,
                default_values: coverage.default_values.as_deref().map(json_f32_values),
                default_raw_values_hex: coverage.default_values.as_deref().map(f32_raw_values_hex),
                material_resource_count: coverage.count.resources,
                material_reference_count: coverage.count.references,
                material_override_resource_count: coverage.override_count.resources,
                material_override_reference_count: coverage.override_count.references,
                non_default_override_resource_count: coverage.non_default_override_count.resources,
                non_default_override_reference_count: coverage
                    .non_default_override_count
                    .references,
                malformed_override_resource_count: coverage.malformed_override_count.resources,
                malformed_override_reference_count: coverage.malformed_override_count.references,
                non_finite_resource_count: coverage.non_finite_count.resources,
                non_finite_reference_count: coverage.non_finite_count.references,
                unresolved_value_resource_count: coverage.unresolved_value_count.resources,
                unresolved_value_reference_count: coverage.unresolved_value_count.references,
                value_width_resource_counts: coverage.value_width_resource_counts,
                malformed_override_value_size_resource_counts: coverage
                    .malformed_override_value_size_resource_counts,
                observed_values: coverage
                    .observed_values
                    .into_values()
                    .map(|count| WeaponMaterialConstantValueCount {
                        values: json_f32_values(&count.values),
                        raw_values_hex: f32_raw_values_hex(&count.values),
                        material_resource_count: count.count.resources,
                        material_reference_count: count.count.references,
                        representatives: count.representatives,
                    })
                    .collect(),
                shader_flag_counts: coverage
                    .shader_flag_counts
                    .into_iter()
                    .map(|(shader_flags, count)| WeaponShaderFlagCount {
                        shader_flags,
                        shader_flags_hex: hex_u32(shader_flags),
                        material_resource_count: count.resources,
                        material_reference_count: count.references,
                    })
                    .collect(),
                representatives: coverage.representatives,
            })
            .collect::<Vec<_>>();
        let unknown_key_category_count = material_key_coverage
            .iter()
            .filter(|coverage| coverage.category_name.is_none())
            .count();
        let unknown_key_value_count = material_key_coverage
            .iter()
            .flat_map(|coverage| &coverage.observed_values)
            .filter(|value| value.value_name.is_none())
            .count();
        let unknown_constant_id_count = material_constant_coverage
            .iter()
            .filter(|coverage| coverage.name.is_none())
            .count();
        let unknown_sampler_role_count = sampler_coverage
            .iter()
            .filter(|coverage| coverage.logical_role.is_none())
            .count();
        let unresolved_sampler_name_count = sampler_coverage
            .iter()
            .filter(|coverage| coverage.texture_usage_name.is_none())
            .count();

        MaterialSemanticCoverageResult {
            unique_material_resources: self.material_resources.len(),
            unique_shader_packages: self.shader_packages.len(),
            sampler_coverage,
            material_key_coverage,
            material_constant_coverage,
            unknown_key_category_count,
            unknown_key_value_count,
            unknown_constant_id_count,
            unknown_sampler_role_count,
            unresolved_sampler_name_count,
            color_table_diffuse: self.color_table_diffuse,
            color_table_specular: self.color_table_specular,
            color_table_emissive: self.color_table_emissive,
            color_table_metalness: self.color_table_metalness,
            color_table_roughness: self.color_table_roughness,
            color_table_gloss_strength: self.color_table_gloss_strength,
            color_table_specular_strength: self.color_table_specular_strength,
            color_table_anisotropy: self.color_table_anisotropy,
            color_table_sheen_rate: self.color_table_sheen_rate,
            color_table_sheen_aptitude: self.color_table_sheen_aptitude,
            color_table_sphere_mask: self.color_table_sphere_mask,
            failures: self.failures,
        }
    }
}

fn add_semantic_representative(
    representatives: &mut Vec<WeaponSemanticRepresentative>,
    representative: &WeaponSemanticRepresentative,
) {
    if representatives.len() >= MAX_SEMANTIC_REPRESENTATIVES
        || representatives
            .iter()
            .any(|existing| existing.material_path == representative.material_path)
    {
        return;
    }
    representatives.push(representative.clone());
}

fn same_f32_bits(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.to_bits() == right.to_bits())
}

fn json_f32_values(values: &[f32]) -> Vec<Option<f32>> {
    values
        .iter()
        .map(|value| value.is_finite().then_some(*value))
        .collect()
}

fn f32_raw_values_hex(values: &[f32]) -> Vec<String> {
    values
        .iter()
        .map(|value| hex_u32(value.to_bits()))
        .collect()
}

fn hex_u32(value: u32) -> String {
    format!("0x{value:08x}")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShapeModel {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    shape_count: usize,
    shape_mesh_count: usize,
    shape_value_count: usize,
    shape_names: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponShaderFamilyCandidate {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    material_path: String,
    shader_package_name: String,
    shader_family: MaterialShaderFamily,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponMaterialResourceCollision {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    candidate_path: String,
    resource_type: String,
    byte_length: usize,
    header: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WeaponUnresolvedMaterialReference {
    item_ids: Vec<u32>,
    item_names: Vec<String>,
    model: PackedModelId,
    model_path: String,
    material_name: String,
    candidate_paths: Vec<String>,
}

#[test]
#[ignore = "scans the installed FFXIV WeaponCatalog and writes target/weapon-shader-family-audit.json"]
fn audit_installed_weapon_shader_families() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let catalog = export_weapon_catalog_from_resource(
        SqPackResource::from_existing(game_dir_text),
        game_dir.display().to_string(),
        game_version(&game_dir),
        "weapon-shader-family-audit".to_string(),
    )
    .context("failed to export weapon catalog")?;
    let catalog_items = catalog.items.len();
    let item_ids = item_ids();
    let selected_items = catalog
        .items
        .iter()
        .filter(|item| item_ids.as_ref().is_none_or(|ids| ids.contains(&item.id)))
        .cloned()
        .collect::<Vec<_>>();
    if item_ids.is_some() {
        for item in &selected_items {
            eprintln!(
                "selected item {} {}: main={:016X} {:?}, sub={:016X} {:?}",
                item.id,
                item.name,
                item.model_main,
                item.primary_model(),
                item.model_sub,
                item.secondary_model()
            );
        }
    }
    let models = catalog_models(&selected_items);
    let unique_models = models.len();
    let scan_limit = scan_limit().unwrap_or(unique_models);
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let mut semantic_coverage = MaterialSemanticCoverageBuilder::default();
    let mut report = WeaponShaderFamilyAudit {
        game_dir: game_dir.display().to_string(),
        catalog_items,
        unique_models,
        scanned_models: 0,
        scanned_materials: 0,
        unique_material_resources: 0,
        unique_shader_packages: 0,
        family_counts: BTreeMap::new(),
        lod0_mesh_range_model_counts: BTreeMap::new(),
        lod0_mesh_range_mesh_counts: BTreeMap::new(),
        sampler_coverage: Vec::new(),
        material_key_coverage: Vec::new(),
        material_constant_coverage: Vec::new(),
        unknown_key_category_count: 0,
        unknown_key_value_count: 0,
        unknown_constant_id_count: 0,
        unknown_sampler_role_count: 0,
        unresolved_sampler_name_count: 0,
        color_table_diffuse: WeaponColorTableScalarCoverage::default(),
        color_table_specular: WeaponColorTableScalarCoverage::default(),
        color_table_emissive: WeaponColorTableScalarCoverage::default(),
        color_table_metalness: WeaponColorTableScalarCoverage::default(),
        color_table_roughness: WeaponColorTableScalarCoverage::default(),
        color_table_gloss_strength: WeaponColorTableScalarCoverage::default(),
        color_table_specular_strength: WeaponColorTableScalarCoverage::default(),
        color_table_anisotropy: WeaponColorTableScalarCoverage::default(),
        color_table_sheen_rate: WeaponColorTableScalarCoverage::default(),
        color_table_sheen_aptitude: WeaponColorTableScalarCoverage::default(),
        color_table_sphere_mask: WeaponColorTableScalarCoverage::default(),
        unknown_constant_dxbc: Vec::new(),
        alpha_shaping_dxbc: Vec::new(),
        vertex_texcoord4_dxbc: Vec::new(),
        tile_mip_dxbc: Vec::new(),
        tile_blend_dxbc: Vec::new(),
        texture_mip_dxbc: Vec::new(),
        material_strength_dxbc: Vec::new(),
        candidates: Vec::new(),
        unclassified_materials: Vec::new(),
        resource_collisions: Vec::new(),
        unresolved_material_references: Vec::new(),
        shape_models: Vec::new(),
        semantic_failures: Vec::new(),
        failures: Vec::new(),
    };

    for (index, (model, items)) in models.into_iter().take(scan_limit).enumerate() {
        scan_model(
            &mut resource,
            model,
            &items,
            &mut report,
            &mut semantic_coverage,
        );
        report.scanned_models += 1;
        if (index + 1) % 250 == 0 {
            eprintln!(
                "scanned {}/{} unique weapon models, {} material references, {} unique material resources, {} bg candidates",
                index + 1,
                scan_limit.min(unique_models),
                report.scanned_materials,
                semantic_coverage.material_resources.len(),
                report.candidates.len()
            );
        }
    }

    let semantic_coverage = semantic_coverage.finish();
    report.unique_material_resources = semantic_coverage.unique_material_resources;
    report.unique_shader_packages = semantic_coverage.unique_shader_packages;
    report.sampler_coverage = semantic_coverage.sampler_coverage;
    report.material_key_coverage = semantic_coverage.material_key_coverage;
    report.material_constant_coverage = semantic_coverage.material_constant_coverage;
    report.unknown_key_category_count = semantic_coverage.unknown_key_category_count;
    report.unknown_key_value_count = semantic_coverage.unknown_key_value_count;
    report.unknown_constant_id_count = semantic_coverage.unknown_constant_id_count;
    report.unknown_sampler_role_count = semantic_coverage.unknown_sampler_role_count;
    report.unresolved_sampler_name_count = semantic_coverage.unresolved_sampler_name_count;
    report.color_table_diffuse = semantic_coverage.color_table_diffuse;
    report.color_table_specular = semantic_coverage.color_table_specular;
    report.color_table_emissive = semantic_coverage.color_table_emissive;
    report.color_table_metalness = semantic_coverage.color_table_metalness;
    report.color_table_roughness = semantic_coverage.color_table_roughness;
    report.color_table_gloss_strength = semantic_coverage.color_table_gloss_strength;
    report.color_table_specular_strength = semantic_coverage.color_table_specular_strength;
    report.color_table_anisotropy = semantic_coverage.color_table_anisotropy;
    report.color_table_sheen_rate = semantic_coverage.color_table_sheen_rate;
    report.color_table_sheen_aptitude = semantic_coverage.color_table_sheen_aptitude;
    report.color_table_sphere_mask = semantic_coverage.color_table_sphere_mask;
    report.semantic_failures = semantic_coverage.failures;

    #[cfg(windows)]
    {
        report.unknown_constant_dxbc = audit_installed_unknown_constant_dxbc(&mut resource)?;
        report.alpha_shaping_dxbc = audit_installed_alpha_shaping_dxbc(&mut resource)?;
        report.vertex_texcoord4_dxbc = audit_vertex_texcoord4_dxbc(&mut resource)?;
        report.tile_mip_dxbc = audit_installed_tile_mip_dxbc(&mut resource)?;
        report.tile_blend_dxbc = audit_installed_tile_blend_dxbc(&mut resource)?;
        report.texture_mip_dxbc = audit_installed_texture_mip_dxbc(&mut resource)?;
        report.material_strength_dxbc = audit_installed_material_strength_dxbc(&mut resource)?;
    }

    let output_path = PathBuf::from("target").join("weapon-shader-family-audit.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    eprintln!(
        "weapon shader audit: models={}, material references={}, unique materials={}, shader packages={}, sampler coverage={}, key coverage={}, constant coverage={}, candidates={}, failures={}, semantic failures={}, report={}",
        report.scanned_models,
        report.scanned_materials,
        report.unique_material_resources,
        report.unique_shader_packages,
        report.sampler_coverage.len(),
        report.material_key_coverage.len(),
        report.material_constant_coverage.len(),
        report.candidates.len(),
        report.failures.len(),
        report.semantic_failures.len(),
        output_path.display()
    );

    assert!(report.scanned_models > 0);
    assert!(report.scanned_materials > 0);
    assert!(report.unique_material_resources > 0);
    assert!(report.unique_shader_packages > 0);
    assert!(!report.sampler_coverage.is_empty());
    assert!(!report.material_key_coverage.is_empty());
    assert!(!report.material_constant_coverage.is_empty());
    assert!(
        report.failures.is_empty(),
        "weapon audit failure: {:?}",
        report.failures.first()
    );
    assert!(
        report.semantic_failures.is_empty(),
        "material semantic audit failure: {:?}",
        report.semantic_failures.first()
    );
    if item_ids.is_none() && scan_limit >= unique_models {
        assert_installed_special_character_boundary(&report);
        assert_installed_remaining_family_boundary(&report);
        assert_installed_shader_color_ranges(&report);
        assert_installed_color_table_hdr_ranges(&report);
        assert_installed_emissive_boundary(&report);
        assert_installed_anisotropy_boundary(&report);
        assert_installed_unverified_extra_lighting_boundary(&report);
        assert_installed_toon_default_boundary(&report);
        assert_installed_texture_mip_bias_boundary(&report);
        #[cfg(windows)]
        {
            assert_installed_tile_mip_dxbc_boundary(&report);
            assert_installed_tile_blend_dxbc_boundary(&report);
            assert_installed_texture_mip_dxbc_boundary(&report);
            assert_installed_material_strength_dxbc_boundary(&report.material_strength_dxbc);
        }
        assert_installed_character_glass_shader_boundary(&mut resource);
        assert_installed_cutout_boundary(&report);
        assert_installed_shadow_mesh_boundary(&report);
        assert_installed_water_environment_boundary(&report);
    }
    Ok(())
}

fn assert_installed_shader_color_ranges(report: &WeaponShaderFamilyAudit) {
    for (package, resources, references, diffuse) in [
        ("character.shpk", 874, 1359, vec![1.0, 1.0, 1.0]),
        ("characterglass.shpk", 5, 12, vec![1.0, 1.0, 1.0]),
        ("characterlegacy.shpk", 5519, 12145, vec![1.0, 1.0, 1.0]),
        ("skin.shpk", 1, 35, vec![1.4, 1.4, 1.4]),
    ] {
        let diffuse_coverage = report
            .material_constant_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == package
                    && coverage.name.as_deref() == Some("g_DiffuseColor")
            })
            .unwrap_or_else(|| panic!("{package} g_DiffuseColor coverage"));
        assert_eq!(diffuse_coverage.material_resource_count, resources);
        assert_eq!(diffuse_coverage.material_reference_count, references);
        assert_eq!(diffuse_coverage.observed_values.len(), 1);
        assert_eq!(
            diffuse_coverage.observed_values[0].values,
            diffuse.into_iter().map(Some).collect::<Vec<_>>(),
            "installed g_DiffuseColor values changed; re-audit linear HDR tint handling"
        );

        let emissive_coverage = report
            .material_constant_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == package
                    && coverage.name.as_deref() == Some("g_EmissiveColor")
            })
            .unwrap_or_else(|| panic!("{package} g_EmissiveColor coverage"));
        assert_eq!(emissive_coverage.material_resource_count, resources);
        assert_eq!(emissive_coverage.material_reference_count, references);
        assert_eq!(emissive_coverage.observed_values.len(), 1);
        assert_eq!(
            emissive_coverage.observed_values[0].values,
            vec![Some(0.0), Some(0.0), Some(0.0)],
            "installed g_EmissiveColor values changed; re-audit linear HDR emission handling"
        );
    }
}

fn assert_installed_color_table_hdr_ranges(report: &WeaponShaderFamilyAudit) {
    let diffuse = &report.color_table_diffuse;
    assert_eq!(diffuse.material_resource_count, 6397);
    assert_eq!(diffuse.material_reference_count, 13515);
    assert_eq!(diffuse.nonzero_material_resource_count, 6397);
    assert_eq!(diffuse.nonzero_material_reference_count, 13515);
    assert_eq!(diffuse.minimum, Some(0.0));
    assert_eq!(diffuse.maximum, Some(6.7929688));

    let specular = &report.color_table_specular;
    assert_eq!(specular.material_resource_count, 6397);
    assert_eq!(specular.material_reference_count, 13515);
    assert_eq!(specular.nonzero_material_resource_count, 6397);
    assert_eq!(specular.nonzero_material_reference_count, 13515);
    assert_eq!(specular.minimum, Some(0.0));
    assert_eq!(specular.maximum, Some(4900.0));

    let metalness = &report.color_table_metalness;
    assert_eq!(metalness.material_resource_count, 6394);
    assert_eq!(metalness.material_reference_count, 13510);
    assert_eq!(metalness.nonzero_material_resource_count, 834);
    assert_eq!(metalness.nonzero_material_reference_count, 1311);
    assert_eq!(metalness.minimum, Some(0.0));
    assert_eq!(metalness.maximum, Some(1.0));

    let roughness = &report.color_table_roughness;
    assert_eq!(roughness.material_resource_count, 6394);
    assert_eq!(roughness.material_reference_count, 13510);
    assert_eq!(roughness.nonzero_material_resource_count, 883);
    assert_eq!(roughness.nonzero_material_reference_count, 1378);
    assert_eq!(roughness.minimum, Some(0.0));
    assert_eq!(roughness.maximum, Some(1.0));

    let gloss_strength = &report.color_table_gloss_strength;
    assert_eq!(gloss_strength.material_resource_count, 6397);
    assert_eq!(gloss_strength.material_reference_count, 13515);
    assert_eq!(gloss_strength.nonzero_material_resource_count, 6397);
    assert_eq!(gloss_strength.nonzero_material_reference_count, 13515);
    assert_eq!(gloss_strength.minimum, Some(0.7998047));
    assert_eq!(gloss_strength.maximum, Some(193.375));

    let specular_strength = &report.color_table_specular_strength;
    assert_eq!(specular_strength.material_resource_count, 6397);
    assert_eq!(specular_strength.material_reference_count, 13515);
    assert_eq!(specular_strength.nonzero_material_resource_count, 5514);
    assert_eq!(specular_strength.nonzero_material_reference_count, 12137);
    assert_eq!(specular_strength.minimum, Some(0.0));
    assert_eq!(specular_strength.maximum, Some(100.0));

    let sheen_aptitude = &report.color_table_sheen_aptitude;
    assert_eq!(sheen_aptitude.material_resource_count, 6394);
    assert_eq!(sheen_aptitude.material_reference_count, 13510);
    assert_eq!(sheen_aptitude.nonzero_material_resource_count, 883);
    assert_eq!(sheen_aptitude.nonzero_material_reference_count, 1378);
    assert_eq!(sheen_aptitude.minimum, Some(0.0));
    assert_eq!(sheen_aptitude.maximum, Some(52.09375));
}

fn assert_installed_emissive_boundary(report: &WeaponShaderFamilyAudit) {
    let emissive = &report.color_table_emissive;
    assert_eq!(emissive.material_resource_count, 6397);
    assert_eq!(emissive.material_reference_count, 13515);
    assert_eq!(emissive.nonzero_material_resource_count, 2811);
    assert_eq!(emissive.nonzero_material_reference_count, 5482);
    assert_eq!(emissive.minimum, Some(0.0));
    assert_eq!(emissive.maximum, Some(61.46875));
}

fn assert_installed_anisotropy_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(
        report.color_table_anisotropy.material_resource_count, 6394,
        "installed ColorTable coverage changed; re-audit anisotropy semantics"
    );
    assert_eq!(
        report
            .color_table_anisotropy
            .nonzero_material_resource_count,
        113,
        "installed nonzero anisotropy resources changed; re-audit the directional lobe"
    );
    assert_eq!(
        report
            .color_table_anisotropy
            .nonzero_material_reference_count,
        170
    );
    assert_eq!(report.color_table_anisotropy.minimum, Some(0.0));
    assert_eq!(report.color_table_anisotropy.maximum, Some(7.0));
}

fn assert_installed_unverified_extra_lighting_boundary(report: &WeaponShaderFamilyAudit) {
    let sheen = &report.color_table_sheen_rate;
    assert_eq!(sheen.material_resource_count, 6394);
    assert_eq!(sheen.material_reference_count, 13510);
    assert_eq!(sheen.nonzero_material_resource_count, 857);
    assert_eq!(sheen.nonzero_material_reference_count, 1335);
    assert_eq!(sheen.minimum, Some(0.0));
    assert_eq!(sheen.maximum, Some(1.0));

    let sphere = &report.color_table_sphere_mask;
    assert_eq!(sphere.material_resource_count, 6394);
    assert_eq!(sphere.material_reference_count, 13510);
    assert_eq!(sphere.nonzero_material_resource_count, 121);
    assert_eq!(sphere.nonzero_material_reference_count, 183);
    assert_eq!(sphere.minimum, Some(0.0));
    assert_eq!(sphere.maximum, Some(1.0));
}

fn assert_installed_toon_default_boundary(report: &WeaponShaderFamilyAudit) {
    let packages = [
        ("character.shpk", 874, 1359),
        ("characterglass.shpk", 5, 12),
        ("characterlegacy.shpk", 5519, 12145),
        ("skin.shpk", 1, 35),
    ];
    let constants = [
        ("g_ToonIndex", 0.0),
        ("g_ToonLightScale", 2.0),
        ("g_ToonLightSpecAperture", 50.0),
        ("g_ToonReflectionScale", 2.5),
        ("g_ToonSpecIndex", 4.0e-45),
    ];
    for (package, resource_count, reference_count) in packages {
        for (name, default_value) in constants {
            let coverage = report
                .material_constant_coverage
                .iter()
                .find(|coverage| {
                    coverage.shader_package_name == package
                        && coverage.name.as_deref() == Some(name)
                })
                .unwrap_or_else(|| panic!("{package} {name} coverage"));
            assert_eq!(coverage.material_resource_count, resource_count);
            assert_eq!(coverage.material_reference_count, reference_count);
            assert_eq!(coverage.non_default_override_resource_count, 0);
            assert_eq!(coverage.non_default_override_reference_count, 0);
            assert_eq!(coverage.default_values, Some(vec![Some(default_value)]));
            assert_eq!(coverage.observed_values.len(), 1);
            assert_eq!(
                coverage.observed_values[0].values,
                vec![Some(default_value)]
            );
        }
    }
}

fn assert_installed_texture_mip_bias_boundary(report: &WeaponShaderFamilyAudit) {
    for (package, resources, references) in [
        ("character.shpk", 874, 1359),
        ("characterglass.shpk", 5, 12),
        ("characterlegacy.shpk", 5519, 12145),
        ("skin.shpk", 1, 35),
    ] {
        let coverage = report
            .material_constant_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == package
                    && coverage.name.as_deref() == Some("g_TextureMipBias")
            })
            .unwrap_or_else(|| panic!("{package} g_TextureMipBias coverage"));
        assert_eq!(coverage.package_byte_offset, Some(228));
        assert_eq!(coverage.package_byte_size, Some(4));
        assert_eq!(coverage.default_values, Some(vec![Some(0.0)]));
        assert_eq!(coverage.material_resource_count, resources);
        assert_eq!(coverage.material_reference_count, references);
        if package != "character.shpk" {
            assert_eq!(coverage.non_default_override_resource_count, 0);
            assert_eq!(coverage.non_default_override_reference_count, 0);
            assert_eq!(coverage.observed_values.len(), 1);
            assert_eq!(coverage.observed_values[0].values, vec![Some(0.0)]);
        }
    }

    let character = report
        .material_constant_coverage
        .iter()
        .find(|coverage| {
            coverage.shader_package_name == "character.shpk"
                && coverage.name.as_deref() == Some("g_TextureMipBias")
        })
        .expect("character.shpk g_TextureMipBias coverage");
    assert_eq!(character.non_default_override_resource_count, 6);
    assert_eq!(character.non_default_override_reference_count, 9);
    let observed = character
        .observed_values
        .iter()
        .map(|value| {
            (
                value.values[0].expect("finite texture mip bias").to_bits(),
                value.material_resource_count,
                value.material_reference_count,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed,
        BTreeSet::from([
            (0.0f32.to_bits(), 868, 1350),
            (1.0f32.to_bits(), 3, 6),
            ((-1.0f32).to_bits(), 3, 3),
        ]),
        "installed g_TextureMipBias values changed; re-audit the verified sampler scope"
    );
}

fn assert_installed_remaining_family_boundary(report: &WeaponShaderFamilyAudit) {
    assert!(
        report.candidates.is_empty(),
        "installed weapons gained a BG/BGUvScroll material; re-audit detail and vertex-color composition"
    );
    assert!(
        report.material_key_coverage.iter().all(|coverage| {
            coverage.category_name.as_deref() != Some("ApplyVertexColor")
                && coverage.category != 0x4F4F_0636
        }),
        "installed weapons gained ApplyVertexColor coverage; verify its RGB formula before consuming it in Final"
    );

    for (package, resources, references) in [
        ("character.shpk", 874, 1359),
        ("characterlegacy.shpk", 5519, 12145),
        ("characterglass.shpk", 5, 12),
        ("skin.shpk", 1, 35),
    ] {
        let coverage = report
            .material_constant_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == package
                    && coverage.name.as_deref() == Some("g_SpecularColorMask")
            })
            .unwrap_or_else(|| panic!("{package} g_SpecularColorMask coverage"));
        assert_eq!(coverage.material_resource_count, resources);
        assert_eq!(coverage.material_reference_count, references);
        assert_eq!(coverage.non_default_override_resource_count, 0);
        assert_eq!(coverage.non_default_override_reference_count, 0);
        assert_eq!(
            coverage.default_values,
            Some(vec![Some(1.0), Some(1.0), Some(1.0)])
        );
        assert_eq!(coverage.observed_values.len(), 1);
        assert_eq!(
            coverage.observed_values[0].values,
            vec![Some(1.0), Some(1.0), Some(1.0)]
        );
    }

    for (package, resources, references) in [
        ("character.shpk", 874, 1359),
        ("characterlegacy.shpk", 5519, 12145),
        ("characterglass.shpk", 5, 12),
        ("skin.shpk", 1, 35),
    ] {
        for (name, expected) in [
            ("g_OutlineColor", vec![Some(0.0), Some(0.0), Some(0.0)]),
            ("g_OutlineWidth", vec![Some(0.0)]),
        ] {
            let coverage = report
                .material_constant_coverage
                .iter()
                .find(|coverage| {
                    coverage.shader_package_name == package
                        && coverage.name.as_deref() == Some(name)
                })
                .unwrap_or_else(|| panic!("{package} {name} coverage"));
            assert_eq!(coverage.material_resource_count, resources);
            assert_eq!(coverage.material_reference_count, references);
            assert_eq!(coverage.non_default_override_resource_count, 0);
            assert_eq!(coverage.non_default_override_reference_count, 0);
            assert_eq!(coverage.default_values, Some(expected.clone()));
            assert_eq!(coverage.observed_values.len(), 1);
            assert_eq!(coverage.observed_values[0].values, expected);
        }
    }

    for unsupported_name in ["g_MultiDiffuseColor", "g_MultiEmissiveColor"] {
        assert!(
            report
                .material_constant_coverage
                .iter()
                .all(|coverage| coverage.name.as_deref() != Some(unsupported_name)),
            "installed weapons gained {unsupported_name} coverage; re-audit BG/Crystal multi-color composition"
        );
    }

    let get_values = report
        .material_key_coverage
        .iter()
        .filter(|coverage| coverage.category_name.as_deref() == Some("GetValues"))
        .collect::<Vec<_>>();
    assert!(!get_values.is_empty());
    let observed_get_values = get_values
        .iter()
        .flat_map(|coverage| coverage.observed_values.iter())
        .filter_map(|value| value.value_name.as_deref())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_get_values,
        BTreeSet::from(["GetValuesCompatibility", "GetValuesMultiMaterial"]),
        "installed weapons gained a new GetValues/AlphaMulti mode; re-audit its channel formula"
    );

    let vertex_movement = report
        .material_key_coverage
        .iter()
        .filter(|coverage| coverage.category_name.as_deref() == Some("ApplyVertexMovement"))
        .collect::<Vec<_>>();
    assert!(!vertex_movement.is_empty());
    assert!(vertex_movement.iter().all(|coverage| {
        coverage.non_default_override_resource_count == 0
            && coverage.observed_values.len() == 1
            && coverage.observed_values[0].value_name.as_deref() == Some("ApplyVertexMovementOff")
    }));

    let character_ao = report
        .material_constant_coverage
        .iter()
        .find(|coverage| {
            coverage.shader_package_name == "character.shpk"
                && coverage.name.as_deref() == Some("g_AmbientOcclusionMask")
        })
        .expect("character.shpk g_AmbientOcclusionMask coverage");
    assert_eq!(character_ao.material_resource_count, 198);
    assert_eq!(character_ao.material_reference_count, 251);
    assert_eq!(character_ao.observed_values.len(), 1);
    assert_eq!(character_ao.observed_values[0].values, vec![Some(0.25)]);

    let character_ssao = report
        .material_constant_coverage
        .iter()
        .find(|coverage| {
            coverage.shader_package_name == "character.shpk"
                && coverage.name.as_deref() == Some("g_SSAOMask")
        })
        .expect("character.shpk g_SSAOMask coverage");
    assert_eq!(character_ssao.non_default_override_resource_count, 4);
    assert_eq!(character_ssao.non_default_override_reference_count, 9);
    let nondefault_ssao = character_ssao
        .observed_values
        .iter()
        .filter_map(|value| value.values.first().copied().flatten())
        .filter(|value| *value != 1.0)
        .map(f32::to_bits)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        nondefault_ssao,
        BTreeSet::from([0x3f66_64ff, 0x3f7a_e100, 0x3f7d_7081]),
        "installed g_SSAOMask values changed; re-audit runtime SSAO composition"
    );

    let character_alpha_constant = |name: &str| {
        report
            .material_constant_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == "character.shpk"
                    && coverage.name.as_deref() == Some(name)
            })
            .unwrap_or_else(|| panic!("character.shpk {name} coverage"))
    };
    let alpha_aperture = character_alpha_constant("g_AlphaAperture");
    assert_eq!(alpha_aperture.non_default_override_resource_count, 7);
    assert_eq!(alpha_aperture.non_default_override_reference_count, 10);
    let aperture_values = alpha_aperture
        .observed_values
        .iter()
        .filter_map(|value| value.values.first().copied().flatten())
        .map(f32::to_bits)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        aperture_values,
        BTreeSet::from([
            0x3f80_0000,
            0x3ffc_20ff,
            0x3ffd_6b01,
            0x3ffe_b503,
            0x4000_0000,
            0x4000_a3d7,
            0x4000_a47f,
        ]),
        "installed g_AlphaAperture values changed; re-audit alpha shaping semantics"
    );
    let alpha_offset = character_alpha_constant("g_AlphaOffset");
    assert_eq!(alpha_offset.non_default_override_resource_count, 3);
    assert_eq!(alpha_offset.non_default_override_reference_count, 4);
    let offset_values = alpha_offset
        .observed_values
        .iter()
        .filter_map(|value| value.values.first().copied().flatten())
        .map(f32::to_bits)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        offset_values,
        BTreeSet::from([0x0000_0000, 0x360f_a0c5, 0x3c23_e004, 0x3d4c_d800]),
        "installed g_AlphaOffset values changed; re-audit alpha shaping semantics"
    );

    for (package, resources, references) in [
        ("character.shpk", 874, 1359),
        ("characterglass.shpk", 5, 12),
        ("characterlegacy.shpk", 5519, 12145),
        ("skin.shpk", 1, 35),
    ] {
        let coverage = report
            .material_constant_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == package
                    && coverage.name.as_deref() == Some("g_TileMipBiasOffset")
            })
            .unwrap_or_else(|| panic!("{package} g_TileMipBiasOffset coverage"));
        assert_eq!(coverage.package_byte_offset, Some(276));
        assert_eq!(coverage.package_byte_size, Some(4));
        assert_eq!(coverage.default_values, Some(vec![Some(0.0)]));
        assert_eq!(coverage.material_resource_count, resources);
        assert_eq!(coverage.material_reference_count, references);
        if package != "character.shpk" {
            assert_eq!(coverage.non_default_override_resource_count, 0);
            assert_eq!(coverage.non_default_override_reference_count, 0);
            assert_eq!(coverage.observed_values.len(), 1);
            assert_eq!(coverage.observed_values[0].values, vec![Some(0.0)]);
        }
    }

    let character_tile_bias = report
        .material_constant_coverage
        .iter()
        .find(|coverage| {
            coverage.shader_package_name == "character.shpk"
                && coverage.name.as_deref() == Some("g_TileMipBiasOffset")
        })
        .expect("character.shpk g_TileMipBiasOffset coverage");
    assert_eq!(character_tile_bias.non_default_override_resource_count, 3);
    assert_eq!(character_tile_bias.non_default_override_reference_count, 4);
    let nonzero_biases = character_tile_bias
        .observed_values
        .iter()
        .filter_map(|value| value.values.first().copied().flatten())
        .filter(|value| *value != 0.0)
        .map(f32::to_bits)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        nonzero_biases,
        BTreeSet::from([(-1.0f32).to_bits(), 1.0f32.to_bits()])
    );
}

fn assert_installed_special_character_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(
        report.family_counts,
        BTreeMap::from([
            ("Character".to_string(), 8091),
            ("CharacterGlass".to_string(), 6),
            ("Skin".to_string(), 15),
        ]),
        "installed special-family coverage changed; re-audit family semantics before extending the renderer"
    );

    let alpha_shaping = report
        .alpha_shaping_dxbc
        .iter()
        .map(|audit| {
            (
                audit.shader_package_name.as_str(),
                (
                    audit.pixel_shader_count,
                    audit.aperture_use_count,
                    audit.offset_use_count,
                    audit.formula_count,
                    audit.alpha_composition_count,
                    audit.unclassified_formula_count,
                    audit.shaping_dot_count,
                    audit.view_from_v6_count,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        alpha_shaping,
        BTreeMap::from([
            ("character.shpk", (1038, 768, 1539, 768, 768, 0, 768, 768)),
            ("characterlegacy.shpk", (1740, 0, 2, 0, 0, 0, 0, 0)),
            ("characterglass.shpk", (38, 34, 70, 34, 34, 0, 34, 34)),
            ("skin.shpk", (384, 0, 3, 0, 0, 0, 0, 0)),
        ]),
        "installed alpha aperture/offset consumer shapes changed; re-audit before changing Final alpha"
    );

    let skin_key = |category_name: &str| {
        report
            .material_key_coverage
            .iter()
            .find(|coverage| {
                coverage.shader_package_name == "skin.shpk"
                    && coverage.category_name.as_deref() == Some(category_name)
            })
            .unwrap_or_else(|| panic!("skin.shpk is missing {category_name} coverage"))
    };
    let material_value = skin_key("GetMaterialValue");
    assert_eq!(material_value.material_resource_count, 1);
    assert_eq!(material_value.material_reference_count, 35);
    assert!(material_value.representatives.iter().any(|representative| {
        representative.material_path
            == "chara/human/c0101/obj/body/b0001/material/v0001/mt_c0101b0001_a.mtrl"
    }));
    assert_eq!(material_value.observed_values.len(), 1);
    assert_eq!(
        material_value.observed_values[0].value_name.as_deref(),
        Some("GetMaterialValueBody")
    );

    let decal_color = skin_key("GetDecalColor");
    assert_eq!(decal_color.material_resource_count, 1);
    assert_eq!(decal_color.material_reference_count, 35);
    assert_eq!(decal_color.observed_values.len(), 1);
    assert_eq!(
        decal_color.observed_values[0].value_name.as_deref(),
        Some("GetDecalColorOff")
    );

    let skin_samplers = report
        .sampler_coverage
        .iter()
        .filter(|coverage| coverage.shader_package_name == "skin.shpk")
        .collect::<Vec<_>>();
    assert_eq!(skin_samplers.len(), 3);
    assert_eq!(
        skin_samplers
            .iter()
            .filter_map(|coverage| coverage.texture_usage_name.as_deref())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["g_SamplerDiffuse", "g_SamplerMask", "g_SamplerNormal"])
    );
    assert!(skin_samplers.iter().all(|coverage| {
        coverage.material_resource_count == 1 && coverage.material_reference_count == 35
    }));
}

#[cfg(windows)]
#[repr(C)]
struct D3dBlob {
    vtable: *const D3dBlobVtable,
}

#[cfg(windows)]
#[repr(C)]
struct D3dBlobVtable {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut D3dBlob) -> u32,
    get_buffer_pointer: unsafe extern "system" fn(*mut D3dBlob) -> *const c_void,
    get_buffer_size: unsafe extern "system" fn(*mut D3dBlob) -> usize,
}

#[cfg(windows)]
type D3dDisassembleFn = unsafe extern "system" fn(
    source_data: *const c_void,
    source_data_size: usize,
    flags: u32,
    comments: *const c_char,
    disassembly: *mut *mut D3dBlob,
) -> i32;

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryA(library_name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, procedure_name: *const c_char) -> *mut c_void;
}

#[cfg(windows)]
fn d3d_disassemble_function() -> Result<D3dDisassembleFn> {
    static FUNCTION: OnceLock<std::result::Result<usize, String>> = OnceLock::new();
    let address = FUNCTION.get_or_init(|| unsafe {
        let module = LoadLibraryA(c"D3DCompiler_47.dll".as_ptr());
        if module.is_null() {
            return Err("failed to load D3DCompiler_47.dll".to_string());
        }
        let function = GetProcAddress(module, c"D3DDisassemble".as_ptr());
        if function.is_null() {
            return Err("D3DCompiler_47.dll does not export D3DDisassemble".to_string());
        }
        Ok(function as usize)
    });
    match address {
        Ok(address) => Ok(unsafe { std::mem::transmute::<usize, D3dDisassembleFn>(*address) }),
        Err(error) => Err(anyhow!(error.clone())),
    }
}

#[cfg(windows)]
fn disassemble_dxbc(bytecode: &[u8]) -> Result<String> {
    let mut blob = ptr::null_mut();
    let disassemble = d3d_disassemble_function()?;
    let result = unsafe {
        disassemble(
            bytecode.as_ptr().cast(),
            bytecode.len(),
            0,
            ptr::null(),
            &mut blob,
        )
    };
    if result < 0 || blob.is_null() {
        return Err(anyhow!(
            "D3DDisassemble failed with HRESULT 0x{:08X}",
            result as u32
        ));
    }

    let text = unsafe {
        let vtable = &*(*blob).vtable;
        let pointer = (vtable.get_buffer_pointer)(blob).cast::<u8>();
        let size = (vtable.get_buffer_size)(blob);
        let bytes = std::slice::from_raw_parts(pointer, size);
        let text = String::from_utf8_lossy(bytes).into_owned();
        (vtable.release)(blob);
        text
    };
    Ok(text)
}

#[cfg(windows)]
fn locate_fxc() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("FXC_PATH").map(PathBuf::from) {
        if path.is_file() {
            return Ok(path);
        }
    }

    let mut sdk_roots = Vec::new();
    if let Some(path) = std::env::var_os("WindowsSdkDir").map(PathBuf::from) {
        sdk_roots.push(path);
    }
    if let Some(program_files_x86) = std::env::var_os("ProgramFiles(x86)") {
        sdk_roots.push(PathBuf::from(program_files_x86).join("Windows Kits\\10"));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        sdk_roots.push(PathBuf::from(program_files).join("Windows Kits\\10"));
    }

    let architecture = if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x64"
    };
    for root in sdk_roots {
        let bin_root = root.join("bin");
        let mut version_dirs = fs::read_dir(&bin_root)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(std::result::Result::ok))
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        version_dirs.sort_by_key(|path| std::cmp::Reverse(path.clone()));
        version_dirs.push(bin_root.clone());
        for version_dir in version_dirs {
            let candidate = version_dir.join(architecture).join("fxc.exe");
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    Err(anyhow!(
        "could not locate fxc.exe; set FXC_PATH to a Windows SDK fxc"
    ))
}

#[cfg(windows)]
fn disassemble_dxbc_with_fxc(bytecode: &[u8]) -> Result<String> {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let fxc = locate_fxc()?;
    let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let input = std::env::temp_dir().join(format!(
        "xiv-companion-dxbc-{}-{id}.dxbc",
        std::process::id()
    ));
    fs::write(&input, bytecode)
        .with_context(|| format!("failed to write temporary DXBC {}", input.display()))?;
    let output = Command::new(&fxc)
        .arg("/dumpbin")
        .arg(&input)
        .output()
        .with_context(|| format!("failed to execute {}", fxc.display()))?;
    let _ = fs::remove_file(&input);
    if !output.status.success() {
        return Err(anyhow!(
            "fxc /dumpbin failed for {} ({} bytes, header {:02X?}): {}",
            input.display(),
            bytecode.len(),
            &bytecode[..bytecode.len().min(16)],
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(windows)]
fn disassemble_dxbc_resilient(bytecode: &[u8]) -> Result<String> {
    let bytecode = declared_dxbc_container(bytecode);
    disassemble_dxbc(bytecode).or_else(|d3d_error| {
        disassemble_dxbc_with_fxc(bytecode).with_context(|| {
            format!("D3DDisassemble failed ({d3d_error:#}); fxc fallback also failed")
        })
    })
}

#[cfg(windows)]
fn declared_dxbc_container(bytecode: &[u8]) -> &[u8] {
    const DXBC_SIZE_OFFSET: usize = 24;
    if bytecode.len() < DXBC_SIZE_OFFSET + size_of::<u32>() || !bytecode.starts_with(b"DXBC") {
        return bytecode;
    }
    let declared_size = u32::from_le_bytes(
        bytecode[DXBC_SIZE_OFFSET..DXBC_SIZE_OFFSET + size_of::<u32>()]
            .try_into()
            .expect("DXBC size slice has fixed width"),
    ) as usize;
    if (DXBC_SIZE_OFFSET + size_of::<u32>()..=bytecode.len()).contains(&declared_size) {
        &bytecode[..declared_size]
    } else {
        bytecode
    }
}

#[cfg(windows)]
fn audit_vertex_texcoord4_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<VertexTexcoord4DxbcPackageAudit>> {
    [
        "character.shpk",
        "characterlegacy.shpk",
        "characterglass.shpk",
    ]
        .into_iter()
        .map(|shader_package_name| {
            let path = format!("shader/sm5/shpk/{shader_package_name}");
            let bytes = resource
                .read(&path)
                .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
            let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
                .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
            let mut texcoord4_output_count = 0;
            let mut texcoord4_registers = BTreeMap::new();
            let mut texcoord4_write_count = 0;
            let mut texcoord4_write_opcodes = BTreeMap::new();
            let mut representative_writes = Vec::new();
            let mut alpha_pixel_shaders = BTreeSet::new();
            let mut texcoord4_w_pixel_shader_count = 0;
            let mut texcoord4_w_output_reach_counts = BTreeMap::new();
            let mut texcoord4_w_output_representatives = BTreeMap::<String, Vec<usize>>::new();
            let mut legacy_gbuffer1_w_producer_pixel_shaders = BTreeSet::new();
            let mut legacy_gbuffer1_producer_o1_write_counts = BTreeMap::new();
            let mut legacy_gbuffer1_producer_o1_write_opcodes = BTreeMap::new();
            let mut legacy_gbuffer1_producer_o1_x_class_by_pixel_shader = BTreeMap::new();
            let mut legacy_gbuffer1_producer_table_o1_reach_counts = BTreeMap::new();
            let mut legacy_gbuffer1_producer_table_o1_representatives = Vec::new();

            for (shader_index, shader) in package.vertex_shaders.iter().enumerate() {
                let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
                    format!(
                        "failed to disassemble {shader_package_name} vertex shader {shader_index}"
                    )
                })?;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                let Some(register) = vertex_texcoord4_output_register(&lines) else {
                    continue;
                };
                texcoord4_output_count += 1;
                *texcoord4_registers.entry(register.clone()).or_default() += 1;
                let target = format!("o{register}");
                for line in lines.iter().filter(|line| {
                    line.split_whitespace().next() != Some("dcl_output")
                        && line
                            .split_whitespace()
                            .nth(1)
                            .is_some_and(|destination| destination.starts_with(&target))
                }) {
                    texcoord4_write_count += 1;
                    if let Some(opcode) = line.split_whitespace().next() {
                        *texcoord4_write_opcodes
                            .entry(opcode.to_string())
                            .or_default() += 1;
                    }
                    if representative_writes.len() < 24 {
                        representative_writes.push(format!("vs{shader_index}: {line}"));
                    }
                }
            }

            for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
                let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
                    format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
                })?;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                if let Some(register) = pixel_texcoord4_input_register(&lines) {
                    texcoord4_w_pixel_shader_count += 1;
                    let control = format!("v{register}.w");
                    let declaration_index = lines
                        .iter()
                        .position(|line| {
                            line.starts_with("dcl_input_ps")
                                && line.split_whitespace().last().is_some_and(|operand| {
                                    register_components_overlap(operand, &control)
                                })
                        })
                        .unwrap_or(0);
                    let reached_outputs =
                        dxbc_register_reached_outputs(&lines, declaration_index, &control);
                    if shader_package_name == "characterlegacy.shpk"
                        && reached_outputs.contains("o1.w")
                    {
                        legacy_gbuffer1_w_producer_pixel_shaders.insert(shader_index);
                        for output in ["o1.x", "o1.y", "o1.z", "o1.w"] {
                            let writes = lines
                                .iter()
                                .filter(|line| instruction_writes_register_component(line, output))
                                .collect::<Vec<_>>();
                            if !writes.is_empty() {
                                *legacy_gbuffer1_producer_o1_write_counts
                                    .entry(output.to_string())
                                    .or_default() += writes.len();
                                for opcode in writes
                                    .into_iter()
                                    .filter_map(|line| line.split_whitespace().next())
                                {
                                    *legacy_gbuffer1_producer_o1_write_opcodes
                                        .entry(format!("{output}:{opcode}"))
                                        .or_default() += 1;
                                }
                            }
                        }
                        let o1_x_opcodes = lines
                            .iter()
                            .filter(|line| instruction_writes_register_component(line, "o1.x"))
                            .filter_map(|line| line.split_whitespace().next())
                            .collect::<BTreeSet<_>>();
                        let o1_x_class = o1_x_opcodes.into_iter().collect::<Vec<_>>().join("+");
                        if o1_x_class.is_empty() {
                            return Err(anyhow!(
                                "{shader_package_name} GBuffer1 producer ps{shader_index} does not write o1.x"
                            ));
                        }
                        legacy_gbuffer1_producer_o1_x_class_by_pixel_shader
                            .insert(shader_index, o1_x_class);
                        for (table_sample_index, table_sample_line) in
                            lines.iter().enumerate().filter(|(_, line)| {
                                dxbc_sample_texture_name(line, &dxbc_texture_bindings(&lines))
                                    .as_deref()
                                    == Some("g_SamplerTable.T")
                            })
                        {
                            let Some(coordinates) = dxbc_sample_texture_coordinates(table_sample_line)
                            else {
                                continue;
                            };
                            let x = dxbc_literal_provenance_at(
                                &lines,
                                table_sample_index,
                                coordinates,
                                0,
                            )
                            .unwrap_or_else(|| "dynamic".to_string());
                            for lane in b"xyzw" {
                                let Some(destination) = dxbc_sample_texture_physical_lane_destination(
                                    table_sample_line,
                                    *lane,
                                ) else {
                                    continue;
                                };
                                for output in ["o1.x", "o1.y", "o1.z", "o1.w"] {
                                    if !dxbc_register_reaches_output(
                                        &lines,
                                        table_sample_index,
                                        &destination,
                                        output,
                                    ) {
                                        continue;
                                    }
                                    let signature = format!(
                                        "tableX={x},lane={},output={output}",
                                        *lane as char
                                    );
                                    *legacy_gbuffer1_producer_table_o1_reach_counts
                                        .entry(signature.clone())
                                        .or_default() += 1;
                                    if legacy_gbuffer1_producer_table_o1_representatives.len() < 24 {
                                        legacy_gbuffer1_producer_table_o1_representatives.push(
                                            format!(
                                                "ps{shader_index} line {table_sample_index}: {table_sample_line} -> {output}"
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    }
                    for output in reached_outputs {
                        *texcoord4_w_output_reach_counts
                            .entry(output.clone())
                            .or_default() += 1;
                        let representatives =
                            texcoord4_w_output_representatives.entry(output).or_default();
                        if representatives.len() < 8 {
                            representatives.push(shader_index);
                        }
                    }
                }
                if lines
                    .iter()
                    .enumerate()
                    .any(|(line_index, _)| alpha_shaping_shape_destination(&lines, line_index).is_some())
                {
                    alpha_pixel_shaders.insert(shader_index);
                }
            }

            let pass_pairs = shader_pass_pairs_from_debug(&package);
            let alpha_vertex_indices = pass_pairs
                .iter()
                .filter(|(_, pixel_shader)| alpha_pixel_shaders.contains(pixel_shader))
                .map(|(vertex_shader, _)| *vertex_shader)
                .collect::<BTreeSet<_>>();
            let legacy_gbuffer1_w_producer_pass_pairs = pass_pairs
                .iter()
                .filter(|(_, pixel_shader)| {
                    legacy_gbuffer1_w_producer_pixel_shaders.contains(pixel_shader)
                })
                .copied()
                .collect::<Vec<_>>();
            let mut legacy_gbuffer1_producer_o1_x_pixel_shader_counts = BTreeMap::new();
            let mut legacy_gbuffer1_producer_o1_x_representative_pixel_shaders =
                BTreeMap::<String, Vec<usize>>::new();
            for (pixel_shader, class) in &legacy_gbuffer1_producer_o1_x_class_by_pixel_shader {
                *legacy_gbuffer1_producer_o1_x_pixel_shader_counts
                    .entry(class.clone())
                    .or_default() += 1;
                let representatives = legacy_gbuffer1_producer_o1_x_representative_pixel_shaders
                    .entry(class.clone())
                    .or_default();
                if representatives.len() < 4 {
                    representatives.push(*pixel_shader);
                }
            }
            let mut legacy_gbuffer1_producer_o1_x_pass_pair_counts = BTreeMap::new();
            for (_, pixel_shader) in &legacy_gbuffer1_w_producer_pass_pairs {
                let class = legacy_gbuffer1_producer_o1_x_class_by_pixel_shader
                    .get(pixel_shader)
                    .expect("Legacy GBuffer1 producer pass must have an o1.x class");
                *legacy_gbuffer1_producer_o1_x_pass_pair_counts
                    .entry(class.clone())
                    .or_default() += 1;
            }
            let mut legacy_gbuffer1_producer_o1_x_node_counts = BTreeMap::new();
            let mut legacy_gbuffer1_producer_o1_x_pass_ids = BTreeMap::new();
            let mut legacy_gbuffer1_producer_o1_x_material_key_sets = BTreeMap::new();
            if shader_package_name == "characterlegacy.shpk" {
                for node in &package.nodes {
                    let producer_passes =
                        shader_pass_records_from_debug_text(&format!("{:#?}", node.passes))
                            .into_iter()
                            .filter_map(|(pass_id, _, pixel_shader)| {
                                legacy_gbuffer1_producer_o1_x_class_by_pixel_shader
                                    .get(&pixel_shader)
                                    .cloned()
                                    .map(|class| (pass_id, class))
                            })
                            .collect::<Vec<_>>();
                    let node_classes = producer_passes
                        .iter()
                        .map(|(_, class)| class.clone())
                        .collect::<BTreeSet<_>>();
                    if node_classes.is_empty() {
                        continue;
                    }
                    let material_key_set = package
                        .material_keys
                        .iter()
                        .zip(&node.material_keys)
                        .map(|(key, value)| format!("{:08X}={value:08X}", key.id))
                        .collect::<Vec<_>>()
                        .join(",");
                    for class in node_classes {
                        *legacy_gbuffer1_producer_o1_x_node_counts
                            .entry(class.clone())
                            .or_default() += 1;
                        for (pass_id, _) in producer_passes
                            .iter()
                            .filter(|(_, pass_class)| pass_class == &class)
                        {
                            *legacy_gbuffer1_producer_o1_x_pass_ids
                                .entry(class.clone())
                                .or_insert_with(BTreeMap::new)
                                .entry(format!("0x{pass_id:08x}"))
                                .or_default() += 1;
                        }
                        *legacy_gbuffer1_producer_o1_x_material_key_sets
                            .entry(class)
                            .or_insert_with(BTreeMap::new)
                            .entry(material_key_set.clone())
                            .or_default() += 1;
                    }
                }
            }
            let legacy_gbuffer1_w_producer_vertex_indices =
                legacy_gbuffer1_w_producer_pass_pairs
                    .iter()
                    .map(|(vertex_shader, _)| *vertex_shader)
                    .collect::<BTreeSet<_>>();
            let mut legacy_gbuffer1_w_producer_vertex_height_clamp_count = 0;
            let mut legacy_gbuffer1_w_producer_vertex_height_clamp_shaders = Vec::new();
            let mut legacy_gbuffer1_w_producer_vertex_other_writes = Vec::new();
            let mut legacy_gbuffer1_w_producer_vertex_wetness_reflection_count = 0;
            let mut legacy_gbuffer1_w_producer_vertex_wetness_reflection_unclassified_shaders =
                Vec::new();
            for shader_index in legacy_gbuffer1_w_producer_vertex_indices.iter().copied() {
                let shader = package.vertex_shaders.get(shader_index).ok_or_else(|| {
                    anyhow!("{shader_package_name} pass references missing VS {shader_index}")
                })?;
                let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
                    format!(
                        "failed to disassemble {shader_package_name} GBuffer1.w-producer vertex shader {shader_index}"
                    )
                })?;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                if dxbc_has_legacy_wetness_parameter_reflection(&lines) {
                    legacy_gbuffer1_w_producer_vertex_wetness_reflection_count += 1;
                } else {
                    legacy_gbuffer1_w_producer_vertex_wetness_reflection_unclassified_shaders
                        .push(shader_index);
                }
                let Some(register) = vertex_texcoord4_output_register(&lines) else {
                    continue;
                };
                let instance_slot = shader
                    .scalar_parameters
                    .iter()
                    .find(|parameter| parameter.name == "g_InstanceParameter")
                    .map(|parameter| parameter.slot)
                    .with_context(|| {
                        format!("Legacy GBuffer1.w producer VS {shader_index} lacks g_InstanceParameter")
                    })?;
                let model_slot = shader
                    .scalar_parameters
                    .iter()
                    .find(|parameter| parameter.name == "g_ModelParameter")
                    .map(|parameter| parameter.slot)
                    .with_context(|| {
                        format!("Legacy GBuffer1.w producer VS {shader_index} lacks g_ModelParameter")
                    })?;
                let target_w = format!("o{register}.w");
                let writes = lines
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| instruction_writes_register_component(line, &target_w))
                    .collect::<Vec<_>>();
                if writes.iter().any(|(line_index, _)| {
                    vertex_texcoord4_w_matches_height_clamp_slots(
                        &lines,
                        *line_index,
                        &target_w,
                        instance_slot,
                        model_slot,
                    )
                }) {
                    legacy_gbuffer1_w_producer_vertex_height_clamp_count += 1;
                    legacy_gbuffer1_w_producer_vertex_height_clamp_shaders.push(shader_index);
                } else {
                    legacy_gbuffer1_w_producer_vertex_other_writes.extend(
                        writes
                            .into_iter()
                            .map(|(_, line)| format!("vs{shader_index}: {line}")),
                    );
                }
            }
            let mut alpha_vertex_texcoord4_registers = BTreeMap::new();
            let mut alpha_vertex_projection_link_count = 0;
            let mut alpha_vertex_representative_writes = Vec::new();
            let mut alpha_vertex_representative_traces = Vec::new();
            let mut traced_sources = BTreeSet::new();
            for shader_index in alpha_vertex_indices.iter().copied() {
                let shader = package
                    .vertex_shaders
                    .get(shader_index)
                    .ok_or_else(|| anyhow!("{shader_package_name} pass references missing VS {shader_index}"))?;
                let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
                    format!("failed to disassemble {shader_package_name} paired vertex shader {shader_index}")
                })?;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                let Some(register) = vertex_texcoord4_output_register(&lines) else {
                    continue;
                };
                *alpha_vertex_texcoord4_registers
                    .entry(register.clone())
                    .or_default() += 1;
                let target = format!("o{register}");
                if vertex_texcoord4_feeds_clip_position(&lines, &target) {
                    alpha_vertex_projection_link_count += 1;
                }
                for (line_index, line) in lines.iter().enumerate().filter(|(_, line)| {
                    line.split_whitespace()
                        .nth(1)
                        .is_some_and(|destination| destination.starts_with(&target))
                }) {
                    if alpha_vertex_representative_writes.len() < 32 {
                        alpha_vertex_representative_writes.push(format!("vs{shader_index}: {line}"));
                    }
                    let Some((_, source)) = line.split_once(',') else {
                        continue;
                    };
                    let source = source.trim().to_string();
                    if line.contains(&format!("{target}.xyz")) && traced_sources.insert(source) {
                        let start = line_index.saturating_sub(20);
                        alpha_vertex_representative_traces.push(format!(
                            "vs{shader_index} TEXCOORD4 xyz trace:\n{}",
                            lines[start..=line_index].join("\n")
                        ));
                    }
                }
            }

            let gloss_o0_pixel_shaders = if shader_package_name == "characterlegacy.shpk" {
                legacy_gloss_o0_rgb_pixel_shaders(&package, shader_package_name)?
            } else {
                BTreeSet::new()
            };
            let gloss_o0_pass_pairs = shader_pass_pairs_from_debug(&package)
                .into_iter()
                .filter(|(_, pixel_shader)| gloss_o0_pixel_shaders.contains(pixel_shader))
                .collect::<Vec<_>>();
            let gloss_o0_vertex_indices = gloss_o0_pass_pairs
                .iter()
                .map(|(vertex_shader, _)| *vertex_shader)
                .collect::<BTreeSet<_>>();
            let mut gloss_o0_vertex_texcoord4_registers = BTreeMap::new();
            let mut gloss_o0_vertex_projection_link_count = 0;
            let mut gloss_o0_vertex_w_write_count = 0;
            let mut gloss_o0_vertex_w_height_clamp_count = 0;
            let mut gloss_o0_vertex_w_write_opcodes = BTreeMap::new();
            let mut gloss_o0_vertex_w_root_source_sets = BTreeMap::new();
            let mut gloss_o0_vertex_scalar_parameters = BTreeMap::new();
            let mut gloss_o0_vertex_representative_writes = Vec::new();
            let mut gloss_o0_vertex_representative_traces = Vec::new();
            for shader_index in gloss_o0_vertex_indices.iter().copied() {
                let shader = package.vertex_shaders.get(shader_index).ok_or_else(|| {
                    anyhow!("{shader_package_name} pass references missing VS {shader_index}")
                })?;
                let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
                    format!(
                        "failed to disassemble {shader_package_name} Gloss-paired vertex shader {shader_index}"
                    )
                })?;
                for parameter in &shader.scalar_parameters {
                    *gloss_o0_vertex_scalar_parameters
                        .entry(format!(
                            "{}@slot{}:size{}",
                            parameter.name, parameter.slot, parameter.size
                        ))
                        .or_default() += 1;
                }
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                let Some(register) = vertex_texcoord4_output_register(&lines) else {
                    continue;
                };
                *gloss_o0_vertex_texcoord4_registers
                    .entry(register.clone())
                    .or_default() += 1;
                let target = format!("o{register}");
                if vertex_texcoord4_feeds_clip_position(&lines, &target) {
                    gloss_o0_vertex_projection_link_count += 1;
                }
                let bindings = dxbc_texture_bindings(&lines);
                let target_w = format!("{target}.w");
                for (line_index, line) in lines.iter().enumerate().filter(|(_, line)| {
                    instruction_writes_register_component(line, &target_w)
                }) {
                    let Some((opcode, operand_text)) = line.split_once(' ') else {
                        continue;
                    };
                    let operands = split_instruction_operands(operand_text);
                    gloss_o0_vertex_w_write_count += 1;
                    gloss_o0_vertex_w_height_clamp_count += usize::from(
                        vertex_texcoord4_w_matches_height_clamp(&lines, line_index, &target_w),
                    );
                    *gloss_o0_vertex_w_write_opcodes
                        .entry(opcode.to_string())
                        .or_default() += 1;
                    let roots = operands
                        .iter()
                        .skip(1)
                        .flat_map(|source| {
                            alpha_shaping_operand_root_sources(
                                &lines,
                                line_index,
                                source,
                                &bindings,
                            )
                        })
                        .collect::<BTreeSet<_>>();
                    let root_signature = if roots.is_empty() {
                        "none".to_string()
                    } else {
                        roots.into_iter().collect::<Vec<_>>().join(" + ")
                    };
                    *gloss_o0_vertex_w_root_source_sets
                        .entry(root_signature)
                        .or_default() += 1;
                    if gloss_o0_vertex_representative_writes.len() < 32 {
                        gloss_o0_vertex_representative_writes
                            .push(format!("vs{shader_index}: {line}"));
                    }
                    if gloss_o0_vertex_representative_traces.len() < 16 {
                        let start = line_index.saturating_sub(28);
                        gloss_o0_vertex_representative_traces.push(format!(
                            "vs{shader_index} TEXCOORD4.w trace lines {start}..{line_index}:\n{}",
                            lines[start..=line_index].join("\n")
                        ));
                    }
                }
            }

            Ok(VertexTexcoord4DxbcPackageAudit {
                shader_package_name: shader_package_name.to_string(),
                vertex_shader_count: package.vertex_shaders.len(),
                texcoord4_output_count,
                texcoord4_registers,
                texcoord4_write_count,
                texcoord4_write_opcodes,
                representative_writes,
                texcoord4_w_pixel_shader_count,
                texcoord4_w_output_reach_counts,
                texcoord4_w_output_representatives,
                legacy_gbuffer1_w_producer_pixel_shader_count:
                    legacy_gbuffer1_w_producer_pixel_shaders.len(),
                legacy_gbuffer1_w_producer_pass_pair_count:
                    legacy_gbuffer1_w_producer_pass_pairs.len(),
                legacy_gbuffer1_w_producer_vertex_shader_count:
                    legacy_gbuffer1_w_producer_vertex_indices.len(),
                legacy_gbuffer1_w_producer_vertex_height_clamp_count,
                legacy_gbuffer1_w_producer_vertex_shaders:
                    legacy_gbuffer1_w_producer_vertex_indices.into_iter().collect(),
                legacy_gbuffer1_w_producer_vertex_height_clamp_shaders,
                legacy_gbuffer1_w_producer_vertex_other_writes,
                legacy_gbuffer1_w_producer_vertex_wetness_reflection_count,
                legacy_gbuffer1_w_producer_vertex_wetness_reflection_unclassified_shaders,
                legacy_gbuffer1_producer_o1_write_counts,
                legacy_gbuffer1_producer_o1_write_opcodes,
                legacy_gbuffer1_producer_o1_x_pixel_shader_counts,
                legacy_gbuffer1_producer_o1_x_representative_pixel_shaders,
                legacy_gbuffer1_producer_o1_x_pass_pair_counts,
                legacy_gbuffer1_producer_o1_x_node_counts,
                legacy_gbuffer1_producer_o1_x_pass_ids,
                legacy_gbuffer1_producer_o1_x_material_key_sets,
                legacy_gbuffer1_producer_table_o1_reach_counts,
                legacy_gbuffer1_producer_table_o1_representatives,
                alpha_pixel_shader_count: alpha_pixel_shaders.len(),
                alpha_vertex_shader_count: alpha_vertex_indices.len(),
                alpha_vertex_texcoord4_registers,
                alpha_vertex_projection_link_count,
                alpha_vertex_representative_writes,
                alpha_vertex_representative_traces,
                gloss_o0_pixel_shader_count: gloss_o0_pixel_shaders.len(),
                gloss_o0_pass_pair_count: gloss_o0_pass_pairs.len(),
                gloss_o0_vertex_shader_count: gloss_o0_vertex_indices.len(),
                gloss_o0_vertex_texcoord4_registers,
                gloss_o0_vertex_projection_link_count,
                gloss_o0_vertex_w_write_count,
                gloss_o0_vertex_w_height_clamp_count,
                gloss_o0_vertex_w_write_opcodes,
                gloss_o0_vertex_w_root_source_sets,
                gloss_o0_vertex_scalar_parameters,
                gloss_o0_vertex_representative_writes,
                gloss_o0_vertex_representative_traces,
            })
        })
        .collect()
}

#[cfg(windows)]
fn legacy_gloss_o0_rgb_pixel_shaders(
    package: &physis::shpk::ShaderPackage,
    shader_package_name: &str,
) -> Result<BTreeSet<usize>> {
    let mut pixel_shaders = BTreeSet::new();
    for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
        let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
            format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
        })?;
        let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
        let bindings = dxbc_texture_bindings(&lines);
        for (line_index, line) in lines.iter().enumerate() {
            if dxbc_sample_texture_name(line, &bindings).as_deref() != Some("g_SamplerTable.T") {
                continue;
            }
            let Some(coordinates) = dxbc_sample_texture_coordinates(line) else {
                continue;
            };
            if dxbc_literal_provenance_at(&lines, line_index, coordinates, 0).as_deref()
                != Some("0.500000")
            {
                continue;
            }
            let Some(gloss_destination) = dxbc_sample_texture_physical_lane_destination(line, b'w')
            else {
                continue;
            };
            if ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                dxbc_register_reaches_output(&lines, line_index, &gloss_destination, output)
            }) {
                pixel_shaders.insert(shader_index);
            }
        }
    }
    Ok(pixel_shaders)
}

#[cfg(windows)]
fn vertex_texcoord4_feeds_clip_position(lines: &[&str], target: &str) -> bool {
    let target_xyz = format!("{target}.xyz");
    let Some(source_register) = lines.iter().find_map(|line| {
        let (destination, source) = line.split_once(',')?;
        (destination.split_whitespace().nth(1) == Some(target_xyz.as_str()))
            .then(|| source.trim().split('.').next())
            .flatten()
    }) else {
        return false;
    };

    [22, 23, 24, 25].into_iter().all(|matrix_row| {
        let matrix_row = format!("[{matrix_row}].xyzw");
        let source = format!(", {source_register}.xyzw");
        lines.iter().any(|line| {
            line.starts_with("dp4 ") && line.contains(&matrix_row) && line.ends_with(&source)
        })
    })
}

#[cfg(windows)]
fn shader_pass_pairs_from_debug(package: &physis::shpk::ShaderPackage) -> Vec<(usize, usize)> {
    shader_pass_pairs_from_debug_text(&format!("{package:#?}"))
}

#[cfg(windows)]
fn shader_pass_pairs_from_debug_text(debug: &str) -> Vec<(usize, usize)> {
    shader_pass_records_from_debug_text(debug)
        .into_iter()
        .map(|(_, vertex_shader, pixel_shader)| (vertex_shader, pixel_shader))
        .collect()
}

#[cfg(windows)]
fn shader_pass_records_from_debug_text(debug: &str) -> Vec<(u32, usize, usize)> {
    fn field_value(chunk: &str, field: &str) -> Option<usize> {
        chunk
            .split_once(field)
            .and_then(|(_, rest)| rest.trim_start().split_once(',').map(|(value, _)| value))
            .and_then(|value| value.trim().parse().ok())
    }

    debug
        .split("Pass {")
        .skip(1)
        .filter_map(|chunk| {
            Some((
                u32::try_from(field_value(chunk, "id:")?).ok()?,
                field_value(chunk, "vertex_shader:")?,
                field_value(chunk, "pixel_shader:")?,
            ))
        })
        .collect()
}

#[cfg(windows)]
fn vertex_texcoord4_output_register(lines: &[&str]) -> Option<String> {
    let mut in_output_signature = false;
    for line in lines {
        if line.contains("Output signature:") {
            in_output_signature = true;
            continue;
        }
        if !in_output_signature {
            continue;
        }
        let fields = line
            .trim_start_matches('/')
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() >= 4 && fields[0] == "TEXCOORD" && fields[1] == "4" {
            return Some(fields[3].to_string());
        }
        if !line.starts_with("//") && !line.is_empty() {
            break;
        }
    }
    None
}

#[cfg(windows)]
fn vertex_texcoord4_w_matches_height_clamp(
    lines: &[&str],
    write_index: usize,
    target_w: &str,
) -> bool {
    vertex_texcoord4_w_matches_height_clamp_slots(lines, write_index, target_w, 2, 3)
}

#[cfg(windows)]
fn dxbc_has_legacy_wetness_parameter_reflection(lines: &[&str]) -> bool {
    let has_offset = |member: &str, expected: usize| {
        lines.iter().any(|line| {
            line.contains(member)
                && line
                    .split_once("Offset:")
                    .and_then(|(_, value)| value.split_whitespace().next())
                    .and_then(|value| value.parse::<usize>().ok())
                    == Some(expected)
        })
    };
    let has_size = |parameter: &str, expected: usize| {
        lines.iter().any(|line| {
            line.contains(parameter)
                && line
                    .split_once("Size:")
                    .and_then(|(_, value)| value.split_whitespace().next())
                    .and_then(|value| value.parse::<usize>().ok())
                    == Some(expected)
        })
    };
    has_offset("float4 m_Wetness;", 64)
        && has_offset("float4 m_Params;", 0)
        && has_size("g_InstanceParameter;", 176)
        && has_size("g_ModelParameter;", 16)
}

#[cfg(windows)]
fn vertex_texcoord4_w_matches_height_clamp_slots(
    lines: &[&str],
    write_index: usize,
    target_w: &str,
    instance_slot: u16,
    model_slot: u16,
) -> bool {
    let instance = format!("cb{instance_slot}[4]");
    let model = format!("cb{model_slot}[0]");
    let instance_x = format!("{instance}.x");
    let instance_y = format!("{instance}.y");
    let instance_z = format!("{instance}.z");
    let instance_w = format!("{instance}.w");
    let model_x = format!("{model}.x");
    let Some(minimum) = lines
        .get(write_index)
        .and_then(|line| instruction_operands(line, "min"))
    else {
        return false;
    };
    if minimum.len() != 3
        || !register_components_overlap(minimum[0], target_w)
        || minimum[2] != instance_w
    {
        return false;
    }
    let value = minimum[1];
    let Some(maximum) = write_index
        .checked_sub(1)
        .and_then(|index| instruction_operands(lines[index], "max"))
    else {
        return false;
    };
    let Some(scale) = write_index
        .checked_sub(2)
        .and_then(|index| instruction_operands(lines[index], "mul"))
    else {
        return false;
    };
    let Some(offset) = write_index
        .checked_sub(3)
        .and_then(|index| instruction_operands(lines[index], "mad"))
    else {
        return false;
    };
    maximum == [value, value, instance_z.as_str()]
        && scale == [value, value, instance_x.as_str()]
        && offset == [value, "v0.y", model_x.as_str(), instance_y.as_str()]
}

#[cfg(windows)]
fn pixel_texcoord4_input_register(lines: &[&str]) -> Option<String> {
    let mut in_input_signature = false;
    for line in lines {
        if line.contains("Input signature:") {
            in_input_signature = true;
            continue;
        }
        if !in_input_signature {
            continue;
        }
        let fields = line
            .trim_start_matches('/')
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() >= 4 && fields[0] == "TEXCOORD" && fields[1] == "4" {
            return Some(fields[3].to_string());
        }
        if !line.starts_with("//") && !line.is_empty() {
            break;
        }
    }
    None
}

#[cfg(windows)]
fn dxbc_has_environment_blend_control(lines: &[&str], control: &str) -> bool {
    let has_positive_test = lines.iter().any(|line| {
        instruction_operands(line, "lt").is_some_and(|operands| {
            operands.len() == 3 && operands[1] == "l(0.000000)" && operands[2] == control
        })
    });
    let has_square = lines.iter().any(|line| {
        instruction_operands(line, "mul").is_some_and(|operands| {
            operands.len() == 3 && operands[1] == control && operands[2] == control
        })
    });
    let has_fade_start = lines.iter().any(|line| {
        instruction_operands(line, "add").is_some_and(|operands| {
            operands.len() == 3 && operands[1] == control && operands[2] == "l(-0.200000)"
        })
    });
    has_positive_test && has_square && has_fade_start
}

#[cfg(windows)]
fn dxbc_gloss_environment_blend_source(
    lines: &[&str],
    bindings: &BTreeMap<u32, String>,
) -> Option<&'static str> {
    if pixel_texcoord4_input_register(lines)
        .map(|register| format!("v{register}.w"))
        .is_some_and(|control| dxbc_has_environment_blend_control(lines, &control))
    {
        return Some("texcoord4.w");
    }
    for line in lines {
        if dxbc_sample_texture_name(line, bindings).as_deref() != Some("g_SamplerGBuffer1") {
            continue;
        }
        let Some(control) = dxbc_sample_texture_physical_lane_destination(line, b'w') else {
            continue;
        };
        if dxbc_has_environment_blend_control(lines, &control) {
            return Some("gbuffer1.w");
        }
    }
    None
}

#[cfg(windows)]
#[test]
#[ignore = "audits installed character vertex-alpha remap DXBC without scanning WeaponCatalog"]
fn audit_installed_vertex_alpha_remap_dxbc() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let report = audit_installed_unknown_constant_dxbc(&mut resource)?;
    assert_eq!(report.len(), 4);
    Ok(())
}

#[cfg(windows)]
#[test]
#[ignore = "audits installed ColorTable gloss/specular-strength DXBC consumers"]
fn audit_installed_material_strength_dxbc_patterns() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let report = audit_installed_material_strength_dxbc(&mut resource)?;
    let output_path = PathBuf::from("target").join("material-strength-dxbc-audit.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&report)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    assert_installed_material_strength_dxbc_boundary(&report);
    write_material_strength_representative_assemblies(&mut resource, &report)?;
    Ok(())
}

#[cfg(windows)]
fn write_material_strength_representative_assemblies(
    resource: &mut SqPackResource,
    report: &[MaterialStrengthDxbcPackageAudit],
) -> Result<()> {
    let legacy = report
        .iter()
        .find(|package| package.shader_package_name == "characterlegacy.shpk")
        .context("missing Legacy material-strength audit")?;
    let path = "shader/sm5/shpk/characterlegacy.shpk";
    let bytes = resource
        .read(path)
        .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
    let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
        .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
    let mut shader_indices = legacy
        .gloss_consumer_classes
        .iter()
        .filter(|class| class.o0_rgb_reach_count > 0)
        .filter_map(|class| class.representative_pixel_shaders.first().copied())
        .collect::<BTreeSet<_>>();
    shader_indices.extend(
        legacy
            .specular_strength_composition_classes
            .iter()
            .filter_map(|class| class.representative_pixel_shaders.first().copied()),
    );
    shader_indices.extend(
        legacy
            .specular_strength_composition_classes
            .iter()
            .filter_map(|class| {
                class
                    .terminal_rgb_multiplier_unclassified_pixel_shaders
                    .first()
                    .copied()
            }),
    );
    shader_indices.extend(
        legacy
            .specular_strength_composition_classes
            .iter()
            .flat_map(|class| {
                class
                    .dynamic_emissive_luminance_scale_unclassified_pixel_shaders
                    .iter()
                    .take(8)
                    .copied()
            }),
    );
    shader_indices.extend(
        legacy
            .gloss_camera_reflection_lobe_unclassified_pixel_shaders
            .iter()
            .take(8)
            .copied(),
    );
    shader_indices.extend(
        legacy
            .gloss_ambient_bake_light_unclassified_pixel_shaders
            .iter()
            .take(16)
            .copied(),
    );
    shader_indices.extend(
        legacy
            .gloss_environment_specular_strength_unjoined_pixel_shaders
            .iter()
            .take(8)
            .copied(),
    );
    for shader_index in shader_indices {
        let shader = package
            .pixel_shaders
            .get(shader_index)
            .with_context(|| format!("missing Legacy pixel shader {shader_index}"))?;
        let assembly = disassemble_dxbc(&shader.bytecode)
            .with_context(|| format!("failed to disassemble Legacy pixel shader {shader_index}"))?;
        let output_path =
            PathBuf::from("target").join(format!("characterlegacy-gloss-ps{shader_index}.asm"));
        fs::write(&output_path, assembly)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
    }
    Ok(())
}

#[cfg(windows)]
#[test]
#[ignore = "audits installed character alpha aperture/offset DXBC without scanning WeaponCatalog"]
fn audit_installed_alpha_shaping_dxbc_patterns() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let audits = audit_installed_alpha_shaping_dxbc(&mut resource)?;
    assert_eq!(audits.len(), 4);
    let counts = audits
        .iter()
        .map(|audit| {
            (
                audit.shader_package_name.as_str(),
                (
                    audit.pixel_shader_count,
                    audit.aperture_use_count,
                    audit.offset_use_count,
                    audit.formula_count,
                    audit.alpha_composition_count,
                    audit.unclassified_formula_count,
                    audit.shaping_dot_count,
                    audit.view_from_v6_count,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        counts,
        BTreeMap::from([
            ("character.shpk", (1038, 768, 1539, 768, 768, 0, 768, 768)),
            ("characterlegacy.shpk", (1740, 0, 2, 0, 0, 0, 0, 0)),
            ("characterglass.shpk", (38, 34, 70, 34, 34, 0, 34, 34)),
            ("skin.shpk", (384, 0, 3, 0, 0, 0, 0, 0)),
        ])
    );
    Ok(())
}

#[cfg(windows)]
#[test]
#[ignore = "audits installed character/glass VS TEXCOORD4 output with fxc fallback"]
fn audit_installed_vertex_texcoord4_dxbc() -> Result<()> {
    let game_dir = normalize_game_dir(&game_dir())?;
    let game_dir_text = game_dir
        .to_str()
        .ok_or_else(|| anyhow!("game dir is not valid UTF-8: {}", game_dir.display()))?;
    let mut resource = SqPackResource::from_existing(game_dir_text);
    let audits = audit_vertex_texcoord4_dxbc(&mut resource)?;
    assert_eq!(audits.len(), 3);
    let output_path = PathBuf::from("target").join("vertex-texcoord4-dxbc-audit.json");
    fs::write(&output_path, serde_json::to_vec_pretty(&audits)?)
        .with_context(|| format!("failed to write {}", output_path.display()))?;
    write_legacy_gbuffer1_producer_representative_assemblies(&mut resource, &audits)?;
    let counts = audits
        .iter()
        .map(|audit| {
            (
                audit.shader_package_name.as_str(),
                (
                    audit.vertex_shader_count,
                    audit.alpha_pixel_shader_count,
                    audit.alpha_vertex_shader_count,
                    audit.alpha_vertex_texcoord4_registers.clone(),
                    audit.alpha_vertex_projection_link_count,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        counts,
        BTreeMap::from([
            (
                "character.shpk",
                (176, 768, 64, BTreeMap::from([("6".to_string(), 64)]), 64),
            ),
            (
                "characterglass.shpk",
                (32, 34, 8, BTreeMap::from([("6".to_string(), 8)]), 8),
            ),
            ("characterlegacy.shpk", (64, 0, 0, BTreeMap::new(), 0),),
        ])
    );
    let legacy = audits
        .iter()
        .find(|audit| audit.shader_package_name == "characterlegacy.shpk")
        .context("missing Legacy vertex TEXCOORD4 audit")?;
    assert_eq!(legacy.texcoord4_w_pixel_shader_count, 1736);
    assert_eq!(
        legacy.texcoord4_w_output_reach_counts,
        BTreeMap::from([
            ("o0.w".to_string(), 148),
            ("o0.x".to_string(), 868),
            ("o0.y".to_string(), 868),
            ("o0.z".to_string(), 868),
            ("o1.w".to_string(), 288),
        ])
    );
    assert_eq!(legacy.legacy_gbuffer1_w_producer_pixel_shader_count, 288);
    assert_eq!(legacy.legacy_gbuffer1_w_producer_pass_pair_count, 6144);
    assert_eq!(legacy.legacy_gbuffer1_w_producer_vertex_shader_count, 16);
    assert_eq!(
        legacy.legacy_gbuffer1_w_producer_vertex_height_clamp_count,
        16
    );
    assert_eq!(
        legacy.legacy_gbuffer1_w_producer_vertex_shaders,
        vec![2, 3, 11, 12, 18, 19, 27, 28, 34, 35, 43, 44, 50, 51, 59, 60]
    );
    assert_eq!(
        legacy.legacy_gbuffer1_w_producer_vertex_height_clamp_shaders,
        legacy.legacy_gbuffer1_w_producer_vertex_shaders
    );
    assert!(
        legacy
            .legacy_gbuffer1_w_producer_vertex_other_writes
            .is_empty()
    );
    assert_eq!(
        legacy.legacy_gbuffer1_w_producer_vertex_wetness_reflection_count,
        16
    );
    assert!(
        legacy
            .legacy_gbuffer1_w_producer_vertex_wetness_reflection_unclassified_shaders
            .is_empty()
    );
    assert_eq!(
        legacy.legacy_gbuffer1_producer_o1_write_counts,
        BTreeMap::from([
            ("o1.w".to_string(), 288),
            ("o1.x".to_string(), 288),
            ("o1.y".to_string(), 288),
            ("o1.z".to_string(), 288),
        ])
    );
    assert_eq!(
        legacy.legacy_gbuffer1_producer_o1_write_opcodes,
        BTreeMap::from([
            ("o1.w:mov".to_string(), 288),
            ("o1.x:mul".to_string(), 144),
            ("o1.x:mov".to_string(), 48),
            ("o1.x:movc".to_string(), 96),
            ("o1.y:exp".to_string(), 288),
            ("o1.z:mov".to_string(), 288),
        ])
    );
    assert_eq!(
        legacy.legacy_gbuffer1_producer_o1_x_pixel_shader_counts,
        BTreeMap::from([
            ("mov".to_string(), 48),
            ("movc".to_string(), 96),
            ("mul".to_string(), 144),
        ])
    );
    assert_eq!(
        legacy
            .legacy_gbuffer1_producer_o1_x_representative_pixel_shaders
            .values()
            .map(Vec::len)
            .sum::<usize>(),
        12
    );
    assert_eq!(
        legacy.legacy_gbuffer1_producer_o1_x_pass_pair_counts,
        BTreeMap::from([
            ("mov".to_string(), 1024),
            ("movc".to_string(), 2048),
            ("mul".to_string(), 3072),
        ])
    );
    assert_eq!(
        legacy.legacy_gbuffer1_producer_o1_x_node_counts,
        legacy.legacy_gbuffer1_producer_o1_x_pass_pair_counts
    );
    assert_eq!(
        legacy.legacy_gbuffer1_producer_o1_x_pass_ids,
        BTreeMap::from([
            (
                "mov".to_string(),
                BTreeMap::from([("0x6006067f".to_string(), 1024)]),
            ),
            (
                "movc".to_string(),
                BTreeMap::from([("0x6006067f".to_string(), 2048)]),
            ),
            (
                "mul".to_string(),
                BTreeMap::from([("0x03ac862e".to_string(), 3072)]),
            ),
        ])
    );
    assert_eq!(
        legacy
            .legacy_gbuffer1_producer_o1_x_material_key_sets
            .get("mov")
            .map(BTreeMap::len),
        Some(8)
    );
    assert_eq!(
        legacy
            .legacy_gbuffer1_producer_o1_x_material_key_sets
            .get("movc")
            .map(BTreeMap::len),
        Some(16)
    );
    assert_eq!(
        legacy
            .legacy_gbuffer1_producer_o1_x_material_key_sets
            .get("mul")
            .map(BTreeMap::len),
        Some(24)
    );
    let mov_material_keys = legacy
        .legacy_gbuffer1_producer_o1_x_material_key_sets
        .get("mov")
        .context("missing Legacy GBuffer1 o1.x mov material keys")?;
    assert!(
        mov_material_keys
            .keys()
            .all(|keys| keys.contains("D2777173=4242B842"))
    );
    let movc_material_keys = legacy
        .legacy_gbuffer1_producer_o1_x_material_key_sets
        .get("movc")
        .context("missing Legacy GBuffer1 o1.x movc material keys")?;
    assert!(
        movc_material_keys.keys().all(|keys| {
            keys.contains("D2777173=584265DD") || keys.contains("D2777173=F35F5131")
        })
    );
    let mul_material_keys = legacy
        .legacy_gbuffer1_producer_o1_x_material_key_sets
        .get("mul")
        .context("missing Legacy GBuffer1 o1.x mul material keys")?;
    for decal_mode in ["4242B842", "584265DD", "F35F5131"] {
        assert!(
            mul_material_keys
                .keys()
                .any(|keys| keys.contains(&format!("D2777173={decal_mode}")))
        );
    }
    assert_eq!(
        legacy.legacy_gbuffer1_producer_table_o1_reach_counts,
        BTreeMap::from([
            ("tableX=0.500000,lane=w,output=o1.y".to_string(), 272),
            ("tableX=1.500000,lane=w,output=o1.x".to_string(), 144),
            ("tableX=6.500000,lane=y,output=o1.x".to_string(), 288),
            ("tableX=6.500000,lane=z,output=o1.x".to_string(), 288),
            ("tableX=7.500000,lane=w,output=o1.x".to_string(), 288),
            ("tableX=7.500000,lane=x,output=o1.x".to_string(), 288),
            ("tableX=7.500000,lane=y,output=o1.x".to_string(), 288),
            ("tableX=7.500000,lane=z,output=o1.x".to_string(), 288),
            ("tableX=dynamic,lane=w,output=o1.y".to_string(), 16),
        ])
    );
    assert_eq!(
        legacy
            .legacy_gbuffer1_producer_table_o1_representatives
            .len(),
        24
    );
    assert_eq!(legacy.gloss_o0_pixel_shader_count, 1440);
    assert_eq!(legacy.gloss_o0_pass_pair_count, 9216);
    assert_eq!(legacy.gloss_o0_vertex_shader_count, 16);
    assert_eq!(
        legacy.gloss_o0_vertex_texcoord4_registers,
        BTreeMap::from([
            ("3".to_string(), 4),
            ("4".to_string(), 4),
            ("6".to_string(), 8),
        ])
    );
    assert_eq!(legacy.gloss_o0_vertex_projection_link_count, 16);
    assert_eq!(legacy.gloss_o0_vertex_w_write_count, 16);
    assert_eq!(legacy.gloss_o0_vertex_w_height_clamp_count, 16);
    assert_eq!(
        legacy.gloss_o0_vertex_w_write_opcodes,
        BTreeMap::from([("min".to_string(), 16)])
    );
    assert_eq!(
        legacy.gloss_o0_vertex_w_root_source_sets,
        BTreeMap::from([(
            "constant:cb2[4] + constant:cb3[0] + vertex:v0".to_string(),
            16,
        )])
    );
    assert_eq!(
        legacy.gloss_o0_vertex_scalar_parameters,
        BTreeMap::from([
            ("g_CameraParameter@slot0:size59".to_string(), 16),
            ("g_InstanceParameter@slot2:size11".to_string(), 16),
            ("g_InstancingMatrix@slot4:size768".to_string(), 8),
            ("g_JointMatrixArray@slot1:size768".to_string(), 8),
            ("g_ModelParameter@slot3:size1".to_string(), 16),
            ("g_WorldViewMatrix@slot1:size6".to_string(), 8),
        ])
    );
    assert!(
        audits
            .iter()
            .filter(|audit| audit.shader_package_name != "characterlegacy.shpk")
            .all(|audit| {
                audit.gloss_o0_pixel_shader_count == 0
                    && audit.gloss_o0_pass_pair_count == 0
                    && audit.gloss_o0_vertex_shader_count == 0
                    && audit.gloss_o0_vertex_w_write_count == 0
                    && audit.legacy_gbuffer1_producer_o1_write_counts.is_empty()
                    && audit.legacy_gbuffer1_producer_o1_write_opcodes.is_empty()
                    && audit
                        .legacy_gbuffer1_producer_o1_x_pixel_shader_counts
                        .is_empty()
                    && audit
                        .legacy_gbuffer1_producer_o1_x_representative_pixel_shaders
                        .is_empty()
                    && audit
                        .legacy_gbuffer1_producer_o1_x_pass_pair_counts
                        .is_empty()
                    && audit.legacy_gbuffer1_producer_o1_x_node_counts.is_empty()
                    && audit.legacy_gbuffer1_producer_o1_x_pass_ids.is_empty()
                    && audit
                        .legacy_gbuffer1_producer_o1_x_material_key_sets
                        .is_empty()
                    && audit
                        .legacy_gbuffer1_producer_table_o1_reach_counts
                        .is_empty()
                    && audit
                        .legacy_gbuffer1_producer_table_o1_representatives
                        .is_empty()
            })
    );
    Ok(())
}

#[cfg(windows)]
fn write_legacy_gbuffer1_producer_representative_assemblies(
    resource: &mut SqPackResource,
    audits: &[VertexTexcoord4DxbcPackageAudit],
) -> Result<()> {
    let legacy = audits
        .iter()
        .find(|audit| audit.shader_package_name == "characterlegacy.shpk")
        .context("missing Legacy vertex TEXCOORD4 audit")?;
    let shader_indices = legacy
        .legacy_gbuffer1_producer_o1_x_representative_pixel_shaders
        .values()
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    if shader_indices.is_empty() {
        return Err(anyhow!(
            "missing Legacy GBuffer1 o1.x producer representatives"
        ));
    }
    let path = "shader/sm5/shpk/characterlegacy.shpk";
    let bytes = resource
        .read(path)
        .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
    let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
        .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
    for shader_index in shader_indices {
        let shader = package
            .pixel_shaders
            .get(shader_index)
            .with_context(|| format!("missing Legacy GBuffer producer PS {shader_index}"))?;
        let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
            format!("failed to disassemble Legacy GBuffer producer PS {shader_index}")
        })?;
        let output_path = PathBuf::from("target").join(format!(
            "characterlegacy-gbuffer1-producer-ps{shader_index}.asm"
        ));
        fs::write(&output_path, assembly)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
    }
    for shader_index in &legacy.legacy_gbuffer1_w_producer_vertex_shaders {
        let shader = package
            .vertex_shaders
            .get(*shader_index)
            .with_context(|| format!("missing Legacy GBuffer producer VS {shader_index}"))?;
        let assembly = disassemble_dxbc_resilient(&shader.bytecode).with_context(|| {
            format!("failed to disassemble Legacy GBuffer producer VS {shader_index}")
        })?;
        let output_path = PathBuf::from("target").join(format!(
            "characterlegacy-gbuffer1-producer-vs{shader_index}.asm"
        ));
        fs::write(&output_path, assembly)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
    }
    Ok(())
}

#[cfg(windows)]
fn audit_installed_alpha_shaping_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<AlphaShapingDxbcPackageAudit>> {
    [
        "character.shpk",
        "characterlegacy.shpk",
        "characterglass.shpk",
        "skin.shpk",
    ]
    .into_iter()
    .map(|shader_package_name| {
        let path = format!("shader/sm5/shpk/{shader_package_name}");
        let bytes = resource
            .read(&path)
            .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
        let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
            .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
        let mut aperture_use_count = 0;
        let mut offset_use_count = 0;
        let mut formula_count = 0;
        let mut alpha_composition_count = 0;
        let mut scaled_alpha_count = 0;
        let mut unclassified_downstream_use_count = 0;
        let mut unclassified_dead_count = 0;
        let mut unclassified_first_use_opcodes = BTreeMap::new();
        let mut shaping_dot_count = 0;
        let mut view_from_v6_count = 0;
        let mut non_view_dot_producer_opcodes = BTreeMap::new();
        let mut offset_sign_gate_count = 0;
        let mut alpha_less_than_one_gate_count = 0;
        let mut shaping_scale_operands = BTreeMap::new();
        let mut shaping_base_operands = BTreeMap::new();
        let mut shaping_scale_root_sources = BTreeMap::new();
        let mut shaping_base_root_sources = BTreeMap::new();
        let mut incomplete_representatives = Vec::new();

        for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
            let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
                format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
            })?;
            let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
            let texture_bindings = dxbc_texture_bindings(&lines);
            for (line_index, line) in lines.iter().enumerate() {
                if line.matches("cb0[12].w").count() > 0 {
                    aperture_use_count += line.matches("cb0[12].w").count();
                }
                offset_use_count += line.matches("cb0[13].x").count();
                let Some(shape_register) = alpha_shaping_shape_destination(&lines, line_index)
                else {
                    continue;
                };
                formula_count += 1;
                if let Some((left, right)) = alpha_shaping_dot_operands(&lines, line_index) {
                    shaping_dot_count += 1;
                    if alpha_shaping_dot_has_view_from_v6(&lines, line_index, left, right) {
                        view_from_v6_count += 1;
                    }
                    if let Some(normal_operand) = alpha_shaping_normal_dot_operand(
                        &lines,
                        line_index,
                        left,
                        right,
                    ) {
                        if let Some(opcode) = alpha_shaping_operand_producer_opcode(
                            &lines,
                            line_index,
                            normal_operand,
                        ) {
                            *non_view_dot_producer_opcodes.entry(opcode).or_default() += 1;
                        }
                    }
                }
                // The `baseAlpha < 1` guard is emitted before the normal/view
                // work in some permutations, while the composition follows it.
                let window_start = line_index.saturating_sub(24);
                let window_end = (line_index + 96).min(lines.len());
                let window = &lines[window_start..window_end];
                let downstream = &lines[line_index + 1..window_end];
                let has_scaled_alpha =
                    alpha_shaping_has_scaled_alpha(downstream, &shape_register);
                let has_composition =
                    alpha_shaping_has_saturated_composition(downstream, &shape_register);
                let has_offset_gate = alpha_shaping_has_offset_sign_gate(downstream);
                let has_alpha_gate = alpha_shaping_has_alpha_less_than_one_gate(window);
                if has_scaled_alpha {
                    scaled_alpha_count += 1;
                }
                if let Some((composition_index, scaled_alpha, base_alpha)) =
                    alpha_shaping_composition(downstream, &shape_register)
                {
                    *shaping_scale_operands
                        .entry(scaled_alpha.to_string())
                        .or_default() += 1;
                    *shaping_base_operands
                        .entry(base_alpha.to_string())
                        .or_default() += 1;
                    let absolute_composition_index = line_index + 1 + composition_index;
                    for source in alpha_shaping_operand_root_sources(
                        &lines,
                        absolute_composition_index,
                        scaled_alpha,
                        &texture_bindings,
                    ) {
                        *shaping_scale_root_sources.entry(source).or_default() += 1;
                    }
                    for source in alpha_shaping_operand_root_sources(
                        &lines,
                        absolute_composition_index,
                        base_alpha,
                        &texture_bindings,
                    ) {
                        *shaping_base_root_sources.entry(source).or_default() += 1;
                    }
                }
                if has_composition {
                    alpha_composition_count += 1;
                } else if let Some(opcode) =
                    alpha_shaping_downstream_use(&lines, line_index, &shape_register)
                {
                    unclassified_downstream_use_count += 1;
                    *unclassified_first_use_opcodes.entry(opcode).or_default() += 1;
                } else {
                    unclassified_dead_count += 1;
                }
                if has_offset_gate {
                    offset_sign_gate_count += 1;
                }
                if has_alpha_gate {
                    alpha_less_than_one_gate_count += 1;
                }
                if incomplete_representatives.len() < 6
                    && (!has_scaled_alpha
                        || !has_composition
                        || !has_offset_gate
                        || !has_alpha_gate)
                {
                    incomplete_representatives.push(format!(
                        "ps{shader_index} scaled={has_scaled_alpha} composition={has_composition} offsetGate={has_offset_gate} alphaGate={has_alpha_gate}:\n{}",
                        window.join("\n")
                    ));
                }
            }
        }

        if aperture_use_count != formula_count
            || scaled_alpha_count != alpha_composition_count
            || offset_sign_gate_count != alpha_composition_count
            || alpha_less_than_one_gate_count != alpha_composition_count
        {
            return Err(anyhow!(
                "{shader_package_name} has unexpected alpha shaping consumers: aperture_uses={aperture_use_count}, formulas={formula_count}, scaled_alpha={scaled_alpha_count}, alpha_composition={alpha_composition_count}, offset_gate={offset_sign_gate_count}, alpha_gate={alpha_less_than_one_gate_count}\n{}",
                incomplete_representatives.join("\n---\n")
            ));
        }

        Ok(AlphaShapingDxbcPackageAudit {
            shader_package_name: shader_package_name.to_string(),
            pixel_shader_count: package.pixel_shaders.len(),
            aperture_use_count,
            offset_use_count,
            formula_count,
            alpha_composition_count,
            unclassified_formula_count: formula_count - alpha_composition_count,
            unclassified_downstream_use_count,
            unclassified_dead_count,
            unclassified_first_use_opcodes,
            shaping_dot_count,
            view_from_v6_count,
            non_view_dot_producer_opcodes,
            scaled_alpha_count,
            shaping_scale_operands,
            shaping_base_operands,
            shaping_scale_root_sources,
            shaping_base_root_sources,
            offset_sign_gate_count,
            alpha_less_than_one_gate_count,
        })
    })
    .collect()
}

#[cfg(windows)]
fn audit_installed_unknown_constant_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<UnknownConstantDxbcPackageAudit>> {
    const UNKNOWN_CONSTANT_OFFSET: u16 = 212;
    const DXBC_OPERAND: &str = "cb0[13].y";

    [
        "character.shpk",
        "characterlegacy.shpk",
        "characterglass.shpk",
        "skin.shpk",
    ]
    .into_iter()
    .map(|shader_package_name| {
        let path = format!("shader/sm5/shpk/{shader_package_name}");
        let bytes = resource
            .read(&path)
            .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
        let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
            .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
        let mut consumer_shader_count = 0;
        let mut use_count = 0;
        let mut vertex_alpha_remap_count = 0;
        let mut immediate_alpha_product_count = 0;
        let mut alpha_threshold_test_count = 0;
        let mut non_product_uses = Vec::new();
        let mut instruction_patterns = BTreeMap::new();
        let mut representative_uses = Vec::new();
        for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
            let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
                format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
            })?;
            let mut shader_use_count = 0;
            let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
            for (line_index, line) in lines.iter().enumerate() {
                let line_use_count = line.matches(DXBC_OPERAND).count();
                if line_use_count == 0 {
                    continue;
                }
                shader_use_count += line_use_count;
                if let Some(remapped_alpha) = vertex_alpha_remap_destination(&lines, line_index) {
                    vertex_alpha_remap_count += 1;
                    if vertex_alpha_has_immediate_product(&lines, line_index, &remapped_alpha) {
                        immediate_alpha_product_count += 1;
                    } else if vertex_alpha_has_threshold_test(&lines, line_index, &remapped_alpha) {
                        alpha_threshold_test_count += 1;
                    } else if non_product_uses.len() < 4 {
                        let start = line_index.saturating_sub(4);
                        let end = (line_index + 17).min(lines.len());
                        non_product_uses.push(format!(
                            "ps{shader_index} lines {start}..{end}:\n{}",
                            lines[start..end].join("\n")
                        ));
                    }
                }
                let (opcode, operands) = line
                    .split_once(' ')
                    .map(|(opcode, operands)| (opcode, split_instruction_operands(operands)))
                    .unwrap_or((line, Vec::new()));
                for (operand_index, operand) in operands.iter().enumerate() {
                    if operand.contains(DXBC_OPERAND) {
                        *instruction_patterns
                            .entry(format!("{opcode}:operand{operand_index}"))
                            .or_default() += 1;
                    }
                }
                if representative_uses.len() < 4 {
                    let start = line_index.saturating_sub(4);
                    let end = (line_index + 49).min(lines.len());
                    representative_uses.push(format!(
                        "ps{shader_index} lines {start}..{end}:\n{}",
                        lines[start..end].join("\n")
                    ));
                }
            }
            use_count += shader_use_count;
            consumer_shader_count += usize::from(shader_use_count > 0);
        }
        if vertex_alpha_remap_count != use_count
            || immediate_alpha_product_count + alpha_threshold_test_count != use_count
        {
            return Err(anyhow!(
                "{shader_package_name} has {use_count} uses of {DXBC_OPERAND}, but only {vertex_alpha_remap_count} match mix(vertexAlpha, 1, constant), {immediate_alpha_product_count} feed a surface-alpha product, and {alpha_threshold_test_count} feed a texture-alpha threshold test:\n{}",
                non_product_uses.join("\n---\n")
            ));
        }
        Ok(UnknownConstantDxbcPackageAudit {
            shader_package_name: shader_package_name.to_string(),
            byte_offset: UNKNOWN_CONSTANT_OFFSET,
            dxbc_operand: DXBC_OPERAND.to_string(),
            pixel_shader_count: package.pixel_shaders.len(),
            consumer_shader_count,
            use_count,
            vertex_alpha_remap_count,
            immediate_alpha_product_count,
            alpha_threshold_test_count,
            instruction_patterns,
            representative_uses,
        })
    })
    .collect()
}

#[cfg(windows)]
fn audit_installed_tile_mip_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<TileMipDxbcPackageAudit>> {
    ["character.shpk", "characterlegacy.shpk"]
        .into_iter()
        .map(|shader_package_name| {
            let path = format!("shader/sm5/shpk/{shader_package_name}");
            let bytes = resource
                .read(&path)
                .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
            let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
                .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;

            let mut declared_shader_count = 0;
            let mut consumer_shader_count = 0;
            let mut use_count = 0;
            let mut formula_count = 0;
            let mut uses_per_shader = BTreeMap::new();
            let mut consumer_sets = BTreeMap::new();

            for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
                let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
                    format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
                })?;
                if !assembly.contains("cb0[17].y") {
                    continue;
                }

                declared_shader_count += 1;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                let texture_bindings = dxbc_texture_bindings(&lines);
                let mut shader_use_count = 0;

                for (line_index, line) in lines.iter().enumerate() {
                    let Some(bias_register) = tile_bias_add_destination(line) else {
                        continue;
                    };
                    shader_use_count += 1;
                    use_count += 1;

                    if !tile_bias_formula_is_expected(&lines, line_index, &bias_register) {
                        let start = line_index.saturating_sub(6);
                        return Err(anyhow!(
                            "{shader_package_name} pixel shader {shader_index} has an unexpected g_TileMipBiasOffset formula near:\n{}",
                            lines[start..=line_index].join("\n")
                        ));
                    }
                    formula_count += 1;

                    let consumers = tile_bias_consumers(
                        &lines,
                        line_index,
                        &bias_register,
                        &texture_bindings,
                    );
                    if consumers.is_empty()
                        || consumers.iter().any(|name| {
                            !matches!(
                                name.as_str(),
                                "g_SamplerTileOrb.T" | "g_SamplerTileNormal"
                            )
                        })
                        || !consumers.contains("g_SamplerTileOrb.T")
                    {
                        return Err(anyhow!(
                            "{shader_package_name} pixel shader {shader_index} sends {bias_register} to unexpected consumers: {:?}",
                            consumers
                        ));
                    }
                    let consumer_key = consumers.into_iter().collect::<Vec<_>>().join(",");
                    *consumer_sets.entry(consumer_key).or_default() += 1;
                }

                if shader_use_count > 0 {
                    consumer_shader_count += 1;
                }
                *uses_per_shader.entry(shader_use_count).or_default() += 1;
            }

            Ok(TileMipDxbcPackageAudit {
                shader_package_name: shader_package_name.to_string(),
                pixel_shader_count: package.pixel_shaders.len(),
                declared_shader_count,
                consumer_shader_count,
                use_count,
                formula_count,
                uses_per_shader,
                consumer_sets,
            })
        })
        .collect()
}

#[cfg(windows)]
fn audit_installed_tile_blend_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<TileBlendDxbcPackageAudit>> {
    ["character.shpk", "characterlegacy.shpk"]
        .into_iter()
        .map(|shader_package_name| {
            let path = format!("shader/sm5/shpk/{shader_package_name}");
            let bytes = resource
                .read(&path)
                .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
            let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
                .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;

            let mut orb_neutral_pair_count = 0;
            let mut orb_blend_pair_count = 0;
            let mut ordered_ab_blend_pair_count = 0;
            let mut normal_blend_pair_count = 0;
            let mut shaping_table_sample_count = 0;
            let mut shaping_anisotropy_a_sample_count = 0;
            let mut index_texture_sample_count = 0;
            let mut inverted_index_weight_count = 0;

            for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
                let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
                    format!(
                        "failed to disassemble {shader_package_name} pixel shader {shader_index}"
                    )
                })?;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                let texture_bindings = dxbc_texture_bindings(&lines);
                let mut neutral_results = Vec::new();

                for (line_index, line) in lines.iter().enumerate() {
                    if dxbc_sample_texture_name(line, &texture_bindings).as_deref()
                        == Some("g_SamplerIndex.T")
                    {
                        index_texture_sample_count += 1;
                        if index_blend_inversion_after_sample(&lines, line_index) {
                            inverted_index_weight_count += 1;
                        }
                    }
                    if dxbc_sample_texture_name(line, &texture_bindings).as_deref()
                        == Some("g_SamplerTable.T")
                        && shaping_dependent_table_sample(&lines, line_index)
                    {
                        shaping_table_sample_count += 1;
                        let operands = line
                            .split_once(' ')
                            .map(|(_, operands)| split_instruction_operands(operands))
                            .unwrap_or_default();
                        let destination = operands.first().copied();
                        let coordinates = operands.get(1).copied();
                        if destination.is_some_and(|destination| {
                            dxbc_operand_components(destination)
                                .iter()
                                .any(|component| component.ends_with(".w"))
                                && dxbc_row_provenance_at(
                                    &lines,
                                    &texture_bindings,
                                    line_index,
                                    destination,
                                ) == Some(DxbcTableRow::A)
                        }) && coordinates.is_some_and(|coordinates| {
                            dxbc_literal_provenance_at(&lines, line_index, coordinates, 0)
                                .as_deref()
                                == Some("4.500000")
                        }) {
                            shaping_anisotropy_a_sample_count += 1;
                        }
                    }
                    if let Some((destination, alpha_source, mad_index)) =
                        tile_orb_neutral_pair(&lines, line_index)
                    {
                        neutral_results.push((destination, alpha_source, mad_index));
                    }
                }

                for pair in neutral_results.windows(2) {
                    let first = &pair[0].0;
                    let second = &pair[1].0;
                    let search_start = pair[1].2 + 1;
                    let search_end = (search_start + 8).min(lines.len());
                    let Some((orb_blend_index, orb_weight, ordered_sources)) =
                        (search_start..search_end).find_map(|line_index| {
                            let blend = dxbc_linear_blend(&lines, line_index)?;
                            let sources = BTreeSet::from([
                                dxbc_register_base(&blend.source_a)?.to_string(),
                                dxbc_register_base(&blend.source_b)?.to_string(),
                            ]);
                            let expected = BTreeSet::from([
                                dxbc_register_base(first)?.to_string(),
                                dxbc_register_base(second)?.to_string(),
                            ]);
                            (sources == expected).then_some((
                                blend.mad_index,
                                blend.weight,
                                dxbc_register_base(blend.source_a) == dxbc_register_base(first)
                                    && dxbc_register_base(blend.source_b)
                                        == dxbc_register_base(second),
                            ))
                        })
                    else {
                        continue;
                    };
                    orb_neutral_pair_count += 2;
                    orb_blend_pair_count += 1;
                    let alpha_a =
                        dxbc_row_provenance_at(&lines, &texture_bindings, pair[0].2, &pair[0].1);
                    let alpha_b =
                        dxbc_row_provenance_at(&lines, &texture_bindings, pair[1].2, &pair[1].1);
                    if ordered_sources
                        && alpha_a == Some(DxbcTableRow::A)
                        && alpha_b == Some(DxbcTableRow::B)
                    {
                        ordered_ab_blend_pair_count += 1;
                    }

                    let normal_search_end = (orb_blend_index + 8).min(lines.len());
                    if (orb_blend_index + 1..normal_search_end)
                        .any(|line_index| dxbc_weighted_result_blend(lines[line_index], orb_weight))
                    {
                        normal_blend_pair_count += 1;
                    }
                }
            }

            Ok(TileBlendDxbcPackageAudit {
                shader_package_name: shader_package_name.to_string(),
                pixel_shader_count: package.pixel_shaders.len(),
                orb_neutral_pair_count,
                orb_blend_pair_count,
                ordered_ab_blend_pair_count,
                normal_blend_pair_count,
                shaping_table_sample_count,
                shaping_anisotropy_a_sample_count,
                index_texture_sample_count,
                inverted_index_weight_count,
            })
        })
        .collect()
}

#[cfg(windows)]
fn audit_installed_material_strength_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<MaterialStrengthDxbcPackageAudit>> {
    [
        "character.shpk",
        "characterlegacy.shpk",
        "characterglass.shpk",
    ]
    .into_iter()
    .map(|shader_package_name| {
        let path = format!("shader/sm5/shpk/{shader_package_name}");
        let bytes = resource
            .read(&path)
            .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
        let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
            .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
        let mut roughness_sample_count = 0;
        let mut roughness_pixel_shaders = BTreeSet::new();
        let mut roughness_consumer_sample_count = 0;
        let mut roughness_consumer_opcodes = BTreeMap::new();
        let mut roughness_o1_y_reach_count = 0;
        let mut roughness_consumer_representatives = Vec::new();
        let mut gloss_sample_count = 0;
        let mut gloss_pixel_shaders = BTreeSet::new();
        let mut gloss_consumer_sample_count = 0;
        let mut gloss_consumer_opcodes = BTreeMap::new();
        let mut gloss_o1_y_reach_count = 0;
        let mut gloss_o0_rgb_reach_count = 0;
        let mut gloss_power_chain_count = 0;
        let mut gloss_power_o0_rgb_reach_count = 0;
        let mut gloss_camera_reflection_power_chain_count = 0;
        let mut gloss_camera_reflection_lobe_count = 0;
        let mut gloss_camera_reflection_lobe_unclassified_pixel_shaders = BTreeSet::new();
        let mut gloss_cube_lod_sample_count = 0;
        let mut gloss_cube_sample_hdr_decode_count = 0;
        let mut gloss_cube_sample_o0_rgb_reach_count = 0;
        let mut gloss_cube_current_location_sample_count = 0;
        let mut gloss_cube_previous_location_sample_count = 0;
        let mut gloss_ambient_location_interpolation_count = 0;
        let mut gloss_ambient_reflection_scale_offset_count = 0;
        let mut gloss_ambient_bake_light_composition_count = 0;
        let mut gloss_ambient_bake_light_unclassified_pixel_shaders = BTreeSet::new();
        let mut gloss_environment_specular_strength_join_count = 0;
        let mut gloss_environment_specular_strength_unjoined_pixel_shaders = BTreeSet::new();
        let mut gloss_cube_specular_strength_pixel_shader_count = 0;
        let mut gloss_non_cube_specular_strength_pixel_shader_count = 0;
        let mut gloss_texcoord4_w_environment_blend_count = 0;
        let mut gloss_gbuffer1_w_environment_blend_count = 0;
        let mut gloss_environment_blend_unclassified_pixel_shaders = BTreeSet::new();
        let mut gloss_consumer_opcode_sequences = BTreeMap::new();
        let mut gloss_consumer_classes = BTreeMap::<
            String,
            MaterialStrengthDxbcGlossConsumerClassAccumulator,
        >::new();
        let mut gloss_consumer_class_by_pixel_shader = BTreeMap::new();
        let mut gloss_consumer_representatives = Vec::new();
        let mut specular_strength_sample_count = 0;
        let mut specular_strength_pixel_shaders = BTreeSet::new();
        let mut specular_strength_consumer_sample_count = 0;
        let mut specular_strength_consumer_opcodes = BTreeMap::new();
        let mut specular_strength_shader_traces = BTreeMap::new();
        let mut specular_strength_terminal_unclassified_pixel_shaders = BTreeSet::new();
        let mut gbuffer1_sample_count = 0;
        let mut gbuffer1_pixel_shaders = BTreeSet::new();
        let mut gbuffer1_lane_sample_counts = BTreeMap::new();
        let mut gbuffer1_lane_consumer_opcodes = BTreeMap::new();
        let mut gbuffer1_lane_o0_rgb_reach_counts = BTreeMap::new();
        let mut gbuffer1_x_consumer_signatures = BTreeMap::new();
        let mut gbuffer1_x_resource_join_counts = BTreeMap::new();
        let mut gbuffer1_x_terminal_multiplier_count = 0;
        let mut gbuffer1_x_terminal_multiplier_o0_rgb_reach_count = 0;
        let mut gbuffer1_x_terminal_multiplier_resource_counts = BTreeMap::new();
        let mut gbuffer1_x_post_multiplier_consumer_signatures = BTreeMap::new();
        let mut gbuffer1_x_post_multiplier_resource_counts = BTreeMap::new();
        let mut gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders = Vec::new();
        let mut gbuffer1_x_pixel_shaders = BTreeSet::new();
        let mut gbuffer1_x_representative_pixel_shaders = Vec::new();
        let mut gbuffer1_consumer_representatives = Vec::new();

        for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
            let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
                format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
            })?;
            let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
            let bindings = dxbc_texture_bindings(&lines);
            for (line_index, line) in lines.iter().enumerate() {
                if dxbc_sample_texture_name(line, &bindings).as_deref()
                    == Some("g_SamplerGBuffer1")
                {
                    gbuffer1_sample_count += 1;
                    gbuffer1_pixel_shaders.insert(shader_index);
                    for lane in b"xyzw" {
                        let Some(destination) =
                            dxbc_sample_texture_physical_lane_destination(line, *lane)
                        else {
                            continue;
                        };
                        let lane_name = (*lane as char).to_string();
                        *gbuffer1_lane_sample_counts
                            .entry(lane_name.clone())
                            .or_default() += 1;
                        let direct_consumers = dxbc_direct_consumer_instruction_indices(
                            &lines,
                            line_index,
                            &destination,
                        );
                        let opcode_counts = gbuffer1_lane_consumer_opcodes
                            .entry(lane_name.clone())
                            .or_insert_with(BTreeMap::new);
                        for opcode in direct_consumers
                            .iter()
                            .filter_map(|(_, instruction)| instruction.split_whitespace().next())
                        {
                            *opcode_counts.entry(opcode.to_string()).or_default() += 1;
                        }
                        if *lane == b'x' {
                            gbuffer1_x_pixel_shaders.insert(shader_index);
                            let signature = direct_consumers
                                .iter()
                                .filter_map(|(_, instruction)| {
                                    instruction.split_whitespace().next()
                                })
                                .collect::<Vec<_>>()
                                .join(" -> ");
                            *gbuffer1_x_consumer_signatures.entry(signature).or_default() += 1;
                            if gbuffer1_x_representative_pixel_shaders.len() < 8 {
                                gbuffer1_x_representative_pixel_shaders.push(shader_index);
                            }
                            let joined_resources = lines
                                .iter()
                                .enumerate()
                                .filter_map(|(resource_index, resource_line)| {
                                    let resource_name =
                                        dxbc_sample_texture_name(resource_line, &bindings)?;
                                    if resource_name == "g_SamplerGBuffer1" {
                                        return None;
                                    }
                                    let resource_destination =
                                        dxbc_instruction_destination(resource_line)?;
                                    let join_index = dxbc_two_producers_join_index(
                                        &lines,
                                        line_index,
                                        &destination,
                                        resource_index,
                                        resource_destination,
                                    )?;
                                    Some((resource_name, join_index))
                                })
                                .fold(
                                    BTreeMap::<String, usize>::new(),
                                    |mut joins, (resource_name, join_index)| {
                                        joins
                                            .entry(resource_name)
                                            .and_modify(|current| {
                                                *current = (*current).min(join_index)
                                            })
                                            .or_insert(join_index);
                                        joins
                                    },
                                );
                            for (resource_name, _) in joined_resources {
                                *gbuffer1_x_resource_join_counts
                                    .entry(resource_name)
                                    .or_default() += 1;
                            }

                            let terminal_multiplier = direct_consumers
                                .last()
                                .and_then(|(consumer_index, consumer)| {
                                    let shaped_x =
                                        dxbc_componentwise_tainted_destinations(consumer, &destination);
                                    (shaped_x.len() == 1)
                                        .then_some((consumer_index, shaped_x.into_iter().next()?))
                                })
                                .and_then(|(consumer_index, shaped_x)| {
                                    dxbc_terminal_rgb_multiplier_after(
                                        &lines,
                                        *consumer_index,
                                        &shaped_x,
                                    )
                                });
                            if let Some((multiplier_index, multiplier_destination, other_source)) =
                                terminal_multiplier
                            {
                                gbuffer1_x_terminal_multiplier_count += 1;
                                if ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                                    dxbc_register_reaches_output(
                                        &lines,
                                        multiplier_index,
                                        &multiplier_destination,
                                        output,
                                    )
                                }) {
                                    gbuffer1_x_terminal_multiplier_o0_rgb_reach_count += 1;
                                }
                                for resource_name in dxbc_resources_reaching_operands(
                                    &lines,
                                    &bindings,
                                    multiplier_index,
                                    &[other_source.as_str()],
                                ) {
                                    *gbuffer1_x_terminal_multiplier_resource_counts
                                        .entry(resource_name)
                                        .or_default() += 1;
                                }
                                if let Some((consumer_index, consumer)) =
                                    dxbc_direct_consumer_instruction_indices(
                                        &lines,
                                        multiplier_index,
                                        &multiplier_destination,
                                    )
                                    .first()
                                    .copied()
                                {
                                    let opcode = consumer
                                        .split_whitespace()
                                        .next()
                                        .unwrap_or("unknown")
                                        .to_string();
                                    *gbuffer1_x_post_multiplier_consumer_signatures
                                        .entry(opcode)
                                        .or_default() += 1;
                                    if let Some((_, raw_operands)) = consumer.split_once(' ') {
                                        let post_sources = split_instruction_operands(raw_operands)
                                            .into_iter()
                                            .skip(1)
                                            .filter(|source| {
                                                !register_components_overlap(
                                                    source.trim_matches(|character| {
                                                        matches!(character, '-' | '|' | '(' | ')')
                                                    }),
                                                    &multiplier_destination,
                                                )
                                            })
                                            .collect::<Vec<_>>();
                                        for resource_name in dxbc_resources_reaching_operands(
                                            &lines,
                                            &bindings,
                                            consumer_index,
                                            &post_sources,
                                        ) {
                                            *gbuffer1_x_post_multiplier_resource_counts
                                                .entry(resource_name)
                                                .or_default() += 1;
                                        }
                                    }
                                }
                            } else {
                                gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders
                                    .push(shader_index);
                            }
                        }
                        let reaches_o0_rgb = ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                            dxbc_register_reaches_output(
                                &lines,
                                line_index,
                                &destination,
                                output,
                            )
                        });
                        if reaches_o0_rgb {
                            *gbuffer1_lane_o0_rgb_reach_counts
                                .entry(lane_name.clone())
                                .or_default() += 1;
                        }
                        if gbuffer1_consumer_representatives.len() < 16 {
                            let first_consumer = direct_consumers
                                .first()
                                .map(|(index, _)| *index)
                                .unwrap_or(line_index);
                            let last_consumer = direct_consumers
                                .last()
                                .map(|(index, _)| *index)
                                .unwrap_or(first_consumer);
                            let trace_start = first_consumer.saturating_sub(4).max(line_index);
                            let trace_end = (last_consumer + 32).min(lines.len());
                            gbuffer1_consumer_representatives.push(format!(
                                "ps{shader_index} GBuffer1.{lane_name} producer line {line_index}:\\n{line}\\nconsumer context lines {trace_start}..{trace_end}:\\n{}",
                                lines[trace_start..trace_end].join("\\n")
                            ));
                        }
                    }
                }
                if dxbc_sample_texture_name(line, &bindings).as_deref() != Some("g_SamplerTable.T")
                {
                    continue;
                }
                let Some(coordinates) = dxbc_sample_texture_coordinates(line) else {
                    continue;
                };
                let Some(x) = dxbc_literal_provenance_at(&lines, line_index, coordinates, 0) else {
                    continue;
                };
                if x == "4.500000" {
                    let Some(roughness_destination) =
                        dxbc_sample_texture_physical_lane_destination(line, b'x')
                    else {
                        continue;
                    };
                    roughness_sample_count += 1;
                    roughness_pixel_shaders.insert(shader_index);
                    if dxbc_register_reaches_output(
                        &lines,
                        line_index,
                        &roughness_destination,
                        "o1.y",
                    ) {
                        roughness_o1_y_reach_count += 1;
                    }
                    let indexed_direct_instructions = dxbc_direct_consumer_instruction_indices(
                        &lines,
                        line_index,
                        &roughness_destination,
                    );
                    let direct_instructions = indexed_direct_instructions
                        .iter()
                        .map(|(_, instruction)| *instruction)
                        .collect::<Vec<_>>();
                    if !direct_instructions.is_empty() {
                        roughness_consumer_sample_count += 1;
                    }
                    for opcode in direct_instructions
                        .iter()
                        .filter_map(|instruction| instruction.split_whitespace().next())
                    {
                        *roughness_consumer_opcodes
                            .entry(opcode.to_string())
                            .or_default() += 1;
                    }
                    if roughness_consumer_representatives.len() < 16 {
                        let first_consumer = indexed_direct_instructions
                            .first()
                            .map(|(index, _)| *index)
                            .unwrap_or(line_index);
                        let last_consumer = indexed_direct_instructions
                            .last()
                            .map(|(index, _)| *index)
                            .unwrap_or(first_consumer);
                        let trace_start = first_consumer.saturating_sub(4).max(line_index);
                        let trace_end = (last_consumer + 25).min(lines.len());
                        roughness_consumer_representatives.push(format!(
                            "ps{shader_index} producer line {line_index}:\n{line}\nconsumer context lines {trace_start}..{trace_end}:\n{}",
                            lines[trace_start..trace_end].join("\n")
                        ));
                    }
                    continue;
                }
                let Some(strength_destination) =
                    dxbc_sample_texture_physical_lane_destination(line, b'w')
                else {
                    continue;
                };
                let (sample_count, consumer_sample_count, consumer_opcodes) = match x.as_str() {
                    "0.500000" => (
                        &mut gloss_sample_count,
                        &mut gloss_consumer_sample_count,
                        &mut gloss_consumer_opcodes,
                    ),
                    "1.500000" => (
                        &mut specular_strength_sample_count,
                        &mut specular_strength_consumer_sample_count,
                        &mut specular_strength_consumer_opcodes,
                    ),
                    _ => continue,
                };
                *sample_count += 1;
                if x == "1.500000" {
                    specular_strength_pixel_shaders.insert(shader_index);
                }
                let indexed_direct_instructions = dxbc_direct_consumer_instruction_indices(
                    &lines,
                    line_index,
                    &strength_destination,
                );
                let direct_instructions = indexed_direct_instructions
                    .iter()
                    .map(|(_, instruction)| *instruction)
                    .collect::<Vec<_>>();
                let direct_opcodes = direct_instructions
                    .iter()
                    .filter_map(|line| line.split_whitespace().next())
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if !direct_opcodes.is_empty() {
                    *consumer_sample_count += 1;
                }
                if x == "1.500000" {
                    let sampled_resources = lines
                        .iter()
                        .filter_map(|line| dxbc_sample_texture_name(line, &bindings))
                        .collect::<BTreeSet<_>>();
                    let terminal = indexed_direct_instructions
                        .first()
                        .and_then(|(product_index, product_line)| {
                            let operands = product_line
                                .split_once(' ')
                                .map(|(opcode, operands)| {
                                    (opcode, split_instruction_operands(operands))
                                })?;
                            (operands.0 == "mul" && operands.1.len() == 3).then_some((
                                *product_index,
                                operands.1,
                            ))
                        })
                        .and_then(|(product_index, operands)| {
                            let product_destination = operands.first()?.to_string();
                            let other_operands = operands
                                .iter()
                                .skip(1)
                                .copied()
                                .filter(|operand| {
                                    !register_components_overlap(
                                        operand.trim_matches(|character| {
                                            matches!(character, '-' | '|' | '(' | ')')
                                        }),
                                        &strength_destination,
                                    )
                                })
                                .collect::<Vec<_>>();
                            (other_operands.len() == 1).then_some((
                                product_index,
                                product_destination,
                                other_operands,
                            ))
                        });
                    if let Some((product_index, product_destination, other_operands)) = terminal {
                        let product_o0_rgb_reaches =
                            ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                                dxbc_register_reaches_output(
                                    &lines,
                                    product_index,
                                    &product_destination,
                                    output,
                                )
                            });
                        let product_other_resources = dxbc_resources_reaching_operands(
                            &lines,
                            &bindings,
                            product_index,
                            &other_operands,
                        );
                        let first_post_product_consumer_opcode =
                            dxbc_direct_consumer_instruction_indices(
                                &lines,
                                product_index,
                                &product_destination,
                            )
                            .first()
                            .and_then(|(_, instruction)| instruction.split_whitespace().next())
                            .map(str::to_string);
                        let terminal_rgb_multiplier = dxbc_tainted_terminal_rgb_multiplier_after(
                            &lines,
                            product_index,
                            &product_destination,
                        );
                        let terminal_rgb_multiplier_o0_reaches = terminal_rgb_multiplier
                            .as_ref()
                            .map(|(multiplier_index, multiplier_destination, _)| {
                                ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                                    dxbc_register_reaches_output(
                                        &lines,
                                        *multiplier_index,
                                        multiplier_destination,
                                        output,
                                    )
                                })
                            });
                        let terminal_rgb_multiplier_resources = terminal_rgb_multiplier
                            .as_ref()
                            .map(|(multiplier_index, _, other_source)| {
                                dxbc_resources_reaching_operands(
                                    &lines,
                                    &bindings,
                                    *multiplier_index,
                                    &[other_source.as_str()],
                                )
                            })
                            .unwrap_or_default();
                        let post_terminal_multiplier = terminal_rgb_multiplier.as_ref().and_then(
                            |(multiplier_index, multiplier_destination, _)| {
                                dxbc_direct_consumer_instruction_indices(
                                    &lines,
                                    *multiplier_index,
                                    multiplier_destination,
                                )
                                .first()
                                .copied()
                            },
                        );
                        let post_terminal_multiplier_opcode = post_terminal_multiplier
                            .and_then(|(_, instruction)| instruction.split_whitespace().next())
                            .map(str::to_string);
                        let post_terminal_multiplier_resources = post_terminal_multiplier
                            .and_then(|(consumer_index, consumer)| {
                                let (_, raw_operands) = consumer.split_once(' ')?;
                                let post_sources = split_instruction_operands(raw_operands)
                                    .into_iter()
                                    .skip(1)
                                    .filter(|source| {
                                        terminal_rgb_multiplier.as_ref().is_some_and(
                                            |(_, multiplier_destination, _)| {
                                                !register_components_overlap(
                                                    source.trim_matches(|character| {
                                                        matches!(character, '-' | '|' | '(' | ')')
                                                    }),
                                                    multiplier_destination,
                                                )
                                            },
                                        )
                                    })
                                    .collect::<Vec<_>>();
                                Some(dxbc_resources_reaching_operands(
                                    &lines,
                                    &bindings,
                                    consumer_index,
                                    &post_sources,
                                ))
                            })
                            .unwrap_or_default();
                        let trace_end = (product_index + 18).min(lines.len());
                        let trace = MaterialStrengthDxbcSpecularShaderTrace {
                            sampled_resources,
                            product_o0_rgb_reaches,
                            product_other_resources,
                            first_post_product_consumer_opcode,
                            has_fifth_root_shaping: dxbc_has_tainted_fifth_root_shaping(
                                &lines,
                                product_index,
                                &product_destination,
                            ),
                            terminal_rgb_multiplier_o0_reaches,
                            terminal_rgb_multiplier_resources,
                            post_terminal_multiplier_opcode,
                            post_terminal_multiplier_resources,
                            dynamic_emissive_o0_rgb_reaches:
                                dxbc_dynamic_emissive_reaches_o0_rgb(&lines),
                            dynamic_emissive_table_join_o0_rgb_reaches:
                                dxbc_dynamic_emissive_table_join_reaches_o0_rgb(&lines),
                            dynamic_emissive_luminance_scale:
                                dxbc_dynamic_emissive_luminance_scale_o0_rgb_trace(&lines),
                            instance_mul_color_o0_rgb_reaches:
                                dxbc_instance_parameter_member_reaches_o0_rgb(
                                    &lines,
                                    "float4 m_MulColor;",
                                    0,
                                    0,
                                ),
                            instance_env_parameter_o0_rgb_reaches:
                                dxbc_instance_parameter_member_reaches_o0_rgb(
                                    &lines,
                                    "float4 m_EnvParameter;",
                                    16,
                                    1,
                                ),
                            instance_camera_diffuse_specular_o0_rgb_reaches:
                                dxbc_instance_parameter_member_reaches_o0_rgb(
                                    &lines,
                                    "float4 m_DiffuseSpecular;",
                                    32,
                                    2,
                                ),
                            instance_camera_rim_o0_rgb_reaches:
                                dxbc_instance_parameter_member_reaches_o0_rgb(
                                    &lines,
                                    "float4 m_Rim;",
                                    48,
                                    3,
                                ),
                            representative_trace: format!(
                                "ps{shader_index} producer line {line_index}:\n{line}\nterminal context lines {product_index}..{trace_end}:\n{}",
                                lines[product_index..trace_end].join("\n")
                            ),
                        };
                        assert!(
                            specular_strength_shader_traces
                                .insert(shader_index, trace)
                                .is_none(),
                            "{shader_package_name} ps{shader_index} has multiple SpecularStrength samples"
                        );
                    } else {
                        specular_strength_terminal_unclassified_pixel_shaders
                            .insert(shader_index);
                    }
                }
                if x == "0.500000" {
                    gloss_pixel_shaders.insert(shader_index);
                    let reaches_o1_y = dxbc_register_reaches_output(
                        &lines,
                        line_index,
                        &strength_destination,
                        "o1.y",
                    );
                    if reaches_o1_y {
                        gloss_o1_y_reach_count += 1;
                    }
                    let reaches_o0_rgb = ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                        dxbc_register_reaches_output(
                            &lines,
                            line_index,
                            &strength_destination,
                            output,
                        )
                    });
                    if reaches_o0_rgb {
                        gloss_o0_rgb_reach_count += 1;
                    }
                    let power_chains = indexed_direct_instructions
                        .iter()
                        .filter_map(|(consumer_index, _)| {
                            dxbc_log_mul_exp_power_destination(
                                &lines,
                                *consumer_index,
                                &strength_destination,
                            )
                        })
                        .collect::<Vec<_>>();
                    let power_reaches_o0_rgb = power_chains.iter().any(
                        |(exp_index, power_destination)| {
                            ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                                dxbc_register_reaches_output(
                                    &lines,
                                    *exp_index,
                                    power_destination,
                                    output,
                                )
                            })
                        },
                    );
                    let camera_reflection_power_chain_count = indexed_direct_instructions
                        .iter()
                        .filter(|(consumer_index, _)| {
                            dxbc_gloss_power_uses_camera_reflection_dot(
                                &lines,
                                *consumer_index,
                                &strength_destination,
                            )
                        })
                        .count();
                    let camera_reflection_lobe_count = indexed_direct_instructions
                        .iter()
                        .filter(|(consumer_index, _)| {
                            dxbc_gloss_power_uses_camera_reflection_dot(
                                &lines,
                                *consumer_index,
                                &strength_destination,
                            ) && dxbc_gloss_power_has_visibility_envelope(
                                &lines,
                                *consumer_index,
                                &strength_destination,
                            )
                        })
                        .count();
                    let cube_lod_samples = dxbc_tainted_cube_lod_sample_indices(
                        &lines,
                        line_index,
                        &strength_destination,
                    );
                    let cube_lod_sample_count = cube_lod_samples.len();
                    let cube_sample_hdr_decode_count = cube_lod_samples
                        .iter()
                        .filter(|sample_index| {
                            dxbc_cube_sample_has_squared_alpha_decode(&lines, **sample_index)
                        })
                        .count();
                    let cube_sample_o0_rgb_reach_count = cube_lod_samples
                        .iter()
                        .filter(|sample_index| {
                            let destination = lines
                                .get(**sample_index)
                                .and_then(|line| line.split_once(' '))
                                .map(|(_, operands)| split_instruction_operands(operands))
                                .and_then(|operands| operands.first().copied());
                            destination.is_some_and(|destination| {
                                let outputs = dxbc_register_reached_outputs(
                                    &lines,
                                    **sample_index,
                                    destination,
                                );
                                ["o0.x", "o0.y", "o0.z"]
                                    .into_iter()
                                    .any(|output| outputs.contains(output))
                            })
                        })
                        .count();
                    let ambient_slot = shader
                        .scalar_parameters
                        .iter()
                        .find(|parameter| parameter.name == "g_AmbientParam")
                        .map(|parameter| parameter.slot);
                    let cube_location_sources = ambient_slot
                        .map(|ambient_slot| {
                            cube_lod_samples
                                .iter()
                                .filter_map(|sample_index| {
                                    dxbc_cube_sample_environment_location_lane(
                                        &lines,
                                        *sample_index,
                                        ambient_slot,
                                    )
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let cube_current_location_sample_count = cube_location_sources
                        .iter()
                        .filter(|source| **source == "current")
                        .count();
                    let cube_previous_location_sample_count = cube_location_sources
                        .iter()
                        .filter(|source| **source == "previous")
                        .count();
                    let ambient_location_interpolation_count = ambient_slot.is_some_and(|slot| {
                        dxbc_has_ambient_location_interpolation(&lines, slot)
                    });
                    let ambient_reflection_scale_offset_count = ambient_slot.is_some_and(|slot| {
                        dxbc_has_ambient_reflection_scale_offset(&lines, slot)
                    });
                    let ambient_bake_light_composition_count = ambient_slot.is_some_and(|slot| {
                        dxbc_has_ambient_bake_light_composition(&lines, slot)
                    });
                    let specular_strength_samples = dxbc_table_lane_samples(
                        &lines,
                        &bindings,
                        "1.500000",
                        b'w',
                    );
                    let environment_specular_strength_join_count =
                        !specular_strength_samples.is_empty()
                            && cube_lod_samples.iter().any(|cube_sample_index| {
                                let cube_destination = dxbc_instruction_destination(
                                    lines[*cube_sample_index],
                                );
                                cube_destination.is_some_and(|cube_destination| {
                                    specular_strength_samples.iter().any(
                                        |(strength_sample_index, strength_destination)| {
                                            dxbc_two_producers_join(
                                                &lines,
                                                *cube_sample_index,
                                                cube_destination,
                                                *strength_sample_index,
                                                strength_destination,
                                            )
                                        },
                                    )
                                })
                            });
                    if !specular_strength_samples.is_empty() {
                        if cube_lod_sample_count > 0 {
                            gloss_cube_specular_strength_pixel_shader_count += 1;
                        } else {
                            gloss_non_cube_specular_strength_pixel_shader_count += 1;
                        }
                    }
                    let environment_blend_source =
                        dxbc_gloss_environment_blend_source(&lines, &bindings);
                    gloss_power_chain_count += power_chains.len();
                    gloss_power_o0_rgb_reach_count += usize::from(power_reaches_o0_rgb);
                    gloss_camera_reflection_power_chain_count +=
                        camera_reflection_power_chain_count;
                    gloss_camera_reflection_lobe_count += camera_reflection_lobe_count;
                    gloss_cube_lod_sample_count += cube_lod_sample_count;
                    gloss_cube_sample_hdr_decode_count += cube_sample_hdr_decode_count;
                    gloss_cube_sample_o0_rgb_reach_count += cube_sample_o0_rgb_reach_count;
                    gloss_cube_current_location_sample_count +=
                        cube_current_location_sample_count;
                    gloss_cube_previous_location_sample_count +=
                        cube_previous_location_sample_count;
                    gloss_ambient_location_interpolation_count +=
                        usize::from(ambient_location_interpolation_count);
                    gloss_ambient_reflection_scale_offset_count +=
                        usize::from(ambient_reflection_scale_offset_count);
                    gloss_ambient_bake_light_composition_count +=
                        usize::from(ambient_bake_light_composition_count);
                    if cube_lod_sample_count > 0 && !ambient_bake_light_composition_count {
                        gloss_ambient_bake_light_unclassified_pixel_shaders.insert(shader_index);
                    }
                    gloss_environment_specular_strength_join_count +=
                        usize::from(environment_specular_strength_join_count);
                    if cube_lod_sample_count > 0
                        && !specular_strength_samples.is_empty()
                        && !environment_specular_strength_join_count
                    {
                        gloss_environment_specular_strength_unjoined_pixel_shaders
                            .insert(shader_index);
                    }
                    gloss_texcoord4_w_environment_blend_count += usize::from(
                        environment_blend_source == Some("texcoord4.w"),
                    );
                    gloss_gbuffer1_w_environment_blend_count += usize::from(
                        environment_blend_source == Some("gbuffer1.w"),
                    );
                    if cube_lod_sample_count > 0 && environment_blend_source.is_none() {
                        gloss_environment_blend_unclassified_pixel_shaders.insert(shader_index);
                    }
                    if camera_reflection_power_chain_count > camera_reflection_lobe_count {
                        gloss_camera_reflection_lobe_unclassified_pixel_shaders
                            .insert(shader_index);
                    }
                    let opcode_sequence = direct_opcodes.join(" -> ");
                    let literal_constants = direct_instructions
                        .iter()
                        .flat_map(|instruction| dxbc_literal_operands(instruction))
                        .collect::<Vec<_>>();
                    let consumer_signature = format!(
                        "{opcode_sequence} | {}",
                        if literal_constants.is_empty() {
                            "no literals".to_string()
                        } else {
                            literal_constants.join(", ")
                        }
                    );
                    *gloss_consumer_opcode_sequences
                        .entry(opcode_sequence.clone())
                        .or_default() += 1;
                    let class = gloss_consumer_classes
                        .entry(consumer_signature.clone())
                        .or_insert_with(|| {
                            MaterialStrengthDxbcGlossConsumerClassAccumulator {
                                opcode_sequence,
                                literal_constants,
                                ..Default::default()
                            }
                        });
                    class.sample_count += 1;
                    class.o1_y_reach_count += usize::from(reaches_o1_y);
                    class.o0_rgb_reach_count += usize::from(reaches_o0_rgb);
                    class.power_chain_count += power_chains.len();
                    class.power_o0_rgb_reach_count += usize::from(power_reaches_o0_rgb);
                    class.camera_reflection_power_chain_count +=
                        camera_reflection_power_chain_count;
                    class.camera_reflection_lobe_count += camera_reflection_lobe_count;
                    class.cube_lod_sample_count += cube_lod_sample_count;
                    class.cube_sample_hdr_decode_count += cube_sample_hdr_decode_count;
                    class.cube_sample_o0_rgb_reach_count += cube_sample_o0_rgb_reach_count;
                    class.cube_current_location_sample_count +=
                        cube_current_location_sample_count;
                    class.cube_previous_location_sample_count +=
                        cube_previous_location_sample_count;
                    class.ambient_location_interpolation_count +=
                        usize::from(ambient_location_interpolation_count);
                    class.ambient_reflection_scale_offset_count +=
                        usize::from(ambient_reflection_scale_offset_count);
                    class.ambient_bake_light_composition_count +=
                        usize::from(ambient_bake_light_composition_count);
                    class.environment_specular_strength_join_count +=
                        usize::from(environment_specular_strength_join_count);
                    class.texcoord4_w_environment_blend_count +=
                        usize::from(environment_blend_source == Some("texcoord4.w"));
                    class.gbuffer1_w_environment_blend_count +=
                        usize::from(environment_blend_source == Some("gbuffer1.w"));
                    class.pixel_shaders.insert(shader_index);
                    if class.representative_trace.is_empty() {
                        let first_consumer = indexed_direct_instructions
                            .first()
                            .map(|(index, _)| *index)
                            .unwrap_or(line_index);
                        let last_consumer = indexed_direct_instructions
                            .last()
                            .map(|(index, _)| *index)
                            .unwrap_or(first_consumer);
                        let trace_start = first_consumer.saturating_sub(4).max(line_index);
                        let trace_end = (last_consumer + 17).min(lines.len());
                        class.representative_trace = format!(
                            "ps{shader_index} producer line {line_index}:\n{}\nconsumer context lines {trace_start}..{trace_end}:\n{}",
                            lines[line_index],
                            lines[trace_start..trace_end].join("\n")
                        );
                    }
                    if let Some(previous) = gloss_consumer_class_by_pixel_shader
                        .insert(shader_index, consumer_signature.clone())
                    {
                        assert_eq!(
                            previous, consumer_signature,
                            "{shader_package_name} ps{shader_index} has multiple Gloss consumer classes"
                        );
                    }
                    if gloss_consumer_representatives.len() < 32 {
                        gloss_consumer_representatives.push(format!(
                            "ps{shader_index}: {line}\n{}",
                            direct_instructions.join("\n")
                        ));
                    }
                }
                for opcode in direct_opcodes {
                    *consumer_opcodes.entry(opcode).or_default() += 1;
                }
            }
        }

        let mut specular_strength_composition_classes = BTreeMap::<
            String,
            MaterialStrengthDxbcSpecularClassAccumulator,
        >::new();
        let mut specular_strength_class_by_pixel_shader = BTreeMap::new();
        for (pixel_shader, trace) in specular_strength_shader_traces {
            let has_gloss = gloss_pixel_shaders.contains(&pixel_shader);
            let has_cube = trace
                .sampled_resources
                .contains("g_SamplerReflectionArray.T");
            let base_class_name = match (has_cube, has_gloss) {
                (true, true) => "cube_gloss",
                (true, false) => "cube_no_gloss",
                (false, true) => "non_cube_gloss",
                (false, false) => "no_gloss",
            };
            let first_post_product_shape = trace
                .first_post_product_consumer_opcode
                .as_deref()
                .unwrap_or("terminal");
            let class_name = format!("{base_class_name}_first_{first_post_product_shape}");
            specular_strength_class_by_pixel_shader.insert(pixel_shader, class_name.clone());
            let class = specular_strength_composition_classes
                .entry(class_name)
                .or_default();
            class.pixel_shaders.insert(pixel_shader);
            class.product_o0_rgb_reach_count += usize::from(trace.product_o0_rgb_reaches);
            for resource_name in trace.product_other_resources {
                *class
                    .product_other_resource_counts
                    .entry(resource_name)
                    .or_default() += 1;
            }
            if let Some(opcode) = trace.first_post_product_consumer_opcode {
                *class
                    .first_post_product_consumer_opcodes
                    .entry(opcode)
                    .or_default() += 1;
            }
            class.fifth_root_shaping_count += usize::from(trace.has_fifth_root_shaping);
            if let Some(reaches_o0) = trace.terminal_rgb_multiplier_o0_reaches {
                class.terminal_rgb_multiplier_count += 1;
                class.terminal_rgb_multiplier_o0_reach_count += usize::from(reaches_o0);
            } else if has_cube && has_gloss {
                class
                    .terminal_rgb_multiplier_unclassified_pixel_shaders
                    .insert(pixel_shader);
            }
            for resource_name in trace.terminal_rgb_multiplier_resources {
                *class
                    .terminal_rgb_multiplier_resource_counts
                    .entry(resource_name)
                    .or_default() += 1;
            }
            if let Some(opcode) = trace.post_terminal_multiplier_opcode {
                *class
                    .post_terminal_multiplier_opcodes
                    .entry(opcode)
                    .or_default() += 1;
            }
            for resource_name in trace.post_terminal_multiplier_resources {
                *class
                    .post_terminal_multiplier_resource_counts
                    .entry(resource_name)
                    .or_default() += 1;
            }
            class.dynamic_emissive_o0_rgb_reach_count +=
                usize::from(trace.dynamic_emissive_o0_rgb_reaches);
            class.dynamic_emissive_table_join_o0_rgb_reach_count +=
                usize::from(trace.dynamic_emissive_table_join_o0_rgb_reaches);
            class.dynamic_emissive_luminance_scale_o0_rgb_reach_count +=
                usize::from(trace.dynamic_emissive_luminance_scale.is_some());
            if let Some(luminance_scale) = &trace.dynamic_emissive_luminance_scale {
                *class
                    .dynamic_emissive_luminance_scale_composition_opcodes
                    .entry(luminance_scale.composition_opcode.clone())
                    .or_default() += 1;
                for resource_name in &luminance_scale.texture_resources {
                    *class
                        .dynamic_emissive_luminance_scale_texture_resource_counts
                        .entry(resource_name.clone())
                        .or_default() += 1;
                }
                for vector in &luminance_scale.constant_buffer_vectors {
                    *class
                        .dynamic_emissive_luminance_scale_constant_buffer_vector_counts
                        .entry(vector.clone())
                        .or_default() += 1;
                }
                if let Some(opcode) = &luminance_scale.source_o0_rgb_composition_opcode {
                    class.dynamic_emissive_luminance_source_o0_rgb_reach_count += 1;
                    *class
                        .dynamic_emissive_luminance_source_composition_opcodes
                        .entry(opcode.clone())
                        .or_default() += 1;
                }
            }
            if trace.dynamic_emissive_table_join_o0_rgb_reaches
                && trace.dynamic_emissive_luminance_scale.is_none()
            {
                class
                    .dynamic_emissive_luminance_scale_unclassified_pixel_shaders
                    .insert(pixel_shader);
            }
            class.instance_mul_color_o0_rgb_reach_count +=
                usize::from(trace.instance_mul_color_o0_rgb_reaches);
            class.instance_env_parameter_o0_rgb_reach_count +=
                usize::from(trace.instance_env_parameter_o0_rgb_reaches);
            class.instance_camera_diffuse_specular_o0_rgb_reach_count +=
                usize::from(trace.instance_camera_diffuse_specular_o0_rgb_reaches);
            class.instance_camera_rim_o0_rgb_reach_count +=
                usize::from(trace.instance_camera_rim_o0_rgb_reaches);
            if class.representative_trace.is_empty() {
                class.representative_trace = trace.representative_trace;
            }
        }

        let mut gbuffer1_x_node_count = 0;
        let mut gbuffer1_x_pass_ids = BTreeMap::new();
        let mut gbuffer1_x_material_key_sets = BTreeMap::new();
        for node in &package.nodes {
            let gbuffer1_x_passes =
                shader_pass_records_from_debug_text(&format!("{:#?}", node.passes))
                    .into_iter()
                    .filter(|(_, _, pixel_shader)| {
                        gbuffer1_x_pixel_shaders.contains(pixel_shader)
                    })
                    .collect::<Vec<_>>();
            if gbuffer1_x_passes.is_empty() {
                continue;
            }
            gbuffer1_x_node_count += 1;
            for (pass_id, _, _) in gbuffer1_x_passes {
                *gbuffer1_x_pass_ids
                    .entry(format!("0x{pass_id:08x}"))
                    .or_default() += 1;
            }
            let material_key_set = package
                .material_keys
                .iter()
                .zip(&node.material_keys)
                .map(|(key, value)| format!("{:08X}={value:08X}", key.id))
                .collect::<Vec<_>>()
                .join(",");
            *gbuffer1_x_material_key_sets
                .entry(material_key_set)
                .or_default() += 1;
        }

        for node in &package.nodes {
            let specular_passes =
                shader_pass_records_from_debug_text(&format!("{:#?}", node.passes))
                    .into_iter()
                    .filter_map(|(pass_id, _, pixel_shader)| {
                        specular_strength_class_by_pixel_shader
                            .get(&pixel_shader)
                            .cloned()
                            .map(|class| (pass_id, class))
                    })
                    .collect::<Vec<_>>();
            let node_classes = specular_passes
                .iter()
                .map(|(_, class)| class.clone())
                .collect::<BTreeSet<_>>();
            if node_classes.is_empty() {
                continue;
            }
            let material_key_set = package
                .material_keys
                .iter()
                .zip(&node.material_keys)
                .map(|(key, value)| format!("{:08X}={value:08X}", key.id))
                .collect::<Vec<_>>()
                .join(",");
            for node_class in node_classes {
                let class = specular_strength_composition_classes
                    .get_mut(&node_class)
                    .expect("SpecularStrength class must exist for mapped pixel shader");
                class.node_count += 1;
                for (pass_id, _) in specular_passes
                    .iter()
                    .filter(|(_, pass_class)| pass_class == &node_class)
                {
                    *class
                        .pass_ids
                        .entry(format!("0x{pass_id:08x}"))
                        .or_default() += 1;
                }
                *class
                    .material_key_sets
                    .entry(material_key_set.clone())
                    .or_default() += 1;
            }
        }

        let mut gloss_node_count = 0;
        let mut gloss_material_key_sets = BTreeMap::new();
        for node in &package.nodes {
            let gloss_passes =
                shader_pass_records_from_debug_text(&format!("{:#?}", node.passes))
                    .into_iter()
                    .filter_map(|(pass_id, _, pixel_shader)| {
                        gloss_consumer_class_by_pixel_shader
                            .get(&pixel_shader)
                            .cloned()
                            .map(|class| (pass_id, class))
                    })
                    .collect::<Vec<_>>();
            let node_classes = gloss_passes
                .iter()
                .map(|(_, class)| class.clone())
                .collect::<BTreeSet<_>>();
            if node_classes.is_empty() {
                continue;
            }
            gloss_node_count += 1;
            let material_key_set = package
                .material_keys
                .iter()
                .zip(&node.material_keys)
                .map(|(key, value)| format!("{:08X}={value:08X}", key.id))
                .collect::<Vec<_>>()
                .join(",");
            *gloss_material_key_sets
                .entry(material_key_set.clone())
                .or_default() += 1;
            for node_class in node_classes {
                let class = gloss_consumer_classes
                    .get_mut(&node_class)
                    .expect("Gloss consumer class must exist for mapped pixel shader");
                class.node_count += 1;
                for (pass_id, _) in gloss_passes
                    .iter()
                    .filter(|(_, pass_class)| pass_class == &node_class)
                {
                    *class
                        .pass_ids
                        .entry(format!("0x{pass_id:08x}"))
                        .or_default() += 1;
                }
                *class
                    .material_key_sets
                    .entry(material_key_set.clone())
                    .or_default() += 1;
            }
        }

        let gloss_consumer_classes = gloss_consumer_classes
            .into_iter()
            .map(|(consumer_signature, class)| {
                MaterialStrengthDxbcGlossConsumerClassAudit {
                    consumer_signature,
                    opcode_sequence: class.opcode_sequence,
                    literal_constants: class.literal_constants,
                    sample_count: class.sample_count,
                    o1_y_reach_count: class.o1_y_reach_count,
                    o0_rgb_reach_count: class.o0_rgb_reach_count,
                    power_chain_count: class.power_chain_count,
                    power_o0_rgb_reach_count: class.power_o0_rgb_reach_count,
                    camera_reflection_power_chain_count: class
                        .camera_reflection_power_chain_count,
                    camera_reflection_lobe_count: class.camera_reflection_lobe_count,
                    cube_lod_sample_count: class.cube_lod_sample_count,
                    cube_sample_hdr_decode_count: class.cube_sample_hdr_decode_count,
                    cube_sample_o0_rgb_reach_count: class.cube_sample_o0_rgb_reach_count,
                    cube_current_location_sample_count: class
                        .cube_current_location_sample_count,
                    cube_previous_location_sample_count: class
                        .cube_previous_location_sample_count,
                    ambient_location_interpolation_count: class
                        .ambient_location_interpolation_count,
                    ambient_reflection_scale_offset_count: class
                        .ambient_reflection_scale_offset_count,
                    ambient_bake_light_composition_count: class
                        .ambient_bake_light_composition_count,
                    environment_specular_strength_join_count: class
                        .environment_specular_strength_join_count,
                    texcoord4_w_environment_blend_count: class
                        .texcoord4_w_environment_blend_count,
                    gbuffer1_w_environment_blend_count: class.gbuffer1_w_environment_blend_count,
                    pixel_shader_count: class.pixel_shaders.len(),
                    node_count: class.node_count,
                    pass_ids: class.pass_ids,
                    material_key_sets: class.material_key_sets,
                    representative_pixel_shaders: class.pixel_shaders.into_iter().take(8).collect(),
                    representative_trace: class.representative_trace,
                }
            })
            .collect();
        let specular_strength_composition_classes = specular_strength_composition_classes
            .into_iter()
            .map(|(class_name, class)| MaterialStrengthDxbcSpecularClassAudit {
                class_name,
                pixel_shader_count: class.pixel_shaders.len(),
                product_o0_rgb_reach_count: class.product_o0_rgb_reach_count,
                product_other_resource_counts: class.product_other_resource_counts,
                first_post_product_consumer_opcodes: class
                    .first_post_product_consumer_opcodes,
                fifth_root_shaping_count: class.fifth_root_shaping_count,
                terminal_rgb_multiplier_count: class.terminal_rgb_multiplier_count,
                terminal_rgb_multiplier_o0_reach_count: class
                    .terminal_rgb_multiplier_o0_reach_count,
                terminal_rgb_multiplier_resource_counts: class
                    .terminal_rgb_multiplier_resource_counts,
                post_terminal_multiplier_opcodes: class.post_terminal_multiplier_opcodes,
                post_terminal_multiplier_resource_counts: class
                    .post_terminal_multiplier_resource_counts,
                terminal_rgb_multiplier_unclassified_pixel_shaders: class
                    .terminal_rgb_multiplier_unclassified_pixel_shaders
                    .into_iter()
                    .collect(),
                dynamic_emissive_o0_rgb_reach_count: class
                    .dynamic_emissive_o0_rgb_reach_count,
                dynamic_emissive_table_join_o0_rgb_reach_count: class
                    .dynamic_emissive_table_join_o0_rgb_reach_count,
                dynamic_emissive_luminance_scale_o0_rgb_reach_count: class
                    .dynamic_emissive_luminance_scale_o0_rgb_reach_count,
                dynamic_emissive_luminance_scale_composition_opcodes: class
                    .dynamic_emissive_luminance_scale_composition_opcodes,
                dynamic_emissive_luminance_scale_texture_resource_counts: class
                    .dynamic_emissive_luminance_scale_texture_resource_counts,
                dynamic_emissive_luminance_scale_constant_buffer_vector_counts: class
                    .dynamic_emissive_luminance_scale_constant_buffer_vector_counts,
                dynamic_emissive_luminance_source_o0_rgb_reach_count: class
                    .dynamic_emissive_luminance_source_o0_rgb_reach_count,
                dynamic_emissive_luminance_source_composition_opcodes: class
                    .dynamic_emissive_luminance_source_composition_opcodes,
                dynamic_emissive_luminance_scale_unclassified_pixel_shaders: class
                    .dynamic_emissive_luminance_scale_unclassified_pixel_shaders
                    .into_iter()
                    .collect(),
                instance_mul_color_o0_rgb_reach_count: class
                    .instance_mul_color_o0_rgb_reach_count,
                instance_env_parameter_o0_rgb_reach_count: class
                    .instance_env_parameter_o0_rgb_reach_count,
                instance_camera_diffuse_specular_o0_rgb_reach_count: class
                    .instance_camera_diffuse_specular_o0_rgb_reach_count,
                instance_camera_rim_o0_rgb_reach_count: class
                    .instance_camera_rim_o0_rgb_reach_count,
                node_count: class.node_count,
                pass_ids: class.pass_ids,
                material_key_sets: class.material_key_sets,
                representative_pixel_shaders: class
                    .pixel_shaders
                    .into_iter()
                    .take(8)
                    .collect(),
                representative_trace: class.representative_trace,
            })
            .collect();

        Ok(MaterialStrengthDxbcPackageAudit {
            shader_package_name: shader_package_name.to_string(),
            pixel_shader_count: package.pixel_shaders.len(),
            roughness_sample_count,
            roughness_pixel_shader_count: roughness_pixel_shaders.len(),
            roughness_consumer_sample_count,
            roughness_consumer_opcodes,
            roughness_o1_y_reach_count,
            roughness_consumer_representatives,
            gloss_sample_count,
            gloss_pixel_shader_count: gloss_pixel_shaders.len(),
            gloss_consumer_sample_count,
            gloss_consumer_opcodes,
            gloss_o1_y_reach_count,
            gloss_o0_rgb_reach_count,
            gloss_power_chain_count,
            gloss_power_o0_rgb_reach_count,
            gloss_camera_reflection_power_chain_count,
            gloss_camera_reflection_lobe_count,
            gloss_camera_reflection_lobe_unclassified_pixel_shaders:
                gloss_camera_reflection_lobe_unclassified_pixel_shaders
                    .into_iter()
                    .collect(),
            gloss_cube_lod_sample_count,
            gloss_cube_sample_hdr_decode_count,
            gloss_cube_sample_o0_rgb_reach_count,
            gloss_cube_current_location_sample_count,
            gloss_cube_previous_location_sample_count,
            gloss_ambient_location_interpolation_count,
            gloss_ambient_reflection_scale_offset_count,
            gloss_ambient_bake_light_composition_count,
            gloss_ambient_bake_light_unclassified_pixel_shaders:
                gloss_ambient_bake_light_unclassified_pixel_shaders
                    .into_iter()
                    .collect(),
            gloss_environment_specular_strength_join_count,
            gloss_environment_specular_strength_unjoined_pixel_shaders:
                gloss_environment_specular_strength_unjoined_pixel_shaders
                    .into_iter()
                    .collect(),
            gloss_cube_specular_strength_pixel_shader_count,
            gloss_non_cube_specular_strength_pixel_shader_count,
            gloss_texcoord4_w_environment_blend_count,
            gloss_gbuffer1_w_environment_blend_count,
            gloss_environment_blend_unclassified_pixel_shaders:
                gloss_environment_blend_unclassified_pixel_shaders
                    .into_iter()
                    .collect(),
            gloss_consumer_opcode_sequences,
            gloss_consumer_classes,
            gloss_consumer_representatives,
            gloss_node_count,
            gloss_material_key_sets,
            specular_strength_sample_count,
            specular_strength_pixel_shader_count: specular_strength_pixel_shaders.len(),
            specular_strength_without_gloss_pixel_shader_count: specular_strength_pixel_shaders
                .difference(&gloss_pixel_shaders)
                .count(),
            specular_strength_consumer_sample_count,
            specular_strength_consumer_opcodes,
            specular_strength_composition_classes,
            specular_strength_terminal_unclassified_pixel_shaders:
                specular_strength_terminal_unclassified_pixel_shaders
                    .into_iter()
                    .collect(),
            gbuffer1_sample_count,
            gbuffer1_pixel_shader_count: gbuffer1_pixel_shaders.len(),
            gbuffer1_lane_sample_counts,
            gbuffer1_lane_consumer_opcodes,
            gbuffer1_lane_o0_rgb_reach_counts,
            gbuffer1_x_consumer_signatures,
            gbuffer1_x_resource_join_counts,
            gbuffer1_x_terminal_multiplier_count,
            gbuffer1_x_terminal_multiplier_o0_rgb_reach_count,
            gbuffer1_x_terminal_multiplier_resource_counts,
            gbuffer1_x_post_multiplier_consumer_signatures,
            gbuffer1_x_post_multiplier_resource_counts,
            gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders,
            gbuffer1_x_node_count,
            gbuffer1_x_pass_ids,
            gbuffer1_x_material_key_sets,
            gbuffer1_x_representative_pixel_shaders,
            gbuffer1_consumer_representatives,
        })
    })
    .collect()
}

#[cfg(windows)]
fn dxbc_sample_texture_coordinates(line: &str) -> Option<&str> {
    let (_, raw_operands) = line.split_once(' ')?;
    let operands = split_instruction_operands(raw_operands);
    operands.get(1).copied()
}

#[cfg(windows)]
fn dxbc_sample_texture_physical_lane_destination(line: &str, physical_lane: u8) -> Option<String> {
    let (_, raw_operands) = line.split_once(' ')?;
    let operands = split_instruction_operands(raw_operands);
    let destination = *operands.first()?;
    let resource_operand = *operands.get(2)?;
    let resource_swizzle = resource_operand
        .split_once('.')
        .map(|(_, swizzle)| swizzle)
        .unwrap_or("xyzw")
        .as_bytes();
    dxbc_operand_components(destination)
        .into_iter()
        .find(|component| {
            component
                .as_bytes()
                .last()
                .and_then(|lane| b"xyzw".iter().position(|candidate| candidate == lane))
                .is_some_and(|lane| resource_swizzle.get(lane) == Some(&physical_lane))
        })
}

#[cfg(windows)]
fn dxbc_direct_consumer_instruction_indices<'a>(
    lines: &'a [&str],
    producer_index: usize,
    register: &str,
) -> Vec<(usize, &'a str)> {
    let mut instructions = Vec::new();
    for (line_index, line) in lines.iter().enumerate().skip(producer_index + 1) {
        let Some((_, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let reads = operands.iter().skip(1).any(|operand| {
            register_components_overlap(
                operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
                register,
            )
        });
        let writes = operands
            .first()
            .is_some_and(|destination| register_components_overlap(destination, register));
        if reads {
            instructions.push((line_index, *line));
        }
        if writes {
            break;
        }
    }
    instructions
}

#[cfg(windows)]
fn dxbc_componentwise_tainted_destinations(line: &str, register: &str) -> Vec<String> {
    let Some((_, raw_operands)) = line.split_once(' ') else {
        return Vec::new();
    };
    let operands = split_instruction_operands(raw_operands);
    let Some(destination) = operands.first() else {
        return Vec::new();
    };
    let destination_components = dxbc_operand_components(destination);
    destination_components
        .into_iter()
        .enumerate()
        .filter_map(|(lane, destination_component)| {
            let reads_register = operands.iter().skip(1).any(|source| {
                let source_components = dxbc_operand_components(source);
                let source_component = match source_components.as_slice() {
                    [] => return false,
                    [component] => component,
                    components => components.get(lane).unwrap_or(&components[0]),
                };
                register_components_overlap(source_component, register)
            });
            reads_register.then_some(destination_component)
        })
        .collect()
}

#[cfg(windows)]
fn dxbc_terminal_rgb_multiplier_after(
    lines: &[&str],
    producer_index: usize,
    scalar_register: &str,
) -> Option<(usize, String, String)> {
    for (line_index, line) in lines.iter().enumerate().skip(producer_index + 1) {
        let Some((opcode, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let destination = *operands.first()?;
        if opcode == "mul" && operands.len() == 3 {
            let scalar_source_index = operands.iter().skip(1).position(|source| {
                let components = dxbc_operand_components(source);
                !components.is_empty()
                    && components
                        .iter()
                        .all(|component| register_components_overlap(component, scalar_register))
            });
            if let Some(scalar_source_index) = scalar_source_index {
                let destination_components = dxbc_operand_components(destination);
                let destination_base = dxbc_operand_base(destination)?;
                let writes_rgb = ["x", "y", "z"].into_iter().all(|lane| {
                    destination_components.contains(&format!("{destination_base}.{lane}"))
                });
                if writes_rgb {
                    let other_source_index = if scalar_source_index == 0 { 2 } else { 1 };
                    return Some((
                        line_index,
                        destination.to_string(),
                        operands[other_source_index].to_string(),
                    ));
                }
            }
        }
        if register_components_overlap(destination, scalar_register) {
            return None;
        }
    }
    None
}

#[cfg(windows)]
fn dxbc_tainted_terminal_rgb_multiplier_after(
    lines: &[&str],
    producer_index: usize,
    producer_destination: &str,
) -> Option<(usize, String, String)> {
    let mut tainted = dxbc_operand_components(producer_destination)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut branches = Vec::<(BTreeSet<String>, Option<BTreeSet<String>>)>::new();
    let mut candidates = Vec::new();
    for (line_index, line) in lines.iter().enumerate().skip(producer_index + 1) {
        let opcode = line.split_whitespace().next().unwrap_or_default();
        if opcode.starts_with("if_") || opcode == "if" {
            branches.push((tainted.clone(), None));
            continue;
        }
        if opcode == "else" {
            if let Some((entry, then_state)) = branches.last_mut() {
                *then_state = Some(tainted.clone());
                tainted.clone_from(entry);
            }
            continue;
        }
        if opcode == "endif" {
            if let Some((entry, then_state)) = branches.pop() {
                if let Some(then_state) = then_state {
                    tainted.extend(then_state);
                } else {
                    tainted.extend(entry);
                }
            }
            continue;
        }
        let Some((_, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first().copied() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }
        let operand_reads_taint = |operand: &str| {
            dxbc_operand_components(
                operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
            )
            .iter()
            .any(|component| {
                tainted
                    .iter()
                    .any(|tainted| register_components_overlap(component, tainted))
            })
        };
        if opcode == "mul" && operands.len() == 3 {
            if dxbc_operand_base(destination).is_some() {
                let destination_lane_count =
                    destination_components.iter().collect::<BTreeSet<_>>().len();
                if destination_lane_count == 3 {
                    for source_index in 1..=2 {
                        let source_components = dxbc_operand_components(operands[source_index]);
                        let unique_components = source_components.iter().collect::<BTreeSet<_>>();
                        if !source_components.is_empty()
                            && unique_components.len() == 1
                            && operand_reads_taint(operands[source_index])
                        {
                            let other_source_index = if source_index == 1 { 2 } else { 1 };
                            candidates.push((
                                line_index,
                                destination.to_string(),
                                operands[other_source_index].to_string(),
                            ));
                        }
                    }
                }
            }
        }
        let reads_tainted = operands
            .iter()
            .skip(1)
            .any(|operand| operand_reads_taint(operand));
        for destination_component in &destination_components {
            tainted.retain(|tainted| !register_components_overlap(destination_component, tainted));
        }
        if reads_tainted {
            tainted.extend(destination_components);
        }
        if tainted.is_empty() {
            break;
        }
    }
    candidates
        .into_iter()
        .rev()
        .find(|(line_index, destination, _)| {
            ["o0.x", "o0.y", "o0.z"]
                .into_iter()
                .any(|output| dxbc_register_reaches_output(lines, *line_index, destination, output))
        })
}

#[cfg(windows)]
fn dxbc_resources_reaching_operands(
    lines: &[&str],
    bindings: &BTreeMap<u32, String>,
    target_index: usize,
    target_operands: &[&str],
) -> BTreeSet<String> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(producer_index, line)| {
            (producer_index < target_index).then_some(())?;
            let resource_name = dxbc_sample_texture_name(line, bindings)?;
            let destination = dxbc_instruction_destination(line)?;
            target_operands
                .iter()
                .any(|target| {
                    dxbc_component_taint_reaches_operand(
                        lines,
                        producer_index,
                        destination,
                        target_index,
                        target,
                    )
                })
                .then_some(resource_name)
        })
        .collect()
}

#[cfg(windows)]
fn dxbc_has_tainted_fifth_root_shaping(
    lines: &[&str],
    product_index: usize,
    product_destination: &str,
) -> bool {
    lines
        .iter()
        .enumerate()
        .skip(product_index + 1)
        .any(|(log_index, line)| {
            let Some((opcode, raw_operands)) = line.split_once(' ') else {
                return false;
            };
            let operands = split_instruction_operands(raw_operands);
            if opcode != "log" || operands.len() != 2 {
                return false;
            }
            if !dxbc_component_taint_reaches_operand(
                lines,
                product_index,
                product_destination,
                log_index,
                operands[1],
            ) {
                return false;
            }
            let log_destination = operands[0];
            let Some((mul_index, mul_line)) =
                dxbc_direct_consumer_instruction_indices(lines, log_index, log_destination)
                    .first()
                    .copied()
            else {
                return false;
            };
            let Some((mul_opcode, mul_operands)) = mul_line
                .split_once(' ')
                .map(|(opcode, operands)| (opcode, split_instruction_operands(operands)))
            else {
                return false;
            };
            if mul_opcode != "mul" || mul_operands.len() != 3 || !mul_line.contains("l(0.200000)") {
                return false;
            }
            let mul_destination = mul_operands[0];
            dxbc_direct_consumer_instruction_indices(lines, mul_index, mul_destination)
                .first()
                .is_some_and(|(_, exp_line)| exp_line.split_whitespace().next() == Some("exp"))
        })
}

#[cfg(windows)]
fn dxbc_component_taint_reaches_operand(
    lines: &[&str],
    producer_index: usize,
    producer_destination: &str,
    target_index: usize,
    target_operand: &str,
) -> bool {
    let mut tainted = dxbc_operand_components(producer_destination)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut branches = Vec::<(BTreeSet<String>, Option<BTreeSet<String>>)>::new();

    for line in lines
        .iter()
        .skip(producer_index + 1)
        .take(target_index.saturating_sub(producer_index + 1))
    {
        let opcode = line.split_whitespace().next().unwrap_or_default();
        if opcode.starts_with("if_") || opcode == "if" {
            branches.push((tainted.clone(), None));
            continue;
        }
        if opcode == "else" {
            if let Some((entry, then_state)) = branches.last_mut() {
                *then_state = Some(tainted.clone());
                tainted.clone_from(entry);
            }
            continue;
        }
        if opcode == "endif" {
            if let Some((entry, then_state)) = branches.pop() {
                if let Some(then_state) = then_state {
                    tainted.extend(then_state);
                } else {
                    tainted.extend(entry);
                }
            }
            continue;
        }
        if opcode.starts_with("sample") || opcode.starts_with("ld") {
            if let Some(destination) = dxbc_instruction_destination(line) {
                for component in dxbc_operand_components(destination) {
                    tainted.remove(&component);
                }
            }
            continue;
        }
        if opcode.starts_with("dcl_")
            || matches!(
                opcode,
                "break"
                    | "breakc"
                    | "continue"
                    | "continuec"
                    | "discard_nz"
                    | "discard_z"
                    | "ret"
                    | "switch"
                    | "case"
                    | "default"
                    | "endswitch"
                    | "loop"
                    | "endloop"
            )
        {
            continue;
        }
        let Some((_, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }
        let dot_product = matches!(opcode, "dp2" | "dp3" | "dp4" | "dp2add");
        let any_source_tainted = operands.iter().skip(1).any(|source| {
            dxbc_operand_components(source)
                .iter()
                .any(|component| tainted.contains(component))
        });
        let destination_taint = destination_components
            .iter()
            .enumerate()
            .map(|(lane, _)| {
                dot_product && any_source_tainted
                    || (!dot_product
                        && operands.iter().skip(1).any(|source| {
                            let source_components = dxbc_operand_components(source);
                            let source_component = match source_components.as_slice() {
                                [] => return false,
                                [component] => component,
                                components => components.get(lane).unwrap_or(&components[0]),
                            };
                            tainted.contains(source_component)
                        }))
            })
            .collect::<Vec<_>>();
        for (component, is_tainted) in destination_components.into_iter().zip(destination_taint) {
            tainted.remove(&component);
            if is_tainted {
                tainted.insert(component);
            }
        }
    }

    dxbc_operand_components(target_operand)
        .iter()
        .any(|component| tainted.contains(component))
}

#[cfg(windows)]
fn dxbc_log_mul_exp_power_destination(
    lines: &[&str],
    mul_index: usize,
    exponent_register: &str,
) -> Option<(usize, String)> {
    let (opcode, raw_operands) = lines.get(mul_index)?.split_once(' ')?;
    if opcode != "mul" {
        return None;
    }
    let mul_operands = split_instruction_operands(raw_operands);
    let mul_destination = *mul_operands.first()?;
    if !mul_operands.iter().skip(1).any(|operand| {
        register_components_overlap(
            operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
            exponent_register,
        )
    }) {
        return None;
    }

    let log_index = mul_index.checked_sub(1)?;
    let (log_opcode, log_raw_operands) = lines.get(log_index)?.split_once(' ')?;
    if log_opcode != "log" {
        return None;
    }
    let log_operands = split_instruction_operands(log_raw_operands);
    if !log_operands
        .first()
        .is_some_and(|destination| register_components_overlap(destination, mul_destination))
        || !log_operands
            .get(1)
            .is_some_and(|source| register_components_overlap(source, mul_destination))
    {
        return None;
    }

    let exp_index = mul_index + 1;
    let (exp_opcode, exp_raw_operands) = lines.get(exp_index)?.split_once(' ')?;
    if exp_opcode != "exp" {
        return None;
    }
    let exp_operands = split_instruction_operands(exp_raw_operands);
    let exp_destination = *exp_operands.first()?;
    if !register_components_overlap(exp_destination, mul_destination)
        || !exp_operands
            .get(1)
            .is_some_and(|source| register_components_overlap(source, mul_destination))
    {
        return None;
    }
    dxbc_operand_components(exp_destination)
        .into_iter()
        .next()
        .map(|destination| (exp_index, destination))
}

#[cfg(windows)]
fn dxbc_gloss_power_uses_camera_reflection_dot(
    lines: &[&str],
    mul_index: usize,
    exponent_register: &str,
) -> bool {
    if dxbc_log_mul_exp_power_destination(lines, mul_index, exponent_register).is_none() {
        return false;
    }
    let Some(power_dot_index) = mul_index.checked_sub(2) else {
        return false;
    };
    let Some(power_dot) = dxbc_instruction_operands(lines, power_dot_index, "dp3_sat") else {
        return false;
    };
    if power_dot.len() != 3 {
        return false;
    }

    for (reflection_operand, light_operand) in
        [(power_dot[1], power_dot[2]), (power_dot[2], power_dot[1])]
    {
        let Some(light_write_index) = dxbc_last_write_before(lines, power_dot_index, light_operand)
        else {
            continue;
        };
        let Some(camera_input) =
            dxbc_normalized_camera_offset_input(lines, light_write_index, light_operand)
        else {
            continue;
        };
        let Some(reflection_write_index) =
            dxbc_last_write_before(lines, power_dot_index, reflection_operand)
        else {
            continue;
        };
        let Some(reflection_mad) = dxbc_instruction_operands(lines, reflection_write_index, "mad")
        else {
            continue;
        };
        if reflection_mad.len() != 4
            || !dxbc_operands_overlap(reflection_mad[0], reflection_operand)
            || !reflection_mad[2].starts_with('-')
            || !reflection_mad[3].starts_with('-')
        {
            continue;
        }
        let Some(double_index) = reflection_write_index.checked_sub(1) else {
            continue;
        };
        let Some(double) = dxbc_instruction_operands(lines, double_index, "add") else {
            continue;
        };
        if double.len() != 3
            || !dxbc_operands_overlap(double[0], reflection_mad[2])
            || !dxbc_operands_overlap(double[1], double[2])
        {
            continue;
        }
        let Some(view_normal_dot_index) = double_index.checked_sub(1) else {
            continue;
        };
        let Some(view_normal_dot) = dxbc_instruction_operands(lines, view_normal_dot_index, "dp3")
        else {
            continue;
        };
        if view_normal_dot.len() != 3 || !dxbc_operands_overlap(view_normal_dot[0], double[0]) {
            continue;
        }
        let reflected_normal = reflection_mad[1];
        let reflected_view = reflection_mad[3];
        let dot_matches_sources = (dxbc_operands_overlap(view_normal_dot[1], reflected_view)
            && dxbc_operands_overlap(view_normal_dot[2], reflected_normal))
            || (dxbc_operands_overlap(view_normal_dot[2], reflected_view)
                && dxbc_operands_overlap(view_normal_dot[1], reflected_normal));
        if !dot_matches_sources {
            continue;
        }
        let Some(view_write_index) =
            dxbc_last_write_before(lines, view_normal_dot_index, reflected_view)
        else {
            continue;
        };
        if dxbc_normalized_camera_input(lines, view_write_index, reflected_view)
            == Some(camera_input)
        {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn dxbc_gloss_power_has_visibility_envelope(
    lines: &[&str],
    mul_index: usize,
    exponent_register: &str,
) -> bool {
    let Some((exp_index, power_destination)) =
        dxbc_log_mul_exp_power_destination(lines, mul_index, exponent_register)
    else {
        return false;
    };
    let Some(square) = dxbc_instruction_operands(lines, exp_index + 1, "mul") else {
        return false;
    };
    let (polynomial, saturated_polynomial) = if let Some(polynomial) =
        dxbc_instruction_operands(lines, exp_index + 2, "mad")
    {
        (polynomial, false)
    } else if let Some(polynomial) = dxbc_instruction_operands(lines, exp_index + 2, "mad_sat") {
        (polynomial, true)
    } else {
        return false;
    };
    if square.len() != 3
        || polynomial.len() != 4
        || !dxbc_operands_overlap(square[1], square[2])
        || !dxbc_operands_overlap(polynomial[0], square[0])
        || !dxbc_operands_overlap(polynomial[1], square[0])
        || polynomial[2] != "l(-3.000000)"
        || polynomial[3] != "l(3.000000)"
    {
        return false;
    }
    let (bound_register, product_index) = if saturated_polynomial {
        (polynomial[0], exp_index + 3)
    } else {
        let Some(bound) = dxbc_instruction_operands(lines, exp_index + 3, "min") else {
            return false;
        };
        if bound.len() != 3
            || !dxbc_operands_overlap(bound[0], polynomial[0])
            || !dxbc_operands_overlap(bound[1], polynomial[0])
            || bound[2] != "l(1.000000)"
        {
            return false;
        }
        (bound[0], exp_index + 4)
    };
    let Some(product) = dxbc_instruction_operands(lines, product_index, "mul") else {
        return false;
    };
    if product.len() != 3
        || !((dxbc_operands_overlap(product[1], bound_register)
            && dxbc_operands_overlap(product[2], &power_destination))
            || (dxbc_operands_overlap(product[2], bound_register)
                && dxbc_operands_overlap(product[1], &power_destination)))
    {
        return false;
    }

    let Some(envelope_source_index) = dxbc_last_write_before(lines, exp_index, square[1]) else {
        return false;
    };
    let Some(one_minus_light) = dxbc_instruction_operands(lines, envelope_source_index, "add")
    else {
        return false;
    };
    if one_minus_light.len() != 3
        || !dxbc_operands_overlap(one_minus_light[0], square[1])
        || !one_minus_light[1].starts_with('-')
        || one_minus_light[2] != "l(1.000000)"
    {
        return false;
    }
    let Some(light_dot_index) = envelope_source_index.checked_sub(1) else {
        return false;
    };
    let Some(light_dot) = dxbc_instruction_operands(lines, light_dot_index, "dp3_sat") else {
        return false;
    };
    light_dot.len() == 3 && dxbc_operands_overlap(light_dot[0], one_minus_light[1])
}

#[cfg(windows)]
fn dxbc_instruction_operands<'a>(
    lines: &'a [&str],
    index: usize,
    expected_opcode: &str,
) -> Option<Vec<&'a str>> {
    let (opcode, raw_operands) = lines.get(index)?.split_once(' ')?;
    (opcode == expected_opcode).then(|| split_instruction_operands(raw_operands))
}

#[cfg(windows)]
fn dxbc_operands_overlap(left: &str, right: &str) -> bool {
    let right = dxbc_operand_components(right);
    dxbc_operand_components(left)
        .iter()
        .any(|left| right.contains(left))
}

#[cfg(windows)]
fn dxbc_last_write_before(lines: &[&str], before_index: usize, register: &str) -> Option<usize> {
    (0..before_index).rev().find(|index| {
        lines[*index]
            .split_once(' ')
            .map(|(_, raw_operands)| split_instruction_operands(raw_operands))
            .and_then(|operands| operands.first().copied())
            .is_some_and(|destination| dxbc_operands_overlap(destination, register))
    })
}

#[cfg(windows)]
fn dxbc_normalized_camera_offset_input<'a>(
    lines: &'a [&str],
    normalize_index: usize,
    expected_destination: &str,
) -> Option<&'a str> {
    let normalize = dxbc_instruction_operands(lines, normalize_index, "mul")?;
    if normalize.len() != 3 || !dxbc_operands_overlap(normalize[0], expected_destination) {
        return None;
    }
    let rsq_index = normalize_index.checked_sub(1)?;
    let rsq = dxbc_instruction_operands(lines, rsq_index, "rsq")?;
    let length_index = rsq_index.checked_sub(1)?;
    let length = dxbc_instruction_operands(lines, length_index, "dp3")?;
    let offset_index = length_index.checked_sub(1)?;
    let offset = dxbc_instruction_operands(lines, offset_index, "add")?;
    if rsq.len() != 2
        || length.len() != 3
        || offset.len() != 3
        || !dxbc_operands_overlap(normalize[1], rsq[0])
        || !dxbc_operands_overlap(rsq[1], length[0])
        || !dxbc_operands_overlap(length[1], offset[0])
        || !dxbc_operands_overlap(length[2], offset[0])
        || !dxbc_operands_overlap(normalize[2], offset[0])
        || !offset[1].starts_with('-')
        || !offset[2].contains("0.200000")
    {
        return None;
    }
    dxbc_operand_base(offset[1]).filter(|base| base.starts_with('v'))
}

#[cfg(windows)]
fn dxbc_normalized_camera_input<'a>(
    lines: &'a [&str],
    normalize_index: usize,
    expected_destination: &str,
) -> Option<&'a str> {
    let normalize = dxbc_instruction_operands(lines, normalize_index, "mul")?;
    let rsq_index = normalize_index.checked_sub(1)?;
    let rsq = dxbc_instruction_operands(lines, rsq_index, "rsq")?;
    let length_index = rsq_index.checked_sub(1)?;
    let length = dxbc_instruction_operands(lines, length_index, "dp3")?;
    if normalize.len() != 3
        || rsq.len() != 2
        || length.len() != 3
        || !dxbc_operands_overlap(normalize[0], expected_destination)
        || !dxbc_operands_overlap(normalize[1], rsq[0])
        || !dxbc_operands_overlap(rsq[1], length[0])
    {
        return None;
    }
    let camera_input = dxbc_operand_base(normalize[2]).filter(|base| base.starts_with('v'))?;
    (dxbc_operand_base(length[1]) == Some(camera_input)
        && dxbc_operand_base(length[2]) == Some(camera_input))
    .then_some(camera_input)
}

#[cfg(windows)]
fn dxbc_register_reaches_output(
    lines: &[&str],
    producer_index: usize,
    register: &str,
    output: &str,
) -> bool {
    let mut tainted = BTreeSet::from([register.to_string()]);
    for line in lines.iter().skip(producer_index + 1) {
        let Some((_, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }
        let reads_tainted = operands.iter().skip(1).any(|operand| {
            dxbc_operand_components(
                operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
            )
            .iter()
            .any(|component| {
                tainted
                    .iter()
                    .any(|tainted| register_components_overlap(component, tainted))
            })
        });
        for destination_component in &destination_components {
            tainted.retain(|tainted| !register_components_overlap(destination_component, tainted));
        }
        if reads_tainted {
            if destination_components
                .iter()
                .any(|component| register_components_overlap(component, output))
            {
                return true;
            }
            tainted.extend(destination_components);
        }
        if tainted.is_empty() {
            return false;
        }
    }
    false
}

#[cfg(windows)]
fn dxbc_register_reached_outputs(
    lines: &[&str],
    producer_index: usize,
    register: &str,
) -> BTreeSet<String> {
    let mut outputs = BTreeSet::new();
    let mut tainted = BTreeSet::from([register.to_string()]);
    for line in lines.iter().skip(producer_index + 1) {
        let Some((_, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }
        let reads_tainted = operands.iter().skip(1).any(|operand| {
            dxbc_operand_components(
                operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
            )
            .iter()
            .any(|component| {
                tainted
                    .iter()
                    .any(|tainted| register_components_overlap(component, tainted))
            })
        });
        for destination_component in &destination_components {
            tainted.retain(|tainted| !register_components_overlap(destination_component, tainted));
        }
        if reads_tainted {
            outputs.extend(
                destination_components
                    .iter()
                    .filter(|component| component.starts_with('o'))
                    .cloned(),
            );
            tainted.extend(destination_components);
        }
        if tainted.is_empty() {
            break;
        }
    }
    outputs
}

#[cfg(windows)]
fn dxbc_tainted_cube_lod_sample_count(
    lines: &[&str],
    producer_index: usize,
    register: &str,
) -> usize {
    dxbc_tainted_cube_lod_sample_indices(lines, producer_index, register).len()
}

#[cfg(windows)]
fn dxbc_tainted_cube_lod_sample_indices(
    lines: &[&str],
    producer_index: usize,
    register: &str,
) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut tainted = BTreeSet::from([register.to_string()]);
    for (line_index, line) in lines.iter().enumerate().skip(producer_index + 1) {
        let Some((opcode, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first() else {
            continue;
        };
        let operand_reads_taint = |operand: &str| {
            dxbc_operand_components(
                operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
            )
            .iter()
            .any(|component| {
                tainted
                    .iter()
                    .any(|tainted| register_components_overlap(component, tainted))
            })
        };
        if opcode.starts_with("sample_l")
            && line.contains("texturecubearray")
            && operands.last().is_some_and(|lod| operand_reads_taint(lod))
        {
            indices.push(line_index);
        }
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }
        let reads_tainted = operands
            .iter()
            .skip(1)
            .any(|operand| operand_reads_taint(operand));
        for destination_component in &destination_components {
            tainted.retain(|tainted| !register_components_overlap(destination_component, tainted));
        }
        if reads_tainted {
            tainted.extend(destination_components);
        }
        if tainted.is_empty() {
            break;
        }
    }
    indices
}

#[cfg(windows)]
fn dxbc_cube_sample_has_squared_alpha_decode(lines: &[&str], sample_index: usize) -> bool {
    let Some(sample_operands) = lines
        .get(sample_index)
        .and_then(|line| line.split_once(' '))
        .map(|(_, operands)| split_instruction_operands(operands))
    else {
        return false;
    };
    let Some(sample_destination) = sample_operands.first().copied() else {
        return false;
    };
    let Some(sample_base) = dxbc_operand_base(sample_destination) else {
        return false;
    };
    let Some(square) = lines
        .get(sample_index + 1)
        .and_then(|line| instruction_operands(line, "mul"))
    else {
        return false;
    };
    let Some(alpha_bias) = lines
        .get(sample_index + 2)
        .and_then(|line| instruction_operands(line, "add"))
    else {
        return false;
    };
    let Some(decode) = lines
        .get(sample_index + 3)
        .and_then(|line| instruction_operands(line, "div"))
    else {
        return false;
    };
    let squared_components = square
        .get(1)
        .map(|source| dxbc_operand_components(source))
        .unwrap_or_default();
    square.len() == 3
        && square[1] == square[2]
        && !squared_components.is_empty()
        && squared_components.iter().all(|component| {
            component.starts_with(&format!("{sample_base}.")) && !component.ends_with(".w")
        })
        && alpha_bias.len() == 3
        && dxbc_operand_components(alpha_bias[1]) == vec![format!("{sample_base}.w")]
        && alpha_bias[2] == "l(0.000100)"
        && decode.len() == 3
        && register_components_overlap(decode[1], square[0])
        && register_components_overlap(decode[2], alpha_bias[0])
}

#[cfg(windows)]
fn dxbc_cube_sample_environment_location_lane(
    lines: &[&str],
    sample_index: usize,
    ambient_slot: u16,
) -> Option<&'static str> {
    let sample_operands = lines
        .get(sample_index)?
        .split_once(' ')
        .map(|(_, operands)| split_instruction_operands(operands))?;
    let coordinates = *sample_operands.get(1)?;
    let coordinate_base = dxbc_operand_base(coordinates)?;
    let target_w = format!("{coordinate_base}.w");
    let add = lines[..sample_index].iter().rev().find_map(|line| {
        let operands = instruction_operands(line, "add")?;
        (operands
            .first()
            .is_some_and(|destination| register_components_overlap(destination, &target_w)))
        .then_some(operands)
    })?;
    if add.len() != 3 || add[2] != "l(0.100000)" {
        return None;
    }
    if add[1] == format!("cb{ambient_slot}[5].w") {
        Some("current")
    } else if add[1] == format!("cb{ambient_slot}[9].y") {
        Some("previous")
    } else {
        None
    }
}

#[cfg(windows)]
fn dxbc_operand_is_replicated_component(value: &str, base: &str, component: char) -> bool {
    value
        .strip_prefix(base)
        .and_then(|suffix| suffix.strip_prefix('.'))
        .is_some_and(|swizzle| {
            !swizzle.is_empty() && swizzle.chars().all(|candidate| candidate == component)
        })
}

#[cfg(windows)]
fn dxbc_has_ambient_location_interpolation(lines: &[&str], ambient_slot: u16) -> bool {
    let interpolation = format!("cb{ambient_slot}[9]");
    let has_positive_gate = lines.iter().any(|line| {
        instruction_operands(line, "lt").is_some_and(|operands| {
            operands.len() == 3
                && operands[1] == "l(0.000000)"
                && dxbc_operand_is_replicated_component(operands[2], &interpolation, 'z')
        })
    });
    let has_blend = lines.iter().any(|line| {
        instruction_operands(line, "mad").is_some_and(|operands| {
            operands.len() == 4
                && dxbc_operand_is_replicated_component(operands[1], &interpolation, 'z')
        })
    });
    has_positive_gate && has_blend
}

#[cfg(windows)]
fn dxbc_has_ambient_reflection_scale_offset(lines: &[&str], ambient_slot: u16) -> bool {
    let reflection = format!("cb{ambient_slot}[5]");
    lines.iter().any(|line| {
        instruction_operands(line, "mad").is_some_and(|operands| {
            operands.len() == 4
                && dxbc_operand_is_replicated_component(operands[2], &reflection, 'x')
                && dxbc_operand_is_replicated_component(operands[3], &reflection, 'y')
        })
    })
}

#[cfg(windows)]
fn dxbc_has_ambient_bake_light_composition(lines: &[&str], ambient_slot: u16) -> bool {
    let reflection = format!("cb{ambient_slot}[5]");
    let has_bake_rate = lines.iter().any(|line| {
        instruction_operands(line, "mad").is_some_and(|operands| {
            operands.len() == 4
                && dxbc_operand_is_replicated_component(operands[1], &reflection, 'z')
        })
    });
    let has_lambert_scale = lines.iter().any(|line| {
        matches!(line.split_whitespace().next(), Some("mul") | Some("mad"))
            && line.contains("2.356194")
    });
    has_bake_rate && has_lambert_scale
}

#[cfg(windows)]
fn dxbc_table_lane_samples(
    lines: &[&str],
    bindings: &BTreeMap<u32, String>,
    x_literal: &str,
    physical_lane: u8,
) -> Vec<(usize, String)> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(line_index, line)| {
            (dxbc_sample_texture_name(line, bindings).as_deref() == Some("g_SamplerTable.T"))
                .then_some(())?;
            let coordinates = dxbc_sample_texture_coordinates(line)?;
            (dxbc_literal_provenance_at(lines, line_index, coordinates, 0).as_deref()
                == Some(x_literal))
            .then_some(())?;
            Some((
                line_index,
                dxbc_sample_texture_physical_lane_destination(line, physical_lane)?,
            ))
        })
        .collect()
}

#[cfg(windows)]
fn dxbc_two_producers_join(
    lines: &[&str],
    first_index: usize,
    first_destination: &str,
    second_index: usize,
    second_destination: &str,
) -> bool {
    dxbc_two_producers_join_index(
        lines,
        first_index,
        first_destination,
        second_index,
        second_destination,
    )
    .is_some()
}

#[cfg(windows)]
fn dxbc_two_producers_join_index(
    lines: &[&str],
    first_index: usize,
    first_destination: &str,
    second_index: usize,
    second_destination: &str,
) -> Option<usize> {
    let start = first_index.min(second_index);
    let mut labels = BTreeMap::<String, u8>::new();
    for (line_index, line) in lines.iter().enumerate().skip(start).take(512) {
        let Some((_, raw_operands)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        let mut source_labels = 0;
        for source in operands.iter().skip(1) {
            for component in dxbc_operand_components(source) {
                source_labels |= labels.get(&component).copied().unwrap_or(0);
            }
        }
        for component in &destination_components {
            labels.remove(component);
        }
        if source_labels != 0 {
            for component in &destination_components {
                labels.insert(component.clone(), source_labels);
            }
            if source_labels == 3 {
                return Some(line_index);
            }
        }
        if line_index == first_index {
            for component in dxbc_operand_components(first_destination) {
                labels.insert(component, 1);
            }
        }
        if line_index == second_index {
            for component in dxbc_operand_components(second_destination) {
                labels.insert(component, 2);
            }
        }
    }
    None
}

#[cfg(windows)]
fn dxbc_literal_operands(instruction: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut rest = instruction;
    while let Some(start) = rest.find("l(") {
        rest = &rest[start..];
        let Some(end) = rest.find(')') else {
            break;
        };
        literals.push(rest[..=end].to_string());
        rest = &rest[end + 1..];
    }
    literals
}

#[cfg(windows)]
fn audit_installed_texture_mip_dxbc(
    resource: &mut SqPackResource,
) -> Result<Vec<TextureMipDxbcPackageAudit>> {
    ["character.shpk", "characterlegacy.shpk", "characterglass.shpk"]
        .into_iter()
        .map(|shader_package_name| {
            let path = format!("shader/sm5/shpk/{shader_package_name}");
            let bytes = resource
                .read(&path)
                .ok_or_else(|| anyhow!("failed to read installed {path}"))?;
            let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
                .ok_or_else(|| anyhow!("failed to parse installed {path}"))?;
            let mut declared_shader_count = 0;
            let mut consumer_shader_count = 0;
            let mut sum_count = 0;
            let mut consumer_sets = BTreeMap::new();

            for (shader_index, shader) in package.pixel_shaders.iter().enumerate() {
                let assembly = disassemble_dxbc(&shader.bytecode).with_context(|| {
                    format!("failed to disassemble {shader_package_name} pixel shader {shader_index}")
                })?;
                if !assembly.contains("cb0[14].y") {
                    continue;
                }
                declared_shader_count += 1;
                let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
                let texture_bindings = dxbc_texture_bindings(&lines);
                let mut shader_sum_count = 0;
                for (line_index, line) in lines.iter().enumerate() {
                    let Some(bias_register) = texture_mip_bias_add_destination(line) else {
                        continue;
                    };
                    shader_sum_count += 1;
                    sum_count += 1;
                    let consumers = texture_mip_bias_consumers(
                        &lines,
                        line_index,
                        &bias_register,
                        &texture_bindings,
                    );
                    if consumers.is_empty() {
                        return Err(anyhow!(
                            "{shader_package_name} pixel shader {shader_index} computes g_TextureMipBias sum without a direct biased sample"
                        ));
                    }
                    if consumers.iter().any(|name| {
                        !matches!(
                            name.as_str(),
                            "g_SamplerDiffuse.T" | "g_SamplerNormal.T" | "g_SamplerMask.T"
                        )
                    }) {
                        return Err(anyhow!(
                            "{shader_package_name} pixel shader {shader_index} sends g_TextureMipBias to unexpected consumers: {:?}",
                            consumers
                        ));
                    }
                    let consumer_key = consumers.into_iter().collect::<Vec<_>>().join(",");
                    *consumer_sets.entry(consumer_key).or_default() += 1;
                }
                if shader_sum_count > 0 {
                    consumer_shader_count += 1;
                }
            }

            Ok(TextureMipDxbcPackageAudit {
                shader_package_name: shader_package_name.to_string(),
                pixel_shader_count: package.pixel_shaders.len(),
                declared_shader_count,
                consumer_shader_count,
                sum_count,
                consumer_sets,
            })
        })
        .collect()
}

#[cfg(windows)]
fn dxbc_texture_bindings(lines: &[&str]) -> BTreeMap<u32, String> {
    let mut bindings = BTreeMap::new();
    for line in lines {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() != Some(&"//") || !fields.contains(&"texture") {
            continue;
        }
        let Some(name) = fields.get(1) else {
            continue;
        };
        let Some(slot) = fields
            .iter()
            .find_map(|field| dxbc_resource_slot(field, 't'))
        else {
            continue;
        };
        bindings.insert(slot, (*name).to_string());
    }
    bindings
}

#[cfg(windows)]
fn dxbc_constant_buffer_binding(lines: &[&str], name: &str) -> Option<u32> {
    lines.iter().find_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        (fields.first() == Some(&"//")
            && fields.get(1) == Some(&name)
            && fields.contains(&"cbuffer"))
        .then_some(())?;
        fields.iter().find_map(|field| {
            let digits = field.strip_prefix("cb")?;
            (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| digits.parse().ok())?
        })
    })
}

#[cfg(windows)]
fn dxbc_constant_buffer_bindings(lines: &[&str]) -> BTreeMap<u32, String> {
    let mut bindings = BTreeMap::new();
    for line in lines {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() != Some(&"//") || !fields.contains(&"cbuffer") {
            continue;
        }
        let Some(name) = fields.get(1) else {
            continue;
        };
        let Some(slot) = fields.iter().find_map(|field| {
            let digits = field.strip_prefix("cb")?;
            (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| digits.parse().ok())?
        }) else {
            continue;
        };
        bindings.insert(slot, (*name).to_string());
    }
    bindings
}

#[cfg(windows)]
fn dxbc_constant_buffer_reference(operand: &str) -> Option<(u32, u32)> {
    let operand = operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')'));
    let reference = operand.strip_prefix("cb")?;
    let (slot, vector) = reference.split_once('[')?;
    let (vector, _) = vector.split_once(']')?;
    Some((slot.parse().ok()?, vector.parse().ok()?))
}

#[cfg(windows)]
fn dxbc_constant_buffer_vectors_reaching_operands(
    lines: &[&str],
    target_index: usize,
    target_operands: &[&str],
) -> BTreeSet<String> {
    let bindings = dxbc_constant_buffer_bindings(lines);
    lines
        .iter()
        .enumerate()
        .filter_map(|(producer_index, line)| {
            (producer_index < target_index).then_some(())?;
            let (_, raw_operands) = line.split_once(' ')?;
            let operands = split_instruction_operands(raw_operands);
            let destination = *operands.first()?;
            let references = operands[1..]
                .iter()
                .filter_map(|operand| dxbc_constant_buffer_reference(operand))
                .collect::<Vec<_>>();
            (!references.is_empty()).then_some(())?;
            target_operands
                .iter()
                .any(|target| {
                    dxbc_component_taint_reaches_operand(
                        lines,
                        producer_index,
                        destination,
                        target_index,
                        target,
                    )
                })
                .then_some(())?;
            Some(
                references
                    .into_iter()
                    .map(|(slot, vector)| {
                        format!(
                            "{}[{vector}]",
                            bindings
                                .get(&slot)
                                .cloned()
                                .unwrap_or_else(|| format!("cb{slot}"))
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

#[cfg(windows)]
fn dxbc_dynamic_emissive_reaches_o0_rgb(lines: &[&str]) -> bool {
    dxbc_reflected_cbuffer_vector_reaches_o0_rgb(
        lines,
        "g_MaterialParameterDynamic",
        "float4 m_EmissiveColor;",
        0,
        16,
        0,
    )
}

#[cfg(windows)]
fn dxbc_dynamic_emissive_table_joins(lines: &[&str]) -> Vec<(usize, String)> {
    let Some(dynamic_slot) = dxbc_constant_buffer_binding(lines, "g_MaterialParameterDynamic")
    else {
        return Vec::new();
    };
    let dynamic_source = format!("cb{dynamic_slot}[0]");
    let texture_bindings = dxbc_texture_bindings(lines);
    let table_emissive_samples = lines
        .iter()
        .enumerate()
        .filter_map(|(sample_index, line)| {
            (dxbc_sample_texture_name(line, &texture_bindings).as_deref()
                == Some("g_SamplerTable.T"))
            .then_some(())?;
            let coordinates = dxbc_sample_texture_coordinates(line)?;
            (dxbc_literal_provenance_at(lines, sample_index, coordinates, 0).as_deref()
                == Some("2.500000"))
            .then_some((
                sample_index,
                dxbc_instruction_destination(line)?.to_string(),
            ))
        })
        .collect::<Vec<_>>();
    lines
        .iter()
        .enumerate()
        .filter_map(|(join_index, line)| {
            let Some((opcode, raw_operands)) = line.split_once(' ') else {
                return None;
            };
            let operands = split_instruction_operands(raw_operands);
            if opcode != "mul" || operands.len() != 3 {
                return None;
            }
            let Some(destination) = operands.first() else {
                return None;
            };
            let dynamic_source_index = operands.iter().skip(1).position(|operand| {
                dxbc_operand_base(
                    operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')')),
                ) == Some(dynamic_source.as_str())
            });
            let Some(dynamic_source_index) = dynamic_source_index else {
                return None;
            };
            let table_source = operands[if dynamic_source_index == 0 { 2 } else { 1 }];
            table_emissive_samples
                .iter()
                .any(|(sample_index, sample_destination)| {
                    *sample_index < join_index
                        && dxbc_component_taint_reaches_operand(
                            lines,
                            *sample_index,
                            sample_destination,
                            join_index,
                            table_source,
                        )
                })
                .then(|| (join_index, (*destination).to_string()))
        })
        .collect()
}

#[cfg(windows)]
fn dxbc_dynamic_emissive_table_join_reaches_o0_rgb(lines: &[&str]) -> bool {
    dxbc_dynamic_emissive_table_joins(lines)
        .into_iter()
        .any(|(join_index, destination)| {
            ["o0.x", "o0.y", "o0.z"]
                .into_iter()
                .any(|output| dxbc_register_reaches_output(lines, join_index, &destination, output))
        })
}

#[cfg(windows)]
fn dxbc_last_writer_before<'a>(
    lines: &'a [&str],
    end_index: usize,
    target: &str,
) -> Option<(usize, &'a str)> {
    lines
        .iter()
        .take(end_index)
        .enumerate()
        .rev()
        .find_map(|(line_index, line)| {
            dxbc_instruction_destination(line)
                .is_some_and(|destination| register_components_overlap(destination, target))
                .then_some((line_index, *line))
        })
}

#[cfg(windows)]
fn dxbc_dynamic_emissive_luminance_scale_o0_rgb_trace(
    lines: &[&str],
) -> Option<DxbcDynamicEmissiveLuminanceScaleTrace> {
    const LUMINANCE_WEIGHTS: &str = "l(0.298910, 0.586610, 0.114480, 0.000000)";

    dxbc_dynamic_emissive_table_joins(lines)
        .into_iter()
        .find_map(|(join_index, join_destination)| {
            dxbc_direct_consumer_instruction_indices(lines, join_index, &join_destination)
                .into_iter()
                .find_map(|(composition_index, composition_line)| {
                    let Some((opcode @ ("mad" | "mul"), raw_composition_operands)) =
                        composition_line.split_once(' ')
                    else {
                        return None;
                    };
                    let composition_operands = split_instruction_operands(raw_composition_operands);
                    let expected_operand_count = if opcode == "mad" { 4 } else { 3 };
                    if composition_operands.len() != expected_operand_count {
                        return None;
                    }
                    let join_operand_index = [1, 2].into_iter().find(|operand_index| {
                        register_components_overlap(
                            composition_operands[*operand_index],
                            &join_destination,
                        )
                    });
                    let Some(join_operand_index) = join_operand_index else {
                        return None;
                    };
                    let scale_operand =
                        composition_operands[if join_operand_index == 1 { 2 } else { 1 }];
                    let Some((max_index, max_line)) =
                        dxbc_last_writer_before(lines, composition_index, scale_operand)
                    else {
                        return None;
                    };
                    let Some(("max", raw_max_operands)) = max_line.split_once(' ') else {
                        return None;
                    };
                    let max_operands = split_instruction_operands(raw_max_operands);
                    if max_operands.len() != 3 {
                        return None;
                    }
                    let luma_operand = match (max_operands[1], max_operands[2]) {
                        ("l(1.000000)", operand) | (operand, "l(1.000000)") => operand,
                        _ => return None,
                    };
                    let Some((luma_index, luma_line)) =
                        dxbc_last_writer_before(lines, max_index, luma_operand)
                    else {
                        return None;
                    };
                    let Some(("dp3", raw_luma_operands)) = luma_line.split_once(' ') else {
                        return None;
                    };
                    let luma_operands = split_instruction_operands(raw_luma_operands);
                    if luma_operands.len() != 3
                        || !luma_operands[1..].contains(&LUMINANCE_WEIGHTS)
                        || !["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                            dxbc_register_reaches_output(
                                lines,
                                composition_index,
                                composition_operands[0],
                                output,
                            )
                        })
                    {
                        return None;
                    }
                    let runtime_composite = luma_operands[1..]
                        .iter()
                        .find(|operand| **operand != LUMINANCE_WEIGHTS)?;
                    let texture_bindings = dxbc_texture_bindings(lines);
                    let source_o0_rgb_composition_opcode =
                        dxbc_direct_consumer_instruction_indices(
                            lines,
                            luma_index,
                            runtime_composite,
                        )
                        .into_iter()
                        .find_map(|(source_composition_index, line)| {
                            let (opcode @ ("mad" | "mul"), raw_operands) = line.split_once(' ')?
                            else {
                                return None;
                            };
                            let operands = split_instruction_operands(raw_operands);
                            let destination = *operands.first()?;
                            (dxbc_operand_components(destination).len() >= 3
                                && ["o0.x", "o0.y", "o0.z"].into_iter().any(|output| {
                                    dxbc_register_reaches_output(
                                        lines,
                                        source_composition_index,
                                        destination,
                                        output,
                                    )
                                }))
                            .then(|| opcode.to_string())
                        });
                    Some(DxbcDynamicEmissiveLuminanceScaleTrace {
                        composition_opcode: opcode.to_string(),
                        texture_resources: dxbc_resources_reaching_operands(
                            lines,
                            &texture_bindings,
                            luma_index,
                            &[*runtime_composite],
                        ),
                        constant_buffer_vectors: dxbc_constant_buffer_vectors_reaching_operands(
                            lines,
                            luma_index,
                            &[*runtime_composite],
                        ),
                        source_o0_rgb_composition_opcode,
                    })
                })
        })
}

#[cfg(windows)]
fn dxbc_instance_parameter_member_reaches_o0_rgb(
    lines: &[&str],
    member: &str,
    offset: usize,
    vector_index: usize,
) -> bool {
    dxbc_reflected_cbuffer_vector_reaches_o0_rgb(
        lines,
        "g_InstanceParameter",
        member,
        offset,
        176,
        vector_index,
    )
}

#[cfg(windows)]
fn dxbc_reflected_cbuffer_vector_reaches_o0_rgb(
    lines: &[&str],
    buffer_name: &str,
    member: &str,
    offset: usize,
    buffer_size: usize,
    vector_index: usize,
) -> bool {
    let expected_offset = offset.to_string();
    let expected_size = buffer_size.to_string();
    let reflected = lines.iter().any(|line| {
        line.contains(member)
            && line
                .split_once("Offset:")
                .and_then(|(_, value)| value.split_whitespace().next())
                == Some(expected_offset.as_str())
    }) && lines.iter().any(|line| {
        line.contains(&format!("{buffer_name};"))
            && line
                .split_once("Size:")
                .and_then(|(_, value)| value.split_whitespace().next())
                == Some(expected_size.as_str())
    });
    if !reflected {
        return false;
    }
    let Some(slot) = dxbc_constant_buffer_binding(lines, buffer_name) else {
        return false;
    };
    let source_base = format!("cb{slot}[{vector_index}]");
    lines.iter().enumerate().any(|(line_index, line)| {
        let Some((_, raw_operands)) = line.split_once(' ') else {
            return false;
        };
        let operands = split_instruction_operands(raw_operands);
        let Some(destination) = operands.first() else {
            return false;
        };
        let reads_member_rgb = operands.iter().skip(1).any(|operand| {
            let operand =
                operand.trim_matches(|character| matches!(character, '-' | '|' | '(' | ')'));
            dxbc_operand_base(operand) == Some(source_base.as_str())
                && operand
                    .split_once('.')
                    .map(|(_, mask)| mask.bytes().any(|lane| b"xyz".contains(&lane)))
                    .unwrap_or(true)
        });
        reads_member_rgb
            && ["o0.x", "o0.y", "o0.z"]
                .into_iter()
                .any(|output| dxbc_register_reaches_output(lines, line_index, destination, output))
    })
}

#[cfg(windows)]
fn dxbc_resource_slot(value: &str, prefix: char) -> Option<u32> {
    let value = value.split('.').next()?;
    let digits = value.strip_prefix(prefix)?;
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())?
}

#[cfg(windows)]
fn vertex_alpha_remap_destination(lines: &[&str], line_index: usize) -> Option<String> {
    let operands = instruction_operands(lines.get(line_index)?, "mad")?;
    if operands.len() != 4
        || operands[1] != "cb0[13].y"
        || operands[2] != operands[0]
        || operands[3] != "v1.w"
    {
        return None;
    }
    let destination = operands[0];
    let previous = instruction_operands(lines.get(line_index.checked_sub(1)?)?, "add")?;
    if previous.len() != 3
        || previous[0] != destination
        || previous[1] != "-v1.w"
        || previous[2] != "l(1.000000)"
    {
        return None;
    }
    Some(destination.to_string())
}

#[cfg(windows)]
fn vertex_alpha_has_immediate_product(
    lines: &[&str],
    line_index: usize,
    remapped_alpha: &str,
) -> bool {
    for line in lines.iter().skip(line_index + 1) {
        if instruction_operands(line, "mul").is_some_and(|operands| {
            operands.len() == 3 && (operands[1] == remapped_alpha || operands[2] == remapped_alpha)
        }) {
            return true;
        }
        if instruction_writes_register_component(line, remapped_alpha) {
            return false;
        }
    }
    false
}

#[cfg(windows)]
fn vertex_alpha_has_threshold_test(
    lines: &[&str],
    line_index: usize,
    remapped_alpha: &str,
) -> bool {
    for (offset, line) in lines.iter().enumerate().skip(line_index + 1) {
        if instruction_operands(line, "mad").is_some_and(|operands| {
            operands.len() == 4
                && operands[0] == remapped_alpha
                && operands[1] == remapped_alpha
                && operands[3] == "-cb0[0].w"
        }) {
            for (compare_offset, compare) in lines.iter().enumerate().skip(offset + 1) {
                if instruction_operands(compare, "lt").is_some_and(|operands| {
                    operands.len() == 3
                        && operands[0] == remapped_alpha
                        && operands[1] == remapped_alpha
                        && operands[2] == "l(0.000000)"
                }) {
                    return lines
                        .get(compare_offset + 1)
                        .and_then(|line| instruction_operands(line, "discard_nz"))
                        .is_some_and(|operands| {
                            operands.len() == 1 && operands[0] == remapped_alpha
                        });
                }
                if instruction_writes_register_component(compare, remapped_alpha) {
                    return false;
                }
            }
            return false;
        }
        if instruction_writes_register_component(line, remapped_alpha) {
            return false;
        }
    }
    false
}

#[cfg(windows)]
fn alpha_shaping_shape_destination(lines: &[&str], line_index: usize) -> Option<String> {
    let operands = instruction_operands(lines.get(line_index)?, "mul")?;
    if operands.len() != 3 || operands[0] != operands[1] || operands[2] != "cb0[13].x" {
        return None;
    }
    let shape = operands[0];
    let exponent = instruction_operands(lines.get(line_index.checked_sub(1)?)?, "exp")?;
    let aperture = instruction_operands(lines.get(line_index.checked_sub(2)?)?, "mul")?;
    let logarithm = instruction_operands(lines.get(line_index.checked_sub(3)?)?, "log")?;
    (exponent.as_slice() == [shape, shape]
        && aperture.as_slice() == [shape, shape, "cb0[12].w"]
        && logarithm.as_slice() == [shape, shape])
    .then(|| shape.to_string())
}

#[cfg(windows)]
fn alpha_shaping_dot_operands<'a>(
    lines: &'a [&'a str],
    offset_index: usize,
) -> Option<(&'a str, &'a str)> {
    let shape = instruction_operands(lines.get(offset_index)?, "mul")?
        .first()?
        .to_string();
    let log = instruction_operands(lines.get(offset_index.checked_sub(3)?)?, "log")?;
    let add = instruction_operands(lines.get(offset_index.checked_sub(4)?)?, "add")?;
    let clamp_line = lines.get(offset_index.checked_sub(5)?)?;
    let dp3 = instruction_operands(lines.get(offset_index.checked_sub(6)?)?, "dp3")?;
    let has_min_clamp = instruction_operands(clamp_line, "min").is_some_and(|operands| {
        operands.len() == 3
            && operands[0] == shape
            && operands[1] == format!("|{shape}|")
            && operands[2] == "l(1.000000)"
    });
    let has_saturate_clamp = instruction_operands(clamp_line, "mov_sat").is_some_and(|operands| {
        operands.len() == 2 && operands[0] == shape && operands[1] == format!("|{shape}|")
    });
    if log.len() != 2
        || log[0] != shape
        || log[1] != shape
        || add.len() != 3
        || add[0] != shape
        || add[1] != format!("-{shape}")
        || dp3.len() != 3
        || dp3[0] != shape
        || (!has_min_clamp && !has_saturate_clamp)
    {
        return None;
    }
    Some((dp3[1], dp3[2]))
}

#[cfg(windows)]
fn alpha_shaping_dot_has_view_from_v6(
    lines: &[&str],
    offset_index: usize,
    left: &str,
    right: &str,
) -> bool {
    [left, right]
        .into_iter()
        .any(|operand| alpha_shaping_operand_has_view_from_v6(lines, offset_index, operand))
}

#[cfg(windows)]
fn alpha_shaping_normal_dot_operand<'a>(
    lines: &[&str],
    offset_index: usize,
    left: &'a str,
    right: &'a str,
) -> Option<&'a str> {
    match (
        alpha_shaping_operand_has_view_from_v6(lines, offset_index, left),
        alpha_shaping_operand_has_view_from_v6(lines, offset_index, right),
    ) {
        (true, false) => Some(right),
        (false, true) => Some(left),
        _ => None,
    }
}

#[cfg(windows)]
fn alpha_shaping_operand_has_view_from_v6(
    lines: &[&str],
    offset_index: usize,
    operand: &str,
) -> bool {
    let Some(base) = dxbc_register_base(operand) else {
        return false;
    };
    lines[..offset_index]
        .iter()
        .rev()
        .find_map(|line| {
            let (_, operands) = line.split_once(' ')?;
            let operands = split_instruction_operands(operands);
            let destination = operands.first()?;
            if !register_components_overlap(destination, operand) {
                return None;
            }
            let is_view = operands[1..]
                .iter()
                .any(|source| source.contains("-v6.") || source == &"-v6");
            Some(is_view && dxbc_register_base(destination) == Some(base))
        })
        .unwrap_or(false)
}

#[cfg(windows)]
fn alpha_shaping_operand_producer_opcode(
    lines: &[&str],
    offset_index: usize,
    operand: &str,
) -> Option<String> {
    lines[..offset_index].iter().rev().find_map(|line| {
        let (opcode, operands) = line.split_once(' ')?;
        let operands = split_instruction_operands(operands);
        operands
            .first()
            .filter(|destination| register_components_overlap(destination, operand))
            .map(|_| opcode.to_string())
    })
}

#[cfg(windows)]
fn alpha_shaping_composition<'a>(
    lines: &[&'a str],
    shape_register: &str,
) -> Option<(usize, &'a str, &'a str)> {
    lines.iter().enumerate().find_map(|(index, line)| {
        let operands = instruction_operands(line, "mad_sat")?;
        (operands.len() == 4 && operands[1] == shape_register)
            .then(|| (index, operands[2], operands[3]))
    })
}

#[cfg(windows)]
fn alpha_shaping_operand_root_sources(
    lines: &[&str],
    before_index: usize,
    operand: &str,
    texture_bindings: &BTreeMap<u32, String>,
) -> BTreeSet<String> {
    fn trace(
        lines: &[&str],
        before_index: usize,
        operand: &str,
        texture_bindings: &BTreeMap<u32, String>,
        depth: usize,
        visited: &mut BTreeSet<(usize, String)>,
        roots: &mut BTreeSet<String>,
    ) {
        if depth == 0 {
            roots.insert("truncated".to_string());
            return;
        }
        let Some(base) = dxbc_operand_base(operand) else {
            if operand.starts_with("l(") {
                roots.insert("literal".to_string());
            }
            return;
        };
        if base.starts_with('v') {
            roots.insert(format!("vertex:{base}"));
            return;
        }
        if base.starts_with("cb") {
            roots.insert(format!("constant:{base}"));
            return;
        }
        if base.starts_with('t') {
            let source = dxbc_resource_slot(base, 't')
                .and_then(|slot| texture_bindings.get(&slot))
                .cloned()
                .unwrap_or_else(|| base.to_string());
            roots.insert(format!("texture:{source}"));
            return;
        }
        let key = (before_index, operand.to_string());
        if !visited.insert(key) {
            roots.insert("cycle".to_string());
            return;
        }
        let Some((line_index, opcode, operands)) = lines[..before_index]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(line_index, line)| {
                let (opcode, rest) = line.split_once(' ')?;
                let operands = split_instruction_operands(rest);
                let writes_operand = operands
                    .first()
                    .is_some_and(|destination| register_components_overlap(destination, operand));
                writes_operand.then_some((line_index, opcode, operands))
            })
        else {
            roots.insert(format!("unresolved:{base}"));
            return;
        };
        let mut traced_input = false;
        for source in operands.iter().skip(1) {
            let Some(source_base) = dxbc_operand_base(source) else {
                if source.starts_with("l(") {
                    roots.insert("literal".to_string());
                }
                continue;
            };
            if source_base.starts_with('s') {
                continue;
            }
            traced_input = true;
            trace(
                lines,
                line_index,
                source,
                texture_bindings,
                depth - 1,
                visited,
                roots,
            );
        }
        if !traced_input {
            roots.insert(format!("opcode:{opcode}"));
        }
    }

    let mut roots = BTreeSet::new();
    trace(
        lines,
        before_index,
        operand,
        texture_bindings,
        48,
        &mut BTreeSet::new(),
        &mut roots,
    );
    roots
}

#[cfg(windows)]
fn alpha_shaping_has_scaled_alpha(lines: &[&str], shape_register: &str) -> bool {
    let Some((composition_index, _, _)) = alpha_shaping_composition(lines, shape_register) else {
        return false;
    };
    lines[..composition_index].iter().any(|line| {
        instruction_operands(line, "mul_sat")
            .is_some_and(|operands| operands.len() == 3 && operands[2].contains("3.333333"))
    })
}

#[cfg(windows)]
fn alpha_shaping_has_saturated_composition(lines: &[&str], shape_register: &str) -> bool {
    alpha_shaping_composition(lines, shape_register).is_some()
}

#[cfg(windows)]
fn alpha_shaping_has_offset_sign_gate(lines: &[&str]) -> bool {
    lines.iter().enumerate().any(|(index, line)| {
        let Some(ge) = instruction_operands(line, "ge") else {
            return false;
        };
        if ge.len() != 3 || ge[1] != "l(0.000000)" || ge[2] != "cb0[13].x" {
            return false;
        }
        lines.iter().skip(index + 1).take(3).any(|line| {
            instruction_operands(line, "movc")
                .is_some_and(|movc| movc.len() == 4 && movc[1] == ge[0])
        })
    })
}

#[cfg(windows)]
fn alpha_shaping_has_alpha_less_than_one_gate(lines: &[&str]) -> bool {
    lines.iter().any(|line| {
        instruction_operands(line, "lt")
            .is_some_and(|operands| operands.len() == 3 && operands[2] == "l(1.000000)")
    })
}

#[cfg(windows)]
fn alpha_shaping_downstream_use(
    lines: &[&str],
    offset_index: usize,
    shape_register: &str,
) -> Option<String> {
    for line in lines.iter().skip(offset_index + 1) {
        let (opcode, operand_text) = line.split_once(' ')?;
        let operands = split_instruction_operands(operand_text);
        if let Some((operand_index, _)) = operands[1..]
            .iter()
            .enumerate()
            .find(|(_, operand)| register_components_overlap(operand, shape_register))
        {
            return Some(format!("{opcode}:operand{}", operand_index + 1));
        }
        if operands
            .first()
            .is_some_and(|destination| register_components_overlap(destination, shape_register))
        {
            return None;
        }
    }
    None
}

#[cfg(windows)]
fn tile_bias_add_destination(line: &str) -> Option<String> {
    let operands = line.strip_prefix("add ")?.split(", ").collect::<Vec<_>>();
    if operands.len() != 3 || operands[0] != operands[1] || !tile_bias_constant_operand(operands[2])
    {
        return None;
    }
    Some(operands[0].to_string())
}

#[cfg(windows)]
fn texture_mip_bias_add_destination(line: &str) -> Option<String> {
    let operands = line.strip_prefix("add ")?.split(", ").collect::<Vec<_>>();
    if operands.len() != 3
        || !operands
            .iter()
            .any(|operand| *operand == "cb0[14].y" || *operand == "cb0[14].yyyy")
        || !operands.iter().any(|operand| {
            matches!(
                operand,
                &"cb1[0].w" | &"cb1[0].wwww" | &"cb2[0].w" | &"cb2[0].wwww"
            )
        })
    {
        return None;
    }
    Some(operands[0].to_string())
}

#[cfg(windows)]
fn tile_bias_constant_operand(operand: &str) -> bool {
    operand
        .strip_prefix("cb0[17].")
        .is_some_and(|swizzle| !swizzle.is_empty() && swizzle.bytes().all(|byte| byte == b'y'))
}

#[cfg(windows)]
fn tile_bias_formula_is_expected(lines: &[&str], add_index: usize, bias_register: &str) -> bool {
    let start = add_index.saturating_sub(6);
    let prefix = &lines[start..add_index];
    let has_scale = prefix.iter().any(|line| {
        line.starts_with("mul ")
            && line.contains("0.007812")
            && instruction_writes_register_component(line, bias_register)
    });
    let has_log = prefix.iter().any(|line| {
        instruction_operands(line, "log").is_some_and(|operands| {
            operands.len() == 2 && operands[0] == bias_register && operands[1] == bias_register
        })
    });
    let has_nonnegative_clamp = prefix.iter().any(|line| {
        instruction_operands(line, "max").is_some_and(|operands| {
            operands.len() == 3
                && operands[0] == bias_register
                && operands[1] == bias_register
                && operands[2].contains("0.000000")
        })
    });
    has_scale && has_log && has_nonnegative_clamp
}

#[cfg(windows)]
fn instruction_operands<'a>(line: &'a str, opcode: &str) -> Option<Vec<&'a str>> {
    let operands = line.strip_prefix(opcode)?.strip_prefix(' ')?;
    Some(operands.split(", ").collect())
}

#[cfg(windows)]
fn instruction_operands_with_vectors<'a>(line: &'a str, opcode: &str) -> Option<Vec<&'a str>> {
    let operands = line.strip_prefix(opcode)?.strip_prefix(' ')?;
    Some(split_instruction_operands(operands))
}

#[cfg(windows)]
fn split_instruction_operands(operands: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut depth = 0;
    for (index, character) in operands.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(operands[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    result.push(operands[start..].trim());
    result
}

#[cfg(windows)]
fn dxbc_sample_texture_name(
    line: &str,
    texture_bindings: &BTreeMap<u32, String>,
) -> Option<String> {
    if !line.starts_with("sample") {
        return None;
    }
    let operands = line.split_once(' ')?.1.split(", ");
    let slot = operands
        .filter_map(|operand| dxbc_resource_slot(operand, 't'))
        .next()?;
    Some(
        texture_bindings
            .get(&slot)
            .cloned()
            .unwrap_or_else(|| format!("t{slot}")),
    )
}

#[cfg(windows)]
fn dxbc_instruction_destination(line: &str) -> Option<&str> {
    let (_, operands) = line.split_once(' ')?;
    split_instruction_operands(operands).into_iter().next()
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DxbcTableRow {
    A,
    B,
}

#[cfg(windows)]
fn dxbc_row_provenance_at(
    lines: &[&str],
    texture_bindings: &BTreeMap<u32, String>,
    end_index: usize,
    target: &str,
) -> Option<DxbcTableRow> {
    let mut provenance = BTreeMap::<String, DxbcTableRow>::new();
    for line in lines.iter().take(end_index + 1) {
        let Some((opcode, operand_text)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(operand_text);
        let Some(destination) = operands.first() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }

        let propagated_rows = destination_components
            .iter()
            .filter_map(|destination_component| {
                let lane = dxbc_component_lane(destination_component)?;
                let rows = operands[1..]
                    .iter()
                    .filter_map(|operand| dxbc_operand_row_for_lane(operand, lane, &provenance))
                    .collect::<BTreeSet<_>>();
                (rows.len() == 1).then(|| {
                    (
                        destination_component.clone(),
                        *rows.first().expect("one propagated row"),
                    )
                })
            })
            .collect::<Vec<_>>();
        let table_sample_row = (opcode.starts_with("sample")
            && dxbc_sample_texture_name(line, texture_bindings).as_deref()
                == Some("g_SamplerTable.T"))
        .then(|| {
            operands
                .get(1)
                .and_then(|coordinates| dxbc_operand_row_for_lane(coordinates, 1, &provenance))
        })
        .flatten();
        for component in &destination_components {
            provenance.remove(component);
        }

        if opcode == "add" {
            let seeded_rows = dxbc_table_row_seeds(&destination_components, &operands[1..]);
            if !seeded_rows.is_empty() {
                provenance.extend(seeded_rows);
                continue;
            }
        }

        if opcode.starts_with("sample") {
            if let Some(row) = table_sample_row {
                for component in destination_components {
                    provenance.insert(component, row);
                }
            }
            continue;
        }

        provenance.extend(propagated_rows);
    }

    dxbc_operand_components(target)
        .into_iter()
        .filter_map(|component| provenance.get(&component).copied())
        .next()
}

#[cfg(windows)]
fn dxbc_table_row_seeds(
    destination_components: &[String],
    source_operands: &[&str],
) -> Vec<(String, DxbcTableRow)> {
    let Some(values) = source_operands.iter().find_map(|operand| {
        operand
            .strip_prefix("l(")
            .and_then(|operand| operand.strip_suffix(')'))
            .map(|operand| operand.split(", ").collect::<Vec<_>>())
    }) else {
        return Vec::new();
    };
    if !values.contains(&"0.500000") || !values.contains(&"1.500000") {
        return Vec::new();
    }

    destination_components
        .iter()
        .filter_map(|component| {
            let component_index = match component.as_bytes().last()? {
                b'x' => 0,
                b'y' => 1,
                b'z' => 2,
                b'w' => 3,
                _ => return None,
            };
            let row = match *values.get(component_index)? {
                "0.500000" => DxbcTableRow::A,
                "1.500000" => DxbcTableRow::B,
                _ => return None,
            };
            Some((component.clone(), row))
        })
        .collect()
}

#[cfg(windows)]
fn dxbc_operand_components(value: &str) -> Vec<String> {
    let value = value.trim_start_matches('-').trim_matches('|');
    let Some((base, mask)) = dxbc_register(value) else {
        return Vec::new();
    };
    mask.unwrap_or("xyzw")
        .chars()
        .map(|component| format!("{base}.{component}"))
        .collect()
}

#[cfg(windows)]
fn dxbc_component_lane(component: &str) -> Option<usize> {
    match component.as_bytes().last()? {
        b'x' => Some(0),
        b'y' => Some(1),
        b'z' => Some(2),
        b'w' => Some(3),
        _ => None,
    }
}

#[cfg(windows)]
fn dxbc_operand_row_for_lane(
    value: &str,
    lane: usize,
    provenance: &BTreeMap<String, DxbcTableRow>,
) -> Option<DxbcTableRow> {
    let value = value.trim_start_matches('-').trim_matches('|');
    let (base, mask) = dxbc_register(value)?;
    let mask = mask.unwrap_or("xyzw");
    let source_component = if mask.len() == 1 {
        mask.chars().next()?
    } else {
        mask.chars().nth(lane)?
    };
    provenance
        .get(&format!("{base}.{source_component}"))
        .copied()
}

#[cfg(windows)]
fn dxbc_literal_provenance_at(
    lines: &[&str],
    end_index: usize,
    target: &str,
    target_lane: usize,
) -> Option<String> {
    let mut provenance = BTreeMap::<String, String>::new();
    for line in lines.iter().take(end_index + 1) {
        let Some((opcode, operand_text)) = line.split_once(' ') else {
            continue;
        };
        let operands = split_instruction_operands(operand_text);
        let Some(destination) = operands.first() else {
            continue;
        };
        let destination_components = dxbc_operand_components(destination);
        if destination_components.is_empty() {
            continue;
        }
        let propagated = destination_components
            .iter()
            .filter_map(|destination_component| {
                let lane = dxbc_component_lane(destination_component)?;
                operands[1..].iter().find_map(|operand| {
                    dxbc_operand_literal_for_lane(operand, lane, &provenance)
                        .map(|value| (destination_component.clone(), value))
                })
            })
            .collect::<Vec<_>>();
        for component in &destination_components {
            provenance.remove(component);
        }
        if opcode == "div" {
            if let Some(literal) = operands.get(1) {
                for component in &destination_components {
                    let Some(lane) = dxbc_component_lane(component) else {
                        continue;
                    };
                    if let Some(value) = dxbc_immediate_value_for_lane(literal, lane) {
                        provenance.insert(component.clone(), value.to_string());
                    }
                }
                continue;
            }
        }
        provenance.extend(propagated);
    }

    dxbc_operand_component_for_lane(target, target_lane)
        .and_then(|component| provenance.get(&component).cloned())
}

#[cfg(windows)]
fn dxbc_operand_literal_for_lane(
    value: &str,
    lane: usize,
    provenance: &BTreeMap<String, String>,
) -> Option<String> {
    let component = dxbc_operand_component_for_lane(value, lane)?;
    provenance.get(&component).cloned()
}

#[cfg(windows)]
fn dxbc_operand_component_for_lane(value: &str, lane: usize) -> Option<String> {
    let value = value.trim_start_matches('-').trim_matches('|');
    let (base, mask) = dxbc_register(value)?;
    let mask = mask.unwrap_or("xyzw");
    let source_component = if mask.len() == 1 {
        mask.chars().next()?
    } else {
        mask.chars().nth(lane)?
    };
    Some(format!("{base}.{source_component}"))
}

#[cfg(windows)]
fn dxbc_immediate_value_for_lane(value: &str, lane: usize) -> Option<&str> {
    value
        .strip_prefix("l(")?
        .strip_suffix(')')?
        .split(", ")
        .nth(lane)
}

#[cfg(windows)]
fn tile_orb_neutral_pair(lines: &[&str], add_index: usize) -> Option<(String, String, usize)> {
    let add = instruction_operands_with_vectors(*lines.get(add_index)?, "add")?;
    if add.len() != 3 || !tile_orb_negative_neutral_constant(add[2]) {
        return None;
    }
    let destination = add[0];
    for mad_index in add_index + 1..(add_index + 18).min(lines.len()) {
        let Some(mad) = instruction_operands_with_vectors(lines[mad_index], "mad") else {
            continue;
        };
        if mad.len() == 4
            && dxbc_register_base(mad[0]) == dxbc_register_base(destination)
            && dxbc_register_base(mad[2]) == dxbc_register_base(destination)
            && tile_orb_positive_neutral_constant(mad[3])
        {
            return Some((mad[0].to_string(), mad[1].to_string(), mad_index));
        }
    }
    None
}

#[cfg(windows)]
fn tile_orb_negative_neutral_constant(value: &str) -> bool {
    let values = value
        .strip_prefix("l(")
        .and_then(|value| value.strip_suffix(')'))
        .map(|value| value.split(", ").collect::<Vec<_>>());
    values.is_some_and(|values| {
        values.len() == 4
            && values.iter().filter(|value| **value == "-1.000000").count() >= 2
            && values
                .iter()
                .all(|value| matches!(*value, "-1.000000" | "-0.500000" | "0.000000"))
    })
}

#[cfg(windows)]
fn tile_orb_positive_neutral_constant(value: &str) -> bool {
    let values = value
        .strip_prefix("l(")
        .and_then(|value| value.strip_suffix(')'))
        .map(|value| value.split(", ").collect::<Vec<_>>());
    values.is_some_and(|values| {
        values.len() == 4
            && values.iter().any(|value| *value == "1.000000")
            && values
                .iter()
                .all(|value| matches!(*value, "1.000000" | "0.500000" | "0.000000"))
    })
}

#[cfg(windows)]
fn index_blend_inversion_after_sample(lines: &[&str], sample_index: usize) -> bool {
    let Some((_, operands)) = lines[sample_index].split_once(' ') else {
        return false;
    };
    let Some(destination) = operands.split(", ").next() else {
        return false;
    };
    let Some(base) = dxbc_register_base(destination) else {
        return false;
    };
    let Some((_, mask)) = destination.split_once('.') else {
        return false;
    };
    let Some(green_component) = mask.chars().nth(1) else {
        return false;
    };
    let source = format!("{base}.{green_component}");
    let end = (sample_index + 12).min(lines.len());
    lines[sample_index + 1..end].iter().any(|line| {
        instruction_operands_with_vectors(line, "add").is_some_and(|operands| {
            operands.len() == 3
                && operands[1].strip_prefix('-') == Some(source.as_str())
                && operands[2].contains("1.000000")
        })
    })
}

#[cfg(windows)]
fn shaping_dependent_table_sample(lines: &[&str], sample_index: usize) -> bool {
    let Some((_, operands)) = lines[sample_index].split_once(' ') else {
        return false;
    };
    let Some(destination) = operands.split(", ").next() else {
        return false;
    };
    let end = (sample_index + 32).min(lines.len());
    let suffix = &lines[sample_index + 1..end];
    let has_reciprocal = suffix.iter().any(|line| {
        line.starts_with(&format!("div {destination}, l(1.000000"))
            && line.ends_with(&format!(", {destination}"))
    });
    let has_power = suffix
        .iter()
        .any(|line| line.starts_with(&format!("mul {destination},")) && line.contains(destination))
        && suffix
            .iter()
            .any(|line| *line == format!("exp {destination}, {destination}"));
    let has_one_minus = suffix
        .iter()
        .any(|line| line.starts_with(&format!("add {destination}, -{destination}, l(1.000000")));
    has_reciprocal && has_power && has_one_minus
}

#[cfg(windows)]
struct DxbcLinearBlend<'a> {
    source_a: &'a str,
    source_b: &'a str,
    weight: &'a str,
    mad_index: usize,
}

#[cfg(windows)]
fn dxbc_linear_blend<'a>(lines: &'a [&'a str], add_index: usize) -> Option<DxbcLinearBlend<'a>> {
    let add = instruction_operands_with_vectors(*lines.get(add_index)?, "add")?;
    if add.len() != 3 {
        return None;
    }
    let (source_a, source_b) = if add[1].starts_with('-') {
        (add[1].strip_prefix('-')?, add[2])
    } else if add[2].starts_with('-') {
        (add[2].strip_prefix('-')?, add[1])
    } else {
        return None;
    };
    for mad_index in add_index + 1..(add_index + 3).min(lines.len()) {
        let Some(mad) = instruction_operands_with_vectors(lines[mad_index], "mad") else {
            continue;
        };
        if mad.len() == 4
            && dxbc_register_base(mad[0]) == dxbc_register_base(add[0])
            && dxbc_register_base(mad[2]) == dxbc_register_base(add[0])
            && dxbc_register_base(mad[3]) == dxbc_register_base(source_a)
        {
            return Some(DxbcLinearBlend {
                source_a,
                source_b,
                weight: mad[1],
                mad_index,
            });
        }
    }
    None
}

#[cfg(windows)]
fn dxbc_weighted_result_blend(line: &str, weight: &str) -> bool {
    instruction_operands_with_vectors(line, "mad").is_some_and(|operands| {
        operands.len() == 4
            && operands[1] == weight
            && dxbc_register_base(operands[0]) == dxbc_register_base(operands[2])
            && dxbc_register_base(operands[3]).is_some()
    })
}

#[cfg(windows)]
fn dxbc_register_base(value: &str) -> Option<&str> {
    let value = value.strip_prefix('-').unwrap_or(value);
    let base = value.split('.').next()?;
    let digits = base
        .strip_prefix('r')
        .or_else(|| base.strip_prefix('o'))
        .or_else(|| base.strip_prefix('v'))?;
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())).then_some(base)
}

#[cfg(windows)]
fn dxbc_operand_base(value: &str) -> Option<&str> {
    let value = value.trim_matches('|').strip_prefix('-').unwrap_or(value);
    let base = value.split('.').next()?;
    (base.starts_with('r')
        || base.starts_with('v')
        || base.starts_with("cb")
        || base.starts_with('t'))
    .then_some(base)
}

#[cfg(windows)]
fn tile_bias_consumers(
    lines: &[&str],
    add_index: usize,
    bias_register: &str,
    texture_bindings: &BTreeMap<u32, String>,
) -> BTreeSet<String> {
    let mut consumers = BTreeSet::new();
    for line in &lines[add_index + 1..] {
        if line.starts_with("sample_b") {
            let operands = line
                .split_once(' ')
                .map(|(_, operands)| operands.split(", ").collect::<Vec<_>>())
                .unwrap_or_default();
            if operands.last() == Some(&bias_register) {
                if let Some(slot) = operands
                    .iter()
                    .find_map(|operand| dxbc_resource_slot(operand, 't'))
                {
                    consumers.insert(
                        texture_bindings
                            .get(&slot)
                            .cloned()
                            .unwrap_or_else(|| format!("t{slot}")),
                    );
                }
            }
        }
        if instruction_writes_register_component(line, bias_register) {
            break;
        }
    }
    consumers
}

#[cfg(windows)]
fn texture_mip_bias_consumers(
    lines: &[&str],
    add_index: usize,
    bias_register: &str,
    texture_bindings: &BTreeMap<u32, String>,
) -> BTreeSet<String> {
    tile_bias_consumers(lines, add_index, bias_register, texture_bindings)
}

#[cfg(windows)]
fn instruction_writes_register_component(line: &str, register: &str) -> bool {
    let Some((_, operands)) = line.split_once(' ') else {
        return false;
    };
    let Some((destination, _)) = operands.split_once(',') else {
        return false;
    };
    register_components_overlap(destination.trim(), register)
}

#[cfg(windows)]
fn register_components_overlap(destination: &str, register: &str) -> bool {
    let Some((destination_base, destination_mask)) = dxbc_register(destination) else {
        return false;
    };
    let Some((register_base, register_mask)) = dxbc_register(register) else {
        return false;
    };
    if destination_base != register_base {
        return false;
    }
    match (destination_mask, register_mask) {
        (None, _) | (_, None) => true,
        (Some(destination_mask), Some(register_mask)) => register_mask
            .bytes()
            .any(|component| destination_mask.as_bytes().contains(&component)),
    }
}

#[cfg(windows)]
fn dxbc_register(value: &str) -> Option<(&str, Option<&str>)> {
    let (base, mask) = value
        .split_once('.')
        .map_or((value, None), |(base, mask)| (base, Some(mask)));
    let digits = base
        .strip_prefix('r')
        .or_else(|| base.strip_prefix('o'))
        .or_else(|| base.strip_prefix('v'))?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((base, mask))
}

#[cfg(windows)]
fn assert_installed_tile_mip_dxbc_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(report.tile_mip_dxbc.len(), 2);
    let package = |name: &str| {
        report
            .tile_mip_dxbc
            .iter()
            .find(|package| package.shader_package_name == name)
            .unwrap_or_else(|| panic!("missing {name} DXBC tile-mip audit"))
    };

    let character = package("character.shpk");
    assert_eq!(character.pixel_shader_count, 1038);
    assert_eq!(character.declared_shader_count, 1024);
    assert_eq!(character.consumer_shader_count, 600);
    assert_eq!(character.use_count, 1200);
    assert_eq!(character.formula_count, 1200);
    assert_eq!(
        character.uses_per_shader,
        BTreeMap::from([(0, 424), (2, 600)])
    );
    assert_eq!(
        character.consumer_sets,
        BTreeMap::from([
            ("g_SamplerTileNormal,g_SamplerTileOrb.T".to_string(), 1072),
            ("g_SamplerTileOrb.T".to_string(), 128),
        ])
    );

    let legacy = package("characterlegacy.shpk");
    assert_eq!(legacy.pixel_shader_count, 1740);
    assert_eq!(legacy.declared_shader_count, 1728);
    assert_eq!(legacy.consumer_shader_count, 1728);
    assert_eq!(legacy.use_count, 3456);
    assert_eq!(legacy.formula_count, 3456);
    assert_eq!(legacy.uses_per_shader, BTreeMap::from([(2, 1728)]));
    assert_eq!(
        legacy.consumer_sets,
        BTreeMap::from([
            ("g_SamplerTileNormal,g_SamplerTileOrb.T".to_string(), 2304),
            ("g_SamplerTileOrb.T".to_string(), 1152),
        ])
    );
}

#[cfg(windows)]
fn assert_installed_tile_blend_dxbc_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(report.tile_blend_dxbc.len(), 2);
    let package = |name: &str| {
        report
            .tile_blend_dxbc
            .iter()
            .find(|package| package.shader_package_name == name)
            .unwrap_or_else(|| panic!("missing {name} DXBC tile-blend audit"))
    };
    let character = package("character.shpk");
    assert_eq!(character.pixel_shader_count, 1038);
    assert_eq!(character.orb_neutral_pair_count, 1440);
    assert_eq!(character.orb_blend_pair_count, 720);
    assert_eq!(character.ordered_ab_blend_pair_count, 720);
    assert_eq!(character.normal_blend_pair_count, 720);
    assert_eq!(character.shaping_table_sample_count, 768);
    assert_eq!(character.shaping_anisotropy_a_sample_count, 768);
    assert_eq!(character.index_texture_sample_count, 1024);
    assert_eq!(character.inverted_index_weight_count, 1024);

    let legacy = package("characterlegacy.shpk");
    assert_eq!(legacy.pixel_shader_count, 1740);
    assert_eq!(legacy.orb_neutral_pair_count, 552);
    assert_eq!(legacy.orb_blend_pair_count, 276);
    assert_eq!(legacy.ordered_ab_blend_pair_count, 276);
    assert_eq!(legacy.normal_blend_pair_count, 52);
    assert_eq!(legacy.shaping_table_sample_count, 0);
    assert_eq!(legacy.shaping_anisotropy_a_sample_count, 0);
    assert_eq!(legacy.index_texture_sample_count, 1728);
    assert_eq!(legacy.inverted_index_weight_count, 1728);
}

#[cfg(windows)]
fn assert_installed_texture_mip_dxbc_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(report.texture_mip_dxbc.len(), 3);
    let package = |name: &str| {
        report
            .texture_mip_dxbc
            .iter()
            .find(|package| package.shader_package_name == name)
            .unwrap_or_else(|| panic!("missing {name} DXBC texture-mip audit"))
    };
    let character = package("character.shpk");
    assert_eq!(character.pixel_shader_count, 1038);
    assert_eq!(character.declared_shader_count, 1032);
    assert_eq!(character.consumer_shader_count, 1032);
    assert_eq!(character.sum_count, 1032);
    assert_eq!(
        character.consumer_sets,
        BTreeMap::from([
            (
                "g_SamplerDiffuse.T,g_SamplerMask.T,g_SamplerNormal.T".to_string(),
                480,
            ),
            ("g_SamplerMask.T,g_SamplerNormal.T".to_string(), 544),
            ("g_SamplerNormal.T".to_string(), 8),
        ])
    );

    let legacy = package("characterlegacy.shpk");
    assert_eq!(legacy.pixel_shader_count, 1740);
    assert_eq!(legacy.declared_shader_count, 1734);
    assert_eq!(legacy.consumer_shader_count, 1734);
    assert_eq!(legacy.sum_count, 1734);
    assert_eq!(
        legacy.consumer_sets,
        BTreeMap::from([
            (
                "g_SamplerDiffuse.T,g_SamplerMask.T,g_SamplerNormal.T".to_string(),
                1056,
            ),
            ("g_SamplerDiffuse.T,g_SamplerNormal.T".to_string(), 96),
            ("g_SamplerMask.T,g_SamplerNormal.T".to_string(), 576),
            ("g_SamplerNormal.T".to_string(), 6),
        ])
    );

    let glass = package("characterglass.shpk");
    assert_eq!(glass.pixel_shader_count, 38);
    assert_eq!(glass.declared_shader_count, 34);
    assert_eq!(glass.consumer_shader_count, 34);
    assert_eq!(glass.sum_count, 34);
    assert_eq!(
        glass.consumer_sets,
        BTreeMap::from([
            ("g_SamplerMask.T,g_SamplerNormal.T".to_string(), 24),
            ("g_SamplerNormal.T".to_string(), 10),
        ])
    );
}

#[cfg(windows)]
fn assert_installed_material_strength_dxbc_boundary(report: &[MaterialStrengthDxbcPackageAudit]) {
    assert_eq!(report.len(), 3);
    let package = |name: &str| {
        report
            .iter()
            .find(|package| package.shader_package_name == name)
            .unwrap_or_else(|| panic!("missing {name} DXBC material-strength audit"))
    };

    let character = package("character.shpk");
    assert_eq!(character.pixel_shader_count, 1038);
    assert_eq!(character.roughness_sample_count, 976);
    assert_eq!(character.roughness_pixel_shader_count, 960);
    assert_eq!(character.roughness_consumer_sample_count, 976);
    assert_eq!(
        character.roughness_consumer_opcodes,
        BTreeMap::from([("mad".to_string(), 960), ("mul".to_string(), 16)])
    );
    assert_eq!(character.roughness_o1_y_reach_count, 768);
    assert_eq!(character.gloss_sample_count, 0);
    assert_eq!(character.gloss_pixel_shader_count, 0);
    assert_eq!(character.gloss_consumer_sample_count, 0);
    assert!(character.gloss_consumer_opcodes.is_empty());
    assert_eq!(character.gloss_o1_y_reach_count, 0);
    assert_eq!(character.gloss_o0_rgb_reach_count, 0);
    assert_eq!(character.gloss_power_chain_count, 0);
    assert_eq!(character.gloss_power_o0_rgb_reach_count, 0);
    assert_eq!(character.gloss_camera_reflection_power_chain_count, 0);
    assert_eq!(character.gloss_camera_reflection_lobe_count, 0);
    assert!(
        character
            .gloss_camera_reflection_lobe_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(character.gloss_cube_lod_sample_count, 0);
    assert_eq!(character.gloss_cube_sample_hdr_decode_count, 0);
    assert_eq!(character.gloss_cube_sample_o0_rgb_reach_count, 0);
    assert_eq!(character.gloss_cube_current_location_sample_count, 0);
    assert_eq!(character.gloss_cube_previous_location_sample_count, 0);
    assert_eq!(character.gloss_ambient_location_interpolation_count, 0);
    assert_eq!(character.gloss_ambient_reflection_scale_offset_count, 0);
    assert_eq!(character.gloss_ambient_bake_light_composition_count, 0);
    assert!(
        character
            .gloss_ambient_bake_light_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(character.gloss_environment_specular_strength_join_count, 0);
    assert!(
        character
            .gloss_environment_specular_strength_unjoined_pixel_shaders
            .is_empty()
    );
    assert_eq!(character.gloss_cube_specular_strength_pixel_shader_count, 0);
    assert_eq!(
        character.gloss_non_cube_specular_strength_pixel_shader_count,
        0
    );
    assert_eq!(character.gloss_texcoord4_w_environment_blend_count, 0);
    assert_eq!(character.gloss_gbuffer1_w_environment_blend_count, 0);
    assert!(
        character
            .gloss_environment_blend_unclassified_pixel_shaders
            .is_empty()
    );
    assert!(character.gloss_consumer_opcode_sequences.is_empty());
    assert!(character.gloss_consumer_classes.is_empty());
    assert_eq!(character.gloss_node_count, 0);
    assert!(character.gloss_material_key_sets.is_empty());
    assert_eq!(character.specular_strength_sample_count, 256);
    assert_eq!(character.specular_strength_pixel_shader_count, 256);
    assert_eq!(
        character.specular_strength_without_gloss_pixel_shader_count,
        256
    );
    assert_eq!(character.specular_strength_consumer_sample_count, 256);
    assert_eq!(
        character.specular_strength_consumer_opcodes,
        BTreeMap::from([("mul".to_string(), 256)])
    );
    assert!(
        character
            .specular_strength_terminal_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(character.specular_strength_composition_classes.len(), 1);
    let character_specular = &character.specular_strength_composition_classes[0];
    assert_eq!(character_specular.class_name, "cube_no_gloss_first_mad");
    assert_eq!(character_specular.pixel_shader_count, 256);
    assert_eq!(character_specular.product_o0_rgb_reach_count, 256);
    assert_eq!(
        character_specular.first_post_product_consumer_opcodes,
        BTreeMap::from([("mad".to_string(), 256)])
    );
    assert_eq!(character_specular.fifth_root_shaping_count, 0);
    assert_eq!(
        character_specular.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        256
    );
    assert_eq!(
        character_specular.dynamic_emissive_luminance_scale_composition_opcodes,
        BTreeMap::from([("mad".to_string(), 64), ("mul".to_string(), 192)])
    );
    assert_eq!(
        character_specular.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        256
    );
    assert_eq!(
        character_specular.dynamic_emissive_luminance_source_composition_opcodes,
        BTreeMap::from([("mad".to_string(), 256)])
    );
    for resource in [
        "g_SamplerOcclusion.T",
        "g_AmbientParam[0]",
        "g_AmbientParam[3]",
        "g_AmbientParam[4]",
        "g_InstanceParameter[0]",
        "g_InstanceParameter[2]",
        "g_MaterialParameter[5]",
        "g_PbrParameterCommon[0]",
    ] {
        let counts = if resource.starts_with("g_Sampler") {
            &character_specular.dynamic_emissive_luminance_scale_texture_resource_counts
        } else {
            &character_specular.dynamic_emissive_luminance_scale_constant_buffer_vector_counts
        };
        assert_eq!(counts.get(resource), Some(&256), "character {resource}");
    }
    assert!(
        character_specular
            .dynamic_emissive_luminance_scale_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(character_specular.node_count, 12288);
    assert_eq!(
        character_specular.pass_ids,
        BTreeMap::from([
            ("0x955c0b73".to_string(), 12288),
            ("0xc885bbd3".to_string(), 12288),
            ("0xf21a038f".to_string(), 12288),
        ])
    );
    assert_eq!(character_specular.material_key_sets.len(), 24);
    assert_eq!(character.gbuffer1_sample_count, 64);
    assert_eq!(character.gbuffer1_pixel_shader_count, 64);
    assert_eq!(
        character.gbuffer1_lane_sample_counts,
        BTreeMap::from([
            ("w".to_string(), 64),
            ("y".to_string(), 64),
            ("z".to_string(), 64),
        ])
    );
    assert_eq!(
        character.gbuffer1_lane_consumer_opcodes,
        BTreeMap::from([
            (
                "w".to_string(),
                BTreeMap::from([
                    ("add".to_string(), 64),
                    ("lt".to_string(), 64),
                    ("mul".to_string(), 64),
                ]),
            ),
            (
                "y".to_string(),
                BTreeMap::from([
                    ("add".to_string(), 320),
                    ("lt".to_string(), 64),
                    ("mad".to_string(), 128),
                    ("mov".to_string(), 64),
                    ("mul".to_string(), 256),
                ]),
            ),
            ("z".to_string(), BTreeMap::from([("mov".to_string(), 64)])),
        ])
    );
    assert_eq!(
        character.gbuffer1_lane_o0_rgb_reach_counts,
        BTreeMap::from([
            ("w".to_string(), 64),
            ("y".to_string(), 64),
            ("z".to_string(), 64),
        ])
    );
    assert!(character.gbuffer1_x_consumer_signatures.is_empty());
    assert_eq!(character.gbuffer1_x_terminal_multiplier_count, 0);
    assert_eq!(
        character.gbuffer1_x_terminal_multiplier_o0_rgb_reach_count,
        0
    );
    assert!(
        character
            .gbuffer1_x_terminal_multiplier_resource_counts
            .is_empty()
    );
    assert!(
        character
            .gbuffer1_x_post_multiplier_consumer_signatures
            .is_empty()
    );
    assert!(
        character
            .gbuffer1_x_post_multiplier_resource_counts
            .is_empty()
    );
    assert!(
        character
            .gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(character.gbuffer1_consumer_representatives.len(), 16);

    let legacy = package("characterlegacy.shpk");
    assert_eq!(legacy.pixel_shader_count, 1740);
    assert_eq!(legacy.roughness_sample_count, 0);
    assert_eq!(legacy.roughness_pixel_shader_count, 0);
    assert_eq!(legacy.roughness_consumer_sample_count, 0);
    assert!(legacy.roughness_consumer_opcodes.is_empty());
    assert_eq!(legacy.roughness_o1_y_reach_count, 0);
    assert_eq!(legacy.gloss_sample_count, 1712);
    assert_eq!(legacy.gloss_pixel_shader_count, 1712);
    assert_eq!(legacy.gloss_consumer_sample_count, 1712);
    assert_eq!(legacy.gloss_o1_y_reach_count, 272);
    assert_eq!(legacy.gloss_o0_rgb_reach_count, 1440);
    assert_eq!(legacy.gloss_power_chain_count, 1440);
    assert_eq!(legacy.gloss_power_o0_rgb_reach_count, 1440);
    assert_eq!(legacy.gloss_camera_reflection_power_chain_count, 1440);
    assert_eq!(legacy.gloss_camera_reflection_lobe_count, 1440);
    assert!(
        legacy
            .gloss_camera_reflection_lobe_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(legacy.gloss_cube_lod_sample_count, 2880);
    assert_eq!(legacy.gloss_cube_sample_hdr_decode_count, 2880);
    assert_eq!(legacy.gloss_cube_sample_o0_rgb_reach_count, 2880);
    assert_eq!(legacy.gloss_cube_current_location_sample_count, 1440);
    assert_eq!(legacy.gloss_cube_previous_location_sample_count, 1440);
    assert_eq!(legacy.gloss_ambient_location_interpolation_count, 1440);
    assert_eq!(legacy.gloss_ambient_reflection_scale_offset_count, 1440);
    assert_eq!(legacy.gloss_ambient_bake_light_composition_count, 1440);
    assert!(
        legacy
            .gloss_ambient_bake_light_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(legacy.gloss_environment_specular_strength_join_count, 864);
    assert!(
        legacy
            .gloss_environment_specular_strength_unjoined_pixel_shaders
            .is_empty()
    );
    assert_eq!(legacy.gloss_cube_specular_strength_pixel_shader_count, 864);
    assert_eq!(
        legacy.gloss_non_cube_specular_strength_pixel_shader_count,
        128
    );
    assert_eq!(legacy.gloss_texcoord4_w_environment_blend_count, 864);
    assert_eq!(legacy.gloss_gbuffer1_w_environment_blend_count, 576);
    assert!(
        legacy
            .gloss_environment_blend_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(
        legacy.gloss_consumer_opcodes,
        BTreeMap::from([
            ("add".to_string(), 2968),
            ("mad".to_string(), 1440),
            ("mov".to_string(), 864),
            ("movc".to_string(), 576),
            ("mul".to_string(), 1624),
        ])
    );
    assert_eq!(
        legacy.gloss_consumer_opcode_sequences,
        BTreeMap::from([
            ("add".to_string(), 88),
            ("add -> mad -> mov -> mul -> add".to_string(), 288),
            ("mul".to_string(), 184),
            ("mul -> add -> add -> mad -> mov".to_string(), 576),
            ("mul -> add -> add -> mad -> movc".to_string(), 576),
        ])
    );
    assert_eq!(legacy.gloss_consumer_classes.len(), 5);
    let gloss_class = |signature: &str| {
        legacy
            .gloss_consumer_classes
            .iter()
            .find(|class| class.consumer_signature == signature)
            .unwrap_or_else(|| panic!("missing Legacy Gloss consumer class {signature}"))
    };
    let add_mad_environment =
        gloss_class("add -> mad -> mov -> mul -> add | l(10.000000), l(9.000000)");
    assert_eq!(add_mad_environment.cube_lod_sample_count, 576);
    assert_eq!(add_mad_environment.cube_sample_hdr_decode_count, 576);
    assert_eq!(add_mad_environment.cube_sample_o0_rgb_reach_count, 576);
    assert_eq!(add_mad_environment.cube_current_location_sample_count, 288);
    assert_eq!(add_mad_environment.cube_previous_location_sample_count, 288);
    assert_eq!(
        add_mad_environment.ambient_location_interpolation_count,
        288
    );
    assert_eq!(
        add_mad_environment.ambient_reflection_scale_offset_count,
        288
    );
    assert_eq!(
        add_mad_environment.ambient_bake_light_composition_count,
        288
    );
    assert_eq!(
        add_mad_environment.environment_specular_strength_join_count,
        288
    );
    assert_eq!(add_mad_environment.texcoord4_w_environment_blend_count, 288);
    assert_eq!(add_mad_environment.gbuffer1_w_environment_blend_count, 0);
    let mov_environment =
        gloss_class("mul -> add -> add -> mad -> mov | l(9.000000), l(10.000000)");
    assert_eq!(mov_environment.cube_lod_sample_count, 1152);
    assert_eq!(mov_environment.cube_sample_hdr_decode_count, 1152);
    assert_eq!(mov_environment.cube_sample_o0_rgb_reach_count, 1152);
    assert_eq!(mov_environment.cube_current_location_sample_count, 576);
    assert_eq!(mov_environment.cube_previous_location_sample_count, 576);
    assert_eq!(mov_environment.ambient_location_interpolation_count, 576);
    assert_eq!(mov_environment.ambient_reflection_scale_offset_count, 576);
    assert_eq!(mov_environment.ambient_bake_light_composition_count, 576);
    assert_eq!(
        mov_environment.environment_specular_strength_join_count,
        576
    );
    assert_eq!(mov_environment.texcoord4_w_environment_blend_count, 576);
    assert_eq!(mov_environment.gbuffer1_w_environment_blend_count, 0);
    let movc_environment =
        gloss_class("mul -> add -> add -> mad -> movc | l(9.000000), l(10.000000)");
    assert_eq!(movc_environment.cube_lod_sample_count, 1152);
    assert_eq!(movc_environment.cube_sample_hdr_decode_count, 1152);
    assert_eq!(movc_environment.cube_sample_o0_rgb_reach_count, 1152);
    assert_eq!(movc_environment.cube_current_location_sample_count, 576);
    assert_eq!(movc_environment.cube_previous_location_sample_count, 576);
    assert_eq!(movc_environment.ambient_location_interpolation_count, 576);
    assert_eq!(movc_environment.ambient_reflection_scale_offset_count, 576);
    assert_eq!(movc_environment.ambient_bake_light_composition_count, 576);
    assert_eq!(movc_environment.environment_specular_strength_join_count, 0);
    assert_eq!(movc_environment.texcoord4_w_environment_blend_count, 0);
    assert_eq!(movc_environment.gbuffer1_w_environment_blend_count, 576);
    for (signature, samples, o1_y, o0_rgb, power, nodes, pass_ids) in [
        (
            "add -> mad -> mov -> mul -> add | l(10.000000), l(9.000000)",
            288,
            0,
            288,
            288,
            1536,
            BTreeMap::from([("0xc885bbd3".to_string(), 1536)]),
        ),
        (
            "add | l(-1.000000)",
            88,
            88,
            0,
            0,
            768,
            BTreeMap::from([
                ("0x03ac862e".to_string(), 640),
                ("0x6006067f".to_string(), 768),
            ]),
        ),
        (
            "mul -> add -> add -> mad -> mov | l(9.000000), l(10.000000)",
            576,
            0,
            576,
            576,
            3072,
            BTreeMap::from([
                ("0xc885bbd3".to_string(), 1536),
                ("0xf21a038f".to_string(), 3072),
            ]),
        ),
        (
            "mul -> add -> add -> mad -> movc | l(9.000000), l(10.000000)",
            576,
            0,
            576,
            576,
            3072,
            BTreeMap::from([("0x955c0b73".to_string(), 3072)]),
        ),
        (
            "mul | l(-0.066667)",
            184,
            184,
            0,
            0,
            2304,
            BTreeMap::from([
                ("0x03ac862e".to_string(), 2112),
                ("0x6006067f".to_string(), 2304),
            ]),
        ),
    ] {
        let class = gloss_class(signature);
        assert_eq!(class.sample_count, samples);
        assert_eq!(class.o1_y_reach_count, o1_y);
        assert_eq!(class.o0_rgb_reach_count, o0_rgb);
        assert_eq!(class.power_chain_count, power);
        assert_eq!(class.power_o0_rgb_reach_count, power);
        assert_eq!(class.camera_reflection_power_chain_count, power);
        assert_eq!(class.camera_reflection_lobe_count, power);
        assert_eq!(class.pixel_shader_count, samples);
        assert_eq!(class.node_count, nodes);
        assert_eq!(class.pass_ids, pass_ids);
        assert!(!class.representative_trace.is_empty());
    }
    let compatibility_mask = gloss_class("add | l(-1.000000)");
    assert_eq!(compatibility_mask.material_key_sets.len(), 6);
    assert!(
        compatibility_mask
            .material_key_sets
            .keys()
            .all(|keys| { keys.starts_with("B616DC5A=600EF9DF,C8BD1DEF=A02F4828,") })
    );
    assert_eq!(legacy.gloss_node_count, 3072);
    assert_eq!(legacy.gloss_material_key_sets.len(), 24);
    assert_eq!(legacy.specular_strength_sample_count, 1008);
    assert_eq!(legacy.specular_strength_pixel_shader_count, 1008);
    assert_eq!(
        legacy.specular_strength_without_gloss_pixel_shader_count,
        16
    );
    assert_eq!(legacy.specular_strength_consumer_sample_count, 1008);
    assert_eq!(
        legacy.specular_strength_consumer_opcodes,
        BTreeMap::from([("mul".to_string(), 1008)])
    );
    assert!(
        legacy
            .specular_strength_terminal_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(legacy.specular_strength_composition_classes.len(), 6);
    let specular_class = |name: &str| {
        legacy
            .specular_strength_composition_classes
            .iter()
            .find(|class| class.class_name == name)
            .unwrap_or_else(|| panic!("missing Legacy SpecularStrength class {name}"))
    };
    let cube_gloss_mul = specular_class("cube_gloss_first_mul");
    let cube_gloss_log = specular_class("cube_gloss_first_log");
    let non_cube_gloss_mul = specular_class("non_cube_gloss_first_mul");
    let non_cube_gloss_terminal = specular_class("non_cube_gloss_first_terminal");
    let no_gloss_mul = specular_class("no_gloss_first_mul");
    let no_gloss_terminal = specular_class("no_gloss_first_terminal");
    assert_eq!(
        cube_gloss_mul.pixel_shader_count + cube_gloss_log.pixel_shader_count,
        864
    );
    assert_eq!(
        cube_gloss_mul.product_o0_rgb_reach_count + cube_gloss_log.product_o0_rgb_reach_count,
        864
    );
    assert_eq!(
        non_cube_gloss_mul.pixel_shader_count
            + non_cube_gloss_terminal.pixel_shader_count
            + no_gloss_mul.pixel_shader_count
            + no_gloss_terminal.pixel_shader_count,
        144
    );
    assert_eq!(
        non_cube_gloss_mul.product_o0_rgb_reach_count
            + non_cube_gloss_terminal.product_o0_rgb_reach_count
            + no_gloss_mul.product_o0_rgb_reach_count
            + no_gloss_terminal.product_o0_rgb_reach_count,
        0
    );
    assert_eq!(cube_gloss_log.pixel_shader_count, 144);
    assert_eq!(cube_gloss_log.product_o0_rgb_reach_count, 144);
    assert_eq!(
        cube_gloss_log.product_other_resource_counts,
        BTreeMap::from([
            ("g_SamplerDecal.T".to_string(), 96),
            ("g_SamplerIndex.T".to_string(), 144),
            ("g_SamplerTable.T".to_string(), 144),
            ("g_SamplerTileOrb.T".to_string(), 144),
        ])
    );
    assert_eq!(
        cube_gloss_log.first_post_product_consumer_opcodes,
        BTreeMap::from([("log".to_string(), 144)])
    );
    assert_eq!(cube_gloss_log.fifth_root_shaping_count, 144);
    assert_eq!(
        cube_gloss_log.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        144
    );
    assert_eq!(
        cube_gloss_log.dynamic_emissive_luminance_scale_composition_opcodes,
        BTreeMap::from([("mad".to_string(), 36), ("mul".to_string(), 108)])
    );
    assert_eq!(
        cube_gloss_log.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        144
    );
    assert_eq!(
        cube_gloss_log.dynamic_emissive_luminance_source_composition_opcodes,
        BTreeMap::from([("mad".to_string(), 144)])
    );
    for resource in [
        "g_SamplerNormal.T",
        "g_SamplerOcclusion.T",
        "g_SamplerTable.T",
        "g_SamplerTileNormal",
        "g_SamplerTileOrb.T",
        "g_AmbientParam[0]",
        "g_AmbientParam[3]",
        "g_AmbientParam[4]",
        "g_InstanceParameter[0]",
        "g_InstanceParameter[2]",
        "g_MaterialParameter[5]",
        "g_MaterialParameter[13]",
    ] {
        let counts = if resource.starts_with("g_Sampler") {
            &cube_gloss_log.dynamic_emissive_luminance_scale_texture_resource_counts
        } else {
            &cube_gloss_log.dynamic_emissive_luminance_scale_constant_buffer_vector_counts
        };
        assert_eq!(counts.get(resource), Some(&144), "legacy log {resource}");
    }
    assert!(
        cube_gloss_log
            .dynamic_emissive_luminance_scale_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(cube_gloss_log.dynamic_emissive_o0_rgb_reach_count, 144);
    assert_eq!(
        cube_gloss_log.dynamic_emissive_table_join_o0_rgb_reach_count,
        144
    );
    assert_eq!(cube_gloss_log.instance_mul_color_o0_rgb_reach_count, 144);
    assert_eq!(cube_gloss_log.instance_env_parameter_o0_rgb_reach_count, 0);
    assert_eq!(
        cube_gloss_log.instance_camera_diffuse_specular_o0_rgb_reach_count,
        144
    );
    assert_eq!(cube_gloss_log.instance_camera_rim_o0_rgb_reach_count, 144);
    assert_eq!(cube_gloss_log.terminal_rgb_multiplier_count, 144);
    assert_eq!(cube_gloss_log.terminal_rgb_multiplier_o0_reach_count, 144);
    assert_eq!(
        cube_gloss_log
            .terminal_rgb_multiplier_resource_counts
            .get("g_SamplerReflectionArray.T"),
        Some(&144)
    );
    assert_eq!(
        cube_gloss_log.post_terminal_multiplier_opcodes,
        BTreeMap::from([("mad".to_string(), 144)])
    );
    assert_eq!(
        cube_gloss_log
            .post_terminal_multiplier_resource_counts
            .get("g_SamplerDiffuse.T"),
        Some(&144)
    );
    assert!(
        !cube_gloss_log
            .post_terminal_multiplier_resource_counts
            .contains_key("g_SamplerReflectionArray.T")
    );
    assert!(
        !cube_gloss_log
            .post_terminal_multiplier_resource_counts
            .contains_key("g_SamplerLightSpecular")
    );
    assert!(
        cube_gloss_log
            .terminal_rgb_multiplier_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(cube_gloss_log.node_count, 384);
    assert_eq!(
        cube_gloss_log.pass_ids,
        BTreeMap::from([
            ("0xc885bbd3".to_string(), 384),
            ("0xf21a038f".to_string(), 384),
        ])
    );
    assert_eq!(cube_gloss_log.material_key_sets.len(), 3);
    assert!(cube_gloss_log.material_key_sets.keys().all(|keys| {
        keys.starts_with("B616DC5A=600EF9DF,C8BD1DEF=198D11CD,")
            && keys.ends_with("F52CCF05=DFE74BAC")
    }));

    assert_eq!(cube_gloss_mul.pixel_shader_count, 720);
    assert_eq!(cube_gloss_mul.product_o0_rgb_reach_count, 720);
    assert_eq!(
        cube_gloss_mul.product_other_resource_counts,
        BTreeMap::from([("g_SamplerMask.T".to_string(), 432)])
    );
    assert_eq!(
        cube_gloss_mul.first_post_product_consumer_opcodes,
        BTreeMap::from([("mul".to_string(), 720)])
    );
    assert_eq!(cube_gloss_mul.fifth_root_shaping_count, 720);
    assert_eq!(
        cube_gloss_mul.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        720
    );
    assert_eq!(
        cube_gloss_mul.dynamic_emissive_luminance_scale_composition_opcodes,
        BTreeMap::from([("mad".to_string(), 180), ("mul".to_string(), 540)])
    );
    assert_eq!(
        cube_gloss_mul.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        720
    );
    assert_eq!(
        cube_gloss_mul.dynamic_emissive_luminance_source_composition_opcodes,
        BTreeMap::from([("mad".to_string(), 720)])
    );
    for resource in [
        "g_SamplerNormal.T",
        "g_SamplerOcclusion.T",
        "g_SamplerTable.T",
        "g_SamplerTileNormal",
        "g_SamplerTileOrb.T",
        "g_AmbientParam[0]",
        "g_AmbientParam[3]",
        "g_AmbientParam[4]",
        "g_InstanceParameter[0]",
        "g_InstanceParameter[2]",
        "g_MaterialParameter[5]",
        "g_MaterialParameter[13]",
    ] {
        let counts = if resource.starts_with("g_Sampler") {
            &cube_gloss_mul.dynamic_emissive_luminance_scale_texture_resource_counts
        } else {
            &cube_gloss_mul.dynamic_emissive_luminance_scale_constant_buffer_vector_counts
        };
        assert_eq!(counts.get(resource), Some(&720), "legacy mul {resource}");
    }
    assert!(
        cube_gloss_mul
            .dynamic_emissive_luminance_scale_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(cube_gloss_mul.dynamic_emissive_o0_rgb_reach_count, 720);
    assert_eq!(
        cube_gloss_mul.dynamic_emissive_table_join_o0_rgb_reach_count,
        720
    );
    assert_eq!(cube_gloss_mul.instance_mul_color_o0_rgb_reach_count, 720);
    assert_eq!(cube_gloss_mul.instance_env_parameter_o0_rgb_reach_count, 0);
    assert_eq!(
        cube_gloss_mul.instance_camera_diffuse_specular_o0_rgb_reach_count,
        720
    );
    assert_eq!(cube_gloss_mul.instance_camera_rim_o0_rgb_reach_count, 720);
    assert_eq!(cube_gloss_mul.terminal_rgb_multiplier_count, 720);
    assert_eq!(cube_gloss_mul.terminal_rgb_multiplier_o0_reach_count, 720);
    assert_eq!(
        cube_gloss_mul
            .terminal_rgb_multiplier_resource_counts
            .get("g_SamplerReflectionArray.T"),
        Some(&720)
    );
    assert_eq!(
        cube_gloss_mul.post_terminal_multiplier_opcodes,
        BTreeMap::from([("mad".to_string(), 720)])
    );
    assert_eq!(
        cube_gloss_mul
            .post_terminal_multiplier_resource_counts
            .get("g_SamplerDiffuse.T"),
        Some(&432)
    );
    assert!(
        !cube_gloss_mul
            .post_terminal_multiplier_resource_counts
            .contains_key("g_SamplerReflectionArray.T")
    );
    assert!(
        !cube_gloss_mul
            .post_terminal_multiplier_resource_counts
            .contains_key("g_SamplerLightSpecular")
    );
    assert!(
        cube_gloss_mul
            .terminal_rgb_multiplier_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(cube_gloss_mul.node_count, 2688);
    assert_eq!(
        cube_gloss_mul.pass_ids,
        BTreeMap::from([
            ("0xc885bbd3".to_string(), 2688),
            ("0xf21a038f".to_string(), 2688),
        ])
    );
    assert_eq!(cube_gloss_mul.material_key_sets.len(), 21);

    assert_eq!(non_cube_gloss_mul.pixel_shader_count, 106);
    assert_eq!(non_cube_gloss_mul.fifth_root_shaping_count, 0);
    assert_eq!(
        non_cube_gloss_mul.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        non_cube_gloss_mul.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        0
    );
    assert!(
        non_cube_gloss_mul
            .dynamic_emissive_luminance_scale_composition_opcodes
            .is_empty()
    );
    assert_eq!(non_cube_gloss_mul.dynamic_emissive_o0_rgb_reach_count, 0);
    assert_eq!(
        non_cube_gloss_mul.dynamic_emissive_table_join_o0_rgb_reach_count,
        0
    );
    assert_eq!(non_cube_gloss_mul.instance_mul_color_o0_rgb_reach_count, 0);
    assert_eq!(
        non_cube_gloss_mul.instance_camera_diffuse_specular_o0_rgb_reach_count,
        0
    );
    assert_eq!(non_cube_gloss_mul.instance_camera_rim_o0_rgb_reach_count, 0);
    assert_eq!(non_cube_gloss_mul.terminal_rgb_multiplier_count, 0);
    assert_eq!(non_cube_gloss_mul.node_count, 2400);
    assert_eq!(
        non_cube_gloss_mul.pass_ids,
        BTreeMap::from([("0x03ac862e".to_string(), 2400)])
    );
    assert_eq!(non_cube_gloss_terminal.pixel_shader_count, 22);
    assert_eq!(non_cube_gloss_terminal.fifth_root_shaping_count, 0);
    assert_eq!(
        non_cube_gloss_terminal.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        non_cube_gloss_terminal.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        0
    );
    assert!(
        non_cube_gloss_terminal
            .dynamic_emissive_luminance_scale_composition_opcodes
            .is_empty()
    );
    assert_eq!(
        non_cube_gloss_terminal.dynamic_emissive_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        non_cube_gloss_terminal.dynamic_emissive_table_join_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        non_cube_gloss_terminal.instance_mul_color_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        non_cube_gloss_terminal.instance_camera_diffuse_specular_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        non_cube_gloss_terminal.instance_camera_rim_o0_rgb_reach_count,
        0
    );
    assert_eq!(non_cube_gloss_terminal.terminal_rgb_multiplier_count, 0);
    assert_eq!(non_cube_gloss_terminal.node_count, 352);
    assert_eq!(
        non_cube_gloss_terminal.pass_ids,
        BTreeMap::from([("0x03ac862e".to_string(), 352)])
    );
    assert_eq!(no_gloss_mul.pixel_shader_count, 14);
    assert_eq!(no_gloss_mul.fifth_root_shaping_count, 0);
    assert_eq!(
        no_gloss_mul.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        no_gloss_mul.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        0
    );
    assert_eq!(no_gloss_mul.dynamic_emissive_o0_rgb_reach_count, 0);
    assert_eq!(
        no_gloss_mul.dynamic_emissive_table_join_o0_rgb_reach_count,
        0
    );
    assert_eq!(no_gloss_mul.instance_mul_color_o0_rgb_reach_count, 0);
    assert_eq!(
        no_gloss_mul.instance_camera_diffuse_specular_o0_rgb_reach_count,
        0
    );
    assert_eq!(no_gloss_mul.instance_camera_rim_o0_rgb_reach_count, 0);
    assert_eq!(no_gloss_mul.terminal_rgb_multiplier_count, 0);
    assert_eq!(no_gloss_mul.node_count, 288);
    assert_eq!(no_gloss_terminal.pixel_shader_count, 2);
    assert_eq!(no_gloss_terminal.fifth_root_shaping_count, 0);
    assert_eq!(
        no_gloss_terminal.dynamic_emissive_luminance_scale_o0_rgb_reach_count,
        0
    );
    assert_eq!(
        no_gloss_terminal.dynamic_emissive_luminance_source_o0_rgb_reach_count,
        0
    );
    assert_eq!(no_gloss_terminal.dynamic_emissive_o0_rgb_reach_count, 0);
    assert_eq!(
        no_gloss_terminal.dynamic_emissive_table_join_o0_rgb_reach_count,
        0
    );
    assert_eq!(no_gloss_terminal.instance_mul_color_o0_rgb_reach_count, 0);
    assert_eq!(
        no_gloss_terminal.instance_camera_diffuse_specular_o0_rgb_reach_count,
        0
    );
    assert_eq!(no_gloss_terminal.instance_camera_rim_o0_rgb_reach_count, 0);
    assert_eq!(no_gloss_terminal.terminal_rgb_multiplier_count, 0);
    assert_eq!(no_gloss_terminal.node_count, 32);
    assert_eq!(legacy.gbuffer1_sample_count, 576);
    assert_eq!(legacy.gbuffer1_pixel_shader_count, 576);
    assert_eq!(
        legacy.gbuffer1_lane_sample_counts,
        BTreeMap::from([("x".to_string(), 576), ("w".to_string(), 576)])
    );
    assert_eq!(
        legacy.gbuffer1_lane_o0_rgb_reach_counts,
        BTreeMap::from([("x".to_string(), 576), ("w".to_string(), 576)])
    );
    assert_eq!(
        legacy.gbuffer1_lane_consumer_opcodes,
        BTreeMap::from([
            (
                "w".to_string(),
                BTreeMap::from([
                    ("add".to_string(), 576),
                    ("lt".to_string(), 576),
                    ("mul".to_string(), 1152),
                ]),
            ),
            (
                "x".to_string(),
                BTreeMap::from([
                    ("log".to_string(), 576),
                    ("mad".to_string(), 576),
                    ("max".to_string(), 1152),
                    ("movc".to_string(), 576),
                ]),
            ),
        ])
    );
    assert_eq!(
        legacy.gbuffer1_x_consumer_signatures,
        BTreeMap::from([("log -> max -> max -> mad -> movc".to_string(), 576)])
    );
    assert_eq!(legacy.gbuffer1_x_terminal_multiplier_count, 576);
    assert_eq!(
        legacy.gbuffer1_x_terminal_multiplier_o0_rgb_reach_count,
        576
    );
    assert_eq!(
        legacy.gbuffer1_x_terminal_multiplier_resource_counts,
        BTreeMap::from([
            ("g_SamplerDecal.T".to_string(), 384),
            ("g_SamplerGBuffer.T".to_string(), 576),
            ("g_SamplerGBuffer1".to_string(), 576),
            ("g_SamplerIndex.T".to_string(), 576),
            ("g_SamplerLightDiffuse".to_string(), 288),
            ("g_SamplerLightSpecular".to_string(), 288),
            ("g_SamplerMask.T".to_string(), 576),
            ("g_SamplerNormal.T".to_string(), 576),
            ("g_SamplerOcclusion.T".to_string(), 576),
            ("g_SamplerReflectionArray.T".to_string(), 576),
            ("g_SamplerTable.T".to_string(), 576),
            ("g_SamplerTileOrb.T".to_string(), 576),
        ])
    );
    assert_eq!(
        legacy.gbuffer1_x_post_multiplier_consumer_signatures,
        BTreeMap::from([("mad".to_string(), 576)])
    );
    assert_eq!(
        legacy.gbuffer1_x_post_multiplier_resource_counts,
        BTreeMap::from([
            ("g_SamplerDecal.T".to_string(), 384),
            ("g_SamplerDiffuse.T".to_string(), 384),
            ("g_SamplerGBuffer.T".to_string(), 576),
            ("g_SamplerGBuffer1".to_string(), 576),
            ("g_SamplerIndex.T".to_string(), 576),
            ("g_SamplerLightDiffuse".to_string(), 288),
            ("g_SamplerMask.T".to_string(), 192),
            ("g_SamplerNormal.T".to_string(), 576),
            ("g_SamplerOcclusion.T".to_string(), 576),
            ("g_SamplerTable.T".to_string(), 576),
            ("g_SamplerTileOrb.T".to_string(), 576),
        ])
    );
    assert!(
        legacy
            .gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(legacy.gbuffer1_x_node_count, 3072);
    assert_eq!(
        legacy.gbuffer1_x_pass_ids,
        BTreeMap::from([("0x955c0b73".to_string(), 3072)])
    );
    assert_eq!(legacy.gbuffer1_x_material_key_sets.len(), 24);
    assert_eq!(legacy.gbuffer1_consumer_representatives.len(), 16);

    let glass = package("characterglass.shpk");
    assert_eq!(glass.pixel_shader_count, 38);
    assert_eq!(glass.roughness_sample_count, 32);
    assert_eq!(glass.roughness_pixel_shader_count, 24);
    assert_eq!(glass.roughness_consumer_sample_count, 32);
    assert_eq!(
        glass.roughness_consumer_opcodes,
        BTreeMap::from([("mad".to_string(), 24), ("mul".to_string(), 8)])
    );
    assert_eq!(glass.roughness_o1_y_reach_count, 0);
    assert_eq!(glass.gloss_sample_count, 0);
    assert_eq!(glass.gloss_pixel_shader_count, 0);
    assert_eq!(glass.gloss_consumer_sample_count, 0);
    assert!(glass.gloss_consumer_opcodes.is_empty());
    assert_eq!(glass.gloss_o1_y_reach_count, 0);
    assert_eq!(glass.gloss_o0_rgb_reach_count, 0);
    assert_eq!(glass.gloss_power_chain_count, 0);
    assert_eq!(glass.gloss_power_o0_rgb_reach_count, 0);
    assert_eq!(glass.gloss_camera_reflection_power_chain_count, 0);
    assert_eq!(glass.gloss_camera_reflection_lobe_count, 0);
    assert!(
        glass
            .gloss_camera_reflection_lobe_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(glass.gloss_cube_lod_sample_count, 0);
    assert_eq!(glass.gloss_cube_sample_hdr_decode_count, 0);
    assert_eq!(glass.gloss_cube_sample_o0_rgb_reach_count, 0);
    assert_eq!(glass.gloss_cube_current_location_sample_count, 0);
    assert_eq!(glass.gloss_cube_previous_location_sample_count, 0);
    assert_eq!(glass.gloss_ambient_location_interpolation_count, 0);
    assert_eq!(glass.gloss_ambient_reflection_scale_offset_count, 0);
    assert_eq!(glass.gloss_ambient_bake_light_composition_count, 0);
    assert!(
        glass
            .gloss_ambient_bake_light_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(glass.gloss_environment_specular_strength_join_count, 0);
    assert!(
        glass
            .gloss_environment_specular_strength_unjoined_pixel_shaders
            .is_empty()
    );
    assert_eq!(glass.gloss_cube_specular_strength_pixel_shader_count, 0);
    assert_eq!(glass.gloss_non_cube_specular_strength_pixel_shader_count, 0);
    assert_eq!(glass.gloss_texcoord4_w_environment_blend_count, 0);
    assert_eq!(glass.gloss_gbuffer1_w_environment_blend_count, 0);
    assert!(
        glass
            .gloss_environment_blend_unclassified_pixel_shaders
            .is_empty()
    );
    assert!(glass.gloss_consumer_opcode_sequences.is_empty());
    assert!(glass.gloss_consumer_classes.is_empty());
    assert_eq!(glass.gloss_node_count, 0);
    assert!(glass.gloss_material_key_sets.is_empty());
    assert_eq!(glass.specular_strength_sample_count, 0);
    assert_eq!(glass.specular_strength_pixel_shader_count, 0);
    assert_eq!(glass.specular_strength_without_gloss_pixel_shader_count, 0);
    assert_eq!(glass.specular_strength_consumer_sample_count, 0);
    assert!(glass.specular_strength_consumer_opcodes.is_empty());
    assert!(glass.specular_strength_composition_classes.is_empty());
    assert!(
        glass
            .specular_strength_terminal_unclassified_pixel_shaders
            .is_empty()
    );
    assert_eq!(glass.gbuffer1_sample_count, 0);
    assert_eq!(glass.gbuffer1_pixel_shader_count, 0);
    assert!(glass.gbuffer1_lane_sample_counts.is_empty());
    assert!(glass.gbuffer1_lane_consumer_opcodes.is_empty());
    assert!(glass.gbuffer1_lane_o0_rgb_reach_counts.is_empty());
    assert!(glass.gbuffer1_x_consumer_signatures.is_empty());
    assert_eq!(glass.gbuffer1_x_terminal_multiplier_count, 0);
    assert_eq!(glass.gbuffer1_x_terminal_multiplier_o0_rgb_reach_count, 0);
    assert!(
        glass
            .gbuffer1_x_terminal_multiplier_resource_counts
            .is_empty()
    );
    assert!(
        glass
            .gbuffer1_x_post_multiplier_consumer_signatures
            .is_empty()
    );
    assert!(glass.gbuffer1_x_post_multiplier_resource_counts.is_empty());
    assert!(
        glass
            .gbuffer1_x_terminal_multiplier_unclassified_pixel_shaders
            .is_empty()
    );
    assert!(glass.gbuffer1_consumer_representatives.is_empty());
}

#[cfg(windows)]
#[test]
fn tile_mip_dxbc_parser_tracks_equivalent_formula_shapes_and_consumers() {
    let assembly = r#"
// g_SamplerTileOrb.T texture float4 2darray t8 1
// g_SamplerTileNormal texture float4 2darray t9 1
mul r5.w, r5.w, l(0.007812)
log r5.w, r5.w
max r5.w, r5.w, l(0.000000)
add r5.w, r5.w, cb0[17].y
sample_b_indexable(texture2darray)(float,float,float,float) r20.xyzw, r16.xyzx, t8.xyzw, s4, r5.w
sample_b_indexable(texture2darray)(float,float,float,float) r8.yz, r16.xyzx, t9.zxyw, s4, r5.w
mov r5.w, r0.x
mul r6.xz, r6.xxzx, l(64.000000, 0.000000, 0.007812, 0.000000)
log r6.z, r6.z
max r6.z, r6.z, l(0.000000)
add r6.z, r6.z, cb0[17].y
sample_b_indexable(texture2darray)(float,float,float,float) r20.xyzw, r17.xyzx, t8.xyzw, s4, r6.z
"#;
    let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
    let bindings = dxbc_texture_bindings(&lines);
    assert_eq!(
        bindings.get(&8).map(String::as_str),
        Some("g_SamplerTileOrb.T")
    );
    assert_eq!(
        bindings.get(&9).map(String::as_str),
        Some("g_SamplerTileNormal")
    );

    let uses = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            tile_bias_add_destination(line).map(|register| (index, register))
        })
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 2);
    assert!(tile_bias_formula_is_expected(&lines, uses[0].0, &uses[0].1));
    assert!(tile_bias_formula_is_expected(&lines, uses[1].0, &uses[1].1));
    assert_eq!(
        tile_bias_consumers(&lines, uses[0].0, &uses[0].1, &bindings),
        BTreeSet::from([
            "g_SamplerTileNormal".to_string(),
            "g_SamplerTileOrb.T".to_string(),
        ])
    );
    assert_eq!(
        tile_bias_consumers(&lines, uses[1].0, &uses[1].1, &bindings),
        BTreeSet::from(["g_SamplerTileOrb.T".to_string()])
    );
}

#[cfg(windows)]
#[test]
fn gbuffer1_x_terminal_multiplier_parser_preserves_packed_movc_lanes() {
    let lines = [
        "sample_indexable(texture2d)(float,float,float,float) r3.xw, r0.xyxx, t2.xyzw, s0",
        "log r4.x, r3.x",
        "movc r3.xy, r0.zzzz, r12.xyxx, r3.xyxx",
        "mul r0.xyz, r3.xxxx, r0.xyzx",
        "mad o0.xyz, r6.xyzx, r5.xyzx, r0.xyzx",
    ];
    assert_eq!(
        dxbc_componentwise_tainted_destinations(lines[2], "r3.x"),
        vec!["r3.x".to_string()]
    );
    assert_eq!(
        dxbc_componentwise_tainted_destinations(lines[2], "r3.y"),
        vec!["r3.y".to_string()]
    );
    assert_eq!(
        dxbc_terminal_rgb_multiplier_after(&lines, 2, "r3.x"),
        Some((3, "r0.xyz".to_string(), "r0.xyzx".to_string()))
    );
    assert_eq!(dxbc_terminal_rgb_multiplier_after(&lines, 2, "r3.y"), None);

    let packed_provenance = [
        "sample_indexable(texture2d)(float,float,float,float) r5.x, r0.xyxx, t4.xyzw, s0",
        "movc r3.xy, r0.zzzz, r12.xyxx, r5.yxxx",
        "mul r6.xyz, r3.xxxx, r6.xyzx",
        "mul r7.xyz, r3.yyyy, r7.xyzx",
    ];
    assert!(!dxbc_component_taint_reaches_operand(
        &packed_provenance,
        0,
        "r5.x",
        2,
        "r3.xxxx",
    ));
    assert!(dxbc_component_taint_reaches_operand(
        &packed_provenance,
        0,
        "r5.x",
        3,
        "r3.yyyy",
    ));

    let branch_provenance = [
        "sample_indexable(texture2d)(float,float,float,float) r5.x, r0.xyxx, t4.xyzw, s0",
        "if_nz r0.w",
        "mov r3.x, r5.x",
        "else",
        "mov r3.x, r4.x",
        "endif",
        "mul r6.xyz, r3.xxxx, r6.xyzx",
    ];
    assert!(dxbc_component_taint_reaches_operand(
        &branch_provenance,
        0,
        "r5.x",
        6,
        "r3.xxxx",
    ));
}

#[cfg(windows)]
#[test]
fn vertex_alpha_remap_dxbc_parser_requires_the_proven_formula_and_product() {
    let lines = [
        "add r3.y, -v1.w, l(1.000000)",
        "mad r3.y, cb0[13].y, r3.y, v1.w",
        "sample_b_indexable(texture2d)(float,float,float,float) r3.zw, v2.xyxx, t2.xyzw, s0, r0.x",
        "mul r17.w, r3.z, r3.y",
    ];
    let destination = vertex_alpha_remap_destination(&lines, 1).expect("vertex alpha remap");
    assert_eq!(destination, "r3.y");
    assert!(vertex_alpha_has_immediate_product(&lines, 1, &destination));

    let wrong_source = [
        "add r3.y, -v0.w, l(1.000000)",
        "mad r3.y, cb0[13].y, r3.y, v0.w",
    ];
    assert!(vertex_alpha_remap_destination(&wrong_source, 1).is_none());

    let overwritten = [
        "add r3.y, -v1.w, l(1.000000)",
        "mad r3.y, cb0[13].y, r3.y, v1.w",
        "mov r3.y, l(1.000000)",
        "mul r17.w, r3.z, r3.y",
    ];
    let destination = vertex_alpha_remap_destination(&overwritten, 1).expect("vertex alpha remap");
    assert!(!vertex_alpha_has_immediate_product(
        &overwritten,
        1,
        &destination
    ));

    let threshold = [
        "add r0.x, -v1.w, l(1.000000)",
        "mad r0.x, cb0[13].y, r0.x, v1.w",
        "sample_b_indexable(texture2d)(float,float,float,float) r1.xyz, v2.xyxx, t1.xyzw, s0, r0.y",
        "mad r0.x, r0.x, r1.z, -cb0[0].w",
        "lt r0.x, r0.x, l(0.000000)",
        "discard_nz r0.x",
    ];
    let destination = vertex_alpha_remap_destination(&threshold, 1).expect("vertex alpha remap");
    assert!(vertex_alpha_has_threshold_test(&threshold, 1, &destination));
}

#[cfg(windows)]
#[test]
fn alpha_shaping_dxbc_parser_separates_alpha_and_non_alpha_consumers() {
    let alpha_lines = [
        "lt r1.w, r3.w, l(1.000000)",
        "log r2.x, r2.x",
        "mul r2.x, r2.x, cb0[12].w",
        "exp r2.x, r2.x",
        "mul r2.x, r2.x, cb0[13].x",
        "mul_sat r2.y, r3.w, l(3.333333)",
        "mad_sat r0.w, r2.x, r2.y, r0.w",
        "ge r2.y, l(0.000000), cb0[13].x",
        "movc r0.w, r2.y, r0.w, r2.x",
    ];
    let shape = alpha_shaping_shape_destination(&alpha_lines, 4).expect("shape formula");
    assert_eq!(shape, "r2.x");
    assert!(alpha_shaping_has_scaled_alpha(&alpha_lines, &shape));
    assert!(alpha_shaping_has_saturated_composition(
        &alpha_lines,
        &shape
    ));
    assert!(alpha_shaping_has_offset_sign_gate(&alpha_lines));
    assert!(alpha_shaping_has_alpha_less_than_one_gate(&alpha_lines));

    let non_alpha_lines = [
        "log r0.z, r0.z",
        "mul r0.z, r0.z, cb0[12].w",
        "exp r0.z, r0.z",
        "mul r0.z, r0.z, cb0[13].x",
        "sample_indexable(texture2d)(float,float,float,float) r3.xyz, r3.xyxx, t3.xyzw, s2",
    ];
    let shape = alpha_shaping_shape_destination(&non_alpha_lines, 3).expect("shape formula");
    assert!(!alpha_shaping_has_scaled_alpha(&non_alpha_lines, &shape));
    assert!(!alpha_shaping_has_saturated_composition(
        &non_alpha_lines,
        &shape
    ));

    let dot_with_min = [
        "dp3 r2.w, r4.xyzx, r9.xyzx",
        "min r2.w, |r2.w|, l(1.000000)",
        "add r2.w, -r2.w, l(1.000000)",
        "log r2.w, r2.w",
        "mul r2.w, r2.w, cb0[12].w",
        "exp r2.w, r2.w",
        "mul r2.w, r2.w, cb0[13].x",
    ];
    assert_eq!(
        alpha_shaping_dot_operands(&dot_with_min, 6),
        Some(("r4.xyzx", "r9.xyzx"))
    );
    let dot_with_saturate = [
        "dp3 r0.z, r6.xyzx, r13.xyzx",
        "mov_sat r0.z, |r0.z|",
        "add r0.z, -r0.z, l(1.000000)",
        "log r0.z, r0.z",
        "mul r0.z, r0.z, cb0[12].w",
        "exp r0.z, r0.z",
        "mul r0.z, r0.z, cb0[13].x",
    ];
    assert_eq!(
        alpha_shaping_dot_operands(&dot_with_saturate, 6),
        Some(("r6.xyzx", "r13.xyzx"))
    );

    let dot_provenance = [
        "mul r4.xyz, r0.xxxx, -v6.xyzx",
        "mul r9.xyz, r1.xxxx, r3.xyzx",
        "dp3 r2.w, r4.xyzx, r9.xyzx",
        "min r2.w, |r2.w|, l(1.000000)",
        "add r2.w, -r2.w, l(1.000000)",
        "log r2.w, r2.w",
        "mul r2.w, r2.w, cb0[12].w",
        "exp r2.w, r2.w",
        "mul r2.w, r2.w, cb0[13].x",
    ];
    assert_eq!(
        alpha_shaping_normal_dot_operand(&dot_provenance, 8, "r4.xyzx", "r9.xyzx"),
        Some("r9.xyzx")
    );
    assert_eq!(
        alpha_shaping_operand_producer_opcode(&dot_provenance, 8, "r9.xyzx"),
        Some("mul".to_string())
    );

    let provenance_lines = [
        "// g_SamplerIndex.T texture float4 2d t4 1",
        "sample_indexable(texture2d)(float,float,float,float) r3.w, v2.xyxx, t4.xyzw, s0",
        "mul r4.z, r3.w, v1.w",
    ];
    let bindings = dxbc_texture_bindings(&provenance_lines);
    assert_eq!(
        alpha_shaping_operand_root_sources(&provenance_lines, 3, "r4.z", &bindings),
        BTreeSet::from([
            "texture:g_SamplerIndex.T".to_string(),
            "vertex:v1".to_string(),
            "vertex:v2".to_string(),
        ])
    );
}

#[cfg(windows)]
#[test]
fn tile_blend_dxbc_parser_tracks_neutral_orb_and_shared_ab_weight() {
    let assembly = r#"
// g_SamplerIndex.T texture float4 2d t6 1
// g_SamplerTable.T texture float4 2d t7 1
sample_l_indexable(texture2d)(float,float,float,float) r6.xy, v2.xyxx, t6.xyzw, s2, l(0.000000)
add r3.w, -r6.y, l(1.000000)
add r6.yz, r4.wwww, l(0.000000, 0.500000, 1.500000, 0.000000)
div r7.xyzw, l(5.500000, 4.500000, 6.500000, 7.500000), r2.wwww
mov r6.x, r7.y
sample_indexable(texture2d)(float,float,float,float) r1.w, r6.xyxx, t7.xyzw, s3
div r1.w, l(1.000000, 1.000000, 1.000000, 1.000000), r1.w
log r4.x, r4.x
mul r1.w, r1.w, r4.x
exp r1.w, r1.w
add r1.w, -r1.w, l(1.000000)
add r5.yz, r4.wwww, l(0.000000, 0.500000, 1.500000, 0.000000)
mul r12.yz, r5.yyzy, l(1.000000, 1.000000, 1.000000, 1.000000)
sample_indexable(texture2d)(float,float,float,float) r8.xyzw, r12.xyxx, t7.xyzw, s3
sample_indexable(texture2d)(float,float,float,float) r10.xyzw, r12.xzxx, t7.xyzw, s3
add r14.xyzw, r20.xyzw, l(-1.000000, -0.500000, -1.000000, -1.000000)
mad r14.xyzw, r8.xxxx, r14.xyzw, l(1.000000, 0.500000, 1.000000, 1.000000)
add r6.xyzw, r20.xyzw, l(-1.000000, -0.500000, -1.000000, -1.000000)
mad r6.xyzw, r10.xxxx, r6.xyzw, l(1.000000, 0.500000, 1.000000, 1.000000)
add r6.xyzw, -r14.xyzw, r6.xyzw
mad r6.xyzw, r1.wwww, r6.xyzw, r14.xyzw
add r8.xyz, r17.xyzx, -r21.xyzx
mad r8.xyz, r1.wwww, r8.xyzx, r21.xyzx
"#;
    let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
    let bindings = dxbc_texture_bindings(&lines);
    assert_eq!(
        lines
            .iter()
            .filter(|line| dxbc_sample_texture_name(line, &bindings).as_deref()
                == Some("g_SamplerIndex.T"))
            .count(),
        1
    );
    let index_sample = lines
        .iter()
        .position(|line| {
            dxbc_sample_texture_name(line, &bindings).as_deref() == Some("g_SamplerIndex.T")
        })
        .expect("index sample");
    assert!(index_blend_inversion_after_sample(&lines, index_sample));
    let table_sample = lines
        .iter()
        .position(|line| {
            dxbc_sample_texture_name(line, &bindings).as_deref() == Some("g_SamplerTable.T")
        })
        .expect("table sample");
    assert!(shaping_dependent_table_sample(&lines, table_sample));
    let roughness_destination = dxbc_instruction_destination(lines[table_sample]).unwrap();
    assert_eq!(
        dxbc_row_provenance_at(&lines, &bindings, table_sample, roughness_destination),
        Some(DxbcTableRow::A)
    );
    let roughness_coordinates = lines[table_sample]
        .split_once(' ')
        .map(|(_, operands)| split_instruction_operands(operands)[1])
        .unwrap();
    assert_eq!(
        dxbc_literal_provenance_at(&lines, table_sample, roughness_coordinates, 0).as_deref(),
        Some("4.500000")
    );
    let neutral = lines
        .iter()
        .enumerate()
        .filter_map(|(index, _)| tile_orb_neutral_pair(&lines, index))
        .collect::<Vec<_>>();
    assert_eq!(neutral.len(), 2);
    let orb = dxbc_linear_blend(&lines, neutral[1].2 + 1).expect("ORB A/B blend");
    assert_eq!(
        dxbc_register_base(orb.source_a),
        dxbc_register_base(&neutral[0].0)
    );
    assert_eq!(
        dxbc_register_base(orb.source_b),
        dxbc_register_base(&neutral[1].0)
    );
    assert_eq!(dxbc_register_base(&neutral[0].1), Some("r8"));
    assert_eq!(dxbc_register_base(&neutral[1].1), Some("r10"));
    assert_eq!(
        dxbc_row_provenance_at(&lines, &bindings, neutral[0].2, &neutral[0].1),
        Some(DxbcTableRow::A)
    );
    assert_eq!(
        dxbc_row_provenance_at(&lines, &bindings, neutral[1].2, &neutral[1].1),
        Some(DxbcTableRow::B)
    );
    assert_eq!(orb.weight, "r1.wwww");
    let normal = dxbc_linear_blend(&lines, orb.mad_index + 1).expect("normal A/B blend");
    assert_eq!(normal.weight, orb.weight);
}

#[cfg(windows)]
#[test]
fn dxbc_gloss_power_parser_requires_adjacent_log_mul_exp_chain() {
    let lines = [
        "log r4.x, r4.x",
        "mul r4.x, r4.x, r7.w",
        "exp r4.x, r4.x",
        "mul r3.w, r3.w, r4.x",
        "mad o0.xyz, r3.wwww, r8.xyzx, r9.xyzx",
    ];
    assert_eq!(
        dxbc_log_mul_exp_power_destination(&lines, 1, "r7.w"),
        Some((2, "r4.x".to_string()))
    );
    assert!(dxbc_register_reaches_output(&lines, 2, "r4.x", "o0.x"));
    assert_eq!(dxbc_log_mul_exp_power_destination(&lines, 1, "r6.w"), None);

    let non_power = [
        "log r4.x, r4.x",
        "mul r4.x, r4.x, r7.w",
        "add r4.x, r4.x, l(1.000000)",
    ];
    assert_eq!(
        dxbc_log_mul_exp_power_destination(&non_power, 1, "r7.w"),
        None
    );

    let camera_reflection = [
        "dp3 r0.w, -v6.xyzx, -v6.xyzx",
        "rsq r0.w, r0.w",
        "mul r3.xyz, r0.wwww, -v6.xyzx",
        "dp3 r4.x, -r3.xyzx, r0.xyzx",
        "add r4.x, r4.x, r4.x",
        "mad r12.xyz, r0.xyzx, -r4.xxxx, -r3.xyzx",
        "add r11.xyz, -v6.xyzx, l(0.000000, 0.200000, 0.000000, 0.000000)",
        "dp3 r2.w, r11.xyzx, r11.xyzx",
        "rsq r2.w, r2.w",
        "mul r11.xyz, r2.wwww, r11.xyzx",
        "dp3_sat r2.w, r0.xyzx, r11.xyzx",
        "add r3.w, -r2.w, l(1.000000)",
        "dp3_sat r4.x, r12.xyzx, r11.xyzx",
        "log r4.x, r4.x",
        "mul r4.x, r4.x, r7.w",
        "exp r4.x, r4.x",
        "mul r3.w, r3.w, r3.w",
        "mad r3.w, r3.w, l(-3.000000), l(3.000000)",
        "min r3.w, r3.w, l(1.000000)",
        "mul r3.w, r3.w, r4.x",
    ];
    assert!(dxbc_gloss_power_uses_camera_reflection_dot(
        &camera_reflection,
        14,
        "r7.w"
    ));
    assert!(dxbc_gloss_power_has_visibility_envelope(
        &camera_reflection,
        14,
        "r7.w"
    ));
    let mut wrong_offset = camera_reflection;
    wrong_offset[6] = "add r11.xyz, -v6.xyzx, l(0.000000, 0.300000, 0.000000, 0.000000)";
    assert!(!dxbc_gloss_power_uses_camera_reflection_dot(
        &wrong_offset,
        14,
        "r7.w"
    ));
}

#[cfg(windows)]
#[test]
fn dxbc_register_taint_reaches_mrt_component_through_scalar_chain() {
    let assembly = r#"
sample_indexable(texture2d)(float,float,float,float) r2.x, r3.xyxx, t3.xyzw, s2
mad r0.w, r2.x, l(2.000000), l(-1.000000)
lt r2.z, r0.w, l(0.000000)
mad r0.z, r0.w, r1.y, r1.z
movc o1.y, r2.z, r0.z, r0.w
    "#;
    let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(dxbc_operand_components("o1.y"), vec!["o1.y"]);
    assert!(register_components_overlap("o1.y", "o1.y"));
    let producer_index = lines
        .iter()
        .position(|line| line.starts_with("sample_indexable"))
        .expect("producer");
    assert!(dxbc_register_reaches_output(
        &lines,
        producer_index,
        "r2.x",
        "o1.y"
    ));
}

#[cfg(windows)]
#[test]
fn legacy_environment_audit_tracks_height_control_and_cube_lod_taint() {
    let assembly = r#"
// Input signature:
// Name                 Index   Mask Register SysValue  Format   Used
// TEXCOORD                 4   xyzw        6     NONE   float   xyzw
sample_indexable(texture2d)(float,float,float,float) r7.w, r0.xyxx, t7.wwww, s3
lt r1.y, l(0.000000), v6.w
mul r1.z, v6.w, v6.w
add r3.w, -r7.w, l(10.000000)
mad r7.y, r1.z, r3.w, r7.w
add r1.z, v6.w, l(-0.200000)
movc r4.z, r1.y, r7.y, r7.w
add r3.w, r4.z, l(9.000000)
div r3.w, l(8.000000), r3.w
add r3.w, -r3.w, l(1.000000)
mad r3.w, -r3.w, r3.w, l(1.000000)
mul r3.w, r3.w, l(6.000000)
add r11.w, cb6[5].w, l(0.100000)
sample_l_indexable(texturecubearray)(float,float,float,float) r12.xyzw, r11.xyzw, t10.xyzw, s7, r3.w
mul r5.xzw, r12.xxyz, r12.xxyz
add r4.z, r12.w, l(0.000100)
div r12.xyz, r5.xzwx, r4.zzzz
lt r8.x, l(0.000000), cb6[9].z
add r11.w, cb6[9].y, l(0.100000)
sample_l_indexable(texturecubearray)(float,float,float,float) r11.xyzw, r11.xyzw, t10.xyzw, s7, r3.w
mul r5.xzw, r11.xxyz, r11.xxyz
add r4.z, r11.w, l(0.000100)
div r11.xyz, r5.xzwx, r4.zzzz
mad r12.xyzw, cb6[9].zzzz, r11.xyzw, r12.xyzw
mad r11.xyz, r12.xyzx, cb6[5].xxxx, cb6[5].yyyy
mul r9.xyz, r12.xyzx, l(2.356194, 2.356194, 2.356194, 0.000000)
mad r11.xyz, cb6[5].zzzz, r11.xyzx, r9.xyzx
mov o1.w, r11.x
    "#;
    let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
    let producer_index = lines
        .iter()
        .position(|line| line.starts_with("sample_indexable"))
        .expect("Gloss producer");
    assert_eq!(pixel_texcoord4_input_register(&lines).as_deref(), Some("6"));
    assert_eq!(
        dxbc_gloss_environment_blend_source(&lines, &BTreeMap::new()),
        Some("texcoord4.w")
    );
    assert_eq!(
        dxbc_tainted_cube_lod_sample_count(&lines, producer_index, "r7.w"),
        2
    );
    let cube_samples = dxbc_tainted_cube_lod_sample_indices(&lines, producer_index, "r7.w");
    for sample_index in &cube_samples {
        assert!(dxbc_cube_sample_has_squared_alpha_decode(
            &lines,
            *sample_index
        ));
    }
    assert_eq!(
        cube_samples
            .iter()
            .filter_map(|sample_index| {
                dxbc_cube_sample_environment_location_lane(&lines, *sample_index, 6)
            })
            .collect::<Vec<_>>(),
        vec!["current", "previous"]
    );
    assert!(dxbc_has_ambient_location_interpolation(&lines, 6));
    assert!(dxbc_has_ambient_reflection_scale_offset(&lines, 6));
    assert!(dxbc_has_ambient_bake_light_composition(&lines, 6));
    assert_eq!(
        dxbc_register_reached_outputs(&lines, 4, "v6.w"),
        BTreeSet::from(["o1.w".to_string()])
    );

    let gbuffer = r#"
// g_SamplerGBuffer1 texture float4 2d t2 1
sample_indexable(texture2d)(float,float,float,float) r3.xw, r0.xyxx, t2.xyzw, s0
lt r0.z, l(0.000000), r3.w
mul r2.w, r3.w, r3.w
add r2.x, r3.w, l(-0.200000)
    "#;
    let gbuffer_lines = gbuffer.lines().map(str::trim).collect::<Vec<_>>();
    assert_eq!(
        dxbc_gloss_environment_blend_source(&gbuffer_lines, &dxbc_texture_bindings(&gbuffer_lines)),
        Some("gbuffer1.w")
    );

    let vertex = r#"
//       float4 m_Wetness;              // Offset:   64
//   } g_InstanceParameter;             // Offset:    0 Size:   176
//       float4 m_Params;               // Offset:    0
//   } g_ModelParameter;                // Offset:    0 Size:    16
mad r0.x, v0.y, cb3[0].x, cb2[4].y
mul r0.x, r0.x, cb2[4].x
max r0.x, r0.x, cb2[4].z
min o6.w, r0.x, cb2[4].w
    "#;
    let vertex_lines = vertex.lines().map(str::trim).collect::<Vec<_>>();
    assert!(vertex_texcoord4_w_matches_height_clamp(
        &vertex_lines,
        8,
        "o6.w"
    ));
    assert!(dxbc_has_legacy_wetness_parameter_reflection(&vertex_lines));
}

#[cfg(windows)]
#[test]
fn texture_mip_dxbc_parser_tracks_sum_order_and_primary_consumers() {
    let assembly = r#"
// g_SamplerDiffuse.T texture float4 2d t0 1
// g_SamplerNormal.T texture float4 2d t1 1
// g_SamplerMask.T texture float4 2d t2 1
add r0.z, cb0[14].y, cb2[0].w
sample_b_indexable(texture2d)(float,float,float,float) r3.xyz, v2.xyxx, t0.xyzw, s0, r0.z
sample_b_indexable(texture2d)(float,float,float,float) r4.xy, v2.xyxx, t1.xyzw, s0, r0.z
sample_b_indexable(texture2d)(float,float,float,float) r5.xyzw, v2.xyxx, t2.xyzw, s0, r0.z
mov r0.z, r1.x
add r7.w, cb1[0].w, cb0[14].y
sample_b_indexable(texture2d)(float,float,float,float) r8.xy, v2.xyxx, t1.xyzw, s0, r7.w
"#;
    let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
    let bindings = dxbc_texture_bindings(&lines);
    let uses = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            texture_mip_bias_add_destination(line).map(|register| (index, register))
        })
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 2);
    assert_eq!(
        texture_mip_bias_consumers(&lines, uses[0].0, &uses[0].1, &bindings),
        BTreeSet::from([
            "g_SamplerDiffuse.T".to_string(),
            "g_SamplerMask.T".to_string(),
            "g_SamplerNormal.T".to_string(),
        ])
    );
    assert_eq!(
        texture_mip_bias_consumers(&lines, uses[1].0, &uses[1].1, &bindings),
        BTreeSet::from(["g_SamplerNormal.T".to_string()])
    );
}

fn assert_installed_character_glass_shader_boundary(resource: &mut SqPackResource) {
    let bytes = resource
        .read("shader/sm5/shpk/characterglass.shpk")
        .expect("installed characterglass.shpk");
    let package = physis::shpk::ShaderPackage::from_existing(resource.platform(), &bytes)
        .expect("parse installed characterglass.shpk");
    assert_eq!(package.pixel_shaders.len(), 38);
    let mut sampled_texture_shader_counts = BTreeMap::<String, usize>::new();
    let mut discard_shader_count = 0;
    let mut alpha_output_shader_count = 0;
    let mut constant_one_alpha_shader_count = 0;
    let mut dynamic_alpha_shader_count = 0;
    let mut normal_sample_with_alpha_component_count = 0;
    for (index, shader) in package.pixel_shaders.iter().enumerate() {
        assert!(
            shader.scalar_parameters.iter().all(|parameter| {
                !matches!(
                    parameter.name.as_str(),
                    "g_GlassIOR" | "g_GlassThicknessMax"
                )
            }),
            "characterglass pixel shader {index} started binding a glass parameter; re-audit its surface formula"
        );
        let assembly = disassemble_dxbc(&shader.bytecode)
            .unwrap_or_else(|error| panic!("characterglass pixel shader {index}: {error}"));
        let lines = assembly.lines().map(str::trim).collect::<Vec<_>>();
        let bindings = dxbc_texture_bindings(&lines);
        let samples = lines
            .iter()
            .filter_map(|line| dxbc_sample_texture_name(line, &bindings))
            .collect::<BTreeSet<_>>();
        normal_sample_with_alpha_component_count += lines
            .iter()
            .filter(|line| {
                dxbc_sample_texture_name(line, &bindings).as_deref() == Some("g_SamplerNormal.T")
                    && line
                        .split_once(' ')
                        .and_then(|(_, operands)| operands.split(',').next())
                        .is_some_and(|destination| destination.contains('w'))
            })
            .count();
        for sample in samples {
            *sampled_texture_shader_counts.entry(sample).or_default() += 1;
        }
        if lines.iter().any(|line| line.starts_with("discard")) {
            discard_shader_count += 1;
        }
        if lines.iter().any(|line| line.contains("o0.w")) {
            alpha_output_shader_count += 1;
        }
        if lines.iter().any(|line| line == &"mov o0.w, l(1.000000)") {
            constant_one_alpha_shader_count += 1;
        }
        if lines.iter().any(|line| line.starts_with("mad o0.w,")) {
            dynamic_alpha_shader_count += 1;
        }
    }
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerIndex.T"),
        Some(&34)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerNormal.T"),
        Some(&34)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerTable.T"),
        Some(&34)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerTileNormal"),
        Some(&34)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerTileOrb.T"),
        Some(&34)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerMask.T"),
        Some(&24)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerReflectionArray.T"),
        Some(&24)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerSphereMap.T"),
        Some(&24)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerDissolveTexture"),
        Some(&19)
    );
    assert_eq!(
        sampled_texture_shader_counts.get("g_SamplerDissolveTexture1"),
        Some(&19)
    );
    assert_eq!(discard_shader_count, 31);
    assert_eq!(alpha_output_shader_count, 32);
    assert_eq!(constant_one_alpha_shader_count, 16);
    assert_eq!(dynamic_alpha_shader_count, 16);
    assert_eq!(normal_sample_with_alpha_component_count, 0);
}

fn assert_installed_cutout_boundary(report: &WeaponShaderFamilyAudit) {
    assert!(
        report
            .material_key_coverage
            .iter()
            .all(|coverage| { coverage.category_name.as_deref() != Some("ApplyAlphaTest") })
    );

    let alpha_clip = report
        .material_key_coverage
        .iter()
        .filter(|coverage| coverage.category_name.as_deref() == Some("ApplyAlphaClip"))
        .collect::<Vec<_>>();
    assert_eq!(alpha_clip.len(), 4);
    assert!(alpha_clip.iter().all(|coverage| {
        coverage.non_default_override_resource_count == 0
            && coverage.observed_values.len() == 1
            && coverage.observed_values[0].value_name.as_deref() == Some("ApplyAlphaClipOff")
    }));
}

fn assert_installed_shadow_mesh_boundary(report: &WeaponShaderFamilyAudit) {
    assert_eq!(
        report.lod0_mesh_range_model_counts,
        BTreeMap::from([("normal".to_string(), 7365)]),
        "installed weapon models gained a new LOD0 range category; re-audit draw-role semantics"
    );
    assert_eq!(
        report.lod0_mesh_range_mesh_counts,
        BTreeMap::from([("normal".to_string(), 8114)])
    );
}

fn assert_installed_water_environment_boundary(report: &WeaponShaderFamilyAudit) {
    assert!(
        report
            .family_counts
            .keys()
            .all(|family| !matches!(family.as_str(), "Water" | "Crystal")),
        "installed weapons gained a water or crystal family; re-audit its shader formula"
    );
    assert!(!report.lod0_mesh_range_model_counts.contains_key("water"));
    assert!(!report.lod0_mesh_range_mesh_counts.contains_key("water"));

    assert!(report.sampler_coverage.iter().all(|coverage| {
        !matches!(
            coverage.texture_kind,
            Some(
                ModelTextureKind::Environment
                    | ModelTextureKind::WaterWave
                    | ModelTextureKind::WaterWaveSecondary
                    | ModelTextureKind::WaterWhitecap
            )
        ) && !matches!(
            coverage.shader_package_name.as_str(),
            "water.shpk" | "river.shpk" | "crystal.shpk"
        )
    }));
    assert!(report.material_constant_coverage.iter().all(|coverage| {
        !matches!(
            coverage.shader_package_name.as_str(),
            "water.shpk" | "river.shpk" | "crystal.shpk"
        )
    }));
}

fn catalog_models(items: &[WeaponCatalogItem]) -> Vec<(PackedModelId, Vec<&WeaponCatalogItem>)> {
    let mut by_model = HashMap::<u64, Vec<&WeaponCatalogItem>>::new();
    for item in items {
        by_model.entry(item.model_main).or_default().push(item);
        if item.model_sub != 0 {
            by_model.entry(item.model_sub).or_default().push(item);
        }
    }
    let mut models = by_model
        .into_iter()
        .map(|(raw, mut items)| {
            items.sort_by_key(|item| std::cmp::Reverse(item.id));
            (PackedModelId::from_raw(raw), items)
        })
        .collect::<Vec<_>>();
    models.sort_by_key(|(model, items)| std::cmp::Reverse((items[0].id, model.raw)));
    models
}

fn scan_model<R: Resource>(
    resource: &mut R,
    model: PackedModelId,
    items: &[&WeaponCatalogItem],
    report: &mut WeaponShaderFamilyAudit,
    semantic_coverage: &mut MaterialSemanticCoverageBuilder,
) {
    let Some((model_path, model_bytes)) = weapon_model_candidate_paths(model)
        .into_iter()
        .find_map(|path| resource.read(&path).map(|bytes| (path, bytes)))
    else {
        report.failures.push(format!(
            "model {:016X} ({}) has no readable candidate",
            model.raw,
            item_label(items)
        ));
        return;
    };
    let metadata = match mdl_metadata_from_mdl_bytes(&model_path, &model_bytes) {
        Ok(metadata) => metadata,
        Err(error) => {
            report.failures.push(format!(
                "{} ({}) metadata: {error:#}",
                model_path,
                item_label(items)
            ));
            return;
        }
    };
    if let Some(lod) = metadata.lods.first() {
        for range in lod.mesh_ranges.iter().filter(|range| range.mesh_count != 0) {
            *report
                .lod0_mesh_range_model_counts
                .entry(range.category.clone())
                .or_default() += 1;
            *report
                .lod0_mesh_range_mesh_counts
                .entry(range.category.clone())
                .or_default() += usize::from(range.mesh_count);
        }
    }
    if !metadata.shapes.is_empty() {
        report.shape_models.push(WeaponShapeModel {
            item_ids: items.iter().map(|item| item.id).collect(),
            item_names: items.iter().map(|item| item.name.clone()).collect(),
            model,
            model_path: model_path.clone(),
            shape_count: metadata.shapes.len(),
            shape_mesh_count: metadata.shape_meshes.len(),
            shape_value_count: metadata.shape_values.len(),
            shape_names: metadata
                .shapes
                .iter()
                .filter_map(|shape| shape.name.clone())
                .collect(),
        });
    }

    for material_name in metadata
        .materials
        .iter()
        .filter_map(|material| material.name.as_deref())
    {
        let material_candidates =
            weapon_material_candidate_paths(model, &model_path, material_name);
        let platform = resource.platform();
        let (material, readable_candidates) = first_valid_candidate(
            &material_candidates,
            |path| resource.read(path),
            |bytes| physis::mtrl::Material::from_existing(platform, bytes),
        );
        let Some((material_path, material_bytes, material)) = material else {
            if !readable_candidates.is_empty() {
                report.failures.push(format!(
                    "{} material {} ({}) has no parseable candidate; rejected: {}",
                    model_path,
                    material_name,
                    item_label(items),
                    readable_candidates
                        .iter()
                        .map(|(path, bytes)| format!(
                            "{} [{}; bytes={}; header={}]",
                            path,
                            resource_type_hint(bytes),
                            bytes.len(),
                            hex_prefix(bytes, 32)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            } else {
                report
                    .unresolved_material_references
                    .push(WeaponUnresolvedMaterialReference {
                        item_ids: items.iter().map(|item| item.id).collect(),
                        item_names: items.iter().map(|item| item.name.clone()).collect(),
                        model,
                        model_path: model_path.clone(),
                        material_name: material_name.to_string(),
                        candidate_paths: material_candidates,
                    });
            }
            continue;
        };
        report
            .resource_collisions
            .extend(
                readable_candidates
                    .into_iter()
                    .map(|(candidate_path, bytes)| WeaponMaterialResourceCollision {
                        item_ids: items.iter().map(|item| item.id).collect(),
                        item_names: items.iter().map(|item| item.name.clone()).collect(),
                        model,
                        model_path: model_path.clone(),
                        material_name: material_name.to_string(),
                        candidate_path,
                        resource_type: resource_type_hint(&bytes).to_string(),
                        byte_length: bytes.len(),
                        header: hex_prefix(&bytes, 32),
                    }),
            );
        let shader_package_name = material.shader_package_name;
        let shader_family = material_shader_family(Some(&shader_package_name));
        semantic_coverage.record_material(
            resource,
            model,
            items,
            &model_path,
            material_name,
            &material_path,
            &shader_package_name,
            &material_bytes,
        );
        *report
            .family_counts
            .entry(format!("{shader_family:?}"))
            .or_default() += 1;
        report.scanned_materials += 1;
        let candidate = WeaponShaderFamilyCandidate {
            item_ids: items.iter().map(|item| item.id).collect(),
            item_names: items.iter().map(|item| item.name.clone()).collect(),
            model,
            model_path: model_path.clone(),
            material_name: material_name.to_string(),
            material_path,
            shader_package_name,
            shader_family,
        };
        if matches!(
            shader_family,
            MaterialShaderFamily::Bg | MaterialShaderFamily::BgUvScroll
        ) {
            report.candidates.push(candidate);
        } else if shader_family == MaterialShaderFamily::Unknown {
            report.unclassified_materials.push(candidate);
        }
    }
}

fn item_label(items: &[&WeaponCatalogItem]) -> String {
    items
        .first()
        .map(|item| format!("{} {}", item.id, item.name))
        .unwrap_or_else(|| "unknown item".to_string())
}

fn hex_prefix(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn resource_type_hint(bytes: &[u8]) -> &'static str {
    match bytes.get(..4) {
        Some(b"pap ") => "pap",
        Some(b"mdl ") => "mdl",
        Some(b"shPk") => "shpk",
        _ => "unknown",
    }
}

fn first_valid_candidate<T, Read, Validate>(
    candidates: &[String],
    mut read: Read,
    mut validate: Validate,
) -> (Option<(String, Vec<u8>, T)>, Vec<(String, Vec<u8>)>)
where
    Read: FnMut(&str) -> Option<Vec<u8>>,
    Validate: FnMut(&[u8]) -> Option<T>,
{
    let mut rejected = Vec::new();
    for path in candidates {
        let Some(bytes) = read(path) else {
            continue;
        };
        if let Some(value) = validate(&bytes) {
            return (Some((path.clone(), bytes, value)), rejected);
        }
        rejected.push((path.clone(), bytes));
    }
    (None, rejected)
}

fn scan_limit() -> Option<usize> {
    std::env::var("XIV_WEAPON_SHADER_SCAN_LIMIT")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|limit| *limit > 0)
}

fn item_ids() -> Option<Vec<u32>> {
    let ids = std::env::var("XIV_WEAPON_SHADER_ITEM_IDS")
        .ok()?
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect::<Vec<_>>();
    (!ids.is_empty()).then_some(ids)
}

fn game_dir() -> PathBuf {
    std::env::var_os("XIV_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"E:\_ff14\game"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_color_table_row() -> xiv_companion_data::MaterialColorTableRowDebug {
        xiv_companion_data::MaterialColorTableRowDebug {
            index: 0,
            diffuse_color: Some([1.0, 0.5, 0.25]),
            specular_color: Some([2.0, 3.0, 4.0]),
            emissive_color: Some([61.46875, 0.0, 0.0]),
            specular_strength: None,
            gloss_strength: None,
            roughness: None,
            metalness: None,
            anisotropy: Some(7.0),
            tile_alpha: None,
            tile_index: None,
            sheen_rate: Some(1.0),
            sheen_tint: Some(1.0),
            sheen_aperture: Some(52.09375),
            sphere_mask: Some(1.0),
            tile_set: None,
            shader_index: None,
            sphere_index: None,
            tile_matrix: None,
            material_repeat: None,
            material_skew: None,
        }
    }

    #[test]
    fn color_table_float_ramp_audit_rejects_nonfinite_and_half_overflow_values() {
        let finite = test_color_table_row();
        assert_eq!(invalid_color_table_float_ramp_value(&[finite]), None);

        let mut nonfinite = test_color_table_row();
        nonfinite.sheen_aperture = Some(f32::NAN);
        let (row, field, value) =
            invalid_color_table_float_ramp_value(&[nonfinite]).expect("NaN must be rejected");
        assert_eq!(row, 0);
        assert_eq!(field, "SheenAptitude");
        assert!(value.is_nan());

        let mut overflow = test_color_table_row();
        overflow.specular_color = Some([MAX_RAMP_F16 + 1.0, 0.0, 0.0]);
        assert_eq!(
            invalid_color_table_float_ramp_value(&[overflow]),
            Some((0, "Specular", MAX_RAMP_F16 + 1.0))
        );
    }

    #[test]
    fn catalog_models_deduplicates_primary_and_secondary_models() {
        let item = |id, model_main, model_sub| WeaponCatalogItem {
            id,
            name: format!("item-{id}"),
            description: String::new(),
            icon: 0,
            item_ui_category: 1,
            item_search_category: 1,
            equip_slot_category: 1,
            price_mid: 0,
            price_low: 0,
            model_main,
            model_sub,
        };
        let items = [item(10, 100, 200), item(20, 100, 0), item(30, 300, 200)];
        let models = catalog_models(&items);

        assert_eq!(models.len(), 3);
        assert_eq!(models[0].0.raw, 300);
        assert_eq!(models[1].0.raw, 200);
        assert_eq!(models[2].0.raw, 100);
        assert_eq!(
            models[2].1.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![20, 10]
        );
    }

    #[test]
    fn resource_type_hint_identifies_pap_hash_collisions() {
        assert_eq!(resource_type_hint(b"pap \x01\x00"), "pap");
        assert_eq!(resource_type_hint(&[0, 0, 3, 1]), "unknown");
    }

    #[test]
    fn candidate_validation_continues_after_wrong_resource_type() {
        let candidates = vec!["collision".to_string(), "material".to_string()];
        let (selected, rejected) = first_valid_candidate(
            &candidates,
            |path| match path {
                "collision" => Some(b"pap ".to_vec()),
                "material" => Some(vec![0, 0, 3, 1]),
                _ => None,
            },
            |bytes| (bytes == [0, 0, 3, 1]).then_some("mtrl"),
        );

        assert_eq!(
            selected,
            Some(("material".to_string(), vec![0, 0, 3, 1], "mtrl"))
        );
        assert_eq!(rejected, vec![("collision".to_string(), b"pap ".to_vec())]);
    }

    #[test]
    fn semantic_coverage_separates_scopes_and_deduplicates_resources() {
        let package = ShaderPackageSemanticDebug {
            path: "shader/sm5/shpk/character.shpk".to_string(),
            name: "character.shpk".to_string(),
            sampler_resources: Vec::new(),
            material_keys: vec![test_package_key(
                0x100,
                Some("MaterialKey"),
                10,
                Some("MaterialDefault"),
            )],
            system_keys: vec![test_package_key(
                0x200,
                Some("SystemKey"),
                20,
                Some("SystemDefault"),
            )],
            scene_keys: vec![test_package_key(
                0x300,
                Some("SceneKey"),
                30,
                Some("SceneDefault"),
            )],
            material_constants: vec![
                test_package_constant(0x500, Some("g_Known"), 0, 4, Some(vec![0.0])),
                test_package_constant(0x900, Some("g_NoDefault"), 4, 4, None),
                test_package_constant(0xA00, Some("g_ZeroWidth"), 8, 0, Some(Vec::new())),
            ],
        };
        let override_keys = vec![
            ObservedMaterialKey {
                category: 0x100,
                category_name: Some("MaterialKey".to_string()),
                value: 11,
                value_name: None,
            },
            ObservedMaterialKey {
                category: 0x300,
                category_name: Some("SceneKey".to_string()),
                value: 31,
                value_name: Some("SceneOverride".to_string()),
            },
            ObservedMaterialKey {
                category: 0x400,
                category_name: None,
                value: 40,
                value_name: None,
            },
        ];
        let override_constants = vec![
            ObservedMaterialConstant {
                id: 0x500,
                name: Some("g_Known".to_string()),
                values: vec![1.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x500,
                name: Some("g_Known".to_string()),
                values: Vec::new(),
                value_size: 2,
                malformed: true,
                resolved: false,
            },
            ObservedMaterialConstant {
                id: 0x600,
                name: None,
                values: vec![2.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x600,
                name: None,
                values: vec![3.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x700,
                name: Some("g_NonFinite".to_string()),
                values: vec![f32::NAN],
                value_size: 6,
                malformed: true,
                resolved: true,
            },
            ObservedMaterialConstant {
                id: 0x800,
                name: Some("g_Malformed".to_string()),
                values: Vec::new(),
                value_size: 2,
                malformed: true,
                resolved: false,
            },
            ObservedMaterialConstant {
                id: 0x900,
                name: Some("g_NoDefault".to_string()),
                values: vec![4.0],
                value_size: 4,
                malformed: false,
                resolved: true,
            },
        ];
        let first = test_representative("a.mtrl", 0x11, 3);
        let second = test_representative("b.mtrl", 0x01, 2);
        let mut builder = MaterialSemanticCoverageBuilder::default();

        for _ in 0..2 {
            builder.observe_material(
                "character.shpk",
                Some(&package),
                "a.mtrl",
                0x11,
                &override_keys,
                &override_constants,
                &[],
                ObservedColorTableScalars {
                    diffuse: &[0.0, 1.5],
                    specular: &[0.0, 3.0],
                    emissive: &[0.0, 2.0],
                    metalness: &[0.0, 1.0],
                    roughness: &[0.0, 1.0],
                    gloss_strength: &[0.0, 1.25],
                    specular_strength: &[0.0, 1.5],
                    anisotropy: &[0.0, 0.5],
                    sheen_rate: &[0.0, 0.25],
                    sheen_aptitude: &[0.0, 4.0],
                    sphere_mask: &[0.0, 0.5],
                },
                &first,
            );
        }
        builder.observe_material(
            "character.shpk",
            Some(&package),
            "b.mtrl",
            0x01,
            &[],
            &[],
            &[],
            ObservedColorTableScalars {
                diffuse: &[0.0],
                specular: &[0.0],
                emissive: &[0.0],
                metalness: &[0.0],
                roughness: &[0.0],
                gloss_strength: &[0.0],
                specular_strength: &[0.0],
                anisotropy: &[0.0],
                sheen_rate: &[0.0],
                sheen_aptitude: &[0.0],
                sphere_mask: &[0.0],
            },
            &second,
        );

        let result = builder.finish();
        assert_eq!(result.unique_material_resources, 2);
        assert_eq!(result.unique_shader_packages, 1);
        assert_eq!(result.unknown_key_category_count, 1);
        assert_eq!(result.unknown_key_value_count, 2);
        assert_eq!(result.color_table_diffuse.maximum, Some(1.5));
        assert_eq!(result.color_table_specular.maximum, Some(3.0));
        assert_eq!(result.color_table_emissive.material_resource_count, 2);
        assert_eq!(result.color_table_emissive.material_reference_count, 8);
        assert_eq!(
            result.color_table_emissive.nonzero_material_resource_count,
            1
        );
        assert_eq!(result.color_table_emissive.minimum, Some(0.0));
        assert_eq!(result.color_table_emissive.maximum, Some(2.0));
        assert_eq!(result.color_table_anisotropy.material_resource_count, 2);
        assert_eq!(result.color_table_anisotropy.material_reference_count, 8);
        assert_eq!(
            result
                .color_table_anisotropy
                .nonzero_material_resource_count,
            1
        );
        assert_eq!(
            result
                .color_table_anisotropy
                .nonzero_material_reference_count,
            6
        );
        assert_eq!(result.color_table_anisotropy.minimum, Some(0.0));
        assert_eq!(result.color_table_anisotropy.maximum, Some(0.5));
        assert_eq!(result.color_table_sheen_rate.material_resource_count, 2);
        assert_eq!(
            result
                .color_table_sheen_rate
                .nonzero_material_resource_count,
            1
        );
        assert_eq!(result.color_table_sheen_rate.maximum, Some(0.25));
        assert_eq!(result.color_table_sheen_aptitude.maximum, Some(4.0));
        assert_eq!(result.color_table_sphere_mask.material_resource_count, 2);
        assert_eq!(
            result
                .color_table_sphere_mask
                .nonzero_material_resource_count,
            1
        );
        assert_eq!(result.color_table_sphere_mask.maximum, Some(0.5));
        assert_eq!(result.unknown_constant_id_count, 1);

        let material_key = result
            .material_key_coverage
            .iter()
            .find(|coverage| {
                coverage.scope == WeaponShaderKeyScope::Material && coverage.category == 0x100
            })
            .expect("material key coverage");
        assert_eq!(material_key.material_resource_count, 2);
        assert_eq!(material_key.material_reference_count, 8);
        assert_eq!(material_key.material_override_resource_count, 1);
        assert_eq!(material_key.material_override_reference_count, 6);
        assert_eq!(material_key.observed_values.len(), 2);
        assert!(
            material_key
                .observed_values
                .iter()
                .any(|value| value.value == 11 && value.value_name.is_none())
        );

        let scene_key = result
            .material_key_coverage
            .iter()
            .find(|coverage| {
                coverage.scope == WeaponShaderKeyScope::Scene && coverage.category == 0x300
            })
            .expect("scene key coverage");
        assert_eq!(scene_key.material_override_reference_count, 6);
        assert!(
            scene_key
                .observed_values
                .iter()
                .any(|value| value.value == 31)
        );
        assert_eq!(scene_key.material_reference_count, 8);
        assert!(!result.material_key_coverage.iter().any(|coverage| {
            coverage.scope == WeaponShaderKeyScope::MaterialOverrideOnly
                && coverage.category == 0x300
        }));

        let known_constant = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x500)
            .expect("known constant coverage");
        assert_eq!(known_constant.material_resource_count, 2);
        assert_eq!(known_constant.material_reference_count, 8);
        assert_eq!(known_constant.material_override_resource_count, 1);
        assert_eq!(known_constant.material_override_reference_count, 6);
        assert_eq!(known_constant.malformed_override_reference_count, 6);
        assert_eq!(known_constant.shader_flag_counts.len(), 2);

        let duplicate_override_only = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x600)
            .expect("duplicate override-only constant coverage");
        assert_eq!(duplicate_override_only.material_resource_count, 1);
        assert_eq!(duplicate_override_only.material_reference_count, 6);
        assert_eq!(duplicate_override_only.observed_values.len(), 1);
        assert_eq!(
            duplicate_override_only.observed_values[0].values,
            vec![Some(3.0)]
        );

        let non_finite = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x700)
            .expect("non-finite constant coverage");
        assert_eq!(non_finite.non_finite_resource_count, 1);
        assert_eq!(non_finite.non_finite_reference_count, 6);
        assert_eq!(non_finite.malformed_override_reference_count, 6);
        assert_eq!(
            non_finite.malformed_override_value_size_resource_counts,
            BTreeMap::from([(6, 1)])
        );
        assert_eq!(non_finite.observed_values[0].values, vec![None]);
        let malformed = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x800)
            .expect("malformed constant coverage");
        assert_eq!(malformed.malformed_override_resource_count, 1);
        assert_eq!(malformed.malformed_override_reference_count, 6);
        assert_eq!(malformed.unresolved_value_reference_count, 6);

        let no_default = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0x900)
            .expect("no-default package constant coverage");
        assert_eq!(no_default.material_reference_count, 8);
        assert_eq!(no_default.material_override_reference_count, 6);
        assert_eq!(no_default.unresolved_value_reference_count, 2);
        assert_eq!(no_default.observed_values[0].values, vec![Some(4.0)]);

        let zero_width = result
            .material_constant_coverage
            .iter()
            .find(|coverage| coverage.id == 0xA00)
            .expect("zero-width package constant coverage");
        assert_eq!(zero_width.default_values, Some(Vec::new()));
        assert_eq!(zero_width.unresolved_value_reference_count, 0);
        assert_eq!(
            zero_width.value_width_resource_counts,
            BTreeMap::from([(0, 2)])
        );
        serde_json::to_vec(&result.material_constant_coverage)
            .expect("non-finite coverage remains JSON serializable");
    }

    #[test]
    fn sampler_coverage_preserves_exact_skin_role_and_package_resource_name() {
        let texture_usage = physis::shpk::ShaderPackage::crc("g_SamplerSkinDiffuse");
        let package = ShaderPackageSemanticDebug {
            path: "shader/sm5/shpk/character.shpk".to_string(),
            name: "character.shpk".to_string(),
            sampler_resources: vec![ShaderPackageSamplerResourceDebug {
                name: "g_SamplerSkinDiffuse".to_string(),
                crc: texture_usage,
                crc_hex: hex_u32(texture_usage),
                slot: 2,
                size: 1,
                logical_role: Some(MaterialSamplerLogicalRole::SkinDiffuse),
                kind: Some(ModelTextureKind::BaseColor),
            }],
            material_keys: Vec::new(),
            system_keys: Vec::new(),
            scene_keys: Vec::new(),
            material_constants: Vec::new(),
        };
        let sampler = ObservedMaterialSampler {
            texture_usage,
            texture_usage_name: Some("known-crc-alias".to_string()),
            logical_role: Some(MaterialSamplerLogicalRole::SkinDiffuse),
            texture_kind: Some(ModelTextureKind::BaseColor),
            flags: 0x1234_5678,
        };
        let representative = test_representative("skin.mtrl", 0x11, 3);
        let mut builder = MaterialSemanticCoverageBuilder::default();

        builder.observe_material(
            "character.shpk",
            Some(&package),
            "skin.mtrl",
            0x11,
            &[],
            &[],
            &[sampler.clone(), sampler],
            ObservedColorTableScalars::default(),
            &representative,
        );

        let result = builder.finish();
        assert_eq!(result.sampler_coverage.len(), 1);
        let coverage = &result.sampler_coverage[0];
        assert_eq!(
            coverage.texture_usage_name.as_deref(),
            Some("g_SamplerSkinDiffuse")
        );
        assert_eq!(
            coverage.logical_role,
            Some(MaterialSamplerLogicalRole::SkinDiffuse)
        );
        assert_eq!(coverage.texture_kind, Some(ModelTextureKind::BaseColor));
        assert_eq!(coverage.flags, 0x1234_5678);
        assert_eq!(coverage.material_resource_count, 1);
        assert_eq!(coverage.material_reference_count, 3);
        assert_eq!(result.unknown_sampler_role_count, 0);
        assert_eq!(result.unresolved_sampler_name_count, 0);
    }

    fn test_package_key(
        id: u32,
        name: Option<&str>,
        default_value: u32,
        default_value_name: Option<&str>,
    ) -> ShaderPackageKeyDefaultDebug {
        ShaderPackageKeyDefaultDebug {
            id,
            id_hex: hex_u32(id),
            name: name.map(str::to_string),
            default_value,
            default_value_hex: hex_u32(default_value),
            default_value_name: default_value_name.map(str::to_string),
        }
    }

    fn test_package_constant(
        id: u32,
        name: Option<&str>,
        byte_offset: u16,
        byte_size: u16,
        default_values: Option<Vec<f32>>,
    ) -> ShaderPackageMaterialConstantDebug {
        ShaderPackageMaterialConstantDebug {
            id,
            id_hex: hex_u32(id),
            name: name.map(str::to_string),
            byte_offset,
            byte_size,
            default_values,
        }
    }

    fn test_representative(
        material_path: &str,
        shader_flags: u32,
        item_reference_count: usize,
    ) -> WeaponSemanticRepresentative {
        WeaponSemanticRepresentative {
            item_reference_count,
            item_ids: vec![1],
            item_names: vec!["item".to_string()],
            model: PackedModelId::from_raw(1),
            model_path: "model.mdl".to_string(),
            material_name: material_path.to_string(),
            material_path: material_path.to_string(),
            shader_flags,
            shader_flags_hex: hex_u32(shader_flags),
        }
    }
}
