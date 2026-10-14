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

/// Prefix of every element ID the layer creates, kept apart from page IDs.
pub const ARIA_ID_PREFIX: &str = "stobie-a11y:";

/// One element of the projected layer.
#[derive(Clone, Debug, PartialEq)]
pub struct AriaElement {
    /// The AccessKit node this element presents, for routing actions back.
    pub node: NodeId,
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
}

/// One DOM change. Applying a diff's operations in order turns the previous
/// projection's DOM into the next one's.
#[derive(Clone, Debug, PartialEq)]
pub enum DomOp {
    Create {
        id: String,
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
        scale,
        tree: &mut tree,
        ids: HashMap::new(),
        used: BTreeSet::new(),
    };
    for child in root_node.children() {
        projection.visit(*child, None, None);
    }
    // Relations name other nodes, so resolve them once every element exists.
    let ids = projection.ids;
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
    tree
}

struct Projection<'a> {
    nodes: &'a HashMap<NodeId, &'a Node>,
    scale: f32,
    tree: &'a mut AriaTree,
    ids: HashMap<NodeId, String>,
    used: BTreeSet<String>,
}

impl Projection<'_> {
    /// Visit `id` under the nearest kept ancestor `parent`, whose absolute
    /// origin (in points) is `origin`.
    fn visit(&mut self, id: NodeId, parent: Option<&str>, origin: Option<(f64, f64)>) {
        let Some(node) = self.nodes.get(&id).copied() else {
            return;
        };
        if node.is_hidden() {
            return;
        }
        if is_transparent(node) {
            for child in node.children() {
                self.visit(*child, parent, origin);
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
        let style = format!(
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
        let (attributes, text) = presentation(node, contains_text);
        self.tree.elements.insert(
            element_id.clone(),
            AriaElement {
                node: id,
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
            self.visit(*child, Some(&element_id), Some((left, top)));
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
    if node.is_disabled() && role.is_some() {
        attributes.insert("aria-disabled", "true".to_owned());
    }
    if node.is_read_only() && naming == Naming::Value {
        attributes.insert("aria-readonly", "true".to_owned());
    }
    if node.role() == Role::MultilineTextInput {
        attributes.insert("aria-multiline", "true".to_owned());
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
    for (id, element) in &next.elements {
        let Some(before) = previous.elements.get(id) else {
            operations.push(DomOp::Create { id: id.clone() });
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
        if before != Some(children) {
            place_children(
                parent,
                before.unwrap_or_default(),
                children,
                &mut operations,
            );
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
