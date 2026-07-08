pub use xiv_companion_data::model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialAlphaMode, MaterialRenderMode, ModelBounds,
    ModelData, ModelMaterial, ModelMesh, ModelRenderData, ModelTexture, ModelTextureKind,
    ModelVertex, PackedModelId, WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage,
    WeaponMaterialAlphaMode, WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData,
    WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture, WeaponModelTextureKind,
    WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "game-data")]
pub use xiv_companion_data::game_data::normalize_game_dir;
#[cfg(feature = "game-data")]
pub use xiv_companion_data::mdl_metadata::mdl_metadata_from_mdl_bytes;
#[cfg(feature = "game-data")]
pub use xiv_companion_data::weapon_models::{
    AsyncGameResource, WeaponModelLoadRequest, load_weapon_model_from_async_resource,
    load_weapon_model_from_game_dir, load_weapon_model_from_resource,
    load_weapon_model_from_resource_request, material_debug_info_from_mtrl_bytes,
    meshes_from_mdl_bytes,
};
