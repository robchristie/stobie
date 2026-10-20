//! Observation wait conditions, evaluated against completed observations.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use stobie_protocol::{CompletedObservation, InspectionSelector, SemanticUiId, UiNode, UiRole};

use crate::ClientError;

/// What a wait waits for. Unknown fields are rejected, so a misspelt field
/// fails rather than being ignored. Node conditions need exactly one match.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    /// A node matches.
    Present {
        #[serde(default)]
        selector: InspectionSelector,
    },
    /// No node matches.
    Absent {
        #[serde(default)]
        selector: InspectionSelector,
    },
    /// The node is rendered enabled.
    Enabled {
        #[serde(default)]
        selector: InspectionSelector,
    },
    /// The application makes the capability available, whether or not its
    /// control is rendered.
    CapabilityEnabled {
        #[serde(default)]
        selector: InspectionSelector,
    },
    Focused {
        #[serde(default)]
        selector: InspectionSelector,
    },
    Selected {
        #[serde(default)]
        selector: InspectionSelector,
    },
    Checked {
        #[serde(default)]
        selector: InspectionSelector,
        value: bool,
    },
    /// The node's name is exactly `value`.
    Name {
        #[serde(default)]
        selector: InspectionSelector,
        value: String,
    },
    /// The node's value (a text field's text, or a combo box's selected
    /// option label) is exactly `value`.
    Value {
        #[serde(default)]
        selector: InspectionSelector,
        value: String,
    },
    /// A status node's text is exactly `value`.
    Status {
        #[serde(default)]
        selector: InspectionSelector,
        value: String,
    },
    /// The selected IDs among matching nodes differ from `previous`, or from
    /// the first observation.
    SelectionChanged {
        #[serde(default)]
        selector: InspectionSelector,
        #[serde(default)]
        previous: Option<Vec<SemanticUiId>>,
    },
    /// An application fact equals `value`.
    Fact {
        key: String,
        value: serde_json::Value,
    },
    /// Semantic and text audits are empty, with coverage retained.
    AuditsClear {},
    /// A presented content text's published excerpt contains `value`, such
    /// as a reply that has arrived. Excerpts are bounded, so text beyond
    /// the first `CONTENT_EXCERPT_CHARS` characters of a long text never
    /// matches, and withheld text never does.
    Content { value: String },
}

impl Condition {
    /// Every condition name, for `capabilities`.
    pub const NAMES: [&'static str; 14] = [
        "present",
        "absent",
        "enabled",
        "capability_enabled",
        "focused",
        "selected",
        "checked",
        "name",
        "value",
        "status",
        "selection_changed",
        "fact",
        "audits_clear",
        "content",
    ];

    /// Parse a condition object, explaining what is wrong with a bad one.
    pub fn parse(value: serde_json::Value) -> Result<Self, ClientError> {
        serde_json::from_value(value).map_err(|error| {
            ClientError::usage(format!(
                "invalid wait condition: {error}; conditions are {}",
                Self::NAMES.join(", ")
            ))
        })
    }

    /// The selector for conditions that need one node.
    pub(crate) fn node_selector(&self) -> Option<InspectionSelector> {
        match self {
            Self::Present { .. }
            | Self::Absent { .. }
            | Self::CapabilityEnabled { .. }
            | Self::SelectionChanged { .. }
            | Self::Fact { .. }
            | Self::AuditsClear {}
            | Self::Content { .. } => None,
            Self::Status { selector, .. } => Some(InspectionSelector {
                role: Some(UiRole::Status),
                ..selector.clone()
            }),
            Self::Enabled { selector }
            | Self::Focused { selector }
            | Self::Selected { selector }
            | Self::Checked { selector, .. }
            | Self::Name { selector, .. }
            | Self::Value { selector, .. } => Some(selector.clone()),
        }
    }
}

/// The one node a condition names, or an `ambiguous` error listing them.
pub(crate) fn one_node<'a>(
    observation: &'a CompletedObservation,
    selector: &InspectionSelector,
) -> Result<Option<&'a UiNode>, ClientError> {
    let matches: Vec<&UiNode> = observation
        .snapshot
        .nodes
        .iter()
        .filter(|node| selector.matches_node(node))
        .collect();
    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(node)),
        nodes => Err(ClientError::new(
            "ambiguous",
            format!(
                "{} nodes match: {}",
                nodes.len(),
                nodes
                    .iter()
                    .take(8)
                    .map(|node| node.id.0.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            "add id, pane or domain to the selector",
        )),
    }
}

/// The selected IDs among nodes that match `selector`.
pub(crate) fn selected_ids(
    observation: &CompletedObservation,
    selector: &InspectionSelector,
) -> BTreeSet<SemanticUiId> {
    observation
        .snapshot
        .nodes
        .iter()
        .filter(|node| node.selected && selector.matches_node(node))
        .map(|node| node.id.clone())
        .collect()
}
