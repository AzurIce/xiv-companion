pub mod model;
pub mod planner;
pub mod resources;
pub mod solver;
pub mod weapon_models;

#[cfg(feature = "renderer")]
pub mod renderer {
    pub use xiv_companion_render::renderer::*;

    #[cfg(all(feature = "render-test-support", not(target_arch = "wasm32")))]
    pub mod test_support {
        pub use xiv_companion_render::test_support::*;
    }
}

#[cfg(feature = "game-data")]
pub mod audit;

#[cfg(feature = "game-data")]
pub mod game_data;

#[cfg(feature = "wasm")]
mod wasm;

pub use model::*;
pub use planner::*;
pub use resources::collection_catalog::*;
pub use resources::craft_data::*;
pub use resources::item_icon::*;
pub use resources::weapon_model::*;
pub use resources::*;

// Re-export the core collection data types from the data crate so the resource
// spec and UI can use them through the main crate.
pub use solver::*;
pub use weapon_models::*;
pub use xiv_companion_data::collection::*;
