use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use stobie_protocol::semantics::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccessKitMismatch {
    MissingNode {
        id: SemanticUiId,
    },
    DuplicateNode {
        id: SemanticUiId,
    },
    Role {
        id: SemanticUiId,
    },
    Name {
        id: SemanticUiId,
    },
    Enabled {
        id: SemanticUiId,
    },
    Selected {
        id: SemanticUiId,
    },
    Description {
        id: SemanticUiId,
    },
    Checked {
        id: SemanticUiId,
    },
    ClickAction {
        id: SemanticUiId,
    },
    AdjustAction {
        id: SemanticUiId,
    },
    CustomActions {
        id: SemanticUiId,
    },
    Bounds {
        id: SemanticUiId,
    },
    InformationalState {
        id: SemanticUiId,
    },
    /// A status lacks an explicit live setting, or another control has one.
    Live {
        id: SemanticUiId,
    },
    /// A result row or thumbnail is not inside a list box.
    Container {
        id: SemanticUiId,
    },
    /// A link's AccessKit URL differs from its semantic value.
    Url {
        id: SemanticUiId,
    },
    /// A dialog is not marked modal for assistive technology.
    Modal {
        id: SemanticUiId,
    },
}

/// Compare the common semantics for Stobie-owned custom controls. Snapshot
/// augmentations such as pane/domain references intentionally have no
/// AccessKit counterpart. Viewport action identities remain application-owned;
/// the audit checks only that a distinct set of AccessKit custom-action
/// transports is advertised, never that their numeric IDs equal action IDs.
pub fn audit_accesskit(
    snapshot: &UiSnapshot,
    update: &egui::accesskit::TreeUpdate,
) -> Vec<AccessKitMismatch> {
    let by_id: BTreeMap<egui::accesskit::NodeId, &egui::accesskit::Node> =
        update.nodes.iter().map(|(id, node)| (*id, node)).collect();
    let parents: BTreeMap<egui::accesskit::NodeId, egui::accesskit::NodeId> = update
        .nodes
        .iter()
        .flat_map(|(id, node)| node.children().iter().map(move |child| (*child, *id)))
        .collect();
    let mut by_author: BTreeMap<&str, Vec<(egui::accesskit::NodeId, &egui::accesskit::Node)>> =
        BTreeMap::new();
    for (id, node) in &update.nodes {
        if let Some(author) = node.author_id() {
            by_author.entry(author).or_default().push((*id, node));
        }
    }
    let mut findings = Vec::new();
    for semantic in snapshot.nodes.iter().filter(|node| {
        matches!(
            node.role,
            UiRole::Button
                | UiRole::NavigationItem
                | UiRole::RadioButton
                | UiRole::ComboBox
                | UiRole::TextInput
                | UiRole::MultilineTextInput
                | UiRole::Slider
                | UiRole::Tab
                | UiRole::Splitter
                | UiRole::Viewport
                | UiRole::ResultRow
                | UiRole::ThumbnailCell
                | UiRole::StatusChip
                | UiRole::Status
                | UiRole::Pane
                | UiRole::Link
                | UiRole::Switch
                | UiRole::Dialog
                | UiRole::AlertDialog
        )
    }) {
        let Some(candidates) = by_author.get(semantic.id.0.as_str()) else {
            findings.push(AccessKitMismatch::MissingNode {
                id: semantic.id.clone(),
            });
            continue;
        };
        if candidates.len() != 1 {
            findings.push(AccessKitMismatch::DuplicateNode {
                id: semantic.id.clone(),
            });
            continue;
        }
        let (node_id, node) = candidates[0];
        if matches!(semantic.role, UiRole::ResultRow | UiRole::ThumbnailCell) {
            let mut ancestor = parents.get(&node_id);
            while let Some(id) = ancestor
                && by_id
                    .get(id)
                    .is_some_and(|node| node.role() != egui::accesskit::Role::ListBox)
            {
                ancestor = parents.get(id);
            }
            if ancestor.is_none() {
                findings.push(AccessKitMismatch::Container {
                    id: semantic.id.clone(),
                });
            }
        }
        let expected_role = match semantic.role {
            UiRole::Button | UiRole::NavigationItem => egui::accesskit::Role::Button,
            UiRole::RadioButton => egui::accesskit::Role::RadioButton,
            UiRole::ComboBox => egui::accesskit::Role::ComboBox,
            UiRole::TextInput => egui::accesskit::Role::TextInput,
            UiRole::MultilineTextInput => egui::accesskit::Role::MultilineTextInput,
            UiRole::Slider => egui::accesskit::Role::Slider,
            UiRole::Tab => egui::accesskit::Role::Tab,
            UiRole::Splitter => egui::accesskit::Role::Splitter,
            UiRole::Viewport => egui::accesskit::Role::Canvas,
            UiRole::ResultRow | UiRole::ThumbnailCell => egui::accesskit::Role::ListBoxOption,
            UiRole::StatusChip => egui::accesskit::Role::Label,
            UiRole::Status => egui::accesskit::Role::Status,
            UiRole::Pane => egui::accesskit::Role::Region,
            UiRole::Link => egui::accesskit::Role::Link,
            UiRole::Switch => egui::accesskit::Role::Switch,
            UiRole::Dialog => egui::accesskit::Role::Dialog,
            UiRole::AlertDialog => egui::accesskit::Role::AlertDialog,
            _ => unreachable!(),
        };
        if node.role() != expected_role {
            findings.push(AccessKitMismatch::Role {
                id: semantic.id.clone(),
            });
        }
        let accessible_name = match semantic.role {
            UiRole::StatusChip => node.value().unwrap_or_default().to_owned(),
            // A status container's text belongs to the labels inside it.
            UiRole::Status if node.label().is_none() => descendant_text(&by_id, node),
            _ => node.label().unwrap_or_default().to_owned(),
        };
        if accessible_name != semantic.name {
            findings.push(AccessKitMismatch::Name {
                id: semantic.id.clone(),
            });
        }
        // Status messages are polite live regions. A readout that changes with
        // the pointer may declare itself off instead; either way it is explicit.
        if (semantic.role == UiRole::Status) != node.live().is_some() {
            findings.push(AccessKitMismatch::Live {
                id: semantic.id.clone(),
            });
        }
        if semantic.role == UiRole::StatusChip
            && (node.supports_action(egui::accesskit::Action::Focus)
                || node.is_selected().is_some()
                || node.toggled().is_some()
                || node.aria_current().is_some()
                || node.live().is_some()
                || node.label().is_some())
        {
            findings.push(AccessKitMismatch::InformationalState {
                id: semantic.id.clone(),
            });
        }
        let expected_description = semantic.disabled_reason.as_ref().map_or_else(
            || semantic.description.clone(),
            |reason| {
                Some(format!(
                    "{}; unavailable: {reason}",
                    semantic.description.as_deref().unwrap_or_default()
                ))
            },
        );
        if node.description().map(ToOwned::to_owned) != expected_description {
            findings.push(AccessKitMismatch::Description {
                id: semantic.id.clone(),
            });
        }
        if node.is_disabled() == semantic.enabled {
            findings.push(AccessKitMismatch::Enabled {
                id: semantic.id.clone(),
            });
        }
        if matches!(
            semantic.role,
            UiRole::Tab | UiRole::Viewport | UiRole::ResultRow | UiRole::ThumbnailCell
        ) && node.is_selected() != Some(semantic.selected)
        {
            findings.push(AccessKitMismatch::Selected {
                id: semantic.id.clone(),
            });
        }
        if semantic.role == UiRole::NavigationItem
            && (node.aria_current()
                != if semantic.selected {
                    Some(egui::accesskit::AriaCurrent::True)
                } else {
                    Some(egui::accesskit::AriaCurrent::False)
                }
                || node.is_selected().is_some())
        {
            findings.push(AccessKitMismatch::Selected {
                id: semantic.id.clone(),
            });
        }
        let expected_toggled = semantic.checked.map(|checked| {
            if checked {
                egui::accesskit::Toggled::True
            } else {
                egui::accesskit::Toggled::False
            }
        });
        // A switch is always on or off.
        if node.toggled() != expected_toggled
            || (semantic.role == UiRole::Switch && semantic.checked.is_none())
        {
            findings.push(AccessKitMismatch::Checked {
                id: semantic.id.clone(),
            });
        }
        let should_click = semantic.enabled
            && matches!(
                semantic.role,
                UiRole::Button
                    | UiRole::NavigationItem
                    | UiRole::RadioButton
                    | UiRole::Tab
                    | UiRole::ResultRow
                    | UiRole::ThumbnailCell
                    | UiRole::Link
                    | UiRole::Switch
            );
        let supports_click = node.supports_action(egui::accesskit::Action::Click);
        if (semantic.role == UiRole::StatusChip && supports_click)
            || (should_click && !supports_click)
            || (matches!(
                semantic.role,
                UiRole::Button | UiRole::NavigationItem | UiRole::Link | UiRole::Switch
            ) && !semantic.enabled
                && supports_click)
        {
            findings.push(AccessKitMismatch::ClickAction {
                id: semantic.id.clone(),
            });
        }
        if matches!(semantic.role, UiRole::Dialog | UiRole::AlertDialog) != node.is_modal() {
            findings.push(AccessKitMismatch::Modal {
                id: semantic.id.clone(),
            });
        }
        if semantic.role == UiRole::Link && node.url() != semantic.value.as_deref() {
            findings.push(AccessKitMismatch::Url {
                id: semantic.id.clone(),
            });
        }
        if matches!(semantic.role, UiRole::Splitter | UiRole::Slider)
            && (!node.supports_action(egui::accesskit::Action::Increment)
                || !node.supports_action(egui::accesskit::Action::Decrement))
        {
            findings.push(AccessKitMismatch::AdjustAction {
                id: semantic.id.clone(),
            });
        }
        if semantic.role == UiRole::Viewport {
            let numeric_ids: BTreeSet<_> = node
                .custom_actions()
                .iter()
                .map(|action| action.id)
                .collect();
            let advertised = if semantic.actions.is_empty() {
                !node.supports_action(egui::accesskit::Action::CustomAction)
                    && node.custom_actions().is_empty()
            } else {
                node.supports_action(egui::accesskit::Action::CustomAction)
                    && node.custom_actions().len() == semantic.actions.len()
                    && numeric_ids.len() == semantic.actions.len()
            };
            if !advertised {
                findings.push(AccessKitMismatch::CustomActions {
                    id: semantic.id.clone(),
                });
            }
        }
        if let Some(bounds) = node.bounds() {
            let accesskit = UiRect {
                min_x: bounds.x0 as f32,
                min_y: bounds.y0 as f32,
                max_x: bounds.x1 as f32,
                max_y: bounds.y1 as f32,
            };
            if !accesskit.contains(semantic.rect, 1.0) || !semantic.rect.contains(accesskit, 1.0) {
                findings.push(AccessKitMismatch::Bounds {
                    id: semantic.id.clone(),
                });
            }
        } else {
            findings.push(AccessKitMismatch::Bounds {
                id: semantic.id.clone(),
            });
        }
    }
    findings
}

/// The text of the labels under `node`, in tree order.
fn descendant_text(
    nodes: &BTreeMap<egui::accesskit::NodeId, &egui::accesskit::Node>,
    node: &egui::accesskit::Node,
) -> String {
    let mut text = Vec::new();
    let mut pending: Vec<_> = node.children().iter().rev().collect();
    while let Some(id) = pending.pop() {
        let Some(child) = nodes.get(id) else {
            continue;
        };
        if child.role() == egui::accesskit::Role::Label {
            text.extend(child.value().or(child.label()));
        } else {
            pending.extend(child.children().iter().rev());
        }
    }
    text.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_actions::TestAction;
    use stobie_core::{DockNodeId, PaneId};

    /// Version 1 of the inspection protocol publishes these exact shapes.
    #[test]
    fn domain_reference_wire_format_is_stable() {
        let cases = [
            (
                DomainReference::Pane(PaneId(3)),
                r#"{"kind":"pane","value":3}"#,
            ),
            (
                DomainReference::DockNode(DockNodeId(9)),
                r#"{"kind":"dock_node","value":9}"#,
            ),
            (
                DomainReference::Result(734_219),
                r#"{"kind":"result","value":734219}"#,
            ),
            (
                DomainReference::Annotation(4),
                r#"{"kind":"annotation","value":4}"#,
            ),
            (
                DomainReference::Thumbnail(TileReference {
                    source: 2,
                    level: 0,
                    x: 7,
                    y: 11,
                }),
                r#"{"kind":"thumbnail","value":{"source":2,"level":0,"x":7,"y":11}}"#,
            ),
            (
                DomainReference::External {
                    namespace: "records".into(),
                    id: "1001".into(),
                },
                r#"{"kind":"external","value":{"namespace":"records","id":"1001"}}"#,
            ),
        ];
        for (reference, json) in cases {
            assert_eq!(serde_json::to_string(&reference).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<DomainReference>(json).unwrap(),
                reference
            );
        }
    }

    fn sample() -> UiSnapshot {
        let root = SemanticUiId::root();
        UiSnapshot {
            frame: 7,
            pixels_per_point: 1.0,
            root: root.clone(),
            nodes: vec![
                UiNode::container(
                    root.clone(),
                    None,
                    UiRole::Application,
                    UiRect {
                        min_x: 0.0,
                        min_y: 0.0,
                        max_x: 800.0,
                        max_y: 600.0,
                    },
                ),
                UiNode {
                    argument: None,
                    value: None,
                    id: SemanticUiId::new("action.undo"),
                    parent: Some(root),
                    role: UiRole::Button,
                    name: "Undo".into(),
                    description: Some("Undo the most recent change".into()),
                    rect: UiRect {
                        min_x: 8.0,
                        min_y: 8.0,
                        max_x: 64.0,
                        max_y: 40.0,
                    },
                    enabled: false,
                    focused: false,
                    selected: false,
                    checked: None,
                    expanded: None,
                    pane: None,
                    domain_reference: None,
                    actions: vec![SemanticActionId::from_action(TestAction::Undo)],
                    text_selectable: false,
                    disabled_reason: Some("History is empty".into()),
                },
            ],
            text: Vec::new(),
            text_audit: Vec::new(),
            text_audit_coverage: None,
            semantic_audit: Vec::new(),
        }
    }

    #[test]
    fn legacy_snapshot_does_not_claim_zero_coverage() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value.as_object_mut().unwrap().remove("text_audit_coverage");
        let snapshot: UiSnapshot = serde_json::from_value(value).unwrap();
        assert!(snapshot.text_audit_coverage.is_none());
    }

    #[test]
    fn snapshot_queries_are_stable_and_bounded() {
        let mut snapshot = sample();
        snapshot.nodes[1].text_selectable = true;
        assert!(snapshot.audit().is_empty());
        assert_eq!(snapshot.by_role(UiRole::Button).count(), 1);
        assert_eq!(snapshot.by_name("Undo").count(), 1);
        assert_eq!(snapshot.by_action(TestAction::Undo).count(), 1);
        assert_eq!(snapshot.selectable_text().count(), 1);
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap()["nodes"][1]["actions"],
            serde_json::json!(["undo"])
        );
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap()["nodes"][1]["text_selectable"],
            serde_json::json!(true)
        );
    }

    #[test]
    fn audit_rejects_duplicate_missing_parent_and_invalid_geometry() {
        let mut snapshot = sample();
        let mut duplicate = snapshot.nodes[1].clone();
        duplicate.parent = Some(SemanticUiId::new("missing"));
        duplicate.rect.max_x = f32::NAN;
        snapshot.nodes.push(duplicate);
        let findings = snapshot.audit();
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, SemanticAuditFinding::DuplicateId { .. }))
        );
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, SemanticAuditFinding::MissingParent { .. }))
        );
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, SemanticAuditFinding::InvalidRect { .. }))
        );
    }

    /// A status, a status chip and a result row with their AccessKit nodes.
    fn live_and_list_fixture(
        status_live: Option<egui::accesskit::Live>,
        chip_live: Option<egui::accesskit::Live>,
        list_box: bool,
    ) -> (UiSnapshot, egui::accesskit::TreeUpdate) {
        use egui::accesskit::{Action, Node, NodeId, Rect, Role, Tree, TreeId, TreeUpdate};
        let rect = UiRect {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 100.0,
            max_y: 20.0,
        };
        let bounds = Rect {
            x0: 0.0,
            y0: 0.0,
            x1: 100.0,
            y1: 20.0,
        };
        let root = SemanticUiId::root();
        let mut root_node = UiNode::container(root.clone(), None, UiRole::Application, rect);
        root_node.rect.max_y = 100.0;
        let mut status = UiNode::container(
            SemanticUiId::new("status"),
            Some(root.clone()),
            UiRole::Status,
            rect,
        );
        status.name = "Saved".into();
        let mut chip = UiNode::container(
            SemanticUiId::new("chip"),
            Some(root.clone()),
            UiRole::StatusChip,
            rect,
        );
        chip.name = "Active".into();
        let mut row = UiNode::container(
            SemanticUiId::new("row"),
            Some(root.clone()),
            UiRole::ResultRow,
            rect,
        );
        row.name = "Row".into();
        let snapshot = UiSnapshot {
            frame: 1,
            pixels_per_point: 1.0,
            root,
            nodes: vec![root_node, status, chip, row],
            ..Default::default()
        };
        let mut status = Node::new(Role::Status);
        status.set_author_id("status");
        status.set_label("Saved");
        status.set_bounds(bounds);
        if let Some(live) = status_live {
            status.set_live(live);
        }
        let mut chip = Node::new(Role::Label);
        chip.set_author_id("chip");
        chip.set_value("Active");
        chip.set_bounds(bounds);
        if let Some(live) = chip_live {
            chip.set_live(live);
        }
        let mut row = Node::new(Role::ListBoxOption);
        row.set_author_id("row");
        row.set_label("Row");
        row.set_selected(false);
        row.add_action(Action::Click);
        row.set_bounds(bounds);
        let mut list = Node::new(if list_box {
            Role::ListBox
        } else {
            Role::GenericContainer
        });
        list.set_children(vec![NodeId(4)]);
        let mut window = Node::new(Role::Window);
        window.set_children(vec![NodeId(2), NodeId(3), NodeId(5)]);
        let update = TreeUpdate {
            nodes: vec![
                (NodeId(1), window),
                (NodeId(2), status),
                (NodeId(3), chip),
                (NodeId(4), row),
                (NodeId(5), list),
            ],
            tree: Some(Tree::new(NodeId(1))),
            tree_id: TreeId::ROOT,
            focus: NodeId(1),
        };
        (snapshot, update)
    }

    #[test]
    fn accesskit_audit_requires_live_statuses_silent_chips_and_list_boxes() {
        use egui::accesskit::Live;
        let (snapshot, update) = live_and_list_fixture(Some(Live::Polite), None, true);
        assert_eq!(audit_accesskit(&snapshot, &update), []);
        let (snapshot, update) = live_and_list_fixture(Some(Live::Off), None, true);
        assert_eq!(
            audit_accesskit(&snapshot, &update),
            [],
            "a readout may opt out"
        );

        let (snapshot, update) = live_and_list_fixture(None, Some(Live::Polite), false);
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.contains(&AccessKitMismatch::Live {
            id: SemanticUiId::new("status")
        }));
        assert!(findings.contains(&AccessKitMismatch::Live {
            id: SemanticUiId::new("chip")
        }));
        assert!(findings.contains(&AccessKitMismatch::Container {
            id: SemanticUiId::new("row")
        }));
    }
}
