pub use xiv_companion_data::model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialRenderMode, ModelBounds, ModelData,
    ModelMaterial, ModelMesh, ModelRenderData, ModelTexture, ModelTextureKind, ModelVertex,
    PackedModelId, WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage,
    WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData, WeaponModelMaterial,
    WeaponModelMesh, WeaponModelTexture, WeaponModelTextureKind, WeaponModelVertex,
    bake_color_table_maps, calculate_model_bounds, is_weapon_equip_slot_category, material_color,
    weapon_material_candidate_paths, weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "game-data")]
pub use xiv_companion_data::game_data::normalize_game_dir;
#[cfg(feature = "game-data")]
pub use xiv_companion_data::weapon_models::{
    load_weapon_model_from_game_dir, load_weapon_model_from_resource, meshes_from_mdl_bytes,
};
