use dioxus::prelude::*;
use wasm_bindgen::JsCast;

mod data;
mod icons;
mod modules;
mod pages;
mod shell;
mod ui;
mod utils;

use shell::{AppShell, Route};

const MAIN_CSS: Asset = asset!("/assets/tailwind.css");

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
        document::Stylesheet { href: MAIN_CSS }
        AppShell { route }
    }
}
