use eframe::egui;
use polyorama_core::PaneId;
use polyorama_ui_egui::{
    ActionEmphasis, ActionTarget, Availability, DesignTokens, SemanticUiId, StatusTone,
    TextLayoutObservation, UiNode, choice_control, range_control, status_badge,
};

use crate::app::GalleryAction;

use super::buttons::gallery_action_button;

pub(super) fn toolbar_story(
    ui: &mut egui::Ui,
    narrow: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
) {
    ui.set_max_width((if narrow { 296.0_f32 } else { 720.0_f32 }).min(ui.available_width()));
    ui.horizontal_wrapped(|ui| {
        for (action, selected) in [
            (GalleryAction::NavigateTool, true),
            (GalleryAction::PolygonTool, false),
            (GalleryAction::EditVerticesTool, false),
            (GalleryAction::FitView, false),
            (GalleryAction::LinkViews, true),
        ] {
            gallery_action_button(
                ui,
                ActionTarget::pane(action, PaneId(1)),
                Availability::Enabled,
                selected,
                ActionEmphasis::Quiet,
                narrow,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
        }
        let parent = SemanticUiId::new("gallery.story");
        let mut map = 0_u8;
        let map = choice_control(
            ui,
            SemanticUiId::new("gallery.image-toolbar.display-map"),
            parent.clone(),
            "Display map",
            &mut map,
            &[(0, "Viridis"), (1, "Greyscale"), (2, "Threshold")],
            GalleryAction::DisplaySettings,
            tokens,
        );
        semantic_nodes.push(map.node);
        let mut low = 0.1;
        let low = range_control(
            ui,
            SemanticUiId::new("gallery.image-toolbar.low"),
            parent.clone(),
            "Low",
            &mut low,
            0.0..=0.8,
            GalleryAction::DisplaySettings,
            tokens,
        );
        semantic_nodes.push(low.node);
        let mut high = 0.9;
        let high = range_control(
            ui,
            SemanticUiId::new("gallery.image-toolbar.high"),
            parent,
            "High",
            &mut high,
            0.2..=1.0,
            GalleryAction::DisplaySettings,
            tokens,
        );
        semantic_nodes.push(high.node);
    });
    ui.add_space(tokens.spacing.section.0);
    status_badge(
        ui,
        45,
        if narrow {
            "Linked • 256 px/pt"
        } else {
            "Camera link A • 256 image pixels per screen point"
        },
        StatusTone::Success,
        tokens,
        font_scale,
        observations,
    );
}
