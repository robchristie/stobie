use polyorama_core::{AnnotationId, PaneId, ResultId};
use polyorama_ui_egui::{ActionId, Availability};

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

pub fn availability(id: ActionId, context: ActionContext) -> Availability {
    match id {
        ActionId::Undo if context.undo_depth == 0 => Availability::Disabled {
            reason: "History is empty".into(),
        },
        ActionId::Redo if context.redo_depth == 0 => Availability::Disabled {
            reason: "Nothing has been undone".into(),
        },
        ActionId::Undo | ActionId::Redo | ActionId::SaveLayout | ActionId::ResetWorkspace => {
            Availability::Enabled
        }
        ActionId::FitView | ActionId::LinkViews
            if !context
                .target_pane
                .is_some_and(|pane| (1..=4).contains(&pane.0)) =>
        {
            Availability::Disabled {
                reason: "An image pane is required".into(),
            }
        }
        ActionId::NavigateTool | ActionId::PolygonTool | ActionId::EditVerticesTool
            if !context
                .target_pane
                .is_some_and(|pane| (1..=2).contains(&pane.0)) =>
        {
            Availability::Hidden
        }
        ActionId::CommitPolygon if context.polygon_vertices < 3 => Availability::Disabled {
            reason: "A polygon needs at least three vertices".into(),
        },
        ActionId::DeleteAnnotation if context.selected_annotation.is_none() => {
            Availability::Disabled {
                reason: "No annotation is selected".into(),
            }
        }
        ActionId::RecenterPrimary if context.selected_result.is_none() => Availability::Disabled {
            reason: "No result is selected".into(),
        },
        ActionId::FitView
        | ActionId::LinkViews
        | ActionId::NavigateTool
        | ActionId::PolygonTool
        | ActionId::EditVerticesTool
        | ActionId::CommitPolygon
        | ActionId::DeleteAnnotation
        | ActionId::RecenterPrimary => Availability::Enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_changes_availability_and_retains_reasons() {
        let context = ActionContext {
            active_pane: PaneId(1),
            target_pane: Some(PaneId(1)),
            ..Default::default()
        };
        assert_eq!(
            availability(ActionId::Undo, context).disabled_reason(),
            Some("History is empty")
        );
        assert_eq!(
            availability(ActionId::DeleteAnnotation, context).disabled_reason(),
            Some("No annotation is selected")
        );
        assert_eq!(
            availability(
                ActionId::CommitPolygon,
                ActionContext {
                    polygon_vertices: 3,
                    ..context
                }
            ),
            Availability::Enabled
        );
        assert_eq!(
            availability(
                ActionId::Undo,
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
            availability(ActionId::PolygonTool, context),
            Availability::Hidden
        );
        assert!(
            availability(ActionId::FitView, context)
                .disabled_reason()
                .is_some()
        );
    }
}
