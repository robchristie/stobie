#![cfg(test)]

use serde::Serialize;

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
