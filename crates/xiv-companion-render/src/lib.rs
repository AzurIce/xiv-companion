pub mod model;
#[cfg(feature = "renderer")]
pub mod renderer;
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub mod test_support;

pub use model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialAlphaMode, MaterialCharacterScrollVariant,
    MaterialDecalColorMode, MaterialDrawDepthMode, MaterialFlowMode, MaterialLightShaftType,
    MaterialLightingMode, MaterialRenderMode, MaterialShaderFamily, MaterialSkinValueMode,
    MaterialSubColorMode, MaterialValueMode, ModelBounds, ModelData, ModelMaterial,
    ModelMaterialTextureArrays, ModelMesh, ModelMeshDrawRole, ModelRenderData, ModelShapeInfo,
    ModelShapeTarget, ModelShapeVertexDelta, ModelSubmeshInfo, ModelTexture, ModelTextureKind,
    ModelVertex, PackedModelId, PreparedAlphaSource, PreparedMaterial, PreparedMaterialAlphaPolicy,
    PreparedMaterialFeatureFlags, PreparedMaterialResourceAvailability,
    PreparedMaterialRuntimeFallbacks, PreparedMaterialUnsupportedInputs, PreparedMaterialUvSources,
    PreparedMesh, PreparedMeshShapeInfluences, PreparedMeshVisibility, PreparedModel,
    PreparedModelOptions, PreparedRenderPass, PreparedRuntimeFallback, PreparedTextureAddressMode,
    PreparedTextureArrayResource, PreparedTextureArrayStatus, PreparedTextureBindings,
    PreparedTextureColorSpace, PreparedTextureFilter, PreparedTextureSampling,
    PreparedTextureSamplingSet, PreparedTextureScrollSet, PreparedTextureUvSources,
    PreparedUvSource, WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage,
    WeaponMaterialAlphaMode, WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData,
    WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture, WeaponModelTextureKind,
    WeaponModelVertex, WeaponStain, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, material_shader_family,
    mesh_draw_role_for_category, model_mesh_vertices_with_shape_mask,
    prepare_material_for_draw_role, prepare_model_for_render,
    prepare_model_for_render_with_options, prepared_material_feature_flags,
    prepared_material_resource_availability, prepared_material_runtime_fallbacks,
    prepared_material_unsupported_inputs, prepared_material_uv_sources, prepared_texture_bindings,
    prepared_texture_sampling_for_kind, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "renderer")]
pub use renderer::{
    ModelDebugMode, ModelGlassBlendMode, ModelRenderOptions, ModelRenderer, WeaponRenderOptions,
    WeaponRenderer,
};
