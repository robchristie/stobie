use eframe::egui;
use polyorama_core::{AnnotationId, ImageIntent, PaneId, ResultId, result_at};
use polyorama_ui_egui::{
    DesignTokens, DomainReference, SemanticUiId, UiNode, UiRole, property_row, section_heading,
};

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
    let selection_id = SemanticUiId::new("pane.7.selection");
    let selection = ui.scope(|ui| {
        section_heading(
            ui,
            7_001,
            "Selection",
            tokens,
            font_scale,
            &mut outputs.ui_geometry.text_layouts,
        );
        if let Some(selected) = selected_result {
            let result = result_at(selected.0);
            let identifier = format!("#{}", result.id.0);
            let position = format!("{:.1}, {:.1}", result.position.x, result.position.y);
            let confidence = format!("{:.2}%", result.confidence * 100.0);
            let category = ["Target", "Edge", "Cluster", "Review"][result.category as usize];
            for (instance, label, value) in [
                (7_010, "Result", identifier.as_str()),
                (7_011, "Position", position.as_str()),
                (7_012, "Confidence", confidence.as_str()),
                (7_013, "Category", category),
            ] {
                property_row(
                    ui,
                    instance,
                    label,
                    value,
                    tokens,
                    font_scale,
                    &mut outputs.ui_geometry.text_layouts,
                );
            }
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
                &selection_id,
                ActionTarget::pane(LabAction::RecenterPrimary, PaneId(7)),
                availability(LabAction::RecenterPrimary, context),
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
            property_row(
                ui,
                7_014,
                "Result",
                "No result selected",
                tokens,
                font_scale,
                &mut outputs.ui_geometry.text_layouts,
            );
        }
    });
    record_section(
        outputs,
        selection_id,
        "Selection",
        selection.response.rect,
        ui.clip_rect(),
        selected_result.map(DomainReference::Result),
    );

    ui.separator();
    let annotation_id = SemanticUiId::new("pane.7.annotation");
    let annotation = ui.scope(|ui| {
        section_heading(
            ui,
            7_002,
            "Annotation",
            tokens,
            font_scale,
            &mut outputs.ui_geometry.text_layouts,
        );
        let value = selected_annotation.map_or_else(
            || "No polygon selected".to_owned(),
            |annotation| format!("Polygon {}", annotation.0),
        );
        property_row(
            ui,
            7_020,
            "Annotation",
            &value,
            tokens,
            font_scale,
            &mut outputs.ui_geometry.text_layouts,
        );
    });
    record_section(
        outputs,
        annotation_id,
        "Annotation",
        annotation.response.rect,
        ui.clip_rect(),
        selected_annotation.map(DomainReference::Annotation),
    );
}

fn record_section(
    outputs: &mut FrameOutput,
    id: SemanticUiId,
    name: &str,
    rect: egui::Rect,
    clip_rect: egui::Rect,
    domain_reference: Option<DomainReference>,
) {
    let Some(root) = outputs.ui_geometry.root else {
        return;
    };
    let root_rect = egui::Rect::from_min_max(
        egui::pos2(root.min_x, root.min_y),
        egui::pos2(root.max_x, root.max_y),
    );
    let bounded = rect.intersect(clip_rect).intersect(root_rect);
    if !bounded.is_positive() {
        return;
    }
    let mut node = UiNode::container(
        id,
        Some(SemanticUiId::pane(PaneId(7))),
        UiRole::Section,
        bounded.into(),
    );
    node.name = name.to_owned();
    node.pane = Some(PaneId(7));
    node.domain_reference = domain_reference;
    outputs.ui_geometry.record_node(node);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_geometry::UiGeometry;

    #[test]
    fn inspector_sections_are_clipped_to_the_current_root() {
        let root = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(240.0, 180.0));
        let mut outputs = FrameOutput {
            ui_geometry: UiGeometry::new(root, 1.0),
            ..Default::default()
        };
        let mut pane = UiNode::container(
            SemanticUiId::pane(PaneId(7)),
            Some(SemanticUiId::root()),
            UiRole::Pane,
            root.into(),
        );
        pane.name = "Inspector".into();
        pane.pane = Some(PaneId(7));
        outputs.ui_geometry.record_node(pane);

        record_section(
            &mut outputs,
            SemanticUiId::new("pane.7.selection"),
            "Selection",
            egui::Rect::from_min_max(egui::pos2(-20.0, 24.0), egui::pos2(180.0, 220.0)),
            egui::Rect::EVERYTHING,
            Some(DomainReference::Result(ResultId(42))),
        );

        let snapshot = outputs.ui_geometry.snapshot(1);
        let section = snapshot
            .node(&SemanticUiId::new("pane.7.selection"))
            .expect("selection section");
        assert_eq!(section.role, UiRole::Section);
        assert_eq!(
            section.domain_reference,
            Some(DomainReference::Result(ResultId(42)))
        );
        assert!(
            snapshot
                .node(&snapshot.root)
                .expect("root")
                .rect
                .contains(section.rect, 0.0)
        );
        assert!(
            snapshot.semantic_audit.is_empty(),
            "{:#?}",
            snapshot.semantic_audit
        );
    }
}
