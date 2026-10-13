#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1080.0, 760.0])
        .with_min_inner_size([360.0, 640.0]);
    stobie_ui_egui::run_native(
        "Record Desk",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        record_desk::app::RecordDesk::new,
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
