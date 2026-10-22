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
//! An ordinary pointer click, such as Playwright's default `click()` or a
//! strict Lantern target, hit-tests the centre of an element. Each enabled
//! clickable element therefore holds a one-pixel hit target at its centre,
//! the only part of the layer that takes pointer input. Every pointer, mouse,
//! touch, wheel and context-menu event on it is dispatched again to the canvas
//! at the same coordinates, and moving onto it does not tell the canvas the
//! pointer left. egui sees the same input as without the layer, so a click
//! there is a physical click, and hover, drags and wheel starting there behave
//! as before, and so do file drops. While the copy is dispatched, hit targets
//! take no pointer input, so eframe's own check that the canvas is under the
//! pointer passes. Only a control whose centre is visible within its list,
//! scroll area or pane has a hit target. Over that pixel the cursor keeps the
//! default shape; Safari's trackpad gesture events are not forwarded; and a
//! touch that starts there and outlives its control's element stops reaching
//! the canvas.
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
//! iOS raises the keyboard only for a focus made while a touch is being
//! dispatched, and eframe focuses its text agent in a later frame. A tap that
//! ends on an editable text field therefore focuses the text agent at once,
//! inside the gesture. A tap on a native field (see `native_text`) instead
//! moves the field's layer element out of the transparent container into a
//! visible host over the canvas, styles it as the field and focuses it, so
//! the platform owns the caret, selection and text input while egui keeps
//! the text. It returns to the layer when it loses DOM focus or egui's focus
//! leaves its field.
//!
//! The layer styles its elements through the CSSOM and its rules through a
//! constructed stylesheet that the document adopts, never through `style`
//! attributes or `<style>` text, so it runs under a Content-Security-Policy
//! whose `style-src` allows only the page's own styles.
//!
//! DOM objects are not `Send`, but egui plugins must be. The plugin therefore
//! holds only a key into a thread-local registry; WASM runs the UI on one
//! thread.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};

use egui::accesskit::{Action, ActionData, ActionRequest, NodeId, TreeId};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Document, Element, Event, EventTarget, HtmlCanvasElement, HtmlElement, KeyboardEvent,
    TouchEvent,
};

use crate::aria::{self, AriaTree, DomOp};
use crate::native_text::{self, NativeTextRegistration};

/// Keeps the layer invisible and out of hit testing without removing it from
/// the accessibility tree: no `display: none`, `visibility: hidden` or
/// `aria-hidden`.
const CONTAINER_STYLE: &str = "position:fixed;margin:0;padding:0;border:0;overflow:hidden;opacity:0;pointer-events:none;contain:strict;user-select:none;";
const ELEMENT_STYLE: &str = "position:absolute;margin:0;padding:0;";
/// A hit target: one CSS pixel centred on its element. A stylesheet rule
/// gives it pointer input (see [`HIT_RULE`]).
const HIT_STYLE: &str = "position:absolute;left:calc(50% - 0.5px);top:calc(50% - 0.5px);width:1px;height:1px;margin:0;padding:0;touch-action:none;";
/// The rule that lets hit targets take pointer input. The layer turns it off
/// while it dispatches a forwarded event, so eframe's own hit test, which
/// accepts a move only where the canvas is the topmost element, finds the
/// canvas. Changing a rule writes nothing to the DOM.
const HIT_RULE: &str = "[data-stobie-hit-target]{pointer-events:auto}";
/// Marks a hit target element.
const HIT_TARGET: &str = "data-stobie-hit-target";
/// Input on a hit target that the canvas receives instead. Enter, leave, over
/// and out stay with the hit target: the pointer has not left the canvas.
const FORWARDED: [&str; 19] = [
    "pointerdown",
    "pointermove",
    "pointerup",
    "pointercancel",
    "mousedown",
    "mousemove",
    "mouseup",
    "click",
    "auxclick",
    "dblclick",
    "contextmenu",
    "wheel",
    "touchstart",
    "touchmove",
    "touchend",
    "touchcancel",
    "dragenter",
    "dragover",
    "drop",
];
/// Canvas notifications that the pointer moved onto a hit target, which the
/// canvas must not see as the pointer leaving it.
const SUPPRESSED: [&str; 5] = [
    "pointerout",
    "pointerleave",
    "mouseout",
    "mouseleave",
    "dragleave",
];

/// The visible host of an active native field: over the canvas, outside the
/// layer's transparent container, taking no pointer input itself.
const NATIVE_HOST_STYLE: &str =
    "position:fixed;margin:0;padding:0;border:0;overflow:hidden;pointer-events:none;";
/// Stands in for an active native field's element in the layer.
const NATIVE_PLACEHOLDER_STYLE: &str = "display:none;";
/// Events on which the page may have moved the canvas without an egui pass.
const NATIVE_VIEWPORT_EVENTS: [&str; 3] = ["resize", "scroll", "orientationchange"];

/// A DOM event listener the layer owns.
type Listener = Closure<dyn FnMut(Event)>;
/// A handler for one event on a layer element, given the layer's key.
type Handler = fn(u32, &Event);

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
                // eframe counts the application focused only while the
                // canvas or its text agent has DOM focus. A focused native
                // field is the application's too, and egui reports a field
                // focused only in a focused viewport.
                if layer.native_focused() {
                    input.focused = true;
                }
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
        let native = native_text::registered_in_completed_pass(context);
        LAYERS.with(|layers| {
            if let Some(layer) = layers.borrow_mut().get_mut(&self.key) {
                if let Err(error) = layer.apply(&update, scale, &native) {
                    log_error(&error);
                }
                layer.cover_agent_tap(&update, &mut platform.ime);
            }
        });
    }
}

struct Layer {
    key: u32,
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
    /// Capture listeners on the document, which see the canvas's own events.
    document_listeners: Vec<(&'static str, Listener)>,
    /// egui's zoom factor at the last pass: CSS pixels per point.
    scale: f64,
    /// The touch that may become a tap: its identifier, where it began in
    /// client coordinates and when, in milliseconds.
    touch: Option<(i32, f64, f64, f64)>,
    /// Whether a tap's `touchend` focused a field, so the same tap's later
    /// `pointerup` must not reach eframe (see [`pointer`]).
    tap_focused: bool,
    /// The ordinary text field a tap gave eframe's text agent to, and how
    /// many more shown passes may cover for egui's IME output.
    agent_tap: Option<(NodeId, u8)>,
    /// The native fields of the last pass, by element ID.
    native_fields: HashMap<String, NativeTextRegistration>,
    /// The rule that lets hit targets take pointer input.
    hit_sheet: Sheet,
    /// The visible host of native fields and its stylesheet, made on first use.
    native_host: Option<(HtmlElement, Sheet)>,
    native: Option<ActiveNative>,
    /// Whether Stobie's face is registered for native fields.
    native_font: bool,
}

/// The native field that is active.
struct ActiveNative {
    id: String,
    /// Holds the element's place in the layer.
    placeholder: Element,
    /// The layer element that owns it for assistive technology, and its
    /// `aria-owns` before.
    owner: Element,
    owns: Option<String>,
    /// Whether egui has reported focus on the field since activation.
    confirmed: bool,
    style: String,
    rules: String,
    /// Window and visual viewport listeners that keep the host in place.
    listeners: Vec<(EventTarget, &'static str, Listener)>,
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
        set_style(&container, CONTAINER_STYLE);
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
        // Hit targets forward their input to the canvas, ahead of the
        // layer's own listeners. Wheel and touch listeners are not passive,
        // so the canvas's `preventDefault` reaches the original event.
        let options = web_sys::AddEventListenerOptions::new();
        options.set_capture(true);
        options.set_passive(false);
        let hit_sheet = Sheet::new(&document, &container, HIT_RULE)?;
        let rule = hit_sheet.first_rule_style()?;
        for event_name in FORWARDED {
            let target = canvas.clone();
            let rule = rule.clone();
            let listener = Listener::new(move |event: Event| forward(&target, &rule, &event));
            container.add_event_listener_with_callback_and_add_event_listener_options(
                event_name,
                listener.as_ref().unchecked_ref(),
                &options,
            )?;
            listeners.push((event_name, listener));
        }
        let mut document_listeners = Vec::new();
        for event_name in SUPPRESSED {
            let target = canvas.clone();
            let listener = Listener::new(move |event: Event| keep_pointer(&target, &event));
            document.add_event_listener_with_callback_and_bool(
                event_name,
                listener.as_ref().unchecked_ref(),
                true,
            )?;
            document_listeners.push((event_name, listener));
        }
        // Enter or Space on the canvas presses a focused copy control on the
        // next pass; the control writes inside this key press instead.
        let target = canvas.clone();
        let keys = context.clone();
        let listener = Listener::new(move |event: Event| {
            let on_canvas = event
                .target()
                .is_some_and(|node| JsValue::from(node) == JsValue::from(target.clone()));
            if let (true, Some(keyboard)) = (on_canvas, event.dyn_ref::<KeyboardEvent>()) {
                crate::clipboard::key_pressed(&keys, &keyboard.key());
            }
        });
        document.add_event_listener_with_callback_and_bool(
            "keydown",
            listener.as_ref().unchecked_ref(),
            true,
        )?;
        document_listeners.push(("keydown", listener));
        // Taps that end on a text field focus it inside the gesture; the
        // capture phase runs before eframe's own `touchend` on the document.
        for (event_name, handler) in [
            ("touchstart", touch as Handler),
            ("touchend", touch),
            ("touchcancel", touch),
            ("pointerdown", pointer),
            ("pointerup", pointer),
        ] {
            let listener = Listener::new(move |event: Event| handler(key, &event));
            document.add_event_listener_with_callback_and_bool(
                event_name,
                listener.as_ref().unchecked_ref(),
                true,
            )?;
            document_listeners.push((event_name, listener));
        }
        let mut layer = Self {
            key,
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
            document_listeners,
            scale: 1.0,
            touch: None,
            tap_focused: false,
            agent_tap: None,
            native_fields: HashMap::new(),
            hit_sheet,
            native_host: None,
            native: None,
            native_font: false,
        };
        layer.name_text_agent()?;
        Ok(layer)
    }

    fn remove(mut self) {
        let _ = self.deactivate_native(false);
        if let Some((host, rules)) = &self.native_host {
            host.remove();
            rules.remove(&self.document);
        }
        self.hit_sheet.remove(&self.document);
        for (event, listener) in &self.listeners {
            let _ = self
                .container
                .remove_event_listener_with_callback(event, listener.as_ref().unchecked_ref());
            let _ = self.container.remove_event_listener_with_callback_and_bool(
                event,
                listener.as_ref().unchecked_ref(),
                true,
            );
        }
        for (event, listener) in &self.document_listeners {
            let _ = self.document.remove_event_listener_with_callback_and_bool(
                event,
                listener.as_ref().unchecked_ref(),
                true,
            );
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
            (self.container.contains(Some(&active))
                || self
                    .native_host
                    .as_ref()
                    .is_some_and(|(host, _)| host.contains(Some(&active))))
                && matches!(active.tag_name().as_str(), "INPUT" | "TEXTAREA")
        });
        // Named as the field is: by its visible label where it has one.
        let name = self
            .tree
            .active
            .as_ref()
            .filter(|_| !in_layer_field)
            .filter(|id| {
                self.tree
                    .elements
                    .get(*id)
                    .is_some_and(|element| element.tag != "div")
            })
            .and_then(|id| self.tree.accessible_name(id))
            .unwrap_or_else(|| "Text entry".to_owned());
        if name != self.text_agent_name {
            agent.set_attribute("aria-label", &name)?;
            self.text_agent_name = name;
        }
        Ok(())
    }

    fn apply(
        &mut self,
        update: &egui::accesskit::TreeUpdate,
        scale: f32,
        native: &[NativeTextRegistration],
    ) -> Result<(), JsValue> {
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
            set_style(&self.container, &format!("{CONTAINER_STYLE}{geometry}"));
            if let Some((host, _)) = &self.native_host {
                set_style(host, &format!("{NATIVE_HOST_STYLE}{geometry}"));
            }
            self.container_geometry = geometry;
        }
        // The canvas, in egui points, bounds what is shown.
        let scale_points = f64::from(scale.max(f32::EPSILON));
        self.scale = scale_points;
        let viewport = (
            0.0,
            0.0,
            bounds.width() / scale_points,
            bounds.height() / scale_points,
        );
        let next = aria::project_native(update, scale, Some(viewport), native);
        if !next.native.is_empty() {
            self.register_native_font();
        }
        self.native_fields = next
            .native
            .keys()
            .filter_map(|id| {
                let node = next.elements.get(id)?.node;
                let field = native
                    .iter()
                    .find(|field| field.id.accesskit_id() == node)?;
                Some((id.clone(), field.clone()))
            })
            .collect();
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
        self.keep_native()?;
        self.name_text_agent()
    }

    fn element(&self, id: &str) -> Result<&HtmlElement, JsValue> {
        self.elements
            .get(id)
            .ok_or_else(|| JsValue::from_str(&format!("missing layer element {id}")))
    }

    /// What stands at `id`'s place in the layer: its element, or the
    /// placeholder of an active native field.
    fn placed(&self, id: &str) -> Result<&web_sys::Node, JsValue> {
        match &self.native {
            Some(active) if active.id == id => Ok(&active.placeholder),
            _ => Ok(self.element(id)?),
        }
    }

    /// Whether the active native field has DOM focus.
    fn native_focused(&self) -> bool {
        self.native
            .as_ref()
            .and_then(|active| self.elements.get(&active.id))
            .is_some_and(|element| {
                self.document.active_element().as_ref() == Some(element.unchecked_ref::<Element>())
            })
    }

    fn is_active_native(&self, id: &str) -> bool {
        self.native.as_ref().is_some_and(|active| active.id == id)
    }

    fn perform(&mut self, operation: DomOp) -> Result<(), JsValue> {
        match operation {
            DomOp::Create { id, tag } => {
                let element: HtmlElement = self.document.create_element(tag)?.unchecked_into();
                element.set_id(&id);
                self.elements.insert(id, element);
            }
            DomOp::SetAttribute { id, name, value } => {
                let element = self.element(&id)?;
                element.set_attribute(name, &value)?;
                if name == aria::HIT_ATTRIBUTE {
                    self.place_hit_target(element)?;
                }
            }
            DomOp::RemoveAttribute { id, name } => {
                let element = self.element(&id)?;
                element.remove_attribute(name)?;
                if name == aria::HIT_ATTRIBUTE {
                    if is_text_field(element) {
                        element.remove_attribute(HIT_TARGET)?;
                    } else if let Some(hit) =
                        element.query_selector(&format!(":scope > [{HIT_TARGET}]"))?
                    {
                        hit.remove();
                    }
                }
            }
            DomOp::SetText { id, text } => {
                let element = self.element(&id)?;
                set_text(element, text.as_deref())?;
                // Text content replaces children, including a hit target.
                if element.has_attribute(aria::HIT_ATTRIBUTE) {
                    self.place_hit_target(element)?;
                }
            }
            // An active native field keeps its native style; it takes its
            // layer style again when it returns.
            DomOp::SetStyle { id, .. } if self.is_active_native(&id) => {}
            DomOp::SetStyle { id, style } => {
                set_style(self.element(&id)?, &format!("{ELEMENT_STYLE}{style}"));
            }
            DomOp::Place { id, parent, before } => {
                // An active native field's placeholder holds its place.
                let element = self.placed(&id)?;
                let parent = match parent {
                    Some(parent) => self.element(&parent)?,
                    None => &self.container,
                };
                let before = before.map(|before| self.placed(&before)).transpose()?;
                parent.insert_before(element, before)?;
            }
            DomOp::Remove { id } => {
                if self.is_active_native(&id) {
                    self.deactivate_native(false)?;
                }
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
        // A copy control writes inside the click, as Safari requires.
        if action == Action::Click {
            crate::clipboard::layer_clicked(&layer.context, element.node);
        }
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

impl Layer {
    /// Give `element` its hit target, unless it has one. A text field, which
    /// cannot hold one, is its own: its style clips it to its centre pixel.
    fn place_hit_target(&self, element: &HtmlElement) -> Result<(), JsValue> {
        if is_text_field(element) {
            element.set_attribute(HIT_TARGET, "")?;
        } else if element
            .query_selector(&format!(":scope > [{HIT_TARGET}]"))?
            .is_none()
        {
            let hit: HtmlElement = self.document.create_element("span")?.unchecked_into();
            hit.set_attribute(HIT_TARGET, "")?;
            hit.set_attribute("aria-hidden", "true")?;
            set_style(&hit, HIT_STYLE);
            element.append_child(&hit)?;
        }
        Ok(())
    }
}

fn is_hit_target(target: Option<web_sys::EventTarget>) -> bool {
    target
        .and_then(|target| target.dyn_into::<Element>().ok())
        .is_some_and(|element| element.has_attribute(HIT_TARGET))
}

/// Dispatch input that landed on a hit target to the canvas instead, as an
/// event of the same type with the same properties, and carry back its
/// `preventDefault`. The original goes no further: the layer's click listener
/// must not also send an AccessKit click.
fn forward(canvas: &HtmlCanvasElement, rule: &JsValue, event: &Event) {
    if !is_hit_target(event.target()) {
        return;
    }
    // A text field is its own hit target, so a scripted `element.click()` on
    // it lands here too. Only real input goes to the canvas; a scripted click
    // stays an AccessKit request, as for every other element.
    let field = event
        .target()
        .and_then(|target| target.dyn_into::<HtmlElement>().ok())
        .is_some_and(|element| is_text_field(&element));
    if field && !event.is_trusted() {
        return;
    }
    event.stop_propagation();
    // The canvas, not a layer element, takes focus from a press.
    if event.type_() == "mousedown" {
        event.prevent_default();
    }
    match copy(event) {
        Ok(copy) => {
            set_pointer_events(rule, "none");
            let accepted = canvas.dispatch_event(&copy).unwrap_or(true);
            set_pointer_events(rule, "auto");
            if !accepted {
                event.prevent_default();
            }
        }
        Err(error) => log_error(&error),
    }
}

/// Set the hit target rule's `pointer-events`.
fn set_pointer_events(rule: &JsValue, value: &str) {
    let result = js_sys::Reflect::get(rule, &"setProperty".into())
        .and_then(|set| set.dyn_into::<js_sys::Function>())
        .and_then(|set| set.call2(rule, &"pointer-events".into(), &value.into()));
    if let Err(error) = result {
        log_error(&error);
    }
}

/// A new event of `event`'s type and class with its properties. Mouse,
/// pointer and wheel events read their initialiser from the event itself; a
/// touch event's touch lists become arrays.
fn copy(event: &Event) -> Result<Event, JsValue> {
    use js_sys::{Array, Object, Reflect};
    let constructor: js_sys::Function = Reflect::get(event, &"constructor".into())?.dyn_into()?;
    let initialiser: JsValue = if event.type_().starts_with("touch") {
        let init = Object::new();
        for name in [
            "bubbles",
            "cancelable",
            "composed",
            "view",
            "detail",
            "ctrlKey",
            "shiftKey",
            "altKey",
            "metaKey",
        ] {
            Reflect::set(&init, &name.into(), &Reflect::get(event, &name.into())?)?;
        }
        for name in ["touches", "targetTouches", "changedTouches"] {
            let list = Reflect::get(event, &name.into())?;
            Reflect::set(&init, &name.into(), &Array::from(&list))?;
        }
        init.into()
    } else {
        event.clone().into()
    };
    let arguments = Array::of2(&event.type_().into(), &initialiser);
    Reflect::construct(&constructor, &arguments)?.dyn_into()
}

/// The pointer moving from the canvas onto a hit target has not left the
/// canvas's control, so the canvas does not hear that it left.
fn keep_pointer(canvas: &HtmlCanvasElement, event: &Event) {
    let onto_hit_target = js_sys::Reflect::get(event, &"relatedTarget".into())
        .ok()
        .and_then(|related| related.dyn_into::<Element>().ok())
        .is_some_and(|element| element.has_attribute(HIT_TARGET));
    let from_canvas = event
        .target()
        .is_some_and(|target| JsValue::from(target) == JsValue::from(canvas.clone()));
    if from_canvas && onto_hit_target {
        event.stop_propagation();
    }
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

/// Follow a touch that may become a tap on a text field.
fn touch(key: u32, event: &Event) {
    // The layer's own forwarded copies are untrusted; the original counts.
    let Some(event) = event
        .dyn_ref::<TouchEvent>()
        .filter(|event| event.is_trusted())
    else {
        return;
    };
    LAYERS.with(|layers| {
        let Ok(mut layers) = layers.try_borrow_mut() else {
            return;
        };
        if let Some(layer) = layers.get_mut(&key)
            && let Err(error) = layer.touch(event)
        {
            log_error(&error);
        }
    });
}

/// eframe's `pointerup` on the canvas focuses the canvas and runs a pass.
/// Where a browser sends a touch's `pointerup` after its `touchend`, that would
/// blur the field the tap just focused, outside the gesture, so it stops
/// here. egui still receives the release, which eframe's `touchend` sends.
fn pointer(key: u32, event: &Event) {
    if !event.is_trusted() {
        return;
    }
    let touch = js_sys::Reflect::get(event, &"pointerType".into())
        .ok()
        .and_then(|kind| kind.as_string())
        .is_some_and(|kind| kind == "touch");
    LAYERS.with(|layers| {
        let Ok(mut layers) = layers.try_borrow_mut() else {
            return;
        };
        let Some(layer) = layers.get_mut(&key) else {
            return;
        };
        let focused = std::mem::take(&mut layer.tap_focused);
        if event.type_() == "pointerup" && touch && focused {
            event.stop_propagation();
        }
    });
}

/// Keep an active native field over its egui field when the page moves the
/// canvas without a pass.
fn reposition(key: u32) {
    LAYERS.with(|layers| {
        let Ok(layers) = layers.try_borrow() else {
            return;
        };
        if let Some(layer) = layers.get(&key)
            && let Some((host, _)) = &layer.native_host
        {
            let bounds = layer.canvas.get_bounding_client_rect();
            let style = format!(
                "{NATIVE_HOST_STYLE}left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px",
                bounds.left(),
                bounds.top(),
                bounds.width(),
                bounds.height()
            );
            set_style(host, &style);
        }
    });
}

/// A native field lost DOM focus; the next pass returns it to the layer.
fn native_blur(key: u32) {
    LAYERS.with(|layers| {
        if let Ok(layers) = layers.try_borrow()
            && let Some(layer) = layers.get(&key)
        {
            layer.context.request_repaint();
        }
    });
}

fn focus_without_scroll(element: &HtmlElement) -> Result<(), JsValue> {
    let options = web_sys::FocusOptions::new();
    options.set_prevent_scroll(true);
    element.focus_with_options(&options)
}

impl Layer {
    fn touch(&mut self, event: &TouchEvent) -> Result<(), JsValue> {
        let first = |list: web_sys::TouchList| (list.length() == 1).then(|| list.get(0)).flatten();
        match event.type_().as_str() {
            "touchstart" => {
                self.touch = first(event.touches()).map(|touch| {
                    (
                        touch.identifier(),
                        f64::from(touch.client_x()),
                        f64::from(touch.client_y()),
                        event.time_stamp(),
                    )
                });
            }
            "touchend" => {
                let Some((identifier, x, y, started)) = self.touch.take() else {
                    return Ok(());
                };
                let Some(end) = first(event.changed_touches())
                    .filter(|end| end.identifier() == identifier && event.touches().length() == 0)
                else {
                    return Ok(());
                };
                if !self.on_canvas(event.target()) {
                    return Ok(());
                }
                let bounds = self.canvas.get_bounding_client_rect();
                let point = |x: f64, y: f64| {
                    (
                        (x - bounds.left()) / self.scale,
                        (y - bounds.top()) / self.scale,
                    )
                };
                let end = point(f64::from(end.client_x()), f64::from(end.client_y()));
                let seconds = (event.time_stamp() - started) / 1000.0;
                if !aria::is_tap(point(x, y), end, seconds) {
                    return Ok(());
                }
                let Some(id) = aria::text_field_at(&self.tree, end).map(ToOwned::to_owned) else {
                    return Ok(());
                };
                if self.tree.native.contains_key(&id) {
                    self.activate_native(&id, end)?;
                } else {
                    self.focus_text_agent()?;
                    self.agent_tap = self.tree.elements.get(&id).map(|element| (element.node, 2));
                }
                self.tap_focused = true;
            }
            _ => self.touch = None,
        }
        Ok(())
    }

    /// Whether a touch began on the canvas or on a layer hit target over it.
    fn on_canvas(&self, target: Option<EventTarget>) -> bool {
        let Some(target) = target.and_then(|target| target.dyn_into::<Element>().ok()) else {
            return false;
        };
        target == *self.canvas.unchecked_ref::<Element>()
            || target.has_attribute(HIT_TARGET) && self.container.contains(Some(&target))
    }

    /// Focus eframe's text agent now, inside the gesture, so iOS raises the
    /// keyboard. egui focuses the field in its next pass, and eframe then
    /// finds the agent focused already.
    fn focus_text_agent(&self) -> Result<(), JsValue> {
        let Some(agent) = self
            .text_agent
            .as_ref()
            .and_then(|agent| agent.dyn_ref::<HtmlElement>())
        else {
            return Ok(());
        };
        if self.document.active_element().as_ref() == Some(agent.unchecked_ref()) {
            return Ok(());
        }
        focus_without_scroll(agent)
    }

    /// In the pass in which egui takes focus from a tap, its text edit emits
    /// no IME output yet, and eframe would blur the text agent the tap just
    /// focused, which closes the keyboard on iOS. While egui's focus is on
    /// the tapped field, give eframe that field's rectangle instead, for at
    /// most two shown passes.
    fn cover_agent_tap(
        &mut self,
        update: &egui::accesskit::TreeUpdate,
        ime: &mut Option<egui::output::IMEOutput>,
    ) {
        let Some((node, passes)) = self.agent_tap.take() else {
            return;
        };
        if ime.is_some() {
            return;
        }
        if update.focus == node
            && let Some(bounds) = update
                .nodes
                .iter()
                .find(|(id, _)| *id == node)
                .and_then(|(_, node)| node.bounds())
        {
            let rect = egui::Rect::from_min_max(
                egui::pos2(bounds.x0 as f32, bounds.y0 as f32),
                egui::pos2(bounds.x1 as f32, bounds.y1 as f32),
            );
            *ime = Some(egui::output::IMEOutput {
                purpose: egui::IMEPurpose::Normal,
                rect,
                cursor_rect: rect,
                should_interrupt_composition: false,
            });
        }
        if passes > 1 {
            self.agent_tap = Some((node, passes - 1));
        }
    }

    /// The visible host of native fields and its stylesheet.
    fn native_host(&mut self) -> Result<(HtmlElement, Sheet), JsValue> {
        if let Some((host, rules)) = &self.native_host {
            return Ok((host.clone(), rules.clone()));
        }
        let host: HtmlElement = self.document.create_element("div")?.unchecked_into();
        host.set_id(&format!("{}native-{}", aria::ARIA_ID_PREFIX, self.key));
        set_style(
            &host,
            &format!("{NATIVE_HOST_STYLE}{}", self.container_geometry),
        );
        let rules = Sheet::new(&self.document, &host, "")?;
        // After eframe's text agent, a sibling of the canvas that eframe
        // keeps at egui's caret, so the field lies above it and a touch there
        // stays the field's.
        match self
            .text_agent
            .as_ref()
            .filter(|agent| agent.parent_node() == self.container.parent_node())
        {
            Some(agent) => agent.after_with_node_1(&host)?,
            None => self.container.after_with_node_1(&host)?,
        }
        // The field edits and hands keys back as it does in the layer.
        let key = self.key;
        let handlers: [(&'static str, Handler); 3] = [
            ("input", edit),
            ("keydown", hand_back),
            ("focusout", |key, _| native_blur(key)),
        ];
        for (event_name, handler) in handlers {
            let listener = Listener::new(move |event: Event| handler(key, &event));
            host.add_event_listener_with_callback(event_name, listener.as_ref().unchecked_ref())?;
            self.listeners.push((event_name, listener));
        }
        self.native_host = Some((host.clone(), rules.clone()));
        Ok((host, rules))
    }

    /// Register Stobie's face, from the bytes egui uses, for native fields.
    fn register_native_font(&mut self) {
        use js_sys::{Array, Function, Reflect, Uint8Array};
        if std::mem::replace(&mut self.native_font, true) {
            return;
        }
        let result = (|| -> Result<(), JsValue> {
            let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
            let constructor: Function = Reflect::get(&window, &"FontFace".into())?.dyn_into()?;
            let bytes = Uint8Array::from(crate::typography::REGULAR_FONT_BYTES);
            let face = Reflect::construct(
                &constructor,
                &Array::of2(&crate::REGULAR_FONT_FAMILY.into(), &bytes),
            )?;
            let fonts = Reflect::get(&self.document, &"fonts".into())?;
            let add: Function = Reflect::get(&fonts, &"add".into())?.dyn_into()?;
            add.call1(&fonts, &face)?;
            Ok(())
        })();
        if let Err(error) = result {
            log_error(&error);
        }
    }

    /// Move native field `id` out of the layer, over its egui field, and
    /// focus it with the caret where the tap at `point` (in points) landed.
    /// Called inside a tap, so iOS raises the keyboard.
    fn activate_native(&mut self, id: &str, point: (f64, f64)) -> Result<(), JsValue> {
        if self.is_active_native(id) {
            return Ok(());
        }
        self.deactivate_native(false)?;
        let (Some(element), Some(field), Some(registration), Some(node)) = (
            self.elements.get(id).cloned(),
            self.tree.native.get(id).cloned(),
            self.native_fields.get(id),
            self.tree.elements.get(id).map(|element| element.node),
        ) else {
            return Ok(());
        };
        let egui_id = registration.id;
        // A programmatic focus puts the caret at the end; egui's layout of
        // the text says where the tap put it.
        let caret = registration.char_at(egui::pos2(point.0 as f32, point.1 as f32));
        let (host, rules) = self.native_host()?;
        // Move before focusing: moving a focused element blurs it.
        let placeholder: HtmlElement = self.document.create_element("span")?.unchecked_into();
        placeholder.set_attribute("aria-hidden", "true")?;
        set_style(&placeholder, NATIVE_PLACEHOLDER_STYLE);
        element.replace_with_with_node_1(&placeholder)?;
        // Its layer parent still owns it for assistive technology.
        let owner = placeholder
            .parent_element()
            .ok_or_else(|| JsValue::from_str("native field placeholder has no parent"))?;
        let owns = owner.get_attribute("aria-owns");
        let owned = match &owns {
            Some(owns) => format!("{owns} {id}"),
            None => id.to_owned(),
        };
        owner.set_attribute("aria-owns", &owned)?;
        // Until the next pass, the field takes no pointer input: eframe's own
        // `touchend` runs after this listener and hit-tests the touch, and it
        // releases egui's press only if it finds the canvas there.
        let style = field
            .style
            .replace("pointer-events:auto", "pointer-events:none");
        set_style(&element, &style);
        element.set_attribute(aria::NATIVE_ACTIVE_ATTRIBUTE, "")?;
        if field.placeholder.is_empty() {
            element.remove_attribute("placeholder")?;
        } else {
            element.set_attribute("placeholder", &field.placeholder)?;
        }
        rules.set_rules(&field.rules)?;
        host.append_child(&element)?;
        focus_without_scroll(&element)?;
        let value = field_value(&element);
        let caret: u32 = value
            .chars()
            .take(caret)
            .map(char::len_utf16)
            .sum::<usize>()
            .try_into()
            .unwrap_or(u32::MAX);
        let select: js_sys::Function =
            js_sys::Reflect::get(&element, &"setSelectionRange".into())?.dyn_into()?;
        select.call2(&element, &caret.into(), &caret.into())?;
        let mut listeners = Vec::new();
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
        let viewport = js_sys::Reflect::get(&window, &"visualViewport".into())?;
        let targets: Vec<(EventTarget, &[&'static str])> = [
            Some((
                window.unchecked_into::<EventTarget>(),
                &NATIVE_VIEWPORT_EVENTS[..],
            )),
            viewport
                .dyn_into::<EventTarget>()
                .ok()
                .map(|viewport| (viewport, &NATIVE_VIEWPORT_EVENTS[..2])),
        ]
        .into_iter()
        .flatten()
        .collect();
        let key = self.key;
        for (target, events) in targets {
            for event_name in events {
                let listener = Listener::new(move |_: Event| reposition(key));
                target.add_event_listener_with_callback(
                    event_name,
                    listener.as_ref().unchecked_ref(),
                )?;
                listeners.push((target.clone(), *event_name, listener));
            }
        }
        native_text::set_native_text_active(&self.context, Some(egui_id));
        self.requests.push(ActionRequest {
            action: Action::Focus,
            target_node: node,
            target_tree: TreeId::ROOT,
            data: None,
        });
        self.context.request_repaint();
        self.native = Some(ActiveNative {
            id: id.to_owned(),
            placeholder: placeholder.into(),
            owner,
            owns,
            confirmed: false,
            style,
            rules: field.rules,
            listeners,
        });
        Ok(())
    }

    /// Return the active native field to the layer. `focus_canvas` gives DOM
    /// focus to the canvas if the field had it, so eframe's text agent can
    /// take it for another field.
    fn deactivate_native(&mut self, focus_canvas: bool) -> Result<(), JsValue> {
        let Some(active) = self.native.take() else {
            return Ok(());
        };
        for (target, event_name, listener) in &active.listeners {
            let _ = target
                .remove_event_listener_with_callback(event_name, listener.as_ref().unchecked_ref());
        }
        native_text::set_native_text_active(&self.context, None);
        self.context.request_repaint();
        match &active.owns {
            Some(owns) => active.owner.set_attribute("aria-owns", owns)?,
            None => active.owner.remove_attribute("aria-owns")?,
        }
        let Some(element) = self.elements.get(&active.id) else {
            active.placeholder.remove();
            return Ok(());
        };
        let focused =
            self.document.active_element().as_ref() == Some(element.unchecked_ref::<Element>());
        element.remove_attribute(aria::NATIVE_ACTIVE_ATTRIBUTE)?;
        element.remove_attribute("placeholder")?;
        if let Some(mirrored) = self.tree.elements.get(&active.id) {
            set_style(element, &format!("{ELEMENT_STYLE}{}", mirrored.style));
        }
        // Its focus-out listener finds the layer busy and does nothing.
        if focused {
            element.blur()?;
        }
        active.placeholder.replace_with_with_node_1(element)?;
        if focus_canvas && focused {
            focus_without_scroll(&self.canvas)?;
        }
        Ok(())
    }

    /// After a pass: keep the active native field's presentation current, and
    /// return it to the layer once it is no longer native, presented or
    /// focused, or once egui's focus leaves the field it had.
    fn keep_native(&mut self) -> Result<(), JsValue> {
        let Some(active) = &self.native else {
            return Ok(());
        };
        let id = active.id.clone();
        let Some(element) = self.elements.get(&id).cloned() else {
            return self.deactivate_native(false);
        };
        let focused =
            self.document.active_element().as_ref() == Some(element.unchecked_ref::<Element>());
        let Some(field) = self.tree.native.get(&id).cloned() else {
            return self.deactivate_native(focused);
        };
        let egui_focus = self.tree.active.as_deref() == Some(id.as_str());
        if !focused {
            // eframe's `pointerup` focuses the canvas for any touch on it,
            // including one on the field's own edge, just outside the
            // element. While egui keeps the field, the field takes DOM focus
            // back; focus moved anywhere else returns it to the layer.
            let canvas_focused = self.document.active_element().as_ref()
                == Some(self.canvas.unchecked_ref::<Element>());
            if !(canvas_focused && egui_focus) {
                return self.deactivate_native(false);
            }
            focus_without_scroll(&element)?;
        }
        let active = self.native.as_mut().expect("an active native field");
        // The diff may have moved the placeholder to another layer parent,
        // which then owns the field instead.
        if let Some(parent) = active.placeholder.parent_element()
            && parent != active.owner
        {
            match &active.owns {
                Some(owns) => active.owner.set_attribute("aria-owns", owns)?,
                None => active.owner.remove_attribute("aria-owns")?,
            }
            let owns = parent.get_attribute("aria-owns");
            let owned = match &owns {
                Some(owns) => format!("{owns} {id}"),
                None => id.clone(),
            };
            parent.set_attribute("aria-owns", &owned)?;
            active.owner = parent;
            active.owns = owns;
        }
        if !active.confirmed && egui_focus {
            active.confirmed = true;
        } else if active.confirmed && !egui_focus {
            return self.deactivate_native(true);
        }
        if field.style != active.style {
            set_style(&element, &field.style);
            active.style = field.style;
        }
        if field.rules != active.rules
            && let Some((_, rules)) = &self.native_host
        {
            rules.set_rules(&field.rules)?;
            active.rules = field.rules;
        }
        Ok(())
    }
}

/// Set an element's whole inline style through the CSSOM, which a
/// Content-Security-Policy without `'unsafe-inline'` allows, unlike a `style`
/// attribute.
fn set_style(element: &HtmlElement, css: &str) {
    element.style().set_css_text(css);
}

/// A stylesheet the layer owns: a constructed sheet that the document
/// adopts, which a Content-Security-Policy without `'unsafe-inline'` allows,
/// unlike `<style>` text. A browser without constructable stylesheets gets a
/// `<style>` element in `parent` instead.
#[derive(Clone)]
enum Sheet {
    Adopted(web_sys::CssStyleSheet),
    Element(Element),
}

impl Sheet {
    fn new(document: &Document, parent: &Element, rules: &str) -> Result<Self, JsValue> {
        let adoptable = js_sys::Reflect::has(document, &"adoptedStyleSheets".into())?;
        if let Some(sheet) = web_sys::CssStyleSheet::new().ok().filter(|_| adoptable) {
            sheet.replace_sync(rules)?;
            let adopted = js_sys::Array::from(&document.adopted_style_sheets());
            adopted.push(&sheet);
            document.set_adopted_style_sheets(&adopted);
            return Ok(Self::Adopted(sheet));
        }
        let element = document.create_element("style")?;
        element.set_text_content(Some(rules));
        parent.append_child(&element)?;
        Ok(Self::Element(element))
    }

    fn sheet(&self) -> Result<JsValue, JsValue> {
        match self {
            Self::Adopted(sheet) => Ok(sheet.into()),
            Self::Element(element) => js_sys::Reflect::get(element, &"sheet".into()),
        }
    }

    /// The style declaration of the sheet's first rule.
    fn first_rule_style(&self) -> Result<JsValue, JsValue> {
        let rules = js_sys::Reflect::get(&self.sheet()?, &"cssRules".into())?;
        let rule = js_sys::Reflect::get(&rules, &0.into())?;
        js_sys::Reflect::get(&rule, &"style".into())
    }

    fn set_rules(&self, rules: &str) -> Result<(), JsValue> {
        match self {
            Self::Adopted(sheet) => sheet.replace_sync(rules),
            Self::Element(element) => {
                element.set_text_content(Some(rules));
                Ok(())
            }
        }
    }

    fn remove(&self, document: &Document) {
        match self {
            Self::Adopted(sheet) => {
                let target = JsValue::from(sheet);
                let kept = js_sys::Array::from(&document.adopted_style_sheets())
                    .filter(&mut |other, _, _| other != target);
                document.set_adopted_style_sheets(&kept);
            }
            Self::Element(element) => element.remove(),
        }
    }
}

fn log_error(error: &JsValue) {
    web_sys::console::error_2(&JsValue::from_str("Stobie ARIA layer:"), error);
}
