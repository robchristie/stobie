//! Analytical Workspace Lab application composition.

mod app;
mod panes;
mod thumbnail_cache;
mod ui_geometry;
#[cfg(target_arch = "wasm32")]
mod web_worker;

pub use app::AnalyticalWorkspaceApp;
#[cfg(target_arch = "wasm32")]
use app::TestAction;

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

    pub fn test_action(&self, value: JsValue) -> Result<JsValue, JsValue> {
        let action: TestAction = serde_wasm_bindgen::from_value(value)?;
        let mut app = self
            .runner
            .app_mut::<AnalyticalWorkspaceApp>()
            .ok_or_else(|| JsValue::from_str("Polyorama application is unavailable"))?;
        let snapshot = app
            .test_action(action)
            .map_err(|error| JsValue::from_str(&error))?;
        serde_wasm_bindgen::to_value(&snapshot).map_err(Into::into)
    }

    pub fn test_snapshot(&self) -> Result<JsValue, JsValue> {
        let app = self
            .runner
            .app_mut::<AnalyticalWorkspaceApp>()
            .ok_or_else(|| JsValue::from_str("Polyorama application is unavailable"))?;
        serde_wasm_bindgen::to_value(&app.test_snapshot()).map_err(Into::into)
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for WebHandle {
    fn default() -> Self {
        Self::new()
    }
}
