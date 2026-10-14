//! The browser half of the hidden ARIA layer: an egui plugin that takes each
//! completed pass's AccessKit update before eframe's web runner discards it,
//! and applies the projected difference to DOM elements over the canvas.
//!
//! DOM objects are not `Send`, but egui plugins must be. The plugin therefore
//! holds only a key into a thread-local registry; WASM runs the UI on one
//! thread.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, HtmlCanvasElement, HtmlElement};

use crate::aria::{self, AriaTree, DomOp};

/// Keeps the layer invisible and out of hit testing without removing it from
/// the accessibility tree: no `display: none`, `visibility: hidden` or
/// `aria-hidden`.
const CONTAINER_STYLE: &str = "position:fixed;margin:0;padding:0;border:0;overflow:hidden;opacity:0;pointer-events:none;contain:strict;";
const ELEMENT_STYLE: &str = "position:absolute;margin:0;padding:0;";

thread_local! {
    static LAYERS: RefCell<BTreeMap<u32, Layer>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_KEY: Cell<u32> = const { Cell::new(0) };
}

/// Mirror `context`'s accessibility tree into a layer over `canvas`, named
/// `name` for assistive technology. Returns the layer's key for [`remove`]. A
/// failure is logged and leaves the application running without the layer.
pub(super) fn install(
    context: &egui::Context,
    canvas: &HtmlCanvasElement,
    name: &str,
) -> Option<u32> {
    let key = NEXT_KEY.with(|next| {
        let key = next.get();
        next.set(key + 1);
        key
    });
    match Layer::new(key, canvas, name) {
        Ok(layer) => {
            LAYERS.with(|layers| layers.borrow_mut().insert(key, layer));
            context.enable_accesskit();
            context.add_plugin(AriaPlugin { key });
            Some(key)
        }
        Err(error) => {
            log_error(&error);
            None
        }
    }
}

/// Remove the layer `key` when its runner stops.
pub(super) fn remove(key: u32) {
    if let Some(layer) = LAYERS.with(|layers| layers.borrow_mut().remove(&key)) {
        layer.remove();
    }
}

struct AriaPlugin {
    key: u32,
}

impl egui::Plugin for AriaPlugin {
    fn debug_name(&self) -> &'static str {
        "stobie.aria"
    }

    fn output_hook(&mut self, context: &egui::Context, output: &mut egui::FullOutput) {
        let platform = &mut output.platform_output;
        // egui runs another pass after this one and discards this pass's
        // output; mirror only the pass that is shown.
        let max_passes = context.options(|options| options.max_passes.get());
        if platform.requested_discard() && platform.num_completed_passes < max_passes {
            return;
        }
        // eframe's web runner discards the update, so take it.
        let Some(update) = platform.accesskit_update.take() else {
            return;
        };
        let scale = context.zoom_factor();
        LAYERS.with(|layers| {
            if let Some(layer) = layers.borrow_mut().get_mut(&self.key)
                && let Err(error) = layer.apply(&update, scale)
            {
                log_error(&error);
            }
        });
    }
}

struct Layer {
    document: Document,
    canvas: HtmlCanvasElement,
    container: HtmlElement,
    container_geometry: String,
    elements: HashMap<String, HtmlElement>,
    tree: AriaTree,
}

impl Layer {
    fn new(key: u32, canvas: &HtmlCanvasElement, name: &str) -> Result<Self, JsValue> {
        let document = canvas
            .owner_document()
            .ok_or_else(|| JsValue::from_str("the canvas has no document"))?;
        let container: HtmlElement = document.create_element("div")?.unchecked_into();
        let container_id = format!("{}layer-{key}", aria::ARIA_ID_PREFIX);
        container.set_id(&container_id);
        container.set_attribute("style", CONTAINER_STYLE)?;
        canvas.after_with_node_1(&container)?;
        canvas.set_attribute("role", "application")?;
        if !canvas.has_attribute("aria-label") {
            canvas.set_attribute("aria-label", name)?;
        }
        canvas.set_attribute("aria-owns", &container_id)?;
        // eframe's text agent is a hidden input beside the canvas that holds
        // keyboard focus while egui edits text. Name it for assistive technology.
        if let Some(parent) = canvas.parent_element()
            && let Some(agent) = parent.query_selector(":scope > input")?
            && !agent.has_attribute("aria-label")
        {
            agent.set_attribute("aria-label", "Text entry")?;
        }
        Ok(Self {
            document,
            canvas: canvas.clone(),
            container,
            container_geometry: String::new(),
            elements: HashMap::new(),
            tree: AriaTree::default(),
        })
    }

    fn remove(self) {
        self.container.remove();
        let _ = self.canvas.remove_attribute("aria-owns");
        let _ = self.canvas.remove_attribute("aria-activedescendant");
    }

    fn apply(&mut self, update: &egui::accesskit::TreeUpdate, scale: f32) -> Result<(), JsValue> {
        // Read before writing, so the read never forces a layout of our writes.
        let bounds = self.canvas.get_bounding_client_rect();
        let geometry = format!(
            "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px",
            bounds.left(),
            bounds.top(),
            bounds.width(),
            bounds.height()
        );
        if geometry != self.container_geometry {
            self.container
                .set_attribute("style", &format!("{CONTAINER_STYLE}{geometry}"))?;
            self.container_geometry = geometry;
        }
        let next = aria::project(update, scale);
        for operation in aria::diff(&self.tree, &next) {
            self.perform(operation)?;
        }
        self.tree = next;
        Ok(())
    }

    fn element(&self, id: &str) -> Result<&HtmlElement, JsValue> {
        self.elements
            .get(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing layer element {id}")))
    }

    fn perform(&mut self, operation: DomOp) -> Result<(), JsValue> {
        match operation {
            DomOp::Create { id } => {
                let element: HtmlElement = self.document.create_element("div")?.unchecked_into();
                element.set_id(&id);
                self.elements.insert(id, element);
            }
            DomOp::SetAttribute { id, name, value } => {
                self.element(&id)?.set_attribute(name, &value)?;
            }
            DomOp::RemoveAttribute { id, name } => {
                self.element(&id)?.remove_attribute(name)?;
            }
            DomOp::SetText { id, text } => {
                self.element(&id)?.set_text_content(text.as_deref());
            }
            DomOp::SetStyle { id, style } => {
                self.element(&id)?
                    .set_attribute("style", &format!("{ELEMENT_STYLE}{style}"))?;
            }
            DomOp::Place { id, parent, before } => {
                let element = self.element(&id)?;
                let parent = match parent {
                    Some(parent) => self.element(&parent)?,
                    None => &self.container,
                };
                let before = before.map(|before| self.element(&before)).transpose()?;
                parent.insert_before(element, before.map(|before| before.as_ref()))?;
            }
            DomOp::Remove { id } => {
                if let Some(element) = self.elements.remove(&id) {
                    element.remove();
                }
            }
            DomOp::Active { id } => match id {
                Some(id) => self.canvas.set_attribute("aria-activedescendant", &id)?,
                None => self.canvas.remove_attribute("aria-activedescendant")?,
            },
        }
        Ok(())
    }
}

fn log_error(error: &JsValue) {
    web_sys::console::error_2(&JsValue::from_str("Stobie ARIA layer:"), error);
}
