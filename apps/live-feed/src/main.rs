#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use live_feed::{APPLICATION_NAME, LiveFeed, Source};
    stobie_ui_egui::run_native(
        APPLICATION_NAME,
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 720.0]),
            ..Default::default()
        },
        |_| LiveFeed::new(Source::Stdin),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
