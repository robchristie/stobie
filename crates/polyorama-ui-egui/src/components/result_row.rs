use egui::{Rect, Response, Sense, Stroke};

use super::{ComponentTextSpec, paint_text_observation};
use crate::{
    DesignTokens, HorizontalTextAlignment, TextComponentId, TextOverflow, TextRole, TextSpec,
};

pub struct ResultRowSpec<'a> {
    pub instance: u64,
    pub identifier: &'a str,
    pub position: &'a str,
    pub confidence: &'a str,
    pub category: &'a str,
    pub selected: bool,
}

pub fn result_row_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens.geometry.control_height.0.max(
        tokens.typography.body_size.0
            * font_scale.clamp(1.0, 1.5)
            * tokens.typography.line_height.0,
    )
}

pub fn result_row(
    ui: &mut egui::Ui,
    spec: ResultRowSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let width = ui.available_width().max(1.0);
    let height = result_row_height(tokens, font_scale);
    let (_, rect) = ui.allocate_space(egui::vec2(width, height));
    let response = ui.interact(
        rect,
        egui::Id::new(("polyorama.result-row", spec.instance)),
        Sense::click(),
    );
    if response.clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::SelectableLabel,
            true,
            format!(
                "{}; {}; {}; {}",
                spec.identifier, spec.position, spec.confidence, spec.category
            ),
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::ListBoxOption);
        node.set_label(format!(
            "{}; {}; {}; {}",
            spec.identifier, spec.position, spec.confidence, spec.category
        ));
        node.set_author_id(format!("polyorama.result-row.{}", spec.instance));
        node.clear_toggled();
        node.set_selected(spec.selected);
        node.add_action(Action::Click);
    });
    if spec.selected {
        ui.painter()
            .rect_filled(rect, 0.0, tokens.colours.selection_background);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let parent = TextComponentId::new(crate::TextComponentKind::ResultRow, spec.instance);
    let values = [
        (spec.identifier, 0.18, HorizontalTextAlignment::Start),
        (spec.position, 0.42, HorizontalTextAlignment::End),
        (spec.confidence, 0.20, HorizontalTextAlignment::End),
        (spec.category, 0.20, HorizontalTextAlignment::Start),
    ];
    let mut x = rect.min.x;
    for (index, (text, fraction, alignment)) in values.into_iter().enumerate() {
        let next = if index == 3 {
            rect.max.x
        } else {
            x + rect.width() * fraction
        };
        let cell = Rect::from_min_max(
            egui::pos2(x + tokens.spacing.unit.0, rect.min.y),
            egui::pos2(
                (next - tokens.spacing.unit.0).max(x + tokens.spacing.unit.0 + 0.5),
                rect.max.y,
            ),
        );
        paint_text_observation(
            ui,
            ComponentTextSpec {
                text,
                rect: cell,
                spec: TextSpec {
                    horizontal_alignment: alignment,
                    ..TextSpec::single_line(
                        if index == 1 || index == 2 {
                            TextRole::TabularValue
                        } else {
                            TextRole::Body
                        },
                        TextOverflow::Ellipsis,
                    )
                },
                component_id: TextComponentId::new(
                    crate::TextComponentKind::ResultRow,
                    spec.instance * 8 + index as u64,
                ),
                parent_id: Some(parent),
                accessible: false,
            },
            tokens,
            font_scale,
            observations,
        );
        x = next;
    }
    response
}
