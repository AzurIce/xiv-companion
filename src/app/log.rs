use wasm_bindgen::JsValue;

pub(crate) fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub(crate) fn elapsed_ms(start_ms: f64) -> f64 {
    now_ms() - start_ms
}

pub(crate) fn format_elapsed(ms: f64) -> String {
    if ms >= 1_000.0 {
        format!("{:.2}s", ms / 1_000.0)
    } else {
        format!("{ms:.0}ms")
    }
}

fn prefix(scope: &str) -> JsValue {
    JsValue::from_str(&format!("[xiv-companion:{scope}]"))
}

pub(crate) fn info(scope: &str, message: impl AsRef<str>) {
    web_sys::console::info_2(&prefix(scope), &JsValue::from_str(message.as_ref()));
}

pub(crate) fn debug(scope: &str, message: impl AsRef<str>) {
    if debug_enabled() {
        web_sys::console::debug_2(&prefix(scope), &JsValue::from_str(message.as_ref()));
    }
}

pub(crate) fn warn(scope: &str, message: impl AsRef<str>) {
    web_sys::console::warn_2(&prefix(scope), &JsValue::from_str(message.as_ref()));
}

pub(crate) fn error(scope: &str, message: impl AsRef<str>) {
    web_sys::console::error_2(&prefix(scope), &JsValue::from_str(message.as_ref()));
}

fn debug_enabled() -> bool {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item("xiv-companion.debug").ok().flatten())
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
}
