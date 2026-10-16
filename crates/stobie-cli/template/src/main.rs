//! The native entry point. Run it with `cargo run --release`; set
//! `STOBIE_AUTOMATION_SOCKET`, or use `stobie launch`, to operate it.

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    stobie_ui_egui::run_native("{{title}}", eframe::NativeOptions::default(), |_| {
        {{crate}}::App::default()
    })
}

// The browser build uses the library's `WebHandle` instead.
#[cfg(target_arch = "wasm32")]
fn main() {}
