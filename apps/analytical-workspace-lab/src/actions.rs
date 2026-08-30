use polyorama_core::{AnnotationId, PaneId, ResultId};
use polyorama_ui_egui::{
    ActionKey, ActionScope, ActionShortcut, ActionSpec, Availability, ShortcutKey,
};
use serde::{Deserialize, Serialize};

/// Stable identities and presentation metadata owned by Analytical Workspace
/// Lab rather than by the egui framework crate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabAction {
    Undo,
    Redo,
    SaveLayout,
    ResetWorkspace,
    FitView,
    LinkViews,
    NavigateTool,
    PolygonTool,
    EditVerticesTool,
    CommitPolygon,
    DeleteAnnotation,
    RecenterPrimary,
    AppearanceSettings,
    CopyDiagnostics,
    DisplaySettings,
}

impl LabAction {
    #[cfg(test)]
    pub const ALL: [Self; 15] = [
        Self::Undo,
        Self::Redo,
        Self::SaveLayout,
        Self::ResetWorkspace,
        Self::AppearanceSettings,
        Self::DisplaySettings,
        Self::CopyDiagnostics,
        Self::FitView,
        Self::LinkViews,
        Self::NavigateTool,
        Self::PolygonTool,
        Self::EditVerticesTool,
        Self::CommitPolygon,
        Self::DeleteAnnotation,
        Self::RecenterPrimary,
    ];
}

impl ActionKey for LabAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::SaveLayout => "save_layout",
            Self::ResetWorkspace => "reset_workspace",
            Self::AppearanceSettings => "appearance_settings",
            Self::DisplaySettings => "display_settings",
            Self::CopyDiagnostics => "copy_diagnostics",
            Self::FitView => "fit_view",
            Self::LinkViews => "link_views",
            Self::NavigateTool => "navigate_tool",
            Self::PolygonTool => "polygon_tool",
            Self::EditVerticesTool => "edit_vertices_tool",
            Self::CommitPolygon => "commit_polygon",
            Self::DeleteAnnotation => "delete_annotation",
            Self::RecenterPrimary => "recenter_primary",
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
            Self::Redo => (
                "Redo",
                "Redo the most recently undone change",
                None,
                Some(ActionShortcut::command_shift(ShortcutKey::Z)),
                ActionScope::Application,
            ),
            Self::SaveLayout => (
                "Save layout",
                "Persist the current workspace layout",
                Some("Save"),
                Some(ActionShortcut::command(ShortcutKey::S)),
                ActionScope::Application,
            ),
            Self::ResetWorkspace => (
                "Reset workspace",
                "Restore the default workspace, cameras and tools",
                Some("Reset"),
                None,
                ActionScope::Application,
            ),
            Self::AppearanceSettings => (
                "Appearance",
                "Adjust theme, contrast, density, text scale and motion",
                Some("Display"),
                None,
                ActionScope::Application,
            ),
            Self::DisplaySettings => (
                "Display",
                "Adjust the image colour map and scalar window",
                None,
                None,
                ActionScope::Pane,
            ),
            Self::CopyDiagnostics => (
                "Copy diagnostics",
                "Copy the current diagnostic snapshot as JSON",
                Some("Copy JSON"),
                None,
                ActionScope::Pane,
            ),
            Self::FitView => (
                "Fit view",
                "Fit the complete image into the active viewport",
                Some("Fit"),
                Some(ActionShortcut::new(ShortcutKey::F)),
                ActionScope::Pane,
            ),
            Self::LinkViews => (
                "Link views",
                "Link or unlink this camera with camera group A",
                Some("Link A"),
                Some(ActionShortcut::new(ShortcutKey::L)),
                ActionScope::Pane,
            ),
            Self::NavigateTool => (
                "Navigate",
                "Pan and zoom the active viewport",
                None,
                Some(ActionShortcut::new(ShortcutKey::One)),
                ActionScope::Pane,
            ),
            Self::PolygonTool => (
                "Polygon",
                "Create a polygon annotation",
                None,
                Some(ActionShortcut::new(ShortcutKey::Two)),
                ActionScope::Pane,
            ),
            Self::EditVerticesTool => (
                "Edit vertices",
                "Move vertices of the selected polygon",
                Some("Edit"),
                Some(ActionShortcut::new(ShortcutKey::Three)),
                ActionScope::Pane,
            ),
            Self::CommitPolygon => (
                "Commit polygon",
                "Commit the current polygon annotation",
                Some("Commit"),
                Some(ActionShortcut::new(ShortcutKey::Enter)),
                ActionScope::ActivePane,
            ),
            Self::DeleteAnnotation => (
                "Delete annotation",
                "Delete the selected polygon annotation",
                Some("Delete"),
                Some(ActionShortcut::new(ShortcutKey::Delete)),
                ActionScope::ActivePane,
            ),
            Self::RecenterPrimary => (
                "Recenter primary view",
                "Centre the primary viewport on the selected result",
                Some("Recenter"),
                Some(ActionShortcut::new(ShortcutKey::R)),
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActionContext {
    pub undo_depth: usize,
    pub redo_depth: usize,
    pub active_pane: PaneId,
    pub target_pane: Option<PaneId>,
    pub selected_annotation: Option<AnnotationId>,
    pub selected_result: Option<ResultId>,
    pub polygon_vertices: usize,
}

pub fn availability(id: LabAction, context: ActionContext) -> Availability {
    match id {
        LabAction::Undo if context.undo_depth == 0 => Availability::Disabled {
            reason: "History is empty".into(),
        },
        LabAction::Redo if context.redo_depth == 0 => Availability::Disabled {
            reason: "Nothing has been undone".into(),
        },
        LabAction::Undo
        | LabAction::Redo
        | LabAction::SaveLayout
        | LabAction::ResetWorkspace
        | LabAction::AppearanceSettings => Availability::Enabled,
        LabAction::CopyDiagnostics if context.target_pane != Some(PaneId(8)) => {
            Availability::Disabled {
                reason: "The Diagnostics pane is required".into(),
            }
        }
        LabAction::FitView | LabAction::LinkViews | LabAction::DisplaySettings
            if !context
                .target_pane
                .is_some_and(|pane| (1..=4).contains(&pane.0)) =>
        {
            Availability::Disabled {
                reason: "An image pane is required".into(),
            }
        }
        LabAction::NavigateTool | LabAction::PolygonTool | LabAction::EditVerticesTool
            if !context
                .target_pane
                .is_some_and(|pane| (1..=2).contains(&pane.0)) =>
        {
            Availability::Hidden
        }
        LabAction::CommitPolygon if context.polygon_vertices < 3 => Availability::Disabled {
            reason: "A polygon needs at least three vertices".into(),
        },
        LabAction::DeleteAnnotation if context.selected_annotation.is_none() => {
            Availability::Disabled {
                reason: "No annotation is selected".into(),
            }
        }
        LabAction::RecenterPrimary if context.selected_result.is_none() => Availability::Disabled {
            reason: "No result is selected".into(),
        },
        LabAction::FitView
        | LabAction::LinkViews
        | LabAction::DisplaySettings
        | LabAction::NavigateTool
        | LabAction::PolygonTool
        | LabAction::EditVerticesTool
        | LabAction::CommitPolygon
        | LabAction::DeleteAnnotation
        | LabAction::RecenterPrimary
        | LabAction::CopyDiagnostics => Availability::Enabled,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn registry_is_complete_unique_and_stable() {
        let ids: BTreeSet<_> = LabAction::ALL.into_iter().collect();
        let stable_ids: BTreeSet<_> = LabAction::ALL
            .into_iter()
            .map(ActionKey::stable_id)
            .collect();
        assert_eq!(ids.len(), LabAction::ALL.len());
        assert_eq!(stable_ids.len(), LabAction::ALL.len());
        for action in LabAction::ALL {
            let spec = action.specification();
            assert_eq!(spec.id, action);
            assert!(!action.stable_id().is_empty());
            assert!(!spec.label.is_empty());
            assert!(!spec.description.is_empty());
        }
    }

    #[test]
    fn shortcuts_are_unique_within_overlapping_scopes() {
        let mut seen = BTreeSet::new();
        for action in LabAction::ALL {
            let spec = action.specification();
            if let Some(shortcut) = spec.shortcut {
                assert!(
                    seen.insert((spec.scope, shortcut)),
                    "duplicate shortcut for {action:?}"
                );
            }
        }
    }

    #[test]
    fn context_changes_availability_and_retains_reasons() {
        let context = ActionContext {
            active_pane: PaneId(1),
            target_pane: Some(PaneId(1)),
            ..Default::default()
        };
        assert_eq!(
            availability(LabAction::Undo, context).disabled_reason(),
            Some("History is empty")
        );
        assert_eq!(
            availability(LabAction::DeleteAnnotation, context).disabled_reason(),
            Some("No annotation is selected")
        );
        assert_eq!(
            availability(
                LabAction::CommitPolygon,
                ActionContext {
                    polygon_vertices: 3,
                    ..context
                }
            ),
            Availability::Enabled
        );
        assert_eq!(
            availability(
                LabAction::Undo,
                ActionContext {
                    undo_depth: 1,
                    ..context
                }
            ),
            Availability::Enabled
        );
    }

    #[test]
    fn tools_are_hidden_outside_supported_image_panes() {
        let context = ActionContext {
            active_pane: PaneId(5),
            target_pane: Some(PaneId(5)),
            ..Default::default()
        };
        assert_eq!(
            availability(LabAction::PolygonTool, context),
            Availability::Hidden
        );
        assert!(
            availability(LabAction::FitView, context)
                .disabled_reason()
                .is_some()
        );
    }
}
