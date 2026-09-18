mod app;
mod appearance;
mod catalogue;
mod stories;

pub use app::{GalleryApp, GalleryConfiguration, GallerySnapshot, GalleryWidth};
pub use catalogue::{STORIES, StoryDefinition, StoryId};

pub const APPLICATION_NAME: &str = "Polyorama Component Gallery";

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
                Box::new(|creation| {
                    startup_mark("application_construct_begin");
                    let app = GalleryApp::new(creation);
                    startup_mark("application_construct_end");
                    Ok(Box::new(app))
                }),
            )
            .await
    }

    pub fn destroy(&self) {
        self.runner.destroy();
    }

    pub fn select_story(&self, story: &str) -> Result<(), JsValue> {
        let story = story
            .parse::<StoryId>()
            .map_err(|error| JsValue::from_str(&error))?;
        let mut app = self
            .runner
            .app_mut::<GalleryApp>()
            .ok_or_else(|| JsValue::from_str("Polyorama gallery is unavailable"))?;
        app.select_story(story);
        Ok(())
    }

    pub fn set_configuration(&self, value: JsValue) -> Result<(), JsValue> {
        let configuration: GalleryConfiguration = serde_wasm_bindgen::from_value(value)?;
        let mut app = self
            .runner
            .app_mut::<GalleryApp>()
            .ok_or_else(|| JsValue::from_str("Polyorama gallery is unavailable"))?;
        app.set_configuration(configuration);
        Ok(())
    }

    pub fn request_test_repaint(&self) -> Result<(), JsValue> {
        let app = self
            .runner
            .app_mut::<GalleryApp>()
            .ok_or_else(|| JsValue::from_str("Polyorama gallery is unavailable"))?;
        app.request_test_repaint();
        Ok(())
    }

    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        let app = self
            .runner
            .app_mut::<GalleryApp>()
            .ok_or_else(|| JsValue::from_str("Polyorama gallery is unavailable"))?;
        serde_wasm_bindgen::to_value(&app.snapshot()).map_err(Into::into)
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
    #[wasm_bindgen(js_name = __polyoramaStartupMark)]
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
