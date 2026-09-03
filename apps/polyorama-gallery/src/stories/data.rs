use eframe::egui;
use polyorama_ui_egui::{
    DesignTokens, StatusTone, TextLayoutObservation, ThumbnailCellSpec, ThumbnailState,
    property_row, status_badge, thumbnail_cell,
};

pub(super) fn property_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    property_row(
        ui,
        20,
        "Dataset identifier",
        "urn:polyorama:observations:antarctic-sector-04:reconstruction-with-an-intentionally-long-unbroken-suffix",
        tokens,
        font_scale,
        observations,
    );
}

pub(super) fn status_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    status_badge(
        ui,
        30,
        "Worker decode failed after three attempts. The original scientific tile remains unavailable; inspect Diagnostics for request token 18446744073709551615.",
        StatusTone::Error,
        tokens,
        font_scale,
        observations,
    );
}

pub(super) fn thumbnail_grid(
    ui: &mut egui::Ui,
    partial: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    let count = if ui.available_width() < 400.0 { 4 } else { 8 };
    ui.horizontal_wrapped(|ui| {
        for index in 0..count {
            let state = if !partial {
                ThumbnailState::Loading
            } else {
                match index % 4 {
                    0 => ThumbnailState::Resident,
                    1 => ThumbnailState::Loading,
                    2 => ThumbnailState::Empty,
                    _ => ThumbnailState::Error,
                }
            };
            thumbnail_cell(
                ui,
                ThumbnailCellSpec {
                    instance: 70 + index,
                    label: &format!("Tile {:06}", 120_000 + index),
                    state,
                    selected: index == 0,
                    texture: None,
                },
                tokens,
                font_scale,
                observations,
            );
        }
    });
}
