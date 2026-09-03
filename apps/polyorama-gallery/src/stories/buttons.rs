use eframe::egui;
use polyorama_core::PaneId;
use polyorama_ui_egui::{
    ActionButtonSpec, ActionEmphasis, ActionKey, ActionTarget, Availability, DesignTokens,
    SemanticUiId, TextLayoutObservation, UiNode, action_button, action_semantic_node,
};

use crate::app::GalleryAction;

#[allow(clippy::too_many_arguments)]
pub(super) fn gallery_action_button(
    ui: &mut egui::Ui,
    target: ActionTarget<GalleryAction>,
    availability: Availability,
    selected: bool,
    emphasis: ActionEmphasis,
    compact: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
) -> egui::Response {
    let response = action_button(
        ui,
        ActionButtonSpec {
            target,
            availability: availability.clone(),
            selected,
            emphasis,
            compact,
        },
        tokens,
        font_scale,
        observations,
    );
    semantic_nodes.push(action_semantic_node(
        &response,
        target,
        &availability,
        selected,
        SemanticUiId::new("gallery.story"),
    ));
    response
}

#[allow(clippy::too_many_arguments)]
pub(super) fn button_story(
    ui: &mut egui::Ui,
    keyboard_focus: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
    focus_story: &mut bool,
) {
    if keyboard_focus {
        if !*focus_story {
            ui.memory_mut(|memory| {
                memory.request_focus(egui::Id::new((
                    "polyorama.action-button",
                    GalleryAction::FitView.stable_id(),
                    Some(PaneId(1)),
                )))
            });
            *focus_story = true;
        }
        let response = gallery_action_button(
            ui,
            ActionTarget::pane(GalleryAction::FitView, PaneId(1)),
            Availability::Enabled,
            false,
            ActionEmphasis::Normal,
            false,
            tokens,
            font_scale,
            observations,
            semantic_nodes,
        );
        debug_assert!(response.has_focus());
    } else {
        ui.horizontal(|ui| {
            gallery_action_button(
                ui,
                ActionTarget::application(GalleryAction::SaveLayout),
                Availability::Enabled,
                false,
                ActionEmphasis::Normal,
                false,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
            gallery_action_button(
                ui,
                ActionTarget::application(GalleryAction::ResetWorkspace),
                Availability::Enabled,
                false,
                ActionEmphasis::Primary,
                false,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
            gallery_action_button(
                ui,
                ActionTarget::pane(GalleryAction::LinkViews, PaneId(1)),
                Availability::Enabled,
                true,
                ActionEmphasis::Quiet,
                false,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
        });
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn disabled_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
) {
    gallery_action_button(
        ui,
        ActionTarget::application(GalleryAction::Undo),
        Availability::Disabled {
            reason: "History is empty".into(),
        },
        false,
        ActionEmphasis::Normal,
        false,
        tokens,
        font_scale,
        observations,
        semantic_nodes,
    );
}
