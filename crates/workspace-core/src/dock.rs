use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{DockNodeId, PaneId};

pub const LAYOUT_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockDrop {
    Tab,
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DockNode {
    Split {
        id: DockNodeId,
        axis: SplitAxis,
        fraction: f32,
        first: Box<DockNode>,
        second: Box<DockNode>,
    },
    Tabs {
        id: DockNodeId,
        tabs: Vec<PaneId>,
        active: usize,
    },
}

impl DockNode {
    pub fn id(&self) -> DockNodeId {
        match self {
            Self::Split { id, .. } | Self::Tabs { id, .. } => *id,
        }
    }

    pub fn node_count(&self) -> usize {
        match self {
            Self::Split { first, second, .. } => 1 + first.node_count() + second.node_count(),
            Self::Tabs { .. } => 1,
        }
    }

    pub fn pane_ids(&self, output: &mut Vec<PaneId>) {
        match self {
            Self::Split { first, second, .. } => {
                first.pane_ids(output);
                second.pane_ids(output);
            }
            Self::Tabs { tabs, .. } => output.extend(tabs),
        }
    }

    pub fn active_panes(&self, output: &mut Vec<PaneId>) {
        match self {
            Self::Split { first, second, .. } => {
                first.active_panes(output);
                second.active_panes(output);
            }
            Self::Tabs { tabs, active, .. } => {
                if let Some(pane) = tabs.get(*active) {
                    output.push(*pane)
                }
            }
        }
    }

    pub fn normalise(&mut self) {
        match self {
            Self::Split {
                fraction,
                first,
                second,
                ..
            } => {
                *fraction = fraction.clamp(0.1, 0.9);
                first.normalise();
                second.normalise();
            }
            Self::Tabs { tabs, active, .. } => {
                *active = (*active).min(tabs.len().saturating_sub(1));
            }
        }
    }

    fn remove_pane(&mut self, pane: PaneId) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.remove_pane(pane) || second.remove_pane(pane)
            }
            Self::Tabs { tabs, active, .. } => {
                let Some(index) = tabs.iter().position(|candidate| *candidate == pane) else {
                    return false;
                };
                tabs.remove(index);
                *active = (*active).min(tabs.len().saturating_sub(1));
                true
            }
        }
    }

    fn insert_at(
        &mut self,
        target: PaneId,
        pane: PaneId,
        drop: DockDrop,
        split_id: DockNodeId,
        tabs_id: DockNodeId,
    ) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.insert_at(target, pane, drop, split_id, tabs_id)
                    || second.insert_at(target, pane, drop, split_id, tabs_id)
            }
            Self::Tabs { tabs, active, .. } if tabs.contains(&target) => {
                if drop == DockDrop::Tab {
                    tabs.push(pane);
                    *active = tabs.len() - 1;
                } else {
                    let old = std::mem::replace(
                        self,
                        Self::Tabs {
                            id: tabs_id,
                            tabs: vec![],
                            active: 0,
                        },
                    );
                    let new = Self::Tabs {
                        id: tabs_id,
                        tabs: vec![pane],
                        active: 0,
                    };
                    let (axis, first, second) = match drop {
                        DockDrop::Left => (SplitAxis::Horizontal, new, old),
                        DockDrop::Right => (SplitAxis::Horizontal, old, new),
                        DockDrop::Top => (SplitAxis::Vertical, new, old),
                        DockDrop::Bottom => (SplitAxis::Vertical, old, new),
                        DockDrop::Tab => unreachable!(),
                    };
                    *self = Self::Split {
                        id: split_id,
                        axis,
                        fraction: 0.5,
                        first: Box::new(first),
                        second: Box::new(second),
                    };
                }
                true
            }
            Self::Tabs { .. } => false,
        }
    }

    fn is_empty(&self) -> bool {
        matches!(self, Self::Tabs { tabs, .. } if tabs.is_empty())
    }

    fn prune_empty(&mut self) {
        if let Self::Split { first, second, .. } = self {
            first.prune_empty();
            second.prune_empty();
            if first.is_empty() {
                let replacement = std::mem::replace(
                    second,
                    Box::new(Self::Tabs {
                        id: DockNodeId(0),
                        tabs: Vec::new(),
                        active: 0,
                    }),
                );
                *self = *replacement;
            } else if second.is_empty() {
                let replacement = std::mem::replace(
                    first,
                    Box::new(Self::Tabs {
                        id: DockNodeId(0),
                        tabs: Vec::new(),
                        active: 0,
                    }),
                );
                *self = *replacement;
            }
        }
    }

    pub fn split_fraction(&self, target: DockNodeId) -> Option<f32> {
        match self {
            Self::Split {
                id,
                fraction,
                first,
                second,
                ..
            } => {
                if *id == target {
                    Some(*fraction)
                } else {
                    first
                        .split_fraction(target)
                        .or_else(|| second.split_fraction(target))
                }
            }
            Self::Tabs { .. } => None,
        }
    }

    pub fn set_split_fraction(&mut self, target: DockNodeId, value: f32) -> bool {
        match self {
            Self::Split {
                id,
                fraction,
                first,
                second,
                ..
            } => {
                if *id == target {
                    *fraction = value.clamp(0.1, 0.9);
                    true
                } else {
                    first.set_split_fraction(target, value)
                        || second.set_split_fraction(target, value)
                }
            }
            Self::Tabs { .. } => false,
        }
    }

    fn contains_pane(&self, pane: PaneId) -> bool {
        match self {
            Self::Split { first, second, .. } => {
                first.contains_pane(pane) || second.contains_pane(pane)
            }
            Self::Tabs { tabs, .. } => tabs.contains(&pane),
        }
    }

    fn node_ids(&self, output: &mut Vec<DockNodeId>) {
        output.push(self.id());
        if let Self::Split { first, second, .. } = self {
            first.node_ids(output);
            second.node_ids(output);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub schema_version: u32,
    pub root: DockNode,
    pub active_pane: PaneId,
    pub closed_optional_panes: BTreeSet<PaneId>,
    pub next_node_id: u64,
}

impl Workspace {
    pub fn analytical_default() -> Self {
        let tabs = |id, ids: &[u32]| DockNode::Tabs {
            id: DockNodeId(id),
            tabs: ids.iter().copied().map(PaneId).collect(),
            active: 0,
        };
        Self {
            schema_version: LAYOUT_SCHEMA_VERSION,
            active_pane: PaneId(1),
            closed_optional_panes: BTreeSet::new(),
            next_node_id: 12,
            root: DockNode::Split {
                id: DockNodeId(1),
                axis: SplitAxis::Horizontal,
                fraction: 0.72,
                first: Box::new(DockNode::Split {
                    id: DockNodeId(2),
                    axis: SplitAxis::Vertical,
                    fraction: 0.68,
                    first: Box::new(DockNode::Split {
                        id: DockNodeId(3),
                        axis: SplitAxis::Horizontal,
                        fraction: 0.5,
                        first: Box::new(tabs(4, &[1])),
                        second: Box::new(tabs(5, &[2])),
                    }),
                    second: Box::new(DockNode::Split {
                        id: DockNodeId(6),
                        axis: SplitAxis::Horizontal,
                        fraction: 0.5,
                        first: Box::new(tabs(7, &[3])),
                        second: Box::new(tabs(8, &[4])),
                    }),
                }),
                second: Box::new(DockNode::Split {
                    id: DockNodeId(9),
                    axis: SplitAxis::Vertical,
                    fraction: 0.52,
                    first: Box::new(tabs(10, &[5, 6])),
                    second: Box::new(tabs(11, &[7, 8])),
                }),
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != LAYOUT_SCHEMA_VERSION {
            return Err(format!("unsupported layout schema {}", self.schema_version));
        }
        let mut panes = Vec::new();
        self.root.pane_ids(&mut panes);
        if panes.is_empty() {
            return Err("workspace contains no panes".into());
        }
        let unique: BTreeSet<_> = panes.iter().collect();
        if panes.len() != unique.len() {
            return Err("a pane occurs more than once".into());
        }
        if !panes.contains(&self.active_pane) {
            return Err("active pane is absent".into());
        }
        let mut node_ids = Vec::new();
        self.root.node_ids(&mut node_ids);
        let unique_node_ids: BTreeSet<_> = node_ids.iter().collect();
        if node_ids.len() != unique_node_ids.len() || node_ids.contains(&DockNodeId(0)) {
            return Err("dock node IDs must be non-zero and unique".into());
        }
        if node_ids
            .iter()
            .any(|node_id| node_id.0 >= self.next_node_id)
        {
            return Err("next dock node ID does not exceed existing IDs".into());
        }
        Ok(())
    }

    pub fn serialised_size(&self) -> usize {
        serde_json::to_vec(self).map_or(0, |bytes| bytes.len())
    }

    pub fn move_pane(&mut self, pane: PaneId, target: PaneId, drop: DockDrop) -> bool {
        if pane == target {
            return false;
        }
        if !self.root.contains_pane(pane) || !self.root.contains_pane(target) {
            return false;
        }
        let before = self.root.clone();
        if !self.root.remove_pane(pane) {
            return false;
        }
        self.root.prune_empty();
        let split_id = DockNodeId(self.next_node_id);
        let tabs_id = DockNodeId(self.next_node_id + 1);
        if !self.root.insert_at(target, pane, drop, split_id, tabs_id) {
            self.root = before;
            return false;
        }
        if drop != DockDrop::Tab {
            self.next_node_id += 2;
        }
        self.root.normalise();
        self.active_pane = pane;
        true
    }

    pub fn activate(&mut self, pane: PaneId) {
        fn activate_node(node: &mut DockNode, pane: PaneId) -> bool {
            match node {
                DockNode::Split { first, second, .. } => {
                    activate_node(first, pane) || activate_node(second, pane)
                }
                DockNode::Tabs { tabs, active, .. } => {
                    if let Some(index) = tabs.iter().position(|candidate| *candidate == pane) {
                        *active = index;
                        true
                    } else {
                        false
                    }
                }
            }
        }
        if activate_node(&mut self.root, pane) {
            self.active_pane = pane;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_tree_has_stable_unique_panes() {
        let workspace = Workspace::analytical_default();
        workspace.validate().unwrap();
        let mut panes = Vec::new();
        workspace.root.pane_ids(&mut panes);
        assert_eq!(panes, (1..=8).map(PaneId).collect::<Vec<_>>());
    }

    #[test]
    fn layout_serialisation_round_trip_is_exact() {
        let workspace = Workspace::analytical_default();
        let encoded = serde_json::to_string(&workspace).unwrap();
        let restored: Workspace = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored, workspace);
        restored.validate().unwrap();
    }

    #[test]
    fn rejects_duplicate_pane_ids() {
        let mut workspace = Workspace::analytical_default();
        workspace.root = DockNode::Tabs {
            id: DockNodeId(1),
            tabs: vec![PaneId(1), PaneId(1)],
            active: 0,
        };
        assert!(workspace.validate().is_err());
    }

    #[test]
    fn rejects_unknown_persistence_schema() {
        let mut workspace = Workspace::analytical_default();
        workspace.schema_version = LAYOUT_SCHEMA_VERSION + 1;
        assert_eq!(
            workspace.validate(),
            Err(format!(
                "unsupported layout schema {}",
                LAYOUT_SCHEMA_VERSION + 1
            ))
        );
    }

    #[test]
    fn rearrangement_mutates_only_the_canonical_tree() {
        let mut workspace = Workspace::analytical_default();
        assert!(workspace.move_pane(PaneId(2), PaneId(1), DockDrop::Tab));
        workspace.validate().unwrap();
        assert_eq!(workspace.active_pane, PaneId(2));
        let encoded = serde_json::to_string(&workspace).unwrap();
        assert_eq!(
            serde_json::from_str::<Workspace>(&encoded).unwrap(),
            workspace
        );
    }

    #[test]
    fn node_ids_remain_unique_after_side_drop() {
        let mut workspace = Workspace::analytical_default();
        assert!(workspace.move_pane(PaneId(2), PaneId(4), DockDrop::Left));
        workspace.validate().unwrap();
        let mut node_ids = Vec::new();
        workspace.root.node_ids(&mut node_ids);
        assert_eq!(
            node_ids.len(),
            node_ids.iter().collect::<BTreeSet<_>>().len()
        );
    }

    #[test]
    fn invalid_move_target_preserves_the_workspace() {
        let mut workspace = Workspace::analytical_default();
        let before = workspace.clone();
        assert!(!workspace.move_pane(PaneId(2), PaneId(99), DockDrop::Tab));
        assert_eq!(workspace, before);
    }
}
