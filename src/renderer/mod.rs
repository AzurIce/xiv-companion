pub mod weapon;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use weapon::WeaponRenderer;

#[cfg(target_arch = "wasm32")]
pub use web::WebWeaponCanvasRenderer;
