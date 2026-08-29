use egui::{Rect, Response, Sense};

use crate::{
    DesignTokens, HorizontalTextAlignment, PaneWidthClass, TextComponentId, TextComponentKind,
    TextLayoutObservation, TextOverflow, TextRole, TextSpec, VerticalTextAlignment, measure_text,
    paint_measured_text,
};

/// A measured full-width content label for pane-local status and explanatory
/// text. It wraps only when requested and keeps the complete text in widget
/// semantics and a truncation tooltip.
#[allow(clippy::too_many_arguments)]
pub fn measured_content_label(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    role: TextRole,
    overflow: TextOverflow,
    max_lines: u8,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    let width = ui.available_width().max(1.0);
    let line_height = tokens.typography.body_size.0 * font_scale * tokens.typography.line_height.0;
    let height = line_height * f32::from(max_lines.max(1));
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    if let Ok(measured) = measure_text(
        ui.painter(),
        text,
        TextSpec {
            role,
            overflow,
            horizontal_alignment: HorizontalTextAlignment::Start,
            vertical_alignment: VerticalTextAlignment::Centre,
            max_lines,
        },
        tokens,
        font_scale,
        rect.width().max(0.5),
    ) {
        let truncated = measured.truncated();
        observations.push(paint_measured_text(
            &ui.painter_at(rect),
            &measured,
            rect,
            TextComponentId::new(TextComponentKind::ContentLabel, instance),
            None,
        ));
        if truncated {
            response.clone().on_hover_text(text);
        }
    }
    response
}

/// Paint one measured, single-line label within an explicit chrome width.
/// The caller owns responsive allocation; the component owns truncation and
/// retains the complete text in widget semantics and its hover completion.
#[allow(clippy::too_many_arguments)]
pub fn measured_inline_label(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    role: TextRole,
    maximum_width: f32,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> egui::Response {
    let line_height = role.style(tokens, font_scale).font_id.size * tokens.typography.line_height.0;
    let size = egui::vec2(
        maximum_width.max(tokens.geometry.minimum_hit_size.0),
        line_height.max(tokens.geometry.minimum_hit_size.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    let spec = TextSpec::single_line(role, TextOverflow::Ellipsis);
    if let Ok(measured) = measure_text(
        ui.painter(),
        text,
        spec,
        tokens,
        font_scale,
        rect.width().max(0.5),
    ) {
        let truncated = measured.truncated();
        observations.push(paint_measured_text(
            &ui.painter_at(rect),
            &measured,
            rect,
            TextComponentId::new(TextComponentKind::ApplicationBarLabel, instance),
            None,
        ));
        if truncated {
            response.clone().on_hover_text(text);
        }
    }
    response
}

/// One measured, single-line section heading with complete label semantics.
/// The visible text elides at the pane edge and the full value remains in the
/// widget label and truncation tooltip.
pub fn section_heading(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    let width = ui.available_width().max(1.0);
    let line_height = tokens.typography.label_size.0 * font_scale * tokens.typography.line_height.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, line_height + tokens.spacing.block.0),
        Sense::hover(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    if let Ok(measured) = measure_text(
        ui.painter(),
        text,
        TextSpec::single_line(TextRole::SectionHeading, TextOverflow::Ellipsis),
        tokens,
        font_scale,
        rect.width().max(0.5),
    ) {
        let truncated = measured.truncated();
        observations.push(paint_measured_text(
            &ui.painter_at(rect),
            &measured,
            rect,
            TextComponentId::new(TextComponentKind::SectionHeading, instance),
            None,
        ));
        if truncated {
            response.clone().on_hover_text(text);
        }
    }
    response
}

/// One technical label/value pair for diagnostics. Regular panes align and
/// end-align the value; narrow panes stack and wrap it to two lines. The full
/// pair remains available to widget semantics and a tooltip when elided.
pub fn diagnostic_row(
    ui: &mut egui::Ui,
    instance: u64,
    label: &str,
    value: &str,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    let width = ui.available_width().max(1.0);
    let narrow = PaneWidthClass::from_points(width) == PaneWidthClass::Narrow;
    let line_height = tokens.typography.body_size.0 * font_scale * tokens.typography.line_height.0;
    let height = if narrow {
        line_height * 3.0 + tokens.spacing.block.0
    } else {
        line_height * 1.5
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("{label}: {value}"))
    });
    let parent = TextComponentId::new(TextComponentKind::DiagnosticRow, instance);
    let (label_rect, value_rect) = diagnostic_rects(rect, narrow, tokens, line_height);
    let label_truncated = paint(
        ui,
        label,
        label_rect,
        TextSpec::single_line(TextRole::Secondary, TextOverflow::Ellipsis),
        TextComponentId::new(TextComponentKind::DiagnosticRow, instance * 4),
        parent,
        tokens,
        font_scale,
        observations,
    );
    let value_truncated = paint(
        ui,
        value,
        value_rect,
        TextSpec {
            role: TextRole::MonospaceTechnical,
            overflow: if narrow {
                TextOverflow::Wrap
            } else {
                TextOverflow::Ellipsis
            },
            horizontal_alignment: if narrow {
                HorizontalTextAlignment::Start
            } else {
                HorizontalTextAlignment::End
            },
            vertical_alignment: VerticalTextAlignment::Centre,
            max_lines: if narrow { 2 } else { 1 },
        },
        TextComponentId::new(TextComponentKind::DiagnosticRow, instance * 4 + 1),
        parent,
        tokens,
        font_scale,
        observations,
    );
    if label_truncated || value_truncated {
        response.clone().on_hover_text(format!("{label}: {value}"));
    }
    response
}

fn diagnostic_rects(
    rect: Rect,
    narrow: bool,
    tokens: &DesignTokens,
    line_height: f32,
) -> (Rect, Rect) {
    if narrow {
        let label = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + line_height));
        let value = Rect::from_min_max(
            egui::pos2(rect.min.x, label.max.y + tokens.spacing.block.0),
            rect.max,
        );
        (label, value)
    } else {
        let split = rect.min.x + rect.width() * 0.42;
        (
            Rect::from_min_max(rect.min, egui::pos2(split, rect.max.y)),
            Rect::from_min_max(
                egui::pos2(split + tokens.spacing.inline.0, rect.min.y),
                rect.max,
            ),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &egui::Ui,
    text: &str,
    rect: Rect,
    spec: TextSpec,
    component: TextComponentId,
    parent: TextComponentId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> bool {
    let Ok(measured) = measure_text(
        ui.painter(),
        text,
        spec,
        tokens,
        font_scale,
        rect.width().max(0.5),
    ) else {
        return false;
    };
    let truncated = measured.truncated();
    observations.push(paint_measured_text(
        &ui.painter_at(rect),
        &measured,
        rect,
        component,
        Some(parent),
    ));
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant, audit_text_layouts};

    #[test]
    fn technical_rows_are_measured_and_end_aligned_until_the_pane_is_narrow() {
        for (width, expected_alignment, expected_lines) in [
            (520.0, HorizontalTextAlignment::End, 1),
            (260.0, HorizontalTextAlignment::Start, 2),
        ] {
            let context = egui::Context::default();
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let mut observations = Vec::new();
            let mut frame = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 180.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    section_heading(ui, 1, "Workers", &tokens, 1.5, &mut observations);
                    diagnostic_row(
                        ui,
                        2,
                        "Last failure",
                        "A deliberately long worker failure value that must remain observable",
                        &tokens,
                        1.5,
                        &mut observations,
                    );
                },
            );
            frame.textures_delta.clear();
            let value = observations
                .iter()
                .find(|item| item.role == TextRole::MonospaceTechnical)
                .expect("technical value");
            assert_eq!(value.horizontal_alignment, expected_alignment);
            assert_eq!(value.declared_max_lines, expected_lines);
            assert!(audit_text_layouts(&observations).is_empty());
        }
    }
}
