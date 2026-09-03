use eframe::egui;
use polyorama_ui_egui::{
    DesignTokens, StatusTone, TextLayoutObservation, property_row, result_row, status_badge,
};

use super::DockSceneState;
use super::{data, dock, toolbars};

pub(super) fn application_shell(
    ui: &mut egui::Ui,
    dock_state: &mut DockSceneState,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<polyorama_ui_egui::UiNode>,
) {
    dock::dock_story(
        ui,
        false,
        dock_state,
        tokens,
        font_scale,
        observations,
        semantic_nodes,
    );
}

pub(super) fn image_toolbar(
    ui: &mut egui::Ui,
    narrow: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<polyorama_ui_egui::UiNode>,
) {
    toolbars::toolbar_story(ui, narrow, tokens, font_scale, observations, semantic_nodes);
}

pub(super) fn thumbnails(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    data::thumbnail_grid(ui, true, tokens, font_scale, observations);
}

pub(super) fn inspector_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, label, value) in [
        (50, "Result", "#000,842,771"),
        (51, "Position", "−12,345.625, 98,765.125 px"),
        (52, "Confidence", "99.875 %"),
        (
            53,
            "Category",
            "Review — exceptionally long deterministic classification name",
        ),
        (54, "Annotation", "Polygon 17 · 128 vertices · selected"),
    ] {
        property_row(ui, instance, label, value, tokens, font_scale, observations);
    }
}

pub(super) fn results_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, id, position, confidence, category, selected) in [
        (
            60,
            "#842769",
            "−12345.6, 98765.1",
            "−0.125 %",
            "Edge",
            false,
        ),
        (
            61,
            "#842770",
            "65536.0, 65536.0",
            "99.875 %",
            "Selected review target with long label",
            true,
        ),
        (
            62,
            "#842771",
            "131071.9, −0.125",
            "100.000 %",
            "Cluster",
            false,
        ),
        (63, "#842772", "0.0, 0.0", "7.500 %", "Target", false),
    ] {
        result_row(
            ui,
            polyorama_ui_egui::ResultRowSpec {
                instance,
                identifier: id,
                position,
                confidence,
                category,
                selected,
            },
            tokens,
            font_scale,
            observations,
        );
    }
}

pub(super) fn diagnostics_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, label, value) in [
        (
            90,
            "Application update CPU p95 across the retained deterministic observation window",
            "126.300 ms",
        ),
        (
            91,
            "Resident texture bytes / configured cache budget",
            "67,108,864 / 67,108,864 bytes",
        ),
        (
            92,
            "Outstanding request token",
            "generation=4 epoch=18 sequence=18,446,744,073,709,551,615",
        ),
        (
            93,
            "GPU timestamp",
            "unavailable — adapter does not expose timestamp queries",
        ),
    ] {
        property_row(ui, instance, label, value, tokens, font_scale, observations);
    }
    status_badge(
        ui,
        99,
        "Worker running · zero queued · zero in flight · event-driven repaint idle",
        StatusTone::Success,
        tokens,
        font_scale,
        observations,
    );
}
