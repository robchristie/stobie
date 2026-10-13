pub mod actions;
pub mod app;
pub mod model;
mod panes;
pub mod store;

#[cfg(target_arch = "wasm32")]
mod browser {
    use crate::app::RecordDesk;
    use wasm_bindgen::prelude::*;

    stobie_ui_egui::web_handle!(RecordDesk, RecordDesk::new);

    #[wasm_bindgen]
    impl WebHandle {
        /// Read-only current-pass inspection. No command injection or repaint.
        pub fn snapshot(&self) -> Result<String, JsValue> {
            let snapshot = self.host.with_host(|host| host.app().snapshot())?;
            serde_json::to_string(&snapshot).map_err(|e| JsValue::from_str(&e.to_string()))
        }
    }
}
