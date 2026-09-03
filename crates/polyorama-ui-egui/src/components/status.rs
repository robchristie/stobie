use egui::{Color32, Response, Sense, Stroke};

use crate::{
    DesignTokens, HorizontalTextAlignment, TextComponentId, TextInteraction, TextOverflow,
    TextRole, TextSpec, VerticalTextAlignment, measure_text, present_measured_text,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusTone {
    Neutral,
    Success,
    Warning,
    Error,
}

pub fn status_badge(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    tone: StatusTone,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let maximum_width = ui.available_width().max(tokens.geometry.minimum_hit_size.0);
    let max_lines = if maximum_width < 360.0 { 3 } else { 2 };
    let spec = TextSpec {
        role: if tone == StatusTone::Error {
            TextRole::Error
        } else {
            TextRole::Status
        },
        overflow: TextOverflow::Wrap,
        interaction: TextInteraction::Selectable,
        horizontal_alignment: HorizontalTextAlignment::Start,
        vertical_alignment: VerticalTextAlignment::Centre,
        max_lines,
    };
    let measured = measure_text(
        ui.painter(),
        text,
        spec,
        tokens,
        font_scale,
        (maximum_width - tokens.spacing.inline.0 * 2.0).max(0.5),
    )
    .ok();
    let height = measured.as_ref().map_or_else(
        || tokens.geometry.control_height.0,
        |text| (text.size().y + tokens.spacing.block.0 * 2.0).max(tokens.geometry.control_height.0),
    );
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(maximum_width, height), Sense::hover());
    let colour: Color32 = match tone {
        StatusTone::Neutral => tokens.colours.text_muted.into(),
        StatusTone::Success => tokens.colours.status_success.into(),
        StatusTone::Warning => tokens.colours.status_warning.into(),
        StatusTone::Error => tokens.colours.status_error.into(),
    };
    ui.painter().rect(
        rect,
        tokens.geometry.control_radius.0,
        colour.linear_multiply(0.14),
        Stroke::new(1.0, colour),
        egui::StrokeKind::Inside,
    );
    if let Some(measured) = measured {
        let text_rect = rect.shrink2(egui::vec2(tokens.spacing.inline.0, tokens.spacing.block.0));
        let (_, observation) = present_measured_text(
            ui,
            &measured,
            text_rect,
            TextComponentId::new(crate::TextComponentKind::StatusBadge, instance),
            None,
        );
        observations.push(observation);
    }
    response
}
