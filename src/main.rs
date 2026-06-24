#[cfg(feature = "web")]
mod app;

#[cfg(feature = "web")]
fn main() {
    console_error_panic_hook::set_once();
    dioxus::launch(app::App);
}

#[cfg(not(feature = "web"))]
fn main() {
    eprintln!("Run this binary with --features web for the Dioxus web app.");
}
