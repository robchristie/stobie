use egui::{Rect, Sense};

use super::{ComponentTextSpec, paint_text_observation};
use crate::{
    DesignTokens, PaneWidthClass, TextComponentId, TextInteraction, TextOverflow, TextRole,
    TextSpec,
};

/// Present a deterministic property label/value pair. Narrow layouts stack;
/// regular and wide layouts align the two columns.
pub fn property_row(
    ui: &mut egui::Ui,
    instance: u64,
    label: &str,
    value: &str,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) {
    let width = ui.available_width().max(1.0);
    let narrow = PaneWidthClass::from_points(width) == PaneWidthClass::Narrow;
    let line_height = tokens.typography.body_size.0 * font_scale * tokens.typography.line_height.0;
    let height = if narrow {
        line_height * 3.0 + tokens.spacing.block.0
    } else {
        line_height * 2.0
    };
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let parent = TextComponentId::new(crate::TextComponentKind::PropertyRow, instance);
    let (label_rect, value_rect) = if narrow {
        let label_rect =
            Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + line_height));
        let value_rect = Rect::from_min_max(
            egui::pos2(rect.min.x, label_rect.max.y + tokens.spacing.block.0),
            rect.max,
        );
        (label_rect, value_rect)
    } else {
        let label_width = (width * 0.34).clamp(96.0, 220.0);
        let label_rect =
            Rect::from_min_max(rect.min, egui::pos2(rect.min.x + label_width, rect.max.y));
        let value_rect = Rect::from_min_max(
            egui::pos2(label_rect.max.x + tokens.spacing.inline.0, rect.min.y),
            rect.max,
        );
        (label_rect, value_rect)
    };
    let label_response = paint_text_observation(
        ui,
        ComponentTextSpec {
            text: label,
            rect: label_rect,
            spec: TextSpec::single_line(TextRole::Secondary, TextOverflow::Ellipsis),
            component_id: TextComponentId::new(crate::TextComponentKind::PropertyRow, instance * 2),
            parent_id: Some(parent),
            accessible: true,
        },
        tokens,
        font_scale,
        observations,
    );
    let value_response = paint_text_observation(
        ui,
        ComponentTextSpec {
            text: value,
            rect: value_rect,
            spec: TextSpec {
                interaction: TextInteraction::Selectable,
                max_lines: if narrow { 2 } else { 1 },
                overflow: if narrow {
                    TextOverflow::Wrap
                } else {
                    TextOverflow::Ellipsis
                },
                ..TextSpec::single_line(TextRole::Body, TextOverflow::Ellipsis)
            },
            component_id: TextComponentId::new(
                crate::TextComponentKind::PropertyRow,
                instance * 2 + 1,
            ),
            parent_id: Some(parent),
            accessible: false,
        },
        tokens,
        font_scale,
        observations,
    );
    if let (Some(label_response), Some(value_response)) = (label_response, value_response) {
        value_response.labelled_by(label_response.id);
    }
}
