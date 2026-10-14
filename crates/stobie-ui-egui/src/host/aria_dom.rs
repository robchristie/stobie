//! The browser half of the hidden ARIA layer: an egui plugin that takes each
//! completed pass's AccessKit update before eframe's web runner discards it,
//! and applies the projected difference to DOM elements over the canvas.
//!
//! A `click` on a layer element, such as a screen reader's default action or
//! `element.click()`, becomes an AccessKit click request for its node, and DOM
//! focus moving onto a layer element becomes a focus request. Both reach egui
//! as input, the same route native assistive technology uses, so a widget acts
//! only if it is enabled and senses clicks, and an action still rechecks its
//! availability. DOM focus returns to the canvas, which keeps keyboard and IME
//! input working; the canvas's `aria-activedescendant` names egui's focus.
//!
//! A layer text field keeps DOM focus instead, so assistive technology and
//! browser agents (`locator.fill()`, typing) can edit it. Each `input` event
//! becomes an AccessKit `SetValue` request with the field's whole text, which
//! the host applies through egui's own text editing. A value egui does not take
//! is restored from the mirror after the pass. Tab and Escape return DOM focus
//! to the canvas and reach egui as the same keys; Cmd or Ctrl chords other
//! than the browser's own editing keys reach egui too, so application
//! shortcuts keep working while a field is edited in place.
//!
//! DOM objects are not `Send`, but egui plugins must be. The plugin therefore
//! holds only a key into a thread-local registry; WASM runs the UI on one
//! thread.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use egui::accesskit::{Action, ActionData, ActionRequest, TreeId};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, Event, HtmlCanvasElement, HtmlElement, KeyboardEvent};

use crate::aria::{self, AriaTree, DomOp};

/// Keeps the layer invisible and out of hit testing without removing it from
/// the accessibility tree: no `display: none`, `visibility: hidden` or
/// `aria-hidden`.
const CONTAINER_STYLE: &str = "position:fixed;margin:0;padding:0;border:0;overflow:hidden;opacity:0;pointer-events:none;contain:strict;";
const ELEMENT_STYLE: &str = "position:absolute;margin:0;padding:0;";

/// A DOM event listener the layer owns.
type Listener = Closure<dyn FnMut(Event)>;

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
    match Layer::new(key, context, canvas, name) {
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

    fn input_hook(&mut self, _context: &egui::Context, input: &mut egui::RawInput) {
        LAYERS.with(|layers| {
            if let Some(layer) = layers.borrow_mut().get_mut(&self.key) {
                input.events.extend(
                    layer
                        .requests
                        .drain(..)
                        .map(egui::Event::AccessKitActionRequest),
                );
                input.events.append(&mut layer.keys);
                // This pass applies the edits; check their fields after it.
                layer.checking = std::mem::take(&mut layer.edited);
            }
        });
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
    context: egui::Context,
    document: Document,
    canvas: HtmlCanvasElement,
    /// The canvas's role before the layer, and whether the layer named it.
    canvas_role: Option<String>,
    canvas_named: bool,
    container: HtmlElement,
    container_geometry: String,
    /// eframe's text agent, whether the layer gave it its ID, and the name
    /// last given to it.
    text_agent: Option<Element>,
    text_agent_identified: bool,
    text_agent_name: String,
    elements: HashMap<String, HtmlElement>,
    tree: AriaTree,
    /// Action requests from the layer, for egui's next input.
    requests: Vec<ActionRequest>,
    /// Keys a layer text field hands back to egui, for its next input.
    keys: Vec<egui::Event>,
    /// Text fields edited since egui last took input, and those whose edits
    /// the current pass applies, checked against the mirror after it.
    edited: Vec<String>,
    checking: Vec<String>,
    /// The layer's event listeners, kept alive while it is installed.
    listeners: Vec<(&'static str, Listener)>,
}

impl Layer {
    fn new(
        key: u32,
        context: &egui::Context,
        canvas: &HtmlCanvasElement,
        name: &str,
    ) -> Result<Self, JsValue> {
        let document = canvas
            .owner_document()
            .ok_or_else(|| JsValue::from_str("the canvas has no document"))?;
        let container: HtmlElement = document.create_element("div")?.unchecked_into();
        let container_id = format!("{}layer-{key}", aria::ARIA_ID_PREFIX);
        container.set_id(&container_id);
        container.set_attribute("style", CONTAINER_STYLE)?;
        canvas.after_with_node_1(&container)?;
        let canvas_role = canvas.get_attribute("role");
        canvas.set_attribute("role", "application")?;
        let canvas_named = !canvas.has_attribute("aria-label");
        if canvas_named {
            canvas.set_attribute("aria-label", name)?;
        }
        // eframe's text agent is a hidden input beside the canvas that holds
        // keyboard focus while egui edits text. The application owns it, and
        // it is named after the focused text field, so assistive technology
        // announces what is being edited.
        let text_agent = canvas
            .parent_element()
            .map(|parent| parent.query_selector(":scope > input"))
            .transpose()?
            .flatten()
            .filter(|agent| !agent.has_attribute("aria-label"));
        let mut owned = container_id.clone();
        let mut text_agent_identified = false;
        if let Some(agent) = &text_agent {
            if agent.id().is_empty() {
                agent.set_id(&format!("{}text-agent-{key}", aria::ARIA_ID_PREFIX));
                text_agent_identified = true;
            }
            owned = format!("{owned} {}", agent.id());
        }
        canvas.set_attribute("aria-owns", &owned)?;
        let mut listeners = Vec::new();
        for (event, action) in [("click", Action::Click), ("focusin", Action::Focus)] {
            let listener = Listener::new(move |event: Event| {
                request(key, &event, action);
            });
            container.add_event_listener_with_callback(event, listener.as_ref().unchecked_ref())?;
            listeners.push((event, listener));
        }
        // An edit made in a layer field sets the egui field's text.
        let listener = Listener::new(move |event: Event| edit(key, &event));
        container.add_event_listener_with_callback("input", listener.as_ref().unchecked_ref())?;
        listeners.push(("input", listener));
        let listener = Listener::new(move |event: Event| hand_back(key, &event));
        container.add_event_listener_with_callback("keydown", listener.as_ref().unchecked_ref())?;
        listeners.push(("keydown", listener));
        let mut layer = Self {
            context: context.clone(),
            document,
            canvas: canvas.clone(),
            canvas_role,
            canvas_named,
            container,
            container_geometry: String::new(),
            text_agent,
            text_agent_identified,
            text_agent_name: String::new(),
            elements: HashMap::new(),
            tree: AriaTree::default(),
            requests: Vec::new(),
            keys: Vec::new(),
            edited: Vec::new(),
            checking: Vec::new(),
            listeners,
        };
        layer.name_text_agent()?;
        Ok(layer)
    }

    fn remove(self) {
        for (event, listener) in &self.listeners {
            let _ = self
                .container
                .remove_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
        }
        self.container.remove();
        // Restore what the layer changed outside its container.
        let _ = self.canvas.remove_attribute("aria-owns");
        let _ = self.canvas.remove_attribute("aria-activedescendant");
        let _ = match &self.canvas_role {
            Some(role) => self.canvas.set_attribute("role", role),
            None => self.canvas.remove_attribute("role"),
        };
        if self.canvas_named {
            let _ = self.canvas.remove_attribute("aria-label");
        }
        if let Some(agent) = &self.text_agent {
            let _ = agent.remove_attribute("aria-label");
            if self.text_agent_identified {
                agent.remove_attribute("id").ok();
            }
        }
    }

    /// Name eframe's text agent after the focused text field, or generically.
    /// While a layer text field itself holds DOM focus, the agent keeps its
    /// generic name, so one field's name finds one textbox.
    fn name_text_agent(&mut self) -> Result<(), JsValue> {
        let Some(agent) = &self.text_agent else {
            return Ok(());
        };
        let in_layer_field = self.document.active_element().is_some_and(|active| {
            self.container.contains(Some(&active))
                && matches!(active.tag_name().as_str(), "INPUT" | "TEXTAREA")
        });
        let name = self
            .tree
            .active
            .as_ref()
            .filter(|_| !in_layer_field)
            .and_then(|id| self.tree.elements.get(id))
            .filter(|element| element.tag != "div")
            .and_then(|element| element.attributes.get("aria-label"))
            .map_or("Text entry", String::as_str);
        if name != self.text_agent_name {
            agent.set_attribute("aria-label", name)?;
            self.text_agent_name = name.to_owned();
        }
        Ok(())
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
            // A field edited again since this pass took its input already
            // holds newer text than this pass mirrors.
            if let DomOp::SetText { id, .. } = &operation
                && self.edited.contains(id)
            {
                continue;
            }
            self.perform(operation)?;
        }
        self.tree = next;
        // Restore any field whose edit egui did not take as typed.
        for id in std::mem::take(&mut self.checking) {
            if self.edited.contains(&id) {
                continue;
            }
            if let (Some(element), Some(mirrored)) =
                (self.elements.get(&id), self.tree.elements.get(&id))
                && is_text_field(element)
                && field_value(element) != mirrored.text.clone().unwrap_or_default()
            {
                set_text(element, mirrored.text.as_deref())?;
            }
        }
        self.name_text_agent()
    }

    fn element(&self, id: &str) -> Result<&HtmlElement, JsValue> {
        self.elements
            .get(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing layer element {id}")))
    }

    fn perform(&mut self, operation: DomOp) -> Result<(), JsValue> {
        match operation {
            DomOp::Create { id, tag } => {
                let element: HtmlElement = self.document.create_element(tag)?.unchecked_into();
                element.set_id(&id);
                self.elements.insert(id, element);
            }
            DomOp::SetAttribute { id, name, value } => {
                self.element(&id)?.set_attribute(name, &value)?;
            }
            DomOp::RemoveAttribute { id, name } => {
                self.element(&id)?.remove_attribute(name)?;
            }
            DomOp::SetText { id, text } => set_text(self.element(&id)?, text.as_deref())?,
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

/// Turn a click or focus on a layer element into an AccessKit request for its
/// node, handled on egui's next pass. Focus returns to the canvas, which owns
/// keyboard input; egui's own focus is then reported through
/// `aria-activedescendant`.
fn request(key: u32, event: &Event, action: Action) {
    let target = event
        .target()
        .and_then(|target| target.dyn_into::<Element>().ok());
    LAYERS.with(|layers| {
        let Ok(mut layers) = layers.try_borrow_mut() else {
            return;
        };
        let Some(layer) = layers.get_mut(&key) else {
            return;
        };
        let element = std::iter::successors(target, |element| element.parent_element())
            .take_while(|element| element != layer.container.unchecked_ref::<Element>())
            .find_map(|element| layer.tree.elements.get(&element.id()));
        let Some(element) = element else {
            return;
        };
        let text_field = element.tag != "div";
        layer.requests.push(ActionRequest {
            action,
            target_node: element.node,
            target_tree: TreeId::ROOT,
            data: None,
        });
        // A text field keeps focus, so it can be edited in place.
        if action == Action::Focus && !text_field {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = layer.canvas.focus_with_options(&options);
        }
        layer.context.request_repaint();
    });
}

/// Set an element's text: a text field's value, or another element's content.
fn set_text(element: &HtmlElement, text: Option<&str>) -> Result<(), JsValue> {
    if matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA") {
        js_sys::Reflect::set(
            element,
            &JsValue::from_str("value"),
            &JsValue::from_str(text.unwrap_or_default()),
        )?;
    } else {
        element.set_text_content(text);
    }
    Ok(())
}

fn is_text_field(element: &HtmlElement) -> bool {
    matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA")
}

fn field_value(element: &HtmlElement) -> String {
    js_sys::Reflect::get(element, &JsValue::from_str("value"))
        .ok()
        .and_then(|value| value.as_string())
        .unwrap_or_default()
}

/// Send a layer text field's edited text to egui as a `SetValue` request for
/// its node; the host applies it as typed input on the next pass.
fn edit(key: u32, event: &Event) {
    let Some(target) = event
        .target()
        .and_then(|target| target.dyn_into::<HtmlElement>().ok())
        .filter(is_text_field)
    else {
        return;
    };
    LAYERS.with(|layers| {
        let Ok(mut layers) = layers.try_borrow_mut() else {
            return;
        };
        let Some(layer) = layers.get_mut(&key) else {
            return;
        };
        let id = target.id();
        let Some(element) = layer.tree.elements.get(&id) else {
            return;
        };
        layer.requests.push(ActionRequest {
            action: Action::SetValue,
            target_node: element.node,
            target_tree: TreeId::ROOT,
            data: Some(ActionData::Value(field_value(&target).into())),
        });
        if !layer.edited.contains(&id) {
            layer.edited.push(id);
        }
        layer.context.request_repaint();
    });
}

/// Tab and Escape leave a layer text field: DOM focus returns to the canvas
/// and egui receives the key, so focus moves or editing ends as in egui. A
/// Cmd or Ctrl chord, other than the field's own editing keys, reaches egui
/// as an application shortcut, and focus stays.
fn hand_back(key: u32, event: &Event) {
    let Some(keyboard) = event.dyn_ref::<KeyboardEvent>() else {
        return;
    };
    let in_field = event
        .target()
        .and_then(|target| target.dyn_into::<HtmlElement>().ok())
        .is_some_and(|target| is_text_field(&target));
    if !in_field {
        return;
    }
    let name = keyboard.key();
    let chord = keyboard.ctrl_key() || keyboard.meta_key();
    let (pressed, leave) = match name.as_str() {
        "Tab" | "Escape" if !chord && !keyboard.alt_key() => {
            (egui::Key::from_name(&name).expect("named key"), true)
        }
        // Selection, clipboard, undo and caret movement edit the field.
        "a" | "A" | "c" | "C" | "v" | "V" | "x" | "X" | "z" | "Z" | "y" | "Y" | "ArrowLeft"
        | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "Backspace" | "Delete" => {
            return;
        }
        _ if chord => match egui::Key::from_name(&name) {
            Some(key) => (key, false),
            None => return,
        },
        _ => return,
    };
    LAYERS.with(|layers| {
        let Ok(mut layers) = layers.try_borrow_mut() else {
            return;
        };
        let Some(layer) = layers.get_mut(&key) else {
            return;
        };
        event.prevent_default();
        let modifiers = egui::Modifiers {
            alt: keyboard.alt_key(),
            ctrl: keyboard.ctrl_key(),
            shift: keyboard.shift_key(),
            mac_cmd: keyboard.meta_key(),
            command: chord,
        };
        for pressed_state in [true, false] {
            layer.keys.push(egui::Event::Key {
                key: pressed,
                physical_key: None,
                pressed: pressed_state,
                repeat: false,
                modifiers,
            });
        }
        if leave {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            let _ = layer.canvas.focus_with_options(&options);
        }
        layer.context.request_repaint();
    });
}

fn log_error(error: &JsValue) {
    web_sys::console::error_2(&JsValue::from_str("Stobie ARIA layer:"), error);
}
