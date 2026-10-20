//! Projection of egui's AccessKit tree into a hidden ARIA layer.
//!
//! The browser host mirrors each completed pass's AccessKit update into
//! absolutely positioned DOM elements over the canvas (see `host::aria_dom`).
//! This module is the platform-independent half: it projects an update into
//! the element tree the layer should contain, and diffs two projections into
//! the minimal DOM operations, so an unchanged tree produces no operation.
//!
//! The AccessKit tree is the only source. Stobie recipes set each control's
//! `author_id` to its semantic ID, which becomes the element's stable DOM ID.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use egui::accesskit::{self, AriaCurrent, Live, Node, NodeId, Orientation, Role, Toggled};

use crate::native_text::{NativeFont, NativeTextRegistration};

/// Prefix of every element ID the layer creates, kept apart from page IDs.
pub const ARIA_ID_PREFIX: &str = "stobie-a11y:";

/// The attribute carrying a node's semantic ID, which agents share with the
/// inspection interface.
pub const SEMANTIC_ID_ATTRIBUTE: &str = "data-stobie-id";

/// Marks an enabled element that an ordinary pointer click can target. The
/// browser host gives it a one-pixel hit target at its centre that forwards
/// pointer input to the canvas.
pub const HIT_ATTRIBUTE: &str = "data-stobie-hit";

/// Marks a text field that the application presents as a native field while
/// it is active (see `native_text`).
pub const NATIVE_ATTRIBUTE: &str = "data-stobie-native";

/// Marks the active native field. Its selection and placeholder rules select
/// it by this attribute.
pub const NATIVE_ACTIVE_ATTRIBUTE: &str = "data-stobie-native-active";

/// iOS zooms the page to focus a text field whose text is smaller than this,
/// in CSS pixels.
const NATIVE_MIN_FONT_PX: f64 = 16.0;

/// One element of the projected layer.
#[derive(Clone, Debug, PartialEq)]
pub struct AriaElement {
    /// The AccessKit node this element presents, for routing actions back.
    pub node: NodeId,
    /// The element's tag: `input` or `textarea` for text fields, whose value
    /// browsers expose reliably, and `div` otherwise.
    pub tag: &'static str,
    /// The parent element; `None` is the layer container.
    pub parent: Option<String>,
    pub children: Vec<String>,
    /// ARIA attributes, including `role`, in a stable order.
    pub attributes: BTreeMap<&'static str, String>,
    /// Text content, for static text and for the value of text-valued controls.
    pub text: Option<String>,
    /// Position and size relative to the parent element, in CSS pixels.
    pub style: String,
}

/// The layer's complete desired state for one pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AriaTree {
    pub elements: BTreeMap<String, AriaElement>,
    /// Top-level elements, in order.
    pub roots: Vec<String>,
    /// The element the canvas's `aria-activedescendant` names.
    pub active: Option<String>,
    /// Elements a pointer acts on, in egui's registration order.
    pub targets: Vec<PointerTarget>,
    /// The native fields' active presentation, by element ID.
    pub native: BTreeMap<String, NativeField>,
}

/// An element a pointer acts on, with the part of it that is shown.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerTarget {
    pub id: String,
    /// `(x0, y0, x1, y1)` in points.
    pub visible: (f64, f64, f64, f64),
    /// Whether it is an enabled, editable text field.
    pub text_field: bool,
}

/// How a native field is presented while it is active: its element's inline
/// style, positioned over the canvas in CSS pixels, the stylesheet rules for
/// its selection and placeholder, and its placeholder text.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeField {
    pub style: String,
    pub rules: String,
    pub placeholder: String,
}

/// The editable text field a tap at `point` (in points) would focus: the
/// topmost element a pointer acts on there, egui's last registered, if it is
/// an enabled, editable text field.
pub fn text_field_at(tree: &AriaTree, (x, y): (f64, f64)) -> Option<&str> {
    tree.targets
        .iter()
        .rev()
        .find(|target| {
            let (x0, y0, x1, y1) = target.visible;
            x0 <= x && x <= x1 && y0 <= y && y <= y1
        })
        .filter(|target| target.text_field)
        .map(|target| target.id.as_str())
}

/// Whether a touch from `start` to `end`, in points, lasting `seconds`, is a
/// tap that egui will take as a click: it stays within egui's click distance
/// and time.
pub fn is_tap(start: (f64, f64), end: (f64, f64), seconds: f64) -> bool {
    let distance = (end.0 - start.0).hypot(end.1 - start.1);
    distance <= 6.0 && (0.0..0.8).contains(&seconds)
}

/// One DOM change. Applying a diff's operations in order turns the previous
/// projection's DOM into the next one's.
#[derive(Clone, Debug, PartialEq)]
pub enum DomOp {
    Create {
        id: String,
        tag: &'static str,
    },
    SetAttribute {
        id: String,
        name: &'static str,
        value: String,
    },
    RemoveAttribute {
        id: String,
        name: &'static str,
    },
    SetText {
        id: String,
        text: Option<String>,
    },
    SetStyle {
        id: String,
        style: String,
    },
    /// Insert `id` into `parent` (`None` is the container) before the sibling
    /// `before`, or last when there is none. `before` is already in place.
    Place {
        id: String,
        parent: Option<String>,
        before: Option<String>,
    },
    Remove {
        id: String,
    },
    /// Set or clear the canvas's `aria-activedescendant`.
    Active {
        id: Option<String>,
    },
}

/// Project one AccessKit update. `scale` converts egui points to CSS pixels
/// (egui's zoom factor on the web).
pub fn project(update: &accesskit::TreeUpdate, scale: f32) -> AriaTree {
    project_within(update, scale, None)
}

/// [`project`], showing only what lies within `viewport` (in points), such as
/// the canvas, where egui's root carries no bounds.
pub fn project_within(
    update: &accesskit::TreeUpdate,
    scale: f32,
    viewport: Option<(f64, f64, f64, f64)>,
) -> AriaTree {
    project_native(update, scale, viewport, &[])
}

/// [`project_within`], presenting the text fields registered in `native` as
/// native fields.
pub(crate) fn project_native(
    update: &accesskit::TreeUpdate,
    scale: f32,
    viewport: Option<(f64, f64, f64, f64)>,
    native: &[NativeTextRegistration],
) -> AriaTree {
    let native: HashMap<NodeId, &NativeTextRegistration> = native
        .iter()
        .map(|field| (field.id.accesskit_id(), field))
        .collect();
    let nodes: HashMap<NodeId, &Node> = update.nodes.iter().map(|(id, node)| (*id, node)).collect();
    let mut tree = AriaTree::default();
    let Some(root) = update.tree.as_ref().map(|tree| tree.root) else {
        return tree;
    };
    let Some(root_node) = nodes.get(&root) else {
        return tree;
    };
    let mut projection = Projection {
        nodes: &nodes,
        native: &native,
        scale,
        tree: &mut tree,
        ids: HashMap::new(),
        used: BTreeSet::new(),
        interactive: Vec::new(),
        hits: Vec::new(),
    };
    // Nothing outside the window is shown.
    let window = root_node
        .bounds()
        .map(|bounds| (bounds.x0, bounds.y0, bounds.x1, bounds.y1))
        .or(viewport);
    for child in root_node.children() {
        projection.visit(*child, None, None, window);
    }
    let hits = projection.hits;
    let interactive = projection.interactive;
    // Relations name other nodes, so resolve them once every element exists.
    let ids = projection.ids;
    // egui gives a click to the control registered last among those under
    // the pointer, which comes later in the tree. A hit target whose centre a
    // later control also covers would click that control instead, so it takes
    // no pointer. An earlier one, such as a dock splitter's grab area that
    // its panes' contents overlap, lies beneath it.
    for (id, (x, y), order) in &hits {
        let contested = interactive[order + 1..].iter().any(
            |PointerTarget {
                 id: other,
                 visible: (x0, y0, x1, y1),
                 ..
             }| {
                !is_ancestor(&tree, other, id)
                    && !is_ancestor(&tree, id, other)
                    && x0 <= x
                    && x <= x1
                    && y0 <= y
                    && y <= y1
            },
        );
        if contested {
            tree.elements
                .get_mut(id)
                .expect("projected hit target")
                .attributes
                .remove(HIT_ATTRIBUTE);
        }
    }
    for (id, node) in &nodes {
        let Some(element_id) = ids.get(id) else {
            continue;
        };
        let labelled_by: Vec<&str> = node
            .labelled_by()
            .iter()
            .filter_map(|label| ids.get(label).map(String::as_str))
            .collect();
        if !labelled_by.is_empty() {
            let element = tree
                .elements
                .get_mut(element_id)
                .expect("projected element");
            element
                .attributes
                .insert("aria-labelledby", labelled_by.join(" "));
        }
    }
    if update.focus != root {
        tree.active = ids.get(&update.focus).cloned();
    }
    tree.targets = interactive;
    tree
}

struct Projection<'a> {
    nodes: &'a HashMap<NodeId, &'a Node>,
    native: &'a HashMap<NodeId, &'a NativeTextRegistration>,
    scale: f32,
    tree: &'a mut AriaTree,
    ids: HashMap<NodeId, String>,
    used: BTreeSet<String>,
    /// Elements a pointer acts on, in tree order, with their visible bounds.
    interactive: Vec<PointerTarget>,
    /// Hit target candidates, with their centres in points and their index
    /// in `interactive`.
    hits: Vec<(String, (f64, f64), usize)>,
}

impl Projection<'_> {
    /// Visit `id` under the nearest kept ancestor `parent`, whose absolute
    /// origin (in points) is `origin`, within the visible area `clip` that
    /// clipping ancestors leave (in points).
    fn visit(&mut self, id: NodeId, parent: Option<&str>, origin: Option<(f64, f64)>, clip: Clip) {
        let Some(node) = self.nodes.get(&id).copied() else {
            return;
        };
        if node.is_hidden() {
            return;
        }
        let clip = narrow(clip, node);
        if is_transparent(node) {
            for child in node.children() {
                self.visit(*child, parent, origin, clip);
            }
            return;
        }
        let element_id = self.element_id(id, node);
        let (left, top, width, height) = match node.bounds() {
            Some(bounds) => (bounds.x0, bounds.y0, bounds.width(), bounds.height()),
            None => {
                let (x, y) = origin.unwrap_or_default();
                (x, y, 0.0, 0.0)
            }
        };
        let (origin_x, origin_y) = origin.unwrap_or_default();
        let scale = f64::from(self.scale);
        let mut style = format!(
            "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px",
            (left - origin_x) * scale,
            (top - origin_y) * scale,
            width.max(0.0) * scale,
            height.max(0.0) * scale,
        );
        // A text role names itself from its own text unless other nodes,
        // such as a status's labels, hold that text.
        let contains_text = node.children().iter().any(|child| {
            self.nodes
                .get(child)
                .is_some_and(|child| child.role() != Role::TextRun)
        });
        let (mut attributes, text) = presentation(node, contains_text);
        let tag = match attributes.get("role").map(String::as_str) {
            Some("textbox") if attributes.contains_key("aria-multiline") => "textarea",
            Some("textbox") => "input",
            _ => "div",
        };
        if tag != "div" {
            // Native text fields carry their role and multiline state. They
            // never join the page's Tab order, and browser autofill and
            // spelling tools must not attach to them.
            attributes.remove("role");
            attributes.remove("aria-multiline");
            attributes.insert("tabindex", "-1".to_owned());
            attributes.insert("autocomplete", "off".to_owned());
            attributes.insert("spellcheck", "false".to_owned());
            // A text field cannot hold a hit target element, so its own
            // centre pixel is its hit target, and only while it takes one.
            style.push_str(";clip-path:inset(calc(50% - 0.5px));touch-action:none");
            if let Some(field) = self.native.get(&id) {
                attributes.insert(NATIVE_ATTRIBUTE, "true".to_owned());
                // A native field edits prose with the platform's help, unless
                // the application turned it off.
                let (spellcheck, autocorrect, autocapitalize) = if field.spellcheck {
                    ("true", "on", "sentences")
                } else {
                    ("false", "off", "off")
                };
                attributes.insert("spellcheck", spellcheck.to_owned());
                attributes.insert("autocorrect", autocorrect.to_owned());
                attributes.insert("autocapitalize", autocapitalize.to_owned());
                self.tree
                    .native
                    .insert(element_id.clone(), native_field(field, self.scale));
            }
        }
        let pointer_control = if tag == "div" {
            attributes.contains_key("role") && node.supports_action(accesskit::Action::Click)
        } else {
            !node.is_read_only()
        };
        if pointer_control
            && !node.is_disabled()
            && width > 0.0
            && height > 0.0
            && clip.is_none_or(|(x0, y0, x1, y1)| {
                let (x, y) = (left + width / 2.0, top + height / 2.0);
                x0 <= x && x <= x1 && y0 <= y && y <= y1
            })
        {
            // egui reports a widget's whole rectangle, even where a list or
            // scroll area clips it, so only a control whose centre is visible
            // takes a pointer: elsewhere the pixel would sit over another
            // control.
            attributes.insert(HIT_ATTRIBUTE, "true".to_owned());
            self.hits.push((
                element_id.clone(),
                (left + width / 2.0, top + height / 2.0),
                self.interactive.len(),
            ));
        }
        // egui clips a widget's interaction to its clip rectangle, so only the
        // visible part of a control can take a click.
        let visible = match clip {
            None => Some((left, top, left + width, top + height)),
            Some((x0, y0, x1, y1)) => {
                let visible = (
                    left.max(x0),
                    top.max(y0),
                    (left + width).min(x1),
                    (top + height).min(y1),
                );
                (visible.0 < visible.2 && visible.1 < visible.3).then_some(visible)
            }
        };
        if let Some(visible) = visible.filter(|_| width > 0.0 && height > 0.0)
            && (tag != "div"
                || attributes.contains_key("role")
                    && (node.supports_action(accesskit::Action::Click)
                        || matches!(node.role(), Role::Splitter | Role::Slider | Role::ScrollBar)))
        {
            self.interactive.push(PointerTarget {
                id: element_id.clone(),
                visible,
                text_field: tag != "div" && !node.is_read_only() && !node.is_disabled(),
            });
        }
        if let Some(author) = node.author_id() {
            attributes.insert(SEMANTIC_ID_ATTRIBUTE, author.to_owned());
        }
        self.tree.elements.insert(
            element_id.clone(),
            AriaElement {
                node: id,
                tag,
                parent: parent.map(ToOwned::to_owned),
                children: Vec::new(),
                attributes,
                text,
                style,
            },
        );
        match parent {
            Some(parent) => self
                .tree
                .elements
                .get_mut(parent)
                .expect("parent is projected first")
                .children
                .push(element_id.clone()),
            None => self.tree.roots.push(element_id.clone()),
        }
        self.ids.insert(id, element_id.clone());
        // Text-presenting nodes own their text; their text runs add nothing.
        if presents_text(node.role()) && !contains_text {
            return;
        }
        for child in node.children() {
            self.visit(*child, Some(&element_id), Some((left, top)), clip);
        }
    }

    /// The semantic ID when Stobie set one and it is unused; otherwise the
    /// egui node ID, which is stable while the widget's egui ID is.
    fn element_id(&mut self, id: NodeId, node: &Node) -> String {
        let candidate = node
            .author_id()
            .map(|author| format!("{ARIA_ID_PREFIX}{author}"))
            .filter(|candidate| !self.used.contains(candidate))
            .unwrap_or_else(|| format!("{ARIA_ID_PREFIX}egui-{:x}", id.0));
        self.used.insert(candidate.clone());
        candidate
    }
}

/// A native field's active presentation over the canvas. Its geometry and
/// style come from its registration, in points, scaled to CSS pixels by
/// `scale`. Text smaller than [`NATIVE_MIN_FONT_PX`] is laid out at that size
/// in an element scaled down to the field, so focusing it does not zoom the
/// page on iOS.
fn native_field(field: &NativeTextRegistration, scale: f32) -> NativeField {
    let scale = f64::from(scale.max(f32::EPSILON));
    let font_px = f64::from(field.font_size) * scale;
    let shrink = if font_px < NATIVE_MIN_FONT_PX {
        font_px / NATIVE_MIN_FONT_PX
    } else {
        1.0
    };
    // Lengths inside the element, which the shrink scales back down.
    let inner = |points: f32| f64::from(points.max(0.0)) * scale / shrink;
    let (rect, text) = (field.rect, field.text_rect);
    let visible = rect.intersect(field.clip);
    let mut style = format!(
        "position:absolute;margin:0;box-sizing:border-box;left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;padding:{:.1}px {:.1}px {:.1}px {:.1}px;",
        f64::from(rect.min.x) * scale,
        f64::from(rect.min.y) * scale,
        inner(rect.width()),
        inner(rect.height()),
        inner(text.min.y - rect.min.y),
        inner(rect.max.x - text.max.x),
        inner(rect.max.y - text.max.y),
        inner(text.min.x - rect.min.x),
    );
    style.push_str(
        "border:0;border-radius:0;outline:none;background:transparent;-webkit-appearance:none;appearance:none;resize:none;overflow-x:hidden;overflow-y:auto;",
    );
    let family = match field.family {
        NativeFont::Proportional => {
            format!(
                "\"{}\",system-ui,-apple-system,sans-serif",
                crate::REGULAR_FONT_FAMILY
            )
        }
        NativeFont::Monospace => "ui-monospace,Menlo,monospace".to_owned(),
    };
    let colour = css_colour(field.text);
    style.push_str(&format!(
        "font-family:{family};font-size:{:.1}px;font-weight:400;font-style:normal;letter-spacing:normal;line-height:{:.1}px;color:{colour};-webkit-text-fill-color:{colour};caret-color:{};",
        font_px / shrink,
        inner(field.line_height),
        css_colour(field.caret),
    ));
    style.push_str(
        "opacity:1;pointer-events:auto;user-select:text;-webkit-user-select:text;touch-action:manipulation;-webkit-tap-highlight-color:transparent;",
    );
    if visible != rect {
        // Only what the field's scroll area or pane shows of it.
        let (top, right, bottom, left) = if visible.is_positive() {
            (
                visible.min.y - rect.min.y,
                rect.max.x - visible.max.x,
                rect.max.y - visible.max.y,
                visible.min.x - rect.min.x,
            )
        } else {
            (rect.height(), 0.0, 0.0, 0.0)
        };
        style.push_str(&format!(
            "clip-path:inset({:.1}px {:.1}px {:.1}px {:.1}px);",
            inner(top),
            inner(right),
            inner(bottom),
            inner(left)
        ));
    }
    if shrink < 1.0 {
        style.push_str(&format!(
            "transform:scale({});transform-origin:0 0;",
            number(shrink)
        ));
    }
    let placeholder = css_colour(field.placeholder);
    let rules = format!(
        "[{NATIVE_ACTIVE_ATTRIBUTE}]::selection{{background-color:{}}}[{NATIVE_ACTIVE_ATTRIBUTE}]::placeholder{{color:{placeholder};-webkit-text-fill-color:{placeholder};opacity:1}}",
        css_colour(field.selection),
    );
    NativeField {
        style,
        rules,
        placeholder: field.hint.clone(),
    }
}

/// A CSS colour from egui's premultiplied colour.
fn css_colour(colour: egui::Color32) -> String {
    let [r, g, b, a] = colour.to_srgba_unmultiplied();
    format!("rgba({r},{g},{b},{})", number(f64::from(a) / 255.0))
}

/// Whether element `ancestor` contains element `id`.
fn is_ancestor(tree: &AriaTree, ancestor: &str, id: &str) -> bool {
    let mut current = tree
        .elements
        .get(id)
        .and_then(|element| element.parent.as_deref());
    while let Some(parent) = current {
        if parent == ancestor {
            return true;
        }
        current = tree
            .elements
            .get(parent)
            .and_then(|element| element.parent.as_deref());
    }
    false
}

/// A visible area in points: `(x0, y0, x1, y1)`, or `None` for no clip.
type Clip = Option<(f64, f64, f64, f64)>;

/// Narrow `clip` to a node that clips its descendants: a scroll area, list,
/// pane or region shows only what lies within its bounds.
fn narrow(clip: Clip, node: &Node) -> Clip {
    let clips = matches!(
        node.role(),
        Role::ScrollView
            | Role::ListBox
            | Role::List
            | Role::Grid
            | Role::Tree
            | Role::Pane
            | Role::Region
            | Role::TabPanel
    );
    let Some(bounds) = node.bounds().filter(|_| clips) else {
        return clip;
    };
    let own = (bounds.x0, bounds.y0, bounds.x1, bounds.y1);
    Some(match clip {
        None => own,
        Some((x0, y0, x1, y1)) => (x0.max(own.0), y0.max(own.1), x1.min(own.2), y1.min(own.3)),
    })
}

/// Unnamed layout containers add nothing for assistive technology, so their
/// children join the nearest presented ancestor.
fn is_transparent(node: &Node) -> bool {
    matches!(
        node.role(),
        Role::GenericContainer | Role::Unknown | Role::ScrollView
    ) && node.label().is_none()
        && node.description().is_none()
        && node.value().is_none()
        && !node.supports_action(accesskit::Action::Click)
        && !node.supports_action(accesskit::Action::Focus)
        || matches!(node.role(), Role::TextRun | Role::ScrollBar)
}

fn presents_text(role: Role) -> bool {
    matches!(
        role,
        Role::Label
            | Role::Paragraph
            | Role::Heading
            | Role::Status
            | Role::TextInput
            | Role::MultilineTextInput
            | Role::SearchInput
            | Role::EmailInput
            | Role::NumberInput
            | Role::PasswordInput
            | Role::PhoneNumberInput
            | Role::UrlInput
            | Role::ComboBox
            | Role::EditableComboBox
    )
}

/// How an element presents its node's name.
#[derive(Clone, Copy, PartialEq)]
enum Naming {
    /// `aria-label` names the element.
    Label,
    /// The text content is the element's name (static text, headings, status).
    Content,
    /// The text content is the control's value and `aria-label` its name.
    Value,
}

/// The ARIA role and naming for an AccessKit role. `None` means a generic
/// element, which takes no name.
fn aria_role(node: &Node) -> (Option<&'static str>, Naming) {
    let named = node.label().is_some();
    match node.role() {
        Role::Button | Role::DefaultButton => (Some("button"), Naming::Label),
        Role::Link => (Some("link"), Naming::Label),
        Role::CheckBox => (Some("checkbox"), Naming::Label),
        Role::Switch => (Some("switch"), Naming::Label),
        Role::RadioButton => (Some("radio"), Naming::Label),
        Role::RadioGroup => (Some("radiogroup"), Naming::Label),
        Role::ComboBox | Role::EditableComboBox => (Some("combobox"), Naming::Value),
        Role::TextInput
        | Role::MultilineTextInput
        | Role::SearchInput
        | Role::EmailInput
        | Role::NumberInput
        | Role::PasswordInput
        | Role::PhoneNumberInput
        | Role::UrlInput => (Some("textbox"), Naming::Value),
        Role::Slider => (Some("slider"), Naming::Label),
        Role::SpinButton => (Some("spinbutton"), Naming::Label),
        Role::ProgressIndicator | Role::Meter => (Some("progressbar"), Naming::Label),
        Role::Splitter => (Some("separator"), Naming::Label),
        Role::TabList => (Some("tablist"), Naming::Label),
        Role::Tab => (Some("tab"), Naming::Label),
        Role::TabPanel => (Some("tabpanel"), Naming::Label),
        Role::ListBox => (Some("listbox"), Naming::Label),
        Role::ListBoxOption => (Some("option"), Naming::Label),
        Role::List => (Some("list"), Naming::Label),
        Role::ListItem => (Some("listitem"), Naming::Label),
        Role::Menu => (Some("menu"), Naming::Label),
        Role::MenuBar => (Some("menubar"), Naming::Label),
        Role::MenuItem => (Some("menuitem"), Naming::Label),
        Role::MenuItemCheckBox => (Some("menuitemcheckbox"), Naming::Label),
        Role::MenuItemRadio => (Some("menuitemradio"), Naming::Label),
        Role::Toolbar => (Some("toolbar"), Naming::Label),
        Role::Grid => (Some("grid"), Naming::Label),
        Role::Row => (Some("row"), Naming::Label),
        Role::Cell | Role::GridCell => (Some("gridcell"), Naming::Label),
        Role::Tree => (Some("tree"), Naming::Label),
        Role::TreeItem => (Some("treeitem"), Naming::Label),
        Role::Dialog => (Some("dialog"), Naming::Label),
        Role::AlertDialog => (Some("alertdialog"), Naming::Label),
        Role::Alert => (Some("alert"), Naming::Content),
        Role::Status => (Some("status"), Naming::Content),
        Role::Heading => (Some("heading"), Naming::Content),
        Role::Image => (Some("img"), Naming::Label),
        Role::Tooltip => (Some("tooltip"), Naming::Content),
        Role::Canvas => (Some("group"), Naming::Label),
        // Named panes and regions are landmarks; unnamed ones are layout.
        Role::Pane | Role::Region if named => (Some("region"), Naming::Label),
        Role::Group | Role::GenericContainer | Role::Unknown if named => {
            (Some("group"), Naming::Label)
        }
        Role::Label | Role::Paragraph => (None, Naming::Content),
        _ => (None, Naming::Content),
    }
}

/// The element's attributes and text content for one node.
fn presentation(
    node: &Node,
    contains_text: bool,
) -> (BTreeMap<&'static str, String>, Option<String>) {
    let (role, mut naming) = aria_role(node);
    if contains_text && naming == Naming::Content {
        naming = Naming::Label;
    }
    let mut attributes = BTreeMap::new();
    let mut text = None;
    if let Some(role) = role {
        attributes.insert("role", role.to_owned());
    }
    match naming {
        Naming::Label => {
            // A generic element takes no name.
            if let Some(label) = node
                .label()
                .filter(|label| role.is_some() && !label.is_empty())
            {
                attributes.insert("aria-label", label.to_owned());
            }
        }
        Naming::Content => {
            // egui stores a label's text as its value.
            text = node
                .value()
                .or(node.label())
                .filter(|content| !content.is_empty())
                .map(ToOwned::to_owned);
        }
        Naming::Value => {
            if let Some(label) = node.label().filter(|label| !label.is_empty()) {
                attributes.insert("aria-label", label.to_owned());
            }
            text = node.value().map(ToOwned::to_owned);
        }
    }
    if role.is_some() {
        if let Some(description) = node.description().filter(|text| !text.is_empty()) {
            attributes.insert("aria-description", description.to_owned());
        }
        if let Some(description) = node.role_description() {
            attributes.insert("aria-roledescription", description.to_owned());
        } else if node.role() == Role::Canvas {
            attributes.insert("aria-roledescription", "viewport".to_owned());
        }
        if let Some(placeholder) = node.placeholder() {
            attributes.insert("aria-placeholder", placeholder.to_owned());
        }
        if let Some(shortcut) = node.keyboard_shortcut() {
            attributes.insert("aria-keyshortcuts", shortcut.to_owned());
        }
    }
    // Focusable controls can take DOM focus from assistive technology, which
    // the browser host passes to egui; a focusable separator also exposes its
    // value. `-1` keeps them out of the page's own Tab order.
    if node.supports_action(accesskit::Action::Focus) && role.is_some() {
        attributes.insert("tabindex", "-1".to_owned());
    }
    if node.is_disabled() && role.is_some() {
        attributes.insert("aria-disabled", "true".to_owned());
    }
    if node.is_read_only() && naming == Naming::Value {
        attributes.insert("aria-readonly", "true".to_owned());
    }
    if node.role() == Role::MultilineTextInput {
        attributes.insert("aria-multiline", "true".to_owned());
    }
    if node.is_modal() && matches!(role, Some("dialog" | "alertdialog")) {
        attributes.insert("aria-modal", "true".to_owned());
    }
    if let Some(selected) = node.is_selected()
        && matches!(
            role,
            Some("tab" | "option" | "gridcell" | "row" | "treeitem")
        )
    {
        attributes.insert("aria-selected", selected.to_string());
    }
    if let Some(toggled) = node.toggled() {
        let value = match toggled {
            Toggled::True => "true",
            Toggled::False => "false",
            Toggled::Mixed => "mixed",
        };
        match role {
            Some("button") => {
                attributes.insert("aria-pressed", value.to_owned());
            }
            Some(
                "checkbox" | "switch" | "radio" | "menuitemcheckbox" | "menuitemradio" | "option",
            ) => {
                attributes.insert("aria-checked", value.to_owned());
            }
            _ => {}
        }
    }
    match node.is_expanded() {
        Some(expanded) if role.is_some() => {
            attributes.insert("aria-expanded", expanded.to_string());
        }
        // ARIA requires a combo box's expanded state; unset means collapsed.
        None if role == Some("combobox") => {
            attributes.insert("aria-expanded", "false".to_owned());
        }
        _ => {}
    }
    if let Some(current) = node.aria_current()
        && role.is_some()
    {
        let value = match current {
            AriaCurrent::False => "false",
            AriaCurrent::True => "true",
            AriaCurrent::Page => "page",
            AriaCurrent::Step => "step",
            AriaCurrent::Location => "location",
            AriaCurrent::Date => "date",
            AriaCurrent::Time => "time",
        };
        attributes.insert("aria-current", value.to_owned());
    }
    if let Some(level) = node.level()
        && matches!(role, Some("heading" | "treeitem"))
    {
        attributes.insert("aria-level", level.to_string());
    }
    if role.is_some() {
        if let Some(position) = node.position_in_set() {
            attributes.insert("aria-posinset", position.to_string());
        }
        if let Some(size) = node.size_of_set() {
            attributes.insert("aria-setsize", size.to_string());
        }
    }
    if let Some(orientation) = node.orientation()
        && matches!(
            role,
            Some(
                "separator"
                    | "slider"
                    | "tablist"
                    | "toolbar"
                    | "listbox"
                    | "menubar"
                    | "scrollbar"
            )
        )
    {
        attributes.insert(
            "aria-orientation",
            match orientation {
                Orientation::Horizontal => "horizontal",
                Orientation::Vertical => "vertical",
            }
            .to_owned(),
        );
    }
    if matches!(
        role,
        Some("slider" | "spinbutton" | "progressbar" | "separator")
    ) {
        if let Some(value) = node.numeric_value() {
            attributes.insert("aria-valuenow", number(value));
        }
        if let Some(value) = node.min_numeric_value() {
            attributes.insert("aria-valuemin", number(value));
        }
        if let Some(value) = node.max_numeric_value() {
            attributes.insert("aria-valuemax", number(value));
        }
        if let Some(value) = node.value() {
            attributes.insert("aria-valuetext", value.to_owned());
        }
    }
    match node.live() {
        Some(Live::Polite) if role != Some("status") => {
            attributes.insert("aria-live", "polite".to_owned());
        }
        Some(Live::Assertive) => {
            attributes.insert("aria-live", "assertive".to_owned());
        }
        Some(Live::Off) if role == Some("status") => {
            attributes.insert("aria-live", "off".to_owned());
        }
        _ => {}
    }
    (attributes, text)
}

/// Place a parent's changed children with as few moves as possible. Children
/// that were already there in the longest run of unchanged relative order stay
/// put; the rest are inserted before their next sibling, from last to first,
/// so that sibling is always already in place.
fn place_children(
    parent: Option<&String>,
    before: &[String],
    children: &[String],
    operations: &mut Vec<DomOp>,
) {
    let previous: HashMap<&String, usize> = before
        .iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();
    let indices: Vec<Option<usize>> = children
        .iter()
        .map(|id| previous.get(id).copied())
        .collect();
    let stable = longest_increasing(&indices);
    for (position, child) in children.iter().enumerate().rev() {
        if !stable.contains(&position) {
            operations.push(DomOp::Place {
                id: child.clone(),
                parent: parent.cloned(),
                before: children.get(position + 1).cloned(),
            });
        }
    }
}

/// The positions of a longest strictly increasing subsequence of the present
/// values: the children that keep their relative order.
fn longest_increasing(values: &[Option<usize>]) -> BTreeSet<usize> {
    // `tails[k]` is the position ending the best run of length k + 1.
    let mut tails: Vec<usize> = Vec::new();
    let mut previous: Vec<Option<usize>> = vec![None; values.len()];
    for (position, value) in values.iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        let length = tails.partition_point(|&tail| values[tail].expect("present") < *value);
        previous[position] = length.checked_sub(1).map(|before| tails[before]);
        if length == tails.len() {
            tails.push(position);
        } else {
            tails[length] = position;
        }
    }
    let mut stable = BTreeSet::new();
    let mut cursor = tails.last().copied();
    while let Some(position) = cursor {
        stable.insert(position);
        cursor = previous[position];
    }
    stable
}

/// Format a number without float noise: integers plainly, fractions to four
/// decimal places with trailing zeros removed.
fn number(value: f64) -> String {
    let text = format!("{value:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

/// The operations that turn `previous`'s DOM into `next`'s. Equal trees
/// produce none.
pub fn diff(previous: &AriaTree, next: &AriaTree) -> Vec<DomOp> {
    let mut operations = Vec::new();
    // An element whose tag changes is removed and created again.
    let mut recreated = BTreeSet::new();
    for (id, element) in &next.elements {
        let before = previous.elements.get(id);
        if before.is_some_and(|before| before.tag != element.tag) {
            operations.push(DomOp::Remove { id: id.clone() });
            recreated.insert(id);
        }
        let Some(before) = before.filter(|before| before.tag == element.tag) else {
            operations.push(DomOp::Create {
                id: id.clone(),
                tag: element.tag,
            });
            for (name, value) in &element.attributes {
                operations.push(DomOp::SetAttribute {
                    id: id.clone(),
                    name,
                    value: value.clone(),
                });
            }
            if element.text.is_some() {
                operations.push(DomOp::SetText {
                    id: id.clone(),
                    text: element.text.clone(),
                });
            }
            operations.push(DomOp::SetStyle {
                id: id.clone(),
                style: element.style.clone(),
            });
            continue;
        };
        for (name, value) in &element.attributes {
            if before.attributes.get(name) != Some(value) {
                operations.push(DomOp::SetAttribute {
                    id: id.clone(),
                    name,
                    value: value.clone(),
                });
            }
        }
        for name in before.attributes.keys() {
            if !element.attributes.contains_key(name) {
                operations.push(DomOp::RemoveAttribute {
                    id: id.clone(),
                    name,
                });
            }
        }
        if before.text != element.text {
            operations.push(DomOp::SetText {
                id: id.clone(),
                text: element.text.clone(),
            });
        }
        if before.style != element.style {
            operations.push(DomOp::SetStyle {
                id: id.clone(),
                style: element.style.clone(),
            });
        }
    }
    // Place top-down in the next tree. A parent is then already in its final
    // position, so moving an element never appends it inside its own subtree.
    let mut pending: Vec<(Option<&String>, &[String])> = vec![(None, &next.roots)];
    while let Some((parent, children)) = pending.pop() {
        let before = match parent {
            None => Some(previous.roots.as_slice()),
            Some(parent) => previous
                .elements
                .get(parent)
                .map(|before| before.children.as_slice()),
        };
        // A recreated child is new to its parent.
        let before: Vec<String> = before
            .unwrap_or_default()
            .iter()
            .filter(|child| !recreated.contains(child))
            .cloned()
            .collect();
        if before != children {
            place_children(parent, &before, children, &mut operations);
        }
        for child in children.iter().rev() {
            pending.push((Some(child), &next.elements[child].children));
        }
    }
    for id in previous.elements.keys() {
        if !next.elements.contains_key(id) {
            operations.push(DomOp::Remove { id: id.clone() });
        }
    }
    if previous.active != next.active {
        operations.push(DomOp::Active {
            id: next.active.clone(),
        });
    }
    operations
}

#[cfg(test)]
mod tests;
