pub mod model;
#[cfg(feature = "renderer")]
pub mod renderer;
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub mod test_support;

pub use model::{
    BakedColorTableMaps, ColorTableRowColors, MaterialRenderMode, ModelBounds, ModelData,
    ModelMaterial, ModelMesh, ModelRenderData, ModelTexture, ModelTextureKind, ModelVertex,
    PackedModelId, WeaponCatalogCounts, WeaponCatalogItem, WeaponCatalogPackage,
    WeaponMaterialRenderMode, WeaponModelBounds, WeaponModelData, WeaponModelMaterial,
    WeaponModelMesh, WeaponModelTexture, WeaponModelTextureKind, WeaponModelVertex,
    bake_color_table_maps, calculate_model_bounds, is_weapon_equip_slot_category, material_color,
    weapon_material_candidate_paths, weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "renderer")]
pub use renderer::{ModelRenderOptions, ModelRenderer, WeaponRenderOptions, WeaponRenderer};
