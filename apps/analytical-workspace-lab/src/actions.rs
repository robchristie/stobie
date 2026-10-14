use serde::{Deserialize, Serialize};
use stobie_core::{AnnotationId, PaneId, ResultId};
use stobie_ui_egui::{Action, Availability};

pub(crate) fn polygon_vertex_count(gesture: Option<&stobie_core::GesturePreview>) -> usize {
    match gesture {
        Some(stobie_core::GesturePreview::Polygon { vertices, .. }) => vertices.len(),
        _ => 0,
    }
}

/// The same fit intent is used by physical controls and semantic invocation.
pub(crate) fn fit_view_intent(
    pane: PaneId,
    rect: stobie_ui_egui::UiRect,
) -> stobie_core::ImageIntent {
    stobie_core::ImageIntent::SetCamera {
        pane,
        camera: stobie_core::Camera::fit(
            f64::from(rect.max_x - rect.min_x),
            f64::from(rect.max_y - rect.min_y),
        ),
    }
}

/// Stable identities and presentation metadata owned by Analytical Workspace
/// Lab rather than by the egui framework crate. The IDs predate the derive's
/// prefixed default, so each variant keeps its published ID.
#[derive(
    Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
#[action(prefix = "lab")]
pub enum LabAction {
    #[action(
        id = "undo",
        label = "Undo",
        description = "Undo the most recent change",
        shortcut = "Cmd+Z"
    )]
    Undo,
    #[action(
        id = "redo",
        label = "Redo",
        description = "Redo the most recently undone change",
        shortcut = "Cmd+Shift+Z"
    )]
    Redo,
    #[action(
        id = "save_layout",
        label = "Save layout",
        description = "Persist the current workspace layout",
        compact = "Save",
        shortcut = "Cmd+S"
    )]
    SaveLayout,
    #[action(
        id = "reset_workspace",
        label = "Reset workspace",
        description = "Restore the default workspace, cameras and tools",
        compact = "Reset"
    )]
    ResetWorkspace,
    #[action(
        id = "view_panels",
        label = "View/Panels",
        description = "Show or hide workspace panes",
        compact = "Panels",
        physical_only = "Opens a menu; invoke toggle_diagnostics instead"
    )]
    ViewPanels,
    #[action(
        id = "toggle_diagnostics",
        label = "Diagnostics",
        description = "Show or hide the Diagnostics pane",
        shortcut = "Cmd+Shift+D"
    )]
    ToggleDiagnostics,
    #[action(
        id = "appearance_settings",
        label = "Appearance",
        description = "Adjust theme, contrast, density, text scale and motion",
        compact = "Display",
        physical_only = "Opens the appearance preferences menu"
    )]
    AppearanceSettings,
    #[action(
        id = "display_settings",
        label = "Display",
        description = "Adjust the image colour map and scalar window",
        scope = "pane",
        physical_only = "Opens the display settings menu"
    )]
    DisplaySettings,
    #[action(
        id = "copy_diagnostics",
        label = "Copy diagnostics",
        description = "Copy the current diagnostic snapshot as JSON",
        compact = "Copy JSON",
        scope = "pane",
        physical_only = "Writes the system clipboard"
    )]
    CopyDiagnostics,
    #[action(
        id = "fit_view",
        label = "Fit view",
        description = "Fit the complete image into the active viewport",
        compact = "Fit",
        shortcut = "F",
        scope = "pane"
    )]
    FitView,
    #[action(
        id = "link_views",
        label = "Link views",
        description = "Link or unlink this camera with camera group A",
        compact = "Link A",
        shortcut = "L",
        scope = "pane"
    )]
    LinkViews,
    #[action(
        id = "navigate_tool",
        label = "Navigate",
        description = "Pan and zoom the active viewport",
        shortcut = "1",
        scope = "pane"
    )]
    NavigateTool,
    #[action(
        id = "polygon_tool",
        label = "Polygon",
        description = "Create a polygon annotation",
        shortcut = "2",
        scope = "pane"
    )]
    PolygonTool,
    #[action(
        id = "edit_vertices_tool",
        label = "Edit vertices",
        description = "Move vertices of the selected polygon",
        compact = "Edit",
        shortcut = "3",
        scope = "pane"
    )]
    EditVerticesTool,
    #[action(
        id = "commit_polygon",
        label = "Commit polygon",
        description = "Commit the current polygon annotation",
        compact = "Commit",
        shortcut = "Enter",
        scope = "active_pane"
    )]
    CommitPolygon,
    #[action(
        id = "delete_annotation",
        label = "Delete annotation",
        description = "Delete the selected polygon annotation",
        compact = "Delete",
        shortcut = "Delete",
        scope = "active_pane"
    )]
    DeleteAnnotation,
    #[action(
        id = "recenter_primary",
        label = "Recenter primary view",
        description = "Centre the primary viewport on the selected result",
        compact = "Recenter",
        shortcut = "R",
        scope = "pane"
    )]
    RecenterPrimary,
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

/// Whether `pane` presents controls for a pane-scoped action. Shortcuts and
/// semantic invocations reach only panes that offer the action, as clicks do.
fn offered_in(action: LabAction, pane: PaneId) -> bool {
    match action {
        LabAction::NavigateTool | LabAction::PolygonTool | LabAction::EditVerticesTool => {
            (1..=2).contains(&pane.0)
        }
        LabAction::CommitPolygon | LabAction::DeleteAnnotation => (1..=4).contains(&pane.0),
        LabAction::RecenterPrimary => matches!(pane.0, 5 | 7),
        _ => true,
    }
}

pub fn availability(id: LabAction, context: ActionContext) -> Availability {
    if let Some(pane) = context.target_pane
        && !offered_in(id, pane)
    {
        return Availability::Hidden;
    }
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
        | LabAction::ViewPanels
        | LabAction::ToggleDiagnostics
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
            if context.target_pane.is_none() =>
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

    use stobie_ui_egui::{ActionKey, ActionSet};

    use super::*;

    #[test]
    fn registry_is_complete_unique_and_stable() {
        let ids: BTreeSet<_> = LabAction::ALL.iter().copied().collect();
        let stable_ids: BTreeSet<_> = LabAction::ALL
            .iter()
            .copied()
            .map(ActionKey::stable_id)
            .collect();
        assert_eq!(ids.len(), LabAction::ALL.len());
        assert_eq!(stable_ids.len(), LabAction::ALL.len());
        for &action in LabAction::ALL {
            let spec = action.specification();
            assert_eq!(spec.id, action);
            assert!(!action.stable_id().is_empty());
            assert!(!spec.label.is_empty());
            assert!(!spec.description.is_empty());
        }
    }

    #[test]
    fn derived_registry_keeps_published_ids_order_and_invocation() {
        let ids: Vec<_> = LabAction::ALL
            .iter()
            .map(|action| action.stable_id())
            .collect();
        assert_eq!(
            ids,
            [
                "undo",
                "redo",
                "save_layout",
                "reset_workspace",
                "view_panels",
                "toggle_diagnostics",
                "appearance_settings",
                "display_settings",
                "copy_diagnostics",
                "fit_view",
                "link_views",
                "navigate_tool",
                "polygon_tool",
                "edit_vertices_tool",
                "commit_polygon",
                "delete_annotation",
                "recenter_primary",
            ]
        );
        let physical_only: Vec<_> = LabAction::ALL
            .iter()
            .filter(|action| !action.invocation().semantic())
            .map(|action| action.stable_id())
            .collect();
        assert_eq!(
            physical_only,
            [
                "view_panels",
                "appearance_settings",
                "display_settings",
                "copy_diagnostics"
            ]
        );
    }

    #[test]
    fn pane_actions_are_offered_only_where_they_are_presented() {
        let context = |pane| ActionContext {
            target_pane: Some(PaneId(pane)),
            selected_annotation: Some(AnnotationId(1)),
            selected_result: Some(ResultId(1)),
            polygon_vertices: 3,
            ..Default::default()
        };
        assert_eq!(
            availability(LabAction::RecenterPrimary, context(1)),
            Availability::Hidden
        );
        for pane in [5, 7] {
            assert!(availability(LabAction::RecenterPrimary, context(pane)).enabled());
        }
        for action in [LabAction::CommitPolygon, LabAction::DeleteAnnotation] {
            assert!(availability(action, context(4)).enabled());
            assert_eq!(availability(action, context(5)), Availability::Hidden);
        }
    }

    #[test]
    fn shortcuts_are_unique_within_overlapping_scopes() {
        let mut seen = BTreeSet::new();
        for &action in LabAction::ALL {
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
