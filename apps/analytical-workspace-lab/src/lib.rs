//! Analytical Workspace Lab application composition.

mod app;
mod panes;
#[cfg(target_arch = "wasm32")]
mod web_worker;

pub use app::AnalyticalWorkspaceApp;

pub const APPLICATION_NAME: &str = "Analytical Workspace Lab";

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        let _ = eframe::WebLogger::init(log::LevelFilter::Info);
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(AnalyticalWorkspaceApp::new(cc)))),
            )
            .await
    }

    pub fn destroy(&self) {
        self.runner.destroy();
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for WebHandle {
    fn default() -> Self {
        Self::new()
    }
}
