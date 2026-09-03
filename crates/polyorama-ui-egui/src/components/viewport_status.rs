use egui::{Rect, Stroke};

use crate::{
    DesignTokens, HorizontalTextAlignment, TextComponentId, TextInteraction, TextOverflow,
    TextRole, TextSpec, measure_text, present_measured_text,
};

pub fn image_status_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    (tokens.typography.body_size.0 * font_scale.clamp(1.0, 1.5) * tokens.typography.line_height.0
        + tokens.spacing.block.0 * 2.0)
        .max(tokens.geometry.minimum_hit_size.0)
}

pub struct ImageStatusSpec<'a> {
    pub instance: u64,
    pub rect: Rect,
    pub coordinates: &'a str,
    pub detail: &'a str,
}

/// Paint a clipped analytical viewport status strip from semantic tokens.
pub fn paint_image_status(
    ui: &mut egui::Ui,
    spec: ImageStatusSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
) -> Vec<crate::TextLayoutObservation> {
    let painter = ui.painter().clone();
    painter.rect_filled(spec.rect, 0.0, tokens.colours.surface_panel);
    painter.line_segment(
        [spec.rect.left_top(), spec.rect.right_top()],
        Stroke::new(1.0, tokens.colours.border_subtle),
    );
    let padding = tokens.spacing.inline.0;
    let right_width = (tokens.geometry.minimum_hit_size.0 * 3.0).min(spec.rect.width() * 0.35);
    let left = Rect::from_min_max(
        spec.rect.min + egui::vec2(padding, 0.0),
        egui::pos2(
            (spec.rect.right() - right_width - padding).max(spec.rect.left()),
            spec.rect.bottom(),
        ),
    );
    let right = Rect::from_min_max(
        egui::pos2(
            (spec.rect.right() - right_width).max(spec.rect.left()),
            spec.rect.top(),
        ),
        spec.rect.max - egui::vec2(padding, 0.0),
    );
    let parent = TextComponentId::new(crate::TextComponentKind::ImageStatus, spec.instance);
    let mut observations = Vec::new();
    for (index, (text, rect, role, alignment)) in [
        (
            spec.coordinates,
            left,
            TextRole::MonospaceTechnical,
            HorizontalTextAlignment::Start,
        ),
        (
            spec.detail,
            right,
            TextRole::Caption,
            HorizontalTextAlignment::End,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let text_spec = TextSpec {
            horizontal_alignment: alignment,
            interaction: TextInteraction::Selectable,
            ..TextSpec::single_line(role, TextOverflow::Ellipsis)
        };
        if let Ok(measured) = measure_text(
            &painter,
            text,
            text_spec,
            tokens,
            font_scale,
            rect.width().max(0.5),
        ) {
            let (_, observation) = present_measured_text(
                ui,
                &measured,
                rect,
                TextComponentId::new(
                    crate::TextComponentKind::ImageStatus,
                    spec.instance * 4 + index as u64,
                ),
                Some(parent),
            );
            observations.push(observation);
        }
    }
    observations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant};

    #[test]
    fn image_status_uses_measured_clipped_text_at_all_supported_scales() {
        let context = egui::Context::default();
        let tokens =
            DesignTokens::resolve(ThemeVariant::LightHighContrast, DensityVariant::Compact);
        for font_scale in [1.0, 1.25, 1.5] {
            let root = Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(280.0, image_status_height(&tokens, font_scale)),
            );
            let mut observations = Vec::new();
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(root),
                    ..Default::default()
                },
                |ui| {
                    observations = paint_image_status(
                        ui,
                        ImageStatusSpec {
                            instance: 1,
                            rect: root,
                            coordinates: "image 123456.7, 765432.1 · world 1234567890.1, 9876543210.2",
                            detail: "L12 · 64 tiles",
                        },
                        &tokens,
                        font_scale,
                    );
                },
            );
            output.textures_delta.clear();
            assert_eq!(observations.len(), 2);
            assert!(crate::audit_text_layouts(&observations).is_empty());
            assert!(observations.iter().all(|item| item.allocated_rect.max_x
                >= item.allocated_rect.min_x
                && item.interaction == TextInteraction::Selectable));
        }
    }
}
