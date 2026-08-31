use std::collections::{BTreeMap, BTreeSet};

use polyorama_core::{AnnotationId, DockNodeId, PaneId, ResultId, TileKey};
use serde::{Deserialize, Serialize};

use crate::{ActionKey, TextAuditCoverage, TextAuditFinding, TextLayoutObservation};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UiRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl UiRect {
    pub fn is_finite(self) -> bool {
        [self.min_x, self.min_y, self.max_x, self.max_y]
            .into_iter()
            .all(f32::is_finite)
    }

    pub fn is_positive(self) -> bool {
        self.is_finite() && self.max_x > self.min_x && self.max_y > self.min_y
    }

    pub fn contains(self, other: Self, tolerance: f32) -> bool {
        other.min_x >= self.min_x - tolerance
            && other.min_y >= self.min_y - tolerance
            && other.max_x <= self.max_x + tolerance
            && other.max_y <= self.max_y + tolerance
    }
}

impl From<egui::Rect> for UiRect {
    fn from(rect: egui::Rect) -> Self {
        Self {
            min_x: rect.min.x,
            min_y: rect.min.y,
            max_x: rect.max.x,
            max_y: rect.max.y,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticUiId(pub String);

impl Default for SemanticUiId {
    fn default() -> Self {
        Self::root()
    }
}

/// Stable serialised action identity retained by semantic snapshots.
///
/// Live controls remain typed by the application's [`ActionKey`]. Snapshots
/// intentionally retain only its stable identity so diagnostic consumers do
/// not need the originating application's Rust enum.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SemanticActionId(pub String);

impl SemanticActionId {
    pub fn from_action<A: ActionKey>(action: A) -> Self {
        Self(action.stable_id().to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SemanticUiId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn root() -> Self {
        Self::new("application")
    }

    pub fn pane(pane: PaneId) -> Self {
        Self::new(format!("pane.{}", pane.0))
    }

    pub fn tab(pane: PaneId) -> Self {
        Self::new(format!("polyorama.dock.tab.{}", pane.0))
    }

    pub fn splitter(node: DockNodeId) -> Self {
        Self::new(format!("polyorama.dock.splitter.{}", node.0))
    }
}

impl From<String> for SemanticUiId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiRole {
    Application,
    ApplicationBar,
    Toolbar,
    Button,
    RadioButton,
    ComboBox,
    Slider,
    Tab,
    Splitter,
    Pane,
    Viewport,
    ScrollArea,
    ResultRow,
    ThumbnailCell,
    Status,
    Section,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DomainReference {
    Pane(PaneId),
    DockNode(DockNodeId),
    Result(ResultId),
    Annotation(AnnotationId),
    Thumbnail(TileKey),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiNode {
    pub id: SemanticUiId,
    pub parent: Option<SemanticUiId>,
    pub role: UiRole,
    pub name: String,
    pub description: Option<String>,
    pub rect: UiRect,
    pub enabled: bool,
    pub focused: bool,
    pub selected: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub pane: Option<PaneId>,
    pub domain_reference: Option<DomainReference>,
    pub actions: Vec<SemanticActionId>,
    pub disabled_reason: Option<String>,
}

impl UiNode {
    pub fn container(
        id: SemanticUiId,
        parent: Option<SemanticUiId>,
        role: UiRole,
        rect: UiRect,
    ) -> Self {
        Self {
            id,
            parent,
            role,
            name: String::new(),
            description: None,
            rect,
            enabled: true,
            focused: false,
            selected: false,
            checked: None,
            expanded: None,
            pane: None,
            domain_reference: None,
            actions: Vec::new(),
            disabled_reason: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UiSnapshot {
    pub frame: u64,
    pub pixels_per_point: f32,
    pub root: SemanticUiId,
    pub nodes: Vec<UiNode>,
    pub text: Vec<TextLayoutObservation>,
    pub text_audit: Vec<TextAuditFinding>,
    /// None means coverage was not collected (including older snapshots).
    #[serde(default)]
    pub text_audit_coverage: Option<TextAuditCoverage>,
    pub semantic_audit: Vec<SemanticAuditFinding>,
}

impl UiSnapshot {
    pub fn node(&self, id: &SemanticUiId) -> Option<&UiNode> {
        self.nodes.iter().find(|node| &node.id == id)
    }

    pub fn by_role(&self, role: UiRole) -> impl Iterator<Item = &UiNode> {
        self.nodes.iter().filter(move |node| node.role == role)
    }

    pub fn by_name<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a UiNode> {
        self.nodes.iter().filter(move |node| node.name == name)
    }

    pub fn by_action<A: ActionKey>(&self, action: A) -> impl Iterator<Item = &UiNode> {
        let action = action.stable_id();
        self.nodes
            .iter()
            .filter(move |node| node.actions.iter().any(|candidate| candidate.0 == action))
    }

    pub fn in_pane(&self, pane: PaneId) -> impl Iterator<Item = &UiNode> {
        self.nodes
            .iter()
            .filter(move |node| node.pane == Some(pane))
    }

    pub fn by_domain<'a>(
        &'a self,
        reference: &'a DomainReference,
    ) -> impl Iterator<Item = &'a UiNode> {
        self.nodes
            .iter()
            .filter(move |node| node.domain_reference.as_ref() == Some(reference))
    }

    pub fn audit(&self) -> Vec<SemanticAuditFinding> {
        let mut findings = Vec::new();
        let mut ids = BTreeSet::new();
        let Some(root) = self.node(&self.root) else {
            return vec![SemanticAuditFinding::MissingRoot {
                id: self.root.clone(),
            }];
        };
        if !root.rect.is_positive() {
            findings.push(SemanticAuditFinding::InvalidRect {
                id: root.id.clone(),
            });
        }
        for node in &self.nodes {
            if !ids.insert(node.id.clone()) {
                findings.push(SemanticAuditFinding::DuplicateId {
                    id: node.id.clone(),
                });
            }
            if !node.rect.is_positive() {
                findings.push(SemanticAuditFinding::InvalidRect {
                    id: node.id.clone(),
                });
            } else if !root.rect.contains(node.rect, 1.0) {
                findings.push(SemanticAuditFinding::OutsideRoot {
                    id: node.id.clone(),
                });
            }
            if let Some(parent) = &node.parent
                && self.node(parent).is_none()
            {
                findings.push(SemanticAuditFinding::MissingParent {
                    node: node.id.clone(),
                    parent: parent.clone(),
                });
            }
            if node.enabled && node.disabled_reason.is_some() {
                findings.push(SemanticAuditFinding::EnabledWithDisabledReason {
                    id: node.id.clone(),
                });
            }
        }
        findings
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SemanticAuditFinding {
    MissingRoot {
        id: SemanticUiId,
    },
    DuplicateId {
        id: SemanticUiId,
    },
    InvalidRect {
        id: SemanticUiId,
    },
    OutsideRoot {
        id: SemanticUiId,
    },
    MissingParent {
        node: SemanticUiId,
        parent: SemanticUiId,
    },
    EnabledWithDisabledReason {
        id: SemanticUiId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AccessKitMismatch {
    MissingNode { id: SemanticUiId },
    DuplicateNode { id: SemanticUiId },
    Role { id: SemanticUiId },
    Name { id: SemanticUiId },
    Enabled { id: SemanticUiId },
    Selected { id: SemanticUiId },
    Description { id: SemanticUiId },
    Checked { id: SemanticUiId },
    ClickAction { id: SemanticUiId },
    AdjustAction { id: SemanticUiId },
    Bounds { id: SemanticUiId },
}

/// Compare the common semantics for Polyorama-owned custom controls. Snapshot
/// augmentations such as pane/domain references and action IDs intentionally
/// have no AccessKit counterpart.
pub fn audit_accesskit(
    snapshot: &UiSnapshot,
    update: &egui::accesskit::TreeUpdate,
) -> Vec<AccessKitMismatch> {
    let mut by_author: BTreeMap<&str, Vec<&egui::accesskit::Node>> = BTreeMap::new();
    for (_, node) in &update.nodes {
        if let Some(author) = node.author_id() {
            by_author.entry(author).or_default().push(node);
        }
    }
    let mut findings = Vec::new();
    for semantic in snapshot.nodes.iter().filter(|node| {
        matches!(
            node.role,
            UiRole::Button
                | UiRole::RadioButton
                | UiRole::ComboBox
                | UiRole::Slider
                | UiRole::Tab
                | UiRole::Splitter
                | UiRole::ResultRow
                | UiRole::ThumbnailCell
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
        let node = candidates[0];
        let expected_role = match semantic.role {
            UiRole::Button => egui::accesskit::Role::Button,
            UiRole::RadioButton => egui::accesskit::Role::RadioButton,
            UiRole::ComboBox => egui::accesskit::Role::ComboBox,
            UiRole::Slider => egui::accesskit::Role::Slider,
            UiRole::Tab => egui::accesskit::Role::Tab,
            UiRole::Splitter => egui::accesskit::Role::Splitter,
            UiRole::ResultRow | UiRole::ThumbnailCell => egui::accesskit::Role::ListBoxOption,
            _ => unreachable!(),
        };
        if node.role() != expected_role {
            findings.push(AccessKitMismatch::Role {
                id: semantic.id.clone(),
            });
        }
        if node.label().unwrap_or_default() != semantic.name {
            findings.push(AccessKitMismatch::Name {
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
            UiRole::Tab | UiRole::ResultRow | UiRole::ThumbnailCell
        ) && node.is_selected() != Some(semantic.selected)
        {
            findings.push(AccessKitMismatch::Selected {
                id: semantic.id.clone(),
            });
        }
        if semantic.role == UiRole::RadioButton {
            let expected = semantic.checked.map(|checked| {
                if checked {
                    egui::accesskit::Toggled::True
                } else {
                    egui::accesskit::Toggled::False
                }
            });
            if node.toggled() != expected {
                findings.push(AccessKitMismatch::Checked {
                    id: semantic.id.clone(),
                });
            }
        }
        let should_click = semantic.enabled
            && matches!(
                semantic.role,
                UiRole::Button
                    | UiRole::RadioButton
                    | UiRole::Tab
                    | UiRole::ResultRow
                    | UiRole::ThumbnailCell
            );
        let supports_click = node.supports_action(egui::accesskit::Action::Click);
        if (should_click && !supports_click)
            || (semantic.role == UiRole::Button && !semantic.enabled && supports_click)
        {
            findings.push(AccessKitMismatch::ClickAction {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_actions::TestAction;

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
        let snapshot = sample();
        assert!(snapshot.audit().is_empty());
        assert_eq!(snapshot.by_role(UiRole::Button).count(), 1);
        assert_eq!(snapshot.by_name("Undo").count(), 1);
        assert_eq!(snapshot.by_action(TestAction::Undo).count(), 1);
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap()["nodes"][1]["actions"],
            serde_json::json!(["undo"])
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
}
