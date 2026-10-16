//! The native entry point. Run it with `cargo run --release`; set
//! `STOBIE_AUTOMATION_SOCKET`, or use `stobie launch`, to operate it.

#[cfg(not(target_arch = "wasm32"))]
use {{crate}}::App;

#[cfg(not(target_arch = "wasm32"))]
const TITLE: &str = "{{title}}";

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();
    stobie_ui_egui::run_native(TITLE, options, |_| App::default())
}

// The browser build uses the library's `WebHandle` instead.
#[cfg(target_arch = "wasm32")]
fn main() {}
