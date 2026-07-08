pub mod weapon;

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;

pub use weapon::{ModelRenderOptions, ModelRenderer, WeaponRenderOptions, WeaponRenderer};

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::{WebModelCanvasRenderer, WebWeaponCanvasRenderer};
