//! The semantic snapshot: stable identities, roles, geometry and state of one
//! completed UI pass, as inspection publishes it.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use stobie_core::{DockNodeId, PaneId};

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

#[cfg(feature = "emath")]
impl From<emath::Rect> for UiRect {
    fn from(rect: emath::Rect) -> Self {
        Self {
            min_x: rect.min.x,
            min_y: rect.min.y,
            max_x: rect.max.x,
            max_y: rect.max.y,
        }
    }
}
#[cfg(feature = "emath")]
impl From<UiRect> for emath::Rect {
    fn from(rect: UiRect) -> Self {
        emath::Rect::from_min_max(
            emath::pos2(rect.min_x, rect.min_y),
            emath::pos2(rect.max_x, rect.max_y),
        )
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

    pub fn viewport(pane: PaneId) -> Self {
        Self::new(format!("pane.{}.viewport", pane.0))
    }

    pub fn tab(pane: PaneId) -> Self {
        Self::new(format!("stobie.dock.tab.{}", pane.0))
    }

    pub fn splitter(node: DockNodeId) -> Self {
        Self::new(format!("stobie.dock.splitter.{}", node.0))
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
    NavigationItem,
    RadioButton,
    ComboBox,
    TextInput,
    MultilineTextInput,
    Slider,
    Tab,
    Splitter,
    Pane,
    Viewport,
    ScrollArea,
    ResultRow,
    ThumbnailCell,
    Status,
    /// Informational measured Label; never a button or live region.
    StatusChip,
    Section,
}

/// The domain identity a semantic node presents, published to agents as
/// `{"kind": ..., "value": ...}`.
///
/// `Result`, `Annotation` and `Thumbnail` are image-application kinds that
/// version 1 of the inspection protocol already publishes; they carry the
/// image IDs' plain values, so the framework needs no image types. Other
/// applications use [`DomainReference::External`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DomainReference {
    Pane(PaneId),
    DockNode(DockNodeId),
    /// An analysis result's ID.
    Result(u64),
    /// A durable annotation's ID.
    Annotation(u64),
    /// A thumbnail's tile.
    Thumbnail(TileReference),
    /// An application-owned domain identity, independent of labels and row positions.
    External {
        namespace: String,
        id: String,
    },
}

/// A tile's identity in a [`DomainReference::Thumbnail`]: its source, pyramid
/// level and column and row at that level.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct TileReference {
    pub source: u32,
    pub level: u8,
    pub x: u32,
    pub y: u32,
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
    #[serde(default, skip_serializing_if = "is_false")]
    pub text_selectable: bool,
    pub disabled_reason: Option<String>,
    /// A text field's current text, published by hosts for framework text
    /// fields while AccessKit is enabled; omitted otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
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
            text_selectable: false,
            disabled_reason: None,
            value: None,
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

    pub fn selectable_text(&self) -> impl Iterator<Item = &UiNode> {
        self.nodes.iter().filter(|node| node.text_selectable)
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
