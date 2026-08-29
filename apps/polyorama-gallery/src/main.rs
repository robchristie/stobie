#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use polyorama_gallery::{APPLICATION_NAME, GalleryApp};
    eframe::run_native(
        APPLICATION_NAME,
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1440.0, 900.0])
                .with_min_inner_size([960.0, 640.0]),
            ..Default::default()
        },
        Box::new(|creation| Ok(Box::new(GalleryApp::new(creation)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
