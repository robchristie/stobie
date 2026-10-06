#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    eframe::run_native(
        "Record Desk",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1080.0, 760.0])
                .with_min_inner_size([360.0, 640.0]),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(record_desk::app::RecordDeskApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
