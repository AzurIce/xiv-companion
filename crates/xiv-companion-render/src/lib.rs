#[cfg(feature = "ui")]
pub mod icons;
pub mod model;
#[cfg(feature = "ui")]
pub mod modules;
#[cfg(feature = "renderer")]
pub mod renderer;
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub mod test_support;
#[cfg(feature = "ui")]
pub mod ui;
#[cfg(feature = "ui")]
pub mod utils;

pub use model::{
    BakedColorTableMaps, ColorTableRowColors, PackedModelId, WeaponCatalogCounts,
    WeaponCatalogItem, WeaponCatalogPackage, WeaponMaterialRenderMode, WeaponModelBounds,
    WeaponModelData, WeaponModelMaterial, WeaponModelMesh, WeaponModelTexture,
    WeaponModelTextureKind, WeaponModelVertex, bake_color_table_maps, calculate_model_bounds,
    is_weapon_equip_slot_category, material_color, weapon_material_candidate_paths,
    weapon_model_candidate_paths, weapon_slot_label,
};

#[cfg(feature = "ui")]
pub use icons::{Icon, IconKind};
#[cfg(feature = "ui")]
pub use modules::{APP_MODULES, AppModule, ModuleGroup, ModuleStatus, module_group_label};
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use renderer::WebWeaponCanvasRenderer;
#[cfg(feature = "renderer")]
pub use renderer::{WeaponRenderOptions, WeaponRenderer};
#[cfg(feature = "ui")]
pub use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, CardContent, CardHeader,
    CardTitle, EmptyState, input_class,
};
#[cfg(feature = "ui")]
pub use utils::{cx, format_integer};
