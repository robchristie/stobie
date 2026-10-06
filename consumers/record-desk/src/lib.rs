pub mod actions;
pub mod app;
pub mod model;
mod panes;
pub mod store;

#[cfg(target_arch = "wasm32")]
mod browser {
    use crate::app::RecordDeskApp;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct WebHandle {
        runner: eframe::WebRunner,
    }

    #[wasm_bindgen]
    impl WebHandle {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            console_error_panic_hook::set_once();
            Self {
                runner: eframe::WebRunner::new(),
            }
        }
        pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
            self.runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(RecordDeskApp::new(cc)))),
                )
                .await
        }
        /// Read-only current-pass inspection. No command injection or repaint.
        pub fn snapshot(&self) -> Result<String, JsValue> {
            let app = self
                .runner
                .app_mut::<RecordDeskApp>()
                .ok_or_else(|| JsValue::from_str("Record Desk is unavailable"))?;
            serde_json::to_string(&app.snapshot()).map_err(|e| JsValue::from_str(&e.to_string()))
        }
        pub fn enable_automation(&self, instance: &str) -> Result<(), JsValue> {
            let mut app = self
                .runner
                .app_mut::<RecordDeskApp>()
                .ok_or_else(|| JsValue::from_str("Record Desk is unavailable"))?;
            app.enable_inspection(instance)
                .map_err(|error| JsValue::from_str(&error))?;
            Ok(())
        }
        /// Fixed, versioned automation dispatcher. Enabled explicitly by the host.
        pub fn automation_request(&self, request: &str) -> Result<String, JsValue> {
            let mut app = self
                .runner
                .app_mut::<RecordDeskApp>()
                .ok_or_else(|| JsValue::from_str("Record Desk is unavailable"))?;
            app.inspection_request(request)
                .map_err(|error| JsValue::from_str(&error))
        }
        pub fn destroy(&self) {
            self.runner.destroy();
        }
    }
    impl Default for WebHandle {
        fn default() -> Self {
            Self::new()
        }
    }
}
