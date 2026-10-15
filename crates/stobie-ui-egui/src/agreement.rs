//! Semantic agreement between the inspection snapshot and the ARIA layer.
//!
//! Agents see an application through two views: the inspection snapshot and,
//! in the browser, the hidden ARIA layer that the host projects from egui's
//! AccessKit tree. Both reduce to the same [`AgreementNode`] per semantic ID:
//! ARIA role, name, enabled, focused, selected, checked, expanded and value.
//! [`semantic_agreement`] compares them and reports every disagreement, so a
//! test can fail when the views drift apart. The projection is the layer's
//! own ([`crate::aria::project`]), so it checks what browser agents read.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::aria::{self, AriaElement, AriaTree, SEMANTIC_ID_ATTRIBUTE};
use crate::{SemanticUiId, UiNode, UiRole, UiSnapshot};

/// What both views say about one identified control.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgreementNode {
    /// The ARIA role: `textbox` for a text field's `input` or `textarea`.
    pub role: String,
    pub name: String,
    pub enabled: bool,
    pub focused: bool,
    /// Selection: `aria-selected`, or `aria-current` for a navigation item.
    pub selected: Option<bool>,
    /// `aria-pressed` or `aria-checked`.
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    /// A text field's text.
    pub value: Option<String>,
}

/// The property two views disagree on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgreementField {
    /// The snapshot presents a control that the layer does not.
    Missing,
    Role,
    Name,
    Enabled,
    Focused,
    Selected,
    Checked,
    Expanded,
    Value,
}

/// One disagreement, with each view's account of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disagreement {
    pub id: SemanticUiId,
    pub field: AgreementField,
    pub snapshot: String,
    pub layer: String,
}

/// The ARIA role the layer gives a semantic role, or `None` for roles the
/// layer flattens or presents as plain text.
pub fn expected_aria_role(role: UiRole) -> Option<&'static str> {
    Some(match role {
        UiRole::Button | UiRole::NavigationItem => "button",
        UiRole::RadioButton => "radio",
        UiRole::ComboBox => "combobox",
        UiRole::TextInput | UiRole::MultilineTextInput => "textbox",
        UiRole::Slider => "slider",
        UiRole::Tab => "tab",
        UiRole::Splitter => "separator",
        UiRole::Pane => "region",
        UiRole::Viewport => "group",
        UiRole::ResultRow | UiRole::ThumbnailCell => "option",
        UiRole::Status => "status",
        UiRole::Application
        | UiRole::ApplicationBar
        | UiRole::Toolbar
        | UiRole::ScrollArea
        | UiRole::StatusChip
        | UiRole::Section => return None,
    })
}

/// The snapshot's account of every node with an ARIA counterpart.
pub fn project_snapshot(snapshot: &UiSnapshot) -> BTreeMap<SemanticUiId, AgreementNode> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| Some((node.id.clone(), snapshot_node(node)?)))
        .collect()
}

fn snapshot_node(node: &UiNode) -> Option<AgreementNode> {
    let role = expected_aria_role(node.role)?;
    Some(AgreementNode {
        role: role.to_owned(),
        name: node.name.clone(),
        enabled: node.enabled,
        focused: node.focused,
        selected: matches!(
            node.role,
            UiRole::Tab | UiRole::ResultRow | UiRole::ThumbnailCell | UiRole::NavigationItem
        )
        .then_some(node.selected),
        checked: node.checked,
        expanded: match node.role {
            // ARIA requires a combo box's state; unset means collapsed.
            UiRole::ComboBox => Some(node.expanded.unwrap_or(false)),
            _ => node.expanded,
        },
        value: node.value.clone(),
    })
}

/// The layer's account of every element that carries a semantic ID.
pub fn project_layer(tree: &AriaTree) -> BTreeMap<SemanticUiId, AgreementNode> {
    tree.elements
        .iter()
        .filter_map(|(element_id, element)| {
            let id = element.attributes.get(SEMANTIC_ID_ATTRIBUTE)?;
            Some((
                SemanticUiId::new(id.clone()),
                layer_node(tree, element_id, element),
            ))
        })
        .collect()
}

fn layer_node(tree: &AriaTree, element_id: &str, element: &AriaElement) -> AgreementNode {
    let attribute = |name: &str| element.attributes.get(name).map(String::as_str);
    let flag = |name: &str| attribute(name).map(|value| value == "true");
    let text_field = element.tag != "div";
    let role = if text_field {
        "textbox"
    } else {
        attribute("role").unwrap_or_default()
    };
    // A text field and a combo box name themselves with `aria-label` and hold
    // their value as text; a status without a label is named by its text.
    let name = attribute("aria-label")
        .map(ToOwned::to_owned)
        .or_else(|| (!text_field && role != "combobox").then(|| text_of(tree, element)))
        .unwrap_or_default();
    AgreementNode {
        role: role.to_owned(),
        name,
        enabled: attribute("aria-disabled") != Some("true"),
        focused: tree.active.as_deref() == Some(element_id),
        selected: flag("aria-selected")
            .or_else(|| attribute("aria-current").map(|current| current != "false")),
        checked: flag("aria-pressed").or_else(|| flag("aria-checked")),
        expanded: flag("aria-expanded"),
        value: text_field.then(|| element.text.clone().unwrap_or_default()),
    }
}

/// An element's own text, or else its descendants' text joined by spaces.
fn text_of(tree: &AriaTree, element: &AriaElement) -> String {
    if let Some(text) = &element.text {
        return text.clone();
    }
    let mut text = Vec::new();
    let mut pending: Vec<&String> = element.children.iter().rev().collect();
    while let Some(id) = pending.pop() {
        let Some(child) = tree.elements.get(id) else {
            continue;
        };
        match &child.text {
            Some(content) => text.push(content.clone()),
            None => pending.extend(child.children.iter().rev()),
        }
    }
    text.join(" ")
}

/// Compare the two accounts. Every snapshot node with an ARIA counterpart
/// must be presented with the same role, name, enabled and focused state,
/// selection, checked and expanded state; values are compared where both
/// views have one. Layer-only elements, such as content labels, are not
/// reported.
pub fn compare(
    snapshot: &BTreeMap<SemanticUiId, AgreementNode>,
    layer: &BTreeMap<SemanticUiId, AgreementNode>,
) -> Vec<Disagreement> {
    let mut disagreements = Vec::new();
    for (id, expected) in snapshot {
        let mut report = |field, snapshot: String, layer: String| {
            disagreements.push(Disagreement {
                id: id.clone(),
                field,
                snapshot,
                layer,
            });
        };
        let Some(actual) = layer.get(id) else {
            report(
                AgreementField::Missing,
                expected.role.clone(),
                String::new(),
            );
            continue;
        };
        let text = |value: &dyn std::fmt::Debug| format!("{value:?}");
        if expected.role != actual.role {
            report(
                AgreementField::Role,
                expected.role.clone(),
                actual.role.clone(),
            );
        }
        if expected.name != actual.name {
            report(
                AgreementField::Name,
                expected.name.clone(),
                actual.name.clone(),
            );
        }
        if expected.enabled != actual.enabled {
            report(
                AgreementField::Enabled,
                text(&expected.enabled),
                text(&actual.enabled),
            );
        }
        if expected.focused != actual.focused {
            report(
                AgreementField::Focused,
                text(&expected.focused),
                text(&actual.focused),
            );
        }
        if expected.selected.is_some() && expected.selected != actual.selected {
            report(
                AgreementField::Selected,
                text(&expected.selected),
                text(&actual.selected),
            );
        }
        if expected.checked != actual.checked {
            report(
                AgreementField::Checked,
                text(&expected.checked),
                text(&actual.checked),
            );
        }
        if expected.expanded != actual.expanded {
            report(
                AgreementField::Expanded,
                text(&expected.expanded),
                text(&actual.expanded),
            );
        }
        if let (Some(expected), Some(actual)) = (&expected.value, &actual.value)
            && expected != actual
        {
            report(AgreementField::Value, expected.clone(), actual.clone());
        }
    }
    disagreements
}

/// Compare a completed pass's snapshot with the ARIA layer projected from the
/// same pass's AccessKit tree.
pub fn semantic_agreement(
    snapshot: &UiSnapshot,
    update: &egui::accesskit::TreeUpdate,
) -> Vec<Disagreement> {
    compare(
        &project_snapshot(snapshot),
        &project_layer(&aria::project(update, 1.0)),
    )
}

#[cfg(test)]
mod tests;
