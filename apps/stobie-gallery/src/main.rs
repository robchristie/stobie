#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use stobie_gallery::{APPLICATION_NAME, Gallery};
    stobie_ui_egui::run_native(
        APPLICATION_NAME,
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([960.0, 640.0]),
            ..Default::default()
        },
        Gallery::new,
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
