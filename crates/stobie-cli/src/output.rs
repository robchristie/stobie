//! Output: a compact text outline by default, or one JSON object with fixed
//! field order.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;
use stobie_client::protocol::{
    ArgumentContract, Availability, CapabilityDescription, CompletedObservation, DomainReference,
    ObservationValue, PaneId, SET_TEXT_CAPABILITY, SemanticUiId, UiNode, UiRole,
};

/// Longest name or value shown in text output, in characters.
const TEXT_LIMIT: usize = 80;

/// Shorten `text` to `TEXT_LIMIT` characters on one line, marking a cut.
pub fn clip(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| {
            if c == '\n' {
                '⏎'
            } else if c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    match flat.char_indices().nth(TEXT_LIMIT) {
        Some((index, _)) => format!("{}…", &flat[..index]),
        None => flat,
    }
}

pub fn role_name(role: UiRole) -> String {
    serde_json::to_value(role)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

/// A capability's argument in short form: `text`, `bool`, `number 0..1`,
/// `integer 1..9` or `choice a|b|c`.
pub fn argument_summary(contract: &ArgumentContract) -> Option<String> {
    Some(match contract {
        ArgumentContract::None => return None,
        ArgumentContract::Text { max_length: None } => "text".into(),
        ArgumentContract::Text {
            max_length: Some(limit),
        } => format!("text ≤{limit}"),
        ArgumentContract::Boolean => "bool".into(),
        ArgumentContract::Number {
            minimum,
            maximum,
            integer,
        } => format!(
            "{} {minimum}..{maximum}",
            if *integer { "integer" } else { "number" }
        ),
        ArgumentContract::Choice { options } => format!(
            "choice {}",
            options
                .iter()
                .map(|option| option.id.as_str())
                .collect::<Vec<_>>()
                .join("|")
        ),
    })
}

/// One capability as an outline annotation: its ID (or `set-text`), its
/// argument, and its availability unless enabled.
pub fn capability_tag(capability: &CapabilityDescription) -> String {
    let mut tag = if capability.capability == SET_TEXT_CAPABILITY {
        "set-text".to_owned()
    } else {
        capability.capability.clone()
    };
    if capability.capability != SET_TEXT_CAPABILITY
        && let Some(argument) = argument_summary(&capability.arguments)
    {
        let _ = write!(tag, ": {argument}");
    }
    if !capability.semantic_invocable {
        tag.push_str(" (physical only)");
    }
    match &capability.availability {
        Availability::Enabled => {}
        Availability::Disabled { reason } => {
            let _ = write!(tag, " (disabled: {})", clip(reason));
        }
        Availability::Hidden => tag.push_str(" (hidden)"),
    }
    tag
}

fn fact_text(value: &ObservationValue) -> String {
    match value {
        ObservationValue::Text(text) => format!("{:?}", clip(text)),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Layout-only nodes the outline folds into their parent.
fn folded(node: &UiNode) -> bool {
    matches!(node.role, UiRole::ScrollArea | UiRole::Section) && node.name.is_empty()
}

fn node_line(node: &UiNode, tags: &[String]) -> String {
    let mut line = role_name(node.role);
    if !node.name.is_empty() {
        let _ = write!(line, " {:?}", clip(&node.name));
    }
    if !node.enabled {
        match &node.disabled_reason {
            Some(reason) => {
                let _ = write!(line, " disabled({})", clip(reason));
            }
            None => line.push_str(" disabled"),
        }
    }
    for (on, state) in [(node.focused, "focused"), (node.selected, "selected")] {
        if on {
            let _ = write!(line, " {state}");
        }
    }
    match node.checked {
        Some(true) => line.push_str(" checked"),
        Some(false) => line.push_str(" unchecked"),
        None => {}
    }
    match node.expanded {
        Some(true) => line.push_str(" expanded"),
        Some(false) => line.push_str(" collapsed"),
        None => {}
    }
    if let Some(value) = &node.value {
        let _ = write!(line, " = {:?}", clip(value));
    }
    let _ = write!(line, " #{}", node.id.0);
    if !tags.is_empty() {
        let _ = write!(line, " [{}]", tags.join("; "));
    }
    line
}

/// The nodes in outline order (depth first from the root), with depths,
/// folding layout-only nodes.
fn ordered(observation: &CompletedObservation) -> Vec<(usize, &UiNode)> {
    let snapshot = &observation.snapshot;
    let mut children: BTreeMap<&SemanticUiId, Vec<&UiNode>> = BTreeMap::new();
    for node in &snapshot.nodes {
        if let Some(parent) = &node.parent {
            children.entry(parent).or_default().push(node);
        }
    }
    let known: std::collections::BTreeSet<&SemanticUiId> =
        snapshot.nodes.iter().map(|node| &node.id).collect();
    let mut out = Vec::new();
    let mut stack: Vec<(usize, &UiNode)> = Vec::new();
    // Roots: the snapshot root's children, then orphans.
    let mut roots: Vec<&UiNode> = children.get(&snapshot.root).cloned().unwrap_or_default();
    roots.extend(snapshot.nodes.iter().filter(|node| {
        node.id != snapshot.root
            && node
                .parent
                .as_ref()
                .is_none_or(|parent| !known.contains(parent))
    }));
    for root in roots.into_iter().rev() {
        stack.push((0, root));
    }
    // A malformed tree whose parents form a cycle must not loop forever.
    let mut visited = std::collections::BTreeSet::new();
    while let Some((depth, node)) = stack.pop() {
        if !visited.insert(&node.id) {
            continue;
        }
        let child_depth = if folded(node) {
            depth
        } else {
            out.push((depth, node));
            depth + 1
        };
        if let Some(nodes) = children.get(&node.id) {
            for child in nodes.iter().rev() {
                stack.push((child_depth, child));
            }
        }
    }
    out
}

/// The text outline of an observation.
pub fn outline(
    observation: &CompletedObservation,
    capabilities: &[CapabilityDescription],
    offset: usize,
    limit: usize,
) -> String {
    let mut text = String::new();
    let application = &observation.application;
    let _ = writeln!(
        text,
        "{} {} · observation {} · instance {}",
        application.name, application.build.version, observation.id, observation.instance
    );
    if !observation.facts.is_empty() {
        let facts: Vec<String> = observation
            .facts
            .iter()
            .map(|(key, value)| format!("{key}={}", fact_text(value)))
            .collect();
        let _ = writeln!(text, "facts: {}", facts.join(" "));
    }
    for collection in &observation.collection.collections {
        let _ = writeln!(
            text,
            "collection {}: {} observed of {}{}",
            collection.id,
            collection.observed,
            collection
                .total
                .map_or_else(|| "unknown".to_owned(), |total| total.to_string()),
            if collection.complete {
                ", complete"
            } else {
                ""
            }
        );
    }
    let snapshot = &observation.snapshot;
    if snapshot.semantic_audit.is_empty() && snapshot.text_audit.is_empty() {
        text.push_str("audits: clear\n");
    } else {
        let _ = writeln!(
            text,
            "audits: {} semantic, {} text findings (see --json)",
            snapshot.semantic_audit.len(),
            snapshot.text_audit.len()
        );
    }
    let mut tags: BTreeMap<&SemanticUiId, Vec<String>> = BTreeMap::new();
    for capability in capabilities {
        tags.entry(&capability.id)
            .or_default()
            .push(capability_tag(capability));
    }
    let nodes = ordered(observation);
    let shown = nodes.iter().skip(offset).take(limit);
    for (depth, node) in shown {
        let node_tags = tags.get(&node.id).cloned().unwrap_or_default();
        let _ = writeln!(
            text,
            "{}- {}",
            "  ".repeat(*depth),
            node_line(node, &node_tags)
        );
    }
    let remaining = nodes.len().saturating_sub(offset + limit);
    if remaining > 0 {
        let _ = writeln!(
            text,
            "… {remaining} more nodes; continue with --offset {}",
            offset + limit
        );
    }
    let rendered: std::collections::BTreeSet<&SemanticUiId> =
        snapshot.nodes.iter().map(|node| &node.id).collect();
    let unrendered: Vec<String> = capabilities
        .iter()
        .filter(|capability| !rendered.contains(&capability.id))
        .map(|capability| {
            let mut tag = capability_tag(capability);
            if let Some(pane) = capability.pane {
                let _ = write!(tag, " (pane {})", pane.0);
            }
            tag
        })
        .collect();
    if !unrendered.is_empty() {
        let _ = writeln!(text, "other capabilities: {}", unrendered.join("; "));
    }
    text
}

/// A node in JSON output: semantics and state, without geometry.
#[derive(Serialize)]
pub struct NodeOutput<'a> {
    pub id: &'a SemanticUiId,
    pub parent: Option<&'a SemanticUiId>,
    pub role: UiRole,
    pub name: &'a str,
    pub enabled: bool,
    pub disabled_reason: Option<&'a str>,
    pub focused: bool,
    pub selected: bool,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub value: Option<&'a str>,
    pub pane: Option<PaneId>,
    pub domain: Option<&'a DomainReference>,
    pub capabilities: Vec<&'a str>,
}

impl<'a> NodeOutput<'a> {
    pub fn new(node: &'a UiNode, capabilities: &'a [CapabilityDescription]) -> Self {
        Self {
            id: &node.id,
            parent: node.parent.as_ref(),
            role: node.role,
            name: &node.name,
            enabled: node.enabled,
            disabled_reason: node.disabled_reason.as_deref(),
            focused: node.focused,
            selected: node.selected,
            checked: node.checked,
            expanded: node.expanded,
            value: node.value.as_deref(),
            pane: node.pane,
            domain: node.domain_reference.as_ref(),
            capabilities: capabilities
                .iter()
                .filter(|capability| capability.id == node.id)
                .map(|capability| capability.capability.as_str())
                .collect(),
        }
    }
}

/// Audit results in JSON output.
#[derive(Serialize)]
pub struct AuditsOutput<'a> {
    pub semantic: &'a [stobie_client::protocol::SemanticAuditFinding],
    pub text: &'a [stobie_client::protocol::TextAuditFinding],
    pub coverage_collected: bool,
}

/// The JSON for `observe`, in a fixed field order.
#[derive(Serialize)]
pub struct ObserveOutput<'a> {
    pub instance: &'a str,
    pub application: &'a stobie_client::protocol::ApplicationIdentity,
    pub observation: u64,
    pub facts: &'a BTreeMap<String, ObservationValue>,
    pub collections: &'a [stobie_client::protocol::CollectionObservation],
    pub audits: AuditsOutput<'a>,
    pub nodes_total: usize,
    pub offset: usize,
    pub nodes: Vec<NodeOutput<'a>>,
    pub truncated: bool,
    pub next_offset: Option<usize>,
    pub capabilities_not_rendered: Vec<&'a str>,
    /// What this output leaves out.
    pub omitted: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_observations: Option<&'a [stobie_client::protocol::TextLayoutObservation]>,
}

pub fn observe_json<'a>(
    observation: &'a CompletedObservation,
    capabilities: &'a [CapabilityDescription],
    offset: usize,
    limit: usize,
    include_text: bool,
) -> ObserveOutput<'a> {
    let nodes = ordered(observation);
    let shown: Vec<NodeOutput> = nodes
        .iter()
        .skip(offset)
        .take(limit)
        .map(|(_, node)| NodeOutput::new(node, capabilities))
        .collect();
    let end = offset + shown.len();
    let snapshot = &observation.snapshot;
    let mut omitted = vec!["geometry"];
    if !include_text {
        omitted.push("text_observations");
    }
    ObserveOutput {
        instance: &observation.instance,
        application: &observation.application,
        observation: observation.id,
        facts: &observation.facts,
        collections: &observation.collection.collections,
        audits: AuditsOutput {
            semantic: &snapshot.semantic_audit,
            text: &snapshot.text_audit,
            coverage_collected: snapshot.text_audit_coverage.is_some(),
        },
        nodes_total: nodes.len(),
        offset,
        nodes: shown,
        truncated: end < nodes.len(),
        next_offset: (end < nodes.len()).then_some(end),
        capabilities_not_rendered: capabilities
            .iter()
            .filter(|capability| snapshot.node(&capability.id).is_none())
            .map(|capability| capability.capability.as_str())
            .collect(),
        omitted,
        text_observations: include_text.then_some(snapshot.text.as_slice()),
    }
}

/// How a node differs between two observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeChangeKind {
    Added,
    Removed,
    Changed,
}

/// One node whose presented state differs between two observations.
#[derive(Serialize)]
pub struct NodeChange<'a> {
    pub id: &'a SemanticUiId,
    pub change: NodeChangeKind,
    /// The node as now presented, or as last presented when removed.
    #[serde(skip)]
    pub node: &'a UiNode,
}

/// Whether two presentations of a node differ in what an agent reads: name,
/// state or value. Geometry and focus are ignored.
fn presented_differently(before: &UiNode, after: &UiNode) -> bool {
    (
        &before.name,
        before.enabled,
        &before.disabled_reason,
        before.selected,
        before.checked,
        before.expanded,
        &before.value,
    ) != (
        &after.name,
        after.enabled,
        &after.disabled_reason,
        after.selected,
        after.checked,
        after.expanded,
        &after.value,
    )
}

/// Nodes added, removed or presented differently, in outline order.
pub fn changed_nodes<'a>(
    before: &'a CompletedObservation,
    after: &'a CompletedObservation,
) -> Vec<NodeChange<'a>> {
    let mut changes: Vec<NodeChange> = ordered(after)
        .into_iter()
        .filter_map(|(_, node)| {
            let change = match before.snapshot.node(&node.id) {
                None => NodeChangeKind::Added,
                Some(old) if presented_differently(old, node) => NodeChangeKind::Changed,
                Some(_) => return None,
            };
            Some(NodeChange {
                id: &node.id,
                change,
                node,
            })
        })
        .collect();
    changes.extend(
        ordered(before)
            .into_iter()
            .filter(|(_, node)| after.snapshot.node(&node.id).is_none())
            .map(|(_, node)| NodeChange {
                id: &node.id,
                change: NodeChangeKind::Removed,
                node,
            }),
    );
    changes
}

/// At most this many changed nodes are listed in text output.
const CHANGED_NODE_LINES: usize = 6;

/// The changed nodes as text lines, each marked `changed`, `added` or
/// `removed`.
pub fn changed_nodes_text(changes: &[NodeChange]) -> String {
    let mut text = String::new();
    for change in changes.iter().take(CHANGED_NODE_LINES) {
        let mark = match change.change {
            NodeChangeKind::Added => "added",
            NodeChangeKind::Removed => "removed",
            NodeChangeKind::Changed => "changed",
        };
        let _ = writeln!(text, "  {mark} {}", node_line(change.node, &[]));
    }
    if changes.len() > CHANGED_NODE_LINES {
        let _ = writeln!(
            text,
            "  … {} more; --json lists up to 32",
            changes.len() - CHANGED_NODE_LINES
        );
    }
    text
}
