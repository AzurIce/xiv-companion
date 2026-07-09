pub mod model;
#[cfg(feature = "renderer")]
pub mod renderer;
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub mod test_support;

pub use model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialAlphaMode, MaterialRenderMode,
    MaterialShaderFamily, ModelBounds, ModelData, ModelMaterial, ModelMesh, ModelMeshDrawRole,
    ModelRenderData, ModelShapeInfo, ModelSubmeshInfo, ModelTexture, ModelTextureKind, ModelVertex,
    PackedModelId, PreparedMaterial, PreparedMaterialFeatureFlags,
    PreparedMaterialUnsupportedInputs, PreparedMaterialUvSources, PreparedMesh,
    PreparedMeshShapeInfluences, PreparedMeshVisibility, PreparedModel, PreparedModelOptions,
    PreparedRenderPass, PreparedTextureAddressMode, PreparedTextureBindings,
    PreparedTextureColorSpace, PreparedTextureFilter, PreparedTextureSampling,
    PreparedTextureSamplingSet, PreparedTextureUvSources, PreparedUvSource, WeaponCatalogCounts,
    WeaponCatalogItem, WeaponCatalogPackage, WeaponMaterialAlphaMode, WeaponMaterialRenderMode,
    WeaponModelBounds, WeaponModelData, WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture,
    WeaponModelTextureKind, WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, material_shader_family,
    mesh_draw_role_for_category, prepare_material_for_draw_role, prepare_model_for_render,
    prepare_model_for_render_with_options, prepared_material_feature_flags,
    prepared_material_unsupported_inputs, prepared_texture_bindings,
    prepared_texture_sampling_for_kind, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "renderer")]
pub use renderer::{
    ModelDebugMode, ModelRenderOptions, ModelRenderer, WeaponRenderOptions, WeaponRenderer,
};
