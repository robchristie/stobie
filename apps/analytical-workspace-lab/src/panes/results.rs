use eframe::egui;
use polyorama_core::{
    ImageIntent, PaneId, RESULT_COUNT, ResultId, VirtualisationMetrics, result_at, virtual_rows,
};
use polyorama_ui_egui::{
    DomainReference, ResultRowSpec, SemanticUiId, TextOverflow, TextRole, UiNode, UiRole,
    measured_content_label, result_row, result_row_height,
};

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
    let toolbar = ui.vertical(|ui| {
        measured_content_label(
            ui,
            5_000,
            &format!("{} logical detections", RESULT_COUNT),
            TextRole::Secondary,
            TextOverflow::Ellipsis,
            1,
            tokens,
            font_scale,
            &mut outputs.ui_geometry.text_layouts,
        );
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
    const OVERSCAN_ROWS: usize = 8;
    let row_height = result_row_height(tokens, font_scale);
    let output = egui::ScrollArea::vertical()
        .id_salt("million-row-results")
        .show_viewport(ui, |ui, viewport| {
            let rows = virtual_rows(
                viewport.top(),
                viewport.height(),
                row_height,
                RESULT_COUNT as usize,
                OVERSCAN_ROWS,
            );
            virtualisation.visible_rows = (rows.visible.start, rows.visible.end);
            virtualisation.materialised_rows = rows.materialised.len();
            virtualisation.row_overscan = rows.overscan;
            let origin = ui.min_rect().min;
            ui.set_min_height(RESULT_COUNT as f32 * row_height);
            for index in rows.materialised {
                let result = result_at(index as u64);
                let selected = selected_result == Some(result.id);
                let row_rect = egui::Rect::from_min_size(
                    origin + egui::vec2(0.0, index as f32 * row_height),
                    egui::vec2(ui.available_width(), row_height),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                    let identifier = format!("#{:07}", result.id.0);
                    let position = format!("{:.0}, {:.0}", result.position.x, result.position.y);
                    let confidence = format!("{:.1}%", result.confidence * 100.0);
                    let category =
                        ["Target", "Edge", "Cluster", "Review"][result.category as usize];
                    let semantic_name =
                        format!("{identifier}; {position}; {confidence}; {category}");
                    let mut row_observations = Vec::new();
                    let selection = result_row(
                        ui,
                        ResultRowSpec {
                            instance: result.id.0,
                            identifier: &identifier,
                            position: &position,
                            confidence: &confidence,
                            category,
                            selected,
                        },
                        tokens,
                        font_scale,
                        &mut row_observations,
                    );
                    let inside_root = outputs
                        .ui_geometry
                        .root
                        .is_some_and(|root| root.contains(selection.rect.into(), 1.0));
                    if selection.rect.intersects(ui.clip_rect()) && inside_root {
                        outputs.ui_geometry.text_layouts.extend(row_observations);
                        outputs
                            .ui_geometry
                            .result_rows
                            .push(crate::ui_geometry::ResultUiRect {
                                result: result.id,
                                rect: selection.rect.into(),
                            });
                        outputs.ui_geometry.record_node(UiNode {
                            id: SemanticUiId::new(format!("polyorama.result-row.{}", result.id.0)),
                            parent: Some(SemanticUiId::pane(PaneId(5))),
                            role: UiRole::ResultRow,
                            name: semantic_name,
                            description: None,
                            rect: selection.rect.into(),
                            enabled: true,
                            focused: selection.has_focus(),
                            selected,
                            checked: None,
                            expanded: None,
                            pane: Some(PaneId(5)),
                            domain_reference: Some(DomainReference::Result(result.id)),
                            // Selection is the ListBoxOption's click behaviour, not a
                            // globally registered application action.
                            actions: Vec::new(),
                            disabled_reason: None,
                        });
                    }
                    if selection.clicked() {
                        outputs
                            .intents
                            .push(ImageIntent::SelectResult { result: result.id });
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_geometry::UiGeometry;
    use polyorama_ui_egui::{DensityVariant, ThemeVariant, audit_text_layouts};

    #[test]
    fn virtual_result_pitch_tracks_density_and_bounded_font_scale() {
        let comfortable = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let compact = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);

        assert_eq!(
            result_row_height(&comfortable, 1.0),
            comfortable
                .geometry
                .control_height
                .0
                .max(comfortable.typography.body_size.0 * comfortable.typography.line_height.0)
        );
        assert_eq!(
            result_row_height(&compact, 1.5),
            compact
                .geometry
                .control_height
                .0
                .max(compact.typography.body_size.0 * 1.5 * compact.typography.line_height.0)
        );
        assert_eq!(
            result_row_height(&compact, f32::INFINITY),
            result_row_height(&compact, 1.5)
        );
    }

    #[test]
    fn visible_virtual_result_rows_have_a_clean_measured_text_audit() {
        let context = egui::Context::default();
        let root = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(720.0, 360.0));
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let mut outputs = FrameOutput {
            ui_geometry: UiGeometry::new(root, 1.0),
            ..Default::default()
        };
        let mut pane = UiNode::container(
            SemanticUiId::pane(PaneId(5)),
            Some(SemanticUiId::root()),
            UiRole::Pane,
            root.into(),
        );
        pane.name = "Results".into();
        pane.pane = Some(PaneId(5));
        outputs.ui_geometry.record_node(pane);
        let mut virtualisation = VirtualisationMetrics::default();

        let mut frame = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |ui| {
                ui.set_clip_rect(root);
                show(
                    ui,
                    Some(ResultId(0)),
                    &mut virtualisation,
                    &tokens,
                    1.5,
                    PaneId(5),
                    &mut outputs,
                );
            },
        );
        frame.textures_delta.clear();

        let findings = audit_text_layouts(&outputs.ui_geometry.text_layouts);
        assert!(findings.is_empty(), "{findings:#?}");
        assert!(virtualisation.materialised_rows < 64);
        assert_eq!(virtualisation.row_overscan, 8);
    }
}
