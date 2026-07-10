pub use xiv_companion_data::model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialAlphaMode, MaterialRenderMode,
    MaterialShaderFamily, ModelBounds, ModelData, ModelMaterial, ModelMaterialTextureArrays,
    ModelMesh, ModelMeshDrawRole, ModelRenderData, ModelShapeInfo, ModelStainingApplication,
    ModelSubmeshInfo, ModelTexture, ModelTextureKind, ModelVertex, PackedModelId, PreparedMaterial,
    PreparedMaterialFeatureFlags, PreparedMaterialResourceAvailability,
    PreparedMaterialRuntimeFallbacks, PreparedMaterialUnsupportedInputs, PreparedMaterialUvSources,
    PreparedMesh, PreparedMeshShapeInfluences, PreparedMeshVisibility, PreparedModel,
    PreparedModelOptions, PreparedRenderPass, PreparedRuntimeFallback, PreparedTextureAddressMode,
    PreparedTextureBindings, PreparedTextureColorSpace, PreparedTextureFilter,
    PreparedTextureSampling, PreparedTextureSamplingSet, PreparedTextureUvSources,
    PreparedUvSource, StainingApplicationReport, WeaponCatalogCounts, WeaponCatalogItem,
    WeaponCatalogPackage, WeaponMaterialAlphaMode, WeaponMaterialRenderMode, WeaponModelBounds,
    WeaponModelData, WeaponModelLoadCandidateDiagnostic, WeaponModelLoadCandidateStatus,
    WeaponModelLoadDiagnostic, WeaponModelLoadRole, WeaponModelMaterial, WeaponModelMesh,
    WeaponModelTexture, WeaponModelTextureKind, WeaponModelVertex, WeaponStain,
    bake_color_table_maps, calculate_model_bounds, is_weapon_equip_slot_category, material_color,
    material_shader_family, mesh_draw_role_for_category, prepare_material_for_draw_role,
    prepare_model_for_render, prepare_model_for_render_with_options,
    prepared_material_feature_flags, prepared_material_resource_availability,
    prepared_material_runtime_fallbacks, prepared_material_unsupported_inputs,
    prepared_texture_bindings, prepared_texture_sampling_for_kind, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "game-data")]
pub use xiv_companion_data::game_data::normalize_game_dir;
#[cfg(feature = "game-data")]
pub use xiv_companion_data::mdl_metadata::mdl_metadata_from_mdl_bytes;
#[cfg(feature = "game-data")]
pub use xiv_companion_data::weapon_models::{
    AsyncGameResource, MaterialResolvedConstantDebug, MaterialResolvedShaderKeyDebug,
    MaterialSamplerFlagSummaryDebug, MaterialSemanticSummaryDebug, MaterialTextureFlagSummaryDebug,
    WeaponModelLoadRequest, load_weapon_model_from_async_resource, load_weapon_model_from_game_dir,
    load_weapon_model_from_resource, load_weapon_model_from_resource_request,
    material_debug_info_from_mtrl_bytes, material_debug_info_from_resource, meshes_from_mdl_bytes,
};
