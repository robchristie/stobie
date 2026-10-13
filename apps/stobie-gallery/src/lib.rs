mod app;
mod appearance;
mod catalogue;
mod stories;

pub use app::{
    Gallery, GalleryApp, GalleryCommand, GalleryConfiguration, GalleryIntent, GallerySnapshot,
    GalleryWidth,
};
pub use catalogue::{STORIES, StoryDefinition, StoryId};
pub use stories::{Destination, IconControl, IconFixtureState, NavigationFixtureState};

pub const APPLICATION_NAME: &str = "Stobie Component Gallery";

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// The browser handle: the framework host plus the Gallery's read-only and
/// test hooks. Automation is enabled explicitly by the page.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WebHandle {
    host: stobie_ui_egui::WebHost<Gallery>,
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
            let app = Gallery::new(creation);
            startup_mark("application_construct_end");
            app
        })
    }

    pub fn destroy(&self) {
        self.host.destroy();
    }

    pub fn enable_automation(&self, instance: &str) -> Result<(), JsValue> {
        self.host.enable_automation(instance)
    }

    pub fn automation_request(&self, request: &str) -> Result<String, JsValue> {
        self.host.automation_request(request)
    }

    pub fn select_story(&self, story: &str) -> Result<(), JsValue> {
        let story = story
            .parse::<StoryId>()
            .map_err(|error| JsValue::from_str(&error))?;
        self.host
            .with_host_mut(|host| host.submit(GalleryIntent::Select(story)))?;
        Ok(())
    }

    pub fn set_configuration(&self, value: JsValue) -> Result<(), JsValue> {
        let configuration: GalleryConfiguration = serde_wasm_bindgen::from_value(value)?;
        self.host
            .with_host_mut(|host| host.submit(GalleryIntent::Configure(configuration)))?;
        Ok(())
    }

    pub fn request_test_repaint(&self) -> Result<(), JsValue> {
        self.host
            .with_host_mut(|host| host.request_repaint("test request"))
    }

    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        let snapshot = self.host.with_host(|host| host.app().snapshot())?;
        serde_wasm_bindgen::to_value(&snapshot).map_err(Into::into)
    }

    pub fn manifest(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&STORIES.as_slice()).map_err(Into::into)
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
