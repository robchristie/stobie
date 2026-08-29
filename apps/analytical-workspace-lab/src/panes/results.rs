use eframe::egui;
use polyorama_core::{
    ImageIntent, PaneId, RESULT_COUNT, ResultId, VirtualisationMetrics, result_at, virtual_rows,
};
use polyorama_ui_egui::{DomainReference, SemanticUiId, UiNode, UiRole};

use crate::actions::{ActionContext, availability};

use super::*;

pub fn show(
    ui: &mut egui::Ui,
    selected_result: Option<ResultId>,
    virtualisation: &mut VirtualisationMetrics,
    tokens: &DesignTokens,
    font_scale: f32,
    active_pane: PaneId,
    outputs: &mut FrameOutput,
) {
    let toolbar_id = SemanticUiId::new("pane.5.toolbar");
    let toolbar = ui.horizontal(|ui| {
        ui.label(format!("{} logical detections", RESULT_COUNT));
        if let Some(selected) = selected_result {
            let context = ActionContext {
                active_pane,
                target_pane: Some(PaneId(5)),
                selected_result,
                ..Default::default()
            };
            if present_action(
                ui,
                outputs,
                tokens,
                font_scale,
                &toolbar_id,
                ActionTarget::pane(ActionId::RecenterPrimary, PaneId(5)),
                availability(ActionId::RecenterPrimary, context),
                false,
                false,
                active_pane == PaneId(5),
                "recenter_primary",
            ) {
                outputs.intents.push(ImageIntent::RecenterOnResult {
                    result: selected,
                    pane: PaneId(1),
                });
            }
        }
    });
    let mut toolbar_node = UiNode::container(
        toolbar_id,
        Some(SemanticUiId::pane(PaneId(5))),
        UiRole::Toolbar,
        toolbar.response.rect.into(),
    );
    toolbar_node.name = "Result actions".into();
    toolbar_node.pane = Some(PaneId(5));
    outputs.ui_geometry.record_node(toolbar_node);
    const ROW_HEIGHT: f32 = 23.0;
    const OVERSCAN_ROWS: usize = 8;
    let output = egui::ScrollArea::vertical()
        .id_salt("million-row-results")
        .show_viewport(ui, |ui, viewport| {
            let rows = virtual_rows(
                viewport.top(),
                viewport.height(),
                ROW_HEIGHT,
                RESULT_COUNT as usize,
                OVERSCAN_ROWS,
            );
            virtualisation.visible_rows = (rows.visible.start, rows.visible.end);
            virtualisation.materialised_rows = rows.materialised.len();
            virtualisation.row_overscan = rows.overscan;
            let origin = ui.min_rect().min;
            ui.set_min_height(RESULT_COUNT as f32 * ROW_HEIGHT);
            for index in rows.materialised {
                let result = result_at(index as u64);
                let selected = selected_result == Some(result.id);
                let row_rect = egui::Rect::from_min_size(
                    origin + egui::vec2(0.0, index as f32 * ROW_HEIGHT),
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                    ui.horizontal(|ui| {
                        let selection =
                            ui.selectable_label(selected, format!("#{:07}", result.id.0));
                        let semantic_name = format!("Result #{:07}", result.id.0);
                        ui.ctx().accesskit_node_builder(selection.id, |node| {
                            use egui::accesskit::{Action, Role};
                            node.set_role(Role::ListBoxOption);
                            node.set_label(semantic_name.clone());
                            node.set_description(format!(
                                "Position {:.1}, {:.1}; confidence {:.1}%",
                                result.position.x,
                                result.position.y,
                                result.confidence * 100.0
                            ));
                            node.set_author_id(format!("result.{}", result.id.0));
                            node.set_selected(selected);
                            node.add_action(Action::Click);
                        });
                        let inside_root = outputs
                            .ui_geometry
                            .root
                            .is_some_and(|root| root.contains(selection.rect.into(), 1.0));
                        if selection.rect.intersects(ui.clip_rect()) && inside_root {
                            outputs.ui_geometry.result_rows.push(
                                crate::ui_geometry::ResultUiRect {
                                    result: result.id,
                                    rect: selection.rect.into(),
                                },
                            );
                            outputs.ui_geometry.record_node(UiNode {
                                id: SemanticUiId::new(format!("result.{}", result.id.0)),
                                parent: Some(SemanticUiId::pane(PaneId(5))),
                                role: UiRole::ResultRow,
                                name: semantic_name,
                                description: Some(format!(
                                    "Position {:.1}, {:.1}; confidence {:.1}%",
                                    result.position.x,
                                    result.position.y,
                                    result.confidence * 100.0
                                )),
                                rect: selection.rect.into(),
                                enabled: true,
                                focused: selection.has_focus(),
                                selected,
                                checked: None,
                                expanded: None,
                                pane: Some(PaneId(5)),
                                domain_reference: Some(DomainReference::Result(result.id)),
                                actions: Vec::new(),
                                disabled_reason: None,
                            });
                        }
                        if selection.clicked() {
                            outputs
                                .intents
                                .push(ImageIntent::SelectResult { result: result.id });
                        }
                        ui.monospace(format!(
                            "{:>8.0}  {:>8.0}",
                            result.position.x, result.position.y
                        ));
                        ui.label(format!("{:>5.1}%", result.confidence * 100.0));
                        ui.label(["Target", "Edge", "Cluster", "Review"][result.category as usize]);
                    });
                });
            }
        });
    outputs.ui_geometry.results_scroll = Some(output.inner_rect.into());
    let mut scroll = UiNode::container(
        SemanticUiId::new("pane.5.results.scroll"),
        Some(SemanticUiId::pane(PaneId(5))),
        UiRole::ScrollArea,
        output.inner_rect.into(),
    );
    scroll.name = "Results".into();
    scroll.pane = Some(PaneId(5));
    outputs.ui_geometry.record_node(scroll);
}
