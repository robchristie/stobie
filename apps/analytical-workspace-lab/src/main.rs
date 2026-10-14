#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use analytical_workspace_lab::{APPLICATION_NAME, Lab};
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        persistence_path: std::env::var_os("STOBIE_PERSISTENCE_PATH").map(std::path::PathBuf::from),
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    stobie_ui_egui::run_native(APPLICATION_NAME, options, Lab::new)
}

#[cfg(target_arch = "wasm32")]
fn main() {}
