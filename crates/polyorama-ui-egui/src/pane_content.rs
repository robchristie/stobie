use egui::{Rect, Response, Sense};

use crate::{
    DesignTokens, HorizontalTextAlignment, PaneWidthClass, TextComponentId, TextComponentKind,
    TextInteraction, TextLayoutObservation, TextOverflow, TextRole, TextSpec,
    VerticalTextAlignment, paint_measured_text, present_accessible_measured_text,
    present_measured_text,
};

/// Measure first, then allocate the actual bounded content height. `max_lines`
/// is an overflow limit, not a request to reserve empty lines.
#[allow(clippy::too_many_arguments)]
pub fn measured_content_label(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    role: TextRole,
    overflow: TextOverflow,
    max_lines: u8,
    interaction: TextInteraction,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    measured_label(
        ui,
        instance,
        text,
        role,
        overflow,
        max_lines,
        interaction,
        tokens,
        font_scale,
        observations,
        false,
    )
}

/// Reserve a deliberate line slot for fixed-height tables or virtualised rows.
/// Invalid requests use a one-line visible fallback and remain audit failures.
#[allow(clippy::too_many_arguments)]
pub fn measured_fixed_slot_label(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    role: TextRole,
    overflow: TextOverflow,
    max_lines: u8,
    interaction: TextInteraction,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    measured_label(
        ui,
        instance,
        text,
        role,
        overflow,
        max_lines,
        interaction,
        tokens,
        font_scale,
        observations,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn measured_label(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    role: TextRole,
    overflow: TextOverflow,
    max_lines: u8,
    interaction: TextInteraction,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    fixed_slot: bool,
) -> Response {
    let width = ui.available_width().max(1.0);
    let measured = crate::measure_component_text(
        ui.painter(),
        text,
        TextSpec {
            role,
            overflow,
            interaction,
            horizontal_alignment: HorizontalTextAlignment::Start,
            vertical_alignment: VerticalTextAlignment::Top,
            max_lines,
        },
        tokens,
        font_scale,
        width,
    );
    let height = if fixed_slot && measured.layout_error.is_none() {
        role.style(tokens, font_scale).line_height * f32::from(max_lines)
    } else {
        measured.size().y
    };
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, height.max(1.0)), Sense::hover());
    let component = TextComponentId::new(TextComponentKind::ContentLabel, instance);
    let (text_response, observation) = if interaction == TextInteraction::Selectable {
        let (response, observation) = present_measured_text(ui, &measured, rect, component, None);
        (
            response.expect("selectable measured text returns a response"),
            observation,
        )
    } else {
        present_accessible_measured_text(ui, &measured, rect, component, None)
    };
    observations.push(observation);
    if measured.truncated() {
        text_response.on_hover_text(measured.galley.text());
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
    interaction: TextInteraction,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> egui::Response {
    let line_height = role.style(tokens, font_scale).line_height;
    let size = egui::vec2(
        maximum_width.max(tokens.geometry.minimum_hit_size.0),
        line_height.max(tokens.geometry.minimum_hit_size.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let spec = TextSpec {
        interaction,
        ..TextSpec::single_line(role, TextOverflow::Ellipsis)
    };
    {
        let measured = crate::measure_component_text(
            ui.painter(),
            text,
            spec,
            tokens,
            font_scale,
            rect.width().max(0.5),
        );
        let truncated = measured.truncated();
        let component = TextComponentId::new(TextComponentKind::ApplicationBarLabel, instance);
        let (text_response, observation) = if interaction == TextInteraction::Selectable {
            let (response, observation) =
                present_measured_text(ui, &measured, rect, component, None);
            (
                response.expect("selectable measured text returns a response"),
                observation,
            )
        } else {
            present_accessible_measured_text(ui, &measured, rect, component, None)
        };
        observations.push(observation);
        if truncated {
            text_response.on_hover_text(text);
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
    let line_height = TextRole::SectionHeading
        .style(tokens, font_scale)
        .line_height;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, line_height + tokens.spacing.block.0),
        Sense::hover(),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    {
        let measured = crate::measure_component_text(
            ui.painter(),
            text,
            TextSpec::single_line(TextRole::SectionHeading, TextOverflow::Ellipsis),
            tokens,
            font_scale,
            rect.width().max(0.5),
        );
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
    let parent = TextComponentId::new(TextComponentKind::DiagnosticRow, instance);
    let (label_rect, value_rect) = diagnostic_rects(rect, narrow, tokens, line_height);
    let (label_truncated, label_response) = paint(
        ui,
        label,
        label_rect,
        TextSpec::single_line(TextRole::Secondary, TextOverflow::Ellipsis),
        TextComponentId::new(TextComponentKind::DiagnosticRow, instance * 4),
        parent,
        tokens,
        font_scale,
        observations,
        true,
    );
    let (value_truncated, value_response) = paint(
        ui,
        value,
        value_rect,
        TextSpec {
            role: TextRole::MonospaceTechnical,
            interaction: TextInteraction::Selectable,
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
        false,
    );
    if let (Some(label_response), Some(value_response)) = (&label_response, &value_response) {
        value_response.clone().labelled_by(label_response.id);
    }
    if label_truncated || value_truncated {
        let tooltip = format!("{label}: {value}");
        for text_response in label_response.into_iter().chain(value_response) {
            text_response.on_hover_text(tooltip.clone());
        }
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
    ui: &mut egui::Ui,
    text: &str,
    rect: Rect,
    spec: TextSpec,
    component: TextComponentId,
    parent: TextComponentId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    accessible: bool,
) -> (bool, Option<Response>) {
    let measured = crate::measure_component_text(
        ui.painter(),
        text,
        spec,
        tokens,
        font_scale,
        rect.width().max(0.5),
    );
    let truncated = measured.truncated();
    let (response, observation) = if accessible {
        let (response, observation) =
            present_accessible_measured_text(ui, &measured, rect, component, Some(parent));
        (Some(response), observation)
    } else {
        present_measured_text(ui, &measured, rect, component, Some(parent))
    };
    observations.push(observation);
    (truncated, response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant, audit_text_layouts};

    #[test]
    fn content_height_slots_and_failed_requests_remain_observable() {
        for scale in [1.0, 1.25, 1.5] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            context.enable_accesskit();
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let mut observations = Vec::new();
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let content = measured_content_label(
                        ui,
                        91,
                        "Actual content",
                        TextRole::Body,
                        TextOverflow::Wrap,
                        2,
                        TextInteraction::Selectable,
                        &tokens,
                        scale,
                        &mut observations,
                    );
                    let fixed = measured_fixed_slot_label(
                        ui,
                        92,
                        "Deliberate slot",
                        TextRole::Body,
                        TextOverflow::Wrap,
                        2,
                        TextInteraction::Selectable,
                        &tokens,
                        scale,
                        &mut observations,
                    );
                    assert!(
                        (content.rect.height() - TextRole::Body.style(&tokens, scale).line_height)
                            .abs()
                            <= 1.0
                    );
                    assert!((fixed.rect.height() - content.rect.height() * 2.0).abs() <= 1.0);
                    measured_content_label(
                        ui,
                        93,
                        "Raw durable evidence",
                        TextRole::Body,
                        TextOverflow::Wrap,
                        24,
                        TextInteraction::Selectable,
                        &tokens,
                        scale,
                        &mut observations,
                    );
                    assert_eq!(observations.len(), 3);
                    assert_eq!(
                        observations[2].layout_error,
                        Some(crate::TextLayoutError::InvalidMaxLines(24))
                    );
                    assert!(
                        audit_text_layouts(&observations)
                            .iter()
                            .any(|finding| matches!(
                                finding,
                                crate::TextAuditFinding::LayoutFailed { .. }
                            ))
                    );
                    let coverage = crate::text_audit_coverage(ui.ctx(), &observations);
                    assert_eq!(
                        (
                            coverage.attempted_components,
                            coverage.successful_components,
                            coverage.failed_components
                        ),
                        (3, 2, 1)
                    );
                    let filtered = crate::text_audit_coverage(ui.ctx(), &observations[..2]);
                    assert_eq!(filtered.measured_components, 2);
                    assert_eq!(
                        filtered.failed_components, 1,
                        "consumer filtering must not erase failure"
                    );
                },
            );
            let update = output.platform_output.accesskit_update.take().unwrap();
            let values: Vec<_> = update
                .nodes
                .iter()
                .filter_map(|(_, node)| node.value())
                .collect();
            assert!(values.contains(&"Actual content"));
            assert!(values.contains(&"Deliberate slot"));
            assert!(
                values
                    .iter()
                    .any(|value| value.contains("InvalidMaxLines(24)")
                        && value.contains("Raw durable evidence")),
                "fallback must really exist in widget semantics: {values:?}"
            );
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text().contains("InvalidMaxLines(24)"))), "production fallback must actually paint");
            output.textures_delta.clear();
            let mut next = context.run_ui(Default::default(), |ui| {
                assert_eq!(
                    crate::text_audit_coverage(ui.ctx(), &[]).attempted_components,
                    0
                );
            });
            next.textures_delta.clear();
        }
    }

    #[test]
    fn content_and_diagnostic_text_have_one_accessible_owner_per_string() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let mut observations = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(520.0, 240.0),
                )),
                ..Default::default()
            },
            |ui| {
                measured_content_label(
                    ui,
                    1,
                    "Selectable content",
                    TextRole::Body,
                    TextOverflow::Wrap,
                    2,
                    TextInteraction::Selectable,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                measured_inline_label(
                    ui,
                    2,
                    "Selectable inline",
                    TextRole::Status,
                    240.0,
                    TextInteraction::Selectable,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                measured_content_label(
                    ui,
                    3,
                    "Accessible inert content",
                    TextRole::Body,
                    TextOverflow::Ellipsis,
                    1,
                    TextInteraction::Inert,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                diagnostic_row(ui, 4, "Generation", "42", &tokens, 1.0, &mut observations);
            },
        );
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        let label_count = |label: &str| {
            update
                .nodes
                .iter()
                .map(|(_, node)| node)
                .filter(|node| {
                    node.role() == egui::accesskit::Role::Label && node.value() == Some(label)
                })
                .count()
        };
        for label in [
            "Selectable content",
            "Selectable inline",
            "Accessible inert content",
            "Generation",
            "42",
        ] {
            assert_eq!(label_count(label), 1, "unexpected owner count for {label}");
        }
        assert_eq!(label_count("Generation: 42"), 0);
        let (diagnostic_label_id, _) = update
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == egui::accesskit::Role::Label && node.value() == Some("Generation")
            })
            .expect("diagnostic label node");
        let (_, diagnostic_value) = update
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == egui::accesskit::Role::Label && node.value() == Some("42")
            })
            .expect("diagnostic value node");
        assert_eq!(diagnostic_value.labelled_by(), &[*diagnostic_label_id]);
    }

    #[test]
    fn elided_selectable_diagnostic_value_opens_its_complete_pair_tooltip() {
        use std::{cell::Cell, rc::Rc};

        use egui_kittest::{Harness, kittest::Queryable};

        const LABEL: &str = "Checksum";
        const VALUE: &str =
            "sha256:76e12b7e4d6fcffb8dbb31e67bc06d0f6be9af7f849dabf6e5c4207ce4f5682a";
        const TOOLTIP: &str =
            "Checksum: sha256:76e12b7e4d6fcffb8dbb31e67bc06d0f6be9af7f849dabf6e5c4207ce4f5682a";
        let value_rect = Rc::new(Cell::new(Rect::NOTHING));
        let observed_rect = Rc::clone(&value_rect);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(520.0, 100.0))
            .build_ui(move |ui| {
                let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
                let mut observations = Vec::new();
                diagnostic_row(ui, 1, LABEL, VALUE, &tokens, 1.0, &mut observations);
                let value = observations
                    .iter()
                    .find(|text| text.role == TextRole::MonospaceTechnical)
                    .expect("diagnostic value observation");
                assert!(value.truncated);
                observed_rect.set(Rect::from_min_max(
                    egui::pos2(value.allocated_rect.min_x, value.allocated_rect.min_y),
                    egui::pos2(value.allocated_rect.max_x, value.allocated_rect.max_y),
                ));
            });

        assert!(harness.query_by_label(TOOLTIP).is_none());
        harness.hover_at(value_rect.get().center());
        harness.run();
        assert!(harness.query_by_label(TOOLTIP).is_some());
    }

    #[test]
    fn technical_rows_are_measured_and_end_aligned_until_the_pane_is_narrow() {
        for (width, expected_alignment, expected_lines) in [
            (520.0, HorizontalTextAlignment::End, 1),
            (260.0, HorizontalTextAlignment::Start, 2),
        ] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
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
            assert_eq!(value.interaction, TextInteraction::Selectable);
            assert!(observations.iter().any(|item| {
                item.role == TextRole::Secondary && item.interaction == TextInteraction::Inert
            }));
            assert!(audit_text_layouts(&observations).is_empty());
        }
    }
}
