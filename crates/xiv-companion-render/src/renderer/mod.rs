pub mod weapon;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;

pub use weapon::{WeaponRenderOptions, WeaponRenderer};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::WebWeaponCanvasRenderer;
