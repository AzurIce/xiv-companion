use dioxus::prelude::*;
use wasm_bindgen::JsCast;

pub(crate) mod browser_sqpack;
mod collection_index;
#[cfg(target_arch = "wasm32")]
mod collection_state;
mod data;
mod icons;
#[cfg(target_arch = "wasm32")]
mod indexed_db_cache;
mod load_progress;
mod log;
#[cfg(target_arch = "wasm32")]
mod model_canvas_renderer;
mod modules;
mod pages;
mod resource_settings;
mod resources;
mod shell;
mod ui;
mod user_local_directory;
mod utils;

use shell::{AppShell, Route};

const _: Asset = asset!(
    "/assets/tailwind.css",
    AssetOptions::css().with_static_head(true)
);

#[component]
pub fn App() -> Element {
    let route = use_signal(Route::from_hash);

    use_effect(move || {
        let mut route = route;
        let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
            route.set(Route::from_hash());
        });
        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
        }
        closure.forget();
    });

    rsx! {
        AppShell { route }
    }
}
