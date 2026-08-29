use eframe::egui;
use polyorama_core::{AnnotationId, ImageIntent, PaneId, ResultId, result_at};
use polyorama_ui_egui::{DesignTokens, SemanticUiId};

use crate::actions::{ActionContext, availability};

use super::*;

pub fn show(
    ui: &mut egui::Ui,
    selected_result: Option<ResultId>,
    selected_annotation: Option<AnnotationId>,
    tokens: &DesignTokens,
    font_scale: f32,
    active_pane: PaneId,
    outputs: &mut FrameOutput,
) {
    ui.heading("Selection");
    if let Some(selected) = selected_result {
        let result = result_at(selected.0);
        egui::Grid::new("result-inspector")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Result");
                ui.monospace(format!("#{}", result.id.0));
                ui.end_row();
                ui.label("Position");
                ui.monospace(format!(
                    "{:.1}, {:.1}",
                    result.position.x, result.position.y
                ));
                ui.end_row();
                ui.label("Confidence");
                ui.label(format!("{:.2}%", result.confidence * 100.0));
                ui.end_row();
                ui.label("Category");
                ui.label(["Target", "Edge", "Cluster", "Review"][result.category as usize]);
                ui.end_row();
            });
        let context = ActionContext {
            active_pane,
            target_pane: Some(PaneId(7)),
            selected_result,
            ..Default::default()
        };
        if present_action(
            ui,
            outputs,
            tokens,
            font_scale,
            &SemanticUiId::pane(PaneId(7)),
            ActionTarget::pane(ActionId::RecenterPrimary, PaneId(7)),
            availability(ActionId::RecenterPrimary, context),
            false,
            false,
            active_pane == PaneId(7),
            "recenter_primary",
        ) {
            outputs.intents.push(ImageIntent::RecenterOnResult {
                result: selected,
                pane: PaneId(1),
            });
        }
    } else {
        ui.label("No result selected");
    }
    ui.separator();
    ui.heading("Annotation");
    if let Some(annotation) = selected_annotation {
        ui.monospace(format!("Polygon {}", annotation.0));
    } else {
        ui.label("No polygon selected");
    }
}
