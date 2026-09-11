mod app;
mod engine;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
use native::Executor;
#[cfg(target_arch = "wasm32")]
mod browser;
pub use app::{Intent, Snapshot, ViewerAction, ViewerApp};
#[cfg(target_arch = "wasm32")]
use browser::Executor;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
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
        Self {
            runner: eframe::WebRunner::new(),
        }
    }
    pub async fn start(
        &self,
        canvas: web_sys::HtmlCanvasElement,
        server: String,
        compressed: usize,
        decoded: usize,
        gpu: usize,
        script: bool,
    ) -> Result<(), JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    Ok(Box::new(ViewerApp::new(
                        cc, server, compressed, decoded, gpu, script, None,
                    )))
                }),
            )
            .await
    }
    pub fn set_script_workload(&self, json: &str) -> Result<(), JsValue> {
        let mut app = self
            .runner
            .app_mut::<ViewerApp>()
            .ok_or_else(|| JsValue::from_str("viewer unavailable"))?;
        app.set_script_workload(json)
            .map_err(|e| JsValue::from_str(&e))
    }
    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        let app = self
            .runner
            .app_mut::<ViewerApp>()
            .ok_or_else(|| JsValue::from_str("viewer unavailable"))?;
        serde_wasm_bindgen::to_value(&app.snapshot()).map_err(Into::into)
    }
    pub fn stages(&self) -> Result<JsValue, JsValue> {
        let app = self
            .runner
            .app_mut::<ViewerApp>()
            .ok_or_else(|| JsValue::from_str("viewer unavailable"))?;
        serde_wasm_bindgen::to_value(app.script_stages()).map_err(Into::into)
    }
    pub fn intent(&self, value: JsValue) -> Result<(), JsValue> {
        let intent = serde_wasm_bindgen::from_value(value)?;
        let mut app = self
            .runner
            .app_mut::<ViewerApp>()
            .ok_or_else(|| JsValue::from_str("viewer unavailable"))?;
        app.intent(intent);
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
impl Default for WebHandle {
    fn default() -> Self {
        Self::new()
    }
}
