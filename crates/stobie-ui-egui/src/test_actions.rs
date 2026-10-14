#![cfg(test)]

use std::collections::BTreeSet;

use serde::Serialize;
use stobie_core::{DockNode, DockNodeId, LAYOUT_SCHEMA_VERSION, PaneId, SplitAxis, Workspace};

use crate::{ActionKey, ActionScope, ActionShortcut, ActionSpec, ShortcutKey};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TestAction {
    Undo,
    FitView,
    CommitPolygon,
    AppearanceSettings,
    DisplaySettings,
}

impl TestAction {
    pub(crate) const ALL: [Self; 5] = [
        Self::Undo,
        Self::FitView,
        Self::CommitPolygon,
        Self::AppearanceSettings,
        Self::DisplaySettings,
    ];
}

impl ActionKey for TestAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::FitView => "fit_view",
            Self::CommitPolygon => "commit_polygon",
            Self::AppearanceSettings => "appearance_settings",
            Self::DisplaySettings => "display_settings",
        }
    }

    fn specification(self) -> ActionSpec<Self> {
        let (label, description, compact_label, shortcut, scope) = match self {
            Self::Undo => (
                "Undo",
                "Undo the most recent change",
                None,
                Some(ActionShortcut::command(ShortcutKey::Z)),
                ActionScope::Application,
            ),
            Self::FitView => (
                "Fit view",
                "Fit the complete image into the active viewport",
                Some("Fit"),
                Some(ActionShortcut::new(ShortcutKey::F)),
                ActionScope::Pane,
            ),
            Self::CommitPolygon => (
                "Commit polygon",
                "Commit the current polygon annotation",
                Some("Commit"),
                Some(ActionShortcut::new(ShortcutKey::Enter)),
                ActionScope::ActivePane,
            ),
            Self::AppearanceSettings => (
                "Appearance",
                "Adjust interface appearance",
                Some("Display"),
                None,
                ActionScope::Application,
            ),
            Self::DisplaySettings => (
                "Display",
                "Adjust image display settings",
                None,
                None,
                ActionScope::Pane,
            ),
        };
        ActionSpec {
            id: self,
            label,
            description,
            compact_label,
            shortcut,
            scope,
        }
    }
}

/// The Lab's eight-pane layout, as a dock fixture.
pub(crate) fn analytical_workspace() -> Workspace {
    let tabs = |id, ids: &[u32]| DockNode::Tabs {
        id: DockNodeId(id),
        tabs: ids.iter().copied().map(PaneId).collect(),
        active: 0,
    };
    Workspace {
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
