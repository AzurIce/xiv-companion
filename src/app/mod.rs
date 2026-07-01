use dioxus::prelude::*;
use wasm_bindgen::JsCast;

pub(crate) mod browser_sqpack;
mod data;
mod load_progress;
mod log;
mod pages;
mod resource_settings;
mod resources;
mod shell;

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
