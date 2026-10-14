//! Analytical Workspace Lab application composition.

mod actions;
mod app;
mod panes;
mod thumbnail_cache;
mod ui_geometry;
#[cfg(target_arch = "wasm32")]
mod web_worker;

#[cfg(target_arch = "wasm32")]
use app::TestAction;
pub use app::{AnalyticalWorkspaceApp, Intent, Lab, LabView};

pub const APPLICATION_NAME: &str = "Analytical Workspace Lab";

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// The browser handle: the framework host plus the Lab's test fixtures.
/// Automation is enabled explicitly by the page.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WebHandle {
    host: stobie_ui_egui::WebHost<Lab>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let _ = eframe::WebLogger::init(log::LevelFilter::Info);
        Self {
            host: Default::default(),
        }
    }

    pub fn start(&self, canvas: web_sys::HtmlCanvasElement) -> js_sys::Promise {
        self.host.start(canvas, |creation| {
            startup_mark("application_construct_begin");
            let app = Lab::new(creation);
            startup_mark("application_construct_end");
            app
        })
    }

    pub fn destroy(&self) {
        self.host.destroy();
    }

    /// Apply a test fixture and return the resulting snapshot.
    pub fn test_action(&self, value: JsValue) -> Result<JsValue, JsValue> {
        let action: TestAction = serde_wasm_bindgen::from_value(value)?;
        let snapshot = self.host.with_host_mut(|host| {
            host.app_mut().test_action(action)?;
            host.request_repaint("test fixture");
            Ok::<_, String>(host.app().test_snapshot(host.ui_state()))
        })?;
        let snapshot = snapshot.map_err(|error| JsValue::from_str(&error))?;
        serde_wasm_bindgen::to_value(&snapshot).map_err(Into::into)
    }

    pub fn test_snapshot(&self) -> Result<JsValue, JsValue> {
        let snapshot = self
            .host
            .with_host(|host| host.app().test_snapshot(host.ui_state()))?;
        serde_wasm_bindgen::to_value(&snapshot).map_err(Into::into)
    }

    pub fn enable_automation(&self, instance: &str) -> Result<(), JsValue> {
        self.host.enable_automation(instance)
    }

    /// Fixed, versioned automation dispatcher. Test fixtures remain separate.
    pub fn automation_request(&self, request: &str) -> Result<String, JsValue> {
        self.host.automation_request(request)
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for WebHandle {
    fn default() -> Self {
        Self::new()
    }
}

// Browser startup boundaries are one-shot and add no repaint demand.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = __stobieStartupMark)]
    pub(crate) fn startup_mark(name: &str);
    #[wasm_bindgen(js_name = __stobieStartupFail)]
    pub(crate) fn startup_fail(phase: &str, message: &str);
    #[wasm_bindgen(js_name = __stobieWorkerUrl)]
    pub(crate) fn startup_worker_url() -> String;
    #[wasm_bindgen(js_name = __stobieStartupWorkerMessage)]
    pub(crate) fn startup_worker_message(value: &JsValue) -> bool;
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn startup_frame(useful_content: bool) {
    static WORKSPACE: std::sync::Once = std::sync::Once::new();
    static CONTENT: std::sync::Once = std::sync::Once::new();
    WORKSPACE.call_once(|| startup_mark("workspace_ui_complete"));
    if useful_content {
        CONTENT.call_once(|| startup_mark("first_useful_content"));
    }
}
