mod buttons;
mod data;
mod dock;
mod reference;
mod toolbars;
mod typography;

use eframe::egui;
use polyorama_ui_egui::{DesignTokens, TextLayoutObservation, UiNode};

use crate::catalogue::{StoryId, story_definition};

pub(crate) use dock::DockSceneState;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_story(
    ui: &mut egui::Ui,
    story: StoryId,
    dock_state: &mut DockSceneState,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
    focus_story: &mut Option<StoryId>,
) {
    let definition = story_definition(story);
    ui.strong(definition.id.as_str());
    ui.label(definition.description);
    ui.separator();
    match story {
        StoryId::TypographyDense | StoryId::TypographyReading => typography::story(
            ui,
            if story == StoryId::TypographyReading {
                polyorama_ui_egui::TypographyProfile::Reading
            } else {
                polyorama_ui_egui::TypographyProfile::Dense
            },
            tokens,
            font_scale,
            observations,
        ),
        StoryId::ButtonDefault => {
            let mut focused = false;
            buttons::button_story(
                ui,
                false,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
                &mut focused,
            )
        }
        StoryId::ButtonDisabled => {
            buttons::disabled_story(ui, tokens, font_scale, observations, semantic_nodes)
        }
        StoryId::ButtonKeyboardFocus => {
            let mut focused = *focus_story == Some(story);
            buttons::button_story(
                ui,
                true,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
                &mut focused,
            );
            if focused {
                *focus_story = Some(story);
            }
        }
        StoryId::TabsManyLongLabels => dock::dock_story(
            ui,
            false,
            dock_state,
            tokens,
            font_scale,
            observations,
            semantic_nodes,
        ),
        StoryId::TabsNarrow => dock::dock_story(
            ui,
            true,
            dock_state,
            tokens,
            font_scale,
            observations,
            semantic_nodes,
        ),
        StoryId::SplitterHoverActive => dock::splitter_story(ui, tokens, font_scale, observations),
        StoryId::ToolbarNarrow => {
            toolbars::toolbar_story(ui, true, tokens, font_scale, observations, semantic_nodes)
        }
        StoryId::PropertyRowLongValue => data::property_story(ui, tokens, font_scale, observations),
        StoryId::StatusErrorLongMessage => data::status_story(ui, tokens, font_scale, observations),
        StoryId::VirtualGridLoading => {
            data::thumbnail_grid(ui, false, tokens, font_scale, observations)
        }
        StoryId::VirtualGridPartial => {
            data::thumbnail_grid(ui, true, tokens, font_scale, observations)
        }
        StoryId::ReferenceApplicationShell => reference::application_shell(
            ui,
            dock_state,
            tokens,
            font_scale,
            observations,
            semantic_nodes,
        ),
        StoryId::ReferenceImageToolbarNarrow => {
            reference::image_toolbar(ui, true, tokens, font_scale, observations, semantic_nodes)
        }
        StoryId::ReferenceImageToolbarWide => {
            reference::image_toolbar(ui, false, tokens, font_scale, observations, semantic_nodes)
        }
        StoryId::ReferenceInspector => {
            reference::inspector_story(ui, tokens, font_scale, observations)
        }
        StoryId::ReferenceResults => reference::results_story(ui, tokens, font_scale, observations),
        StoryId::ReferenceThumbnails => reference::thumbnails(ui, tokens, font_scale, observations),
        StoryId::ReferenceDiagnostics => {
            reference::diagnostics_story(ui, tokens, font_scale, observations)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GalleryConfiguration, GalleryWidth};
    use polyorama_ui_egui::{apply_design_system, audit_text_layouts};

    #[test]
    fn every_story_renders_through_production_components_without_text_audit_findings() {
        let representative = [
            GalleryConfiguration::default(),
            GalleryConfiguration {
                appearance: polyorama_ui_egui::AppearancePreference::Light,
                density: polyorama_ui_egui::DensityPreference::Compact,
                width: GalleryWidth::Narrow,
                ..GalleryConfiguration::default()
            },
            GalleryConfiguration {
                contrast: polyorama_ui_egui::ContrastPreference::High,
                font_scale: 1.25,
                ..GalleryConfiguration::default()
            },
            GalleryConfiguration {
                appearance: polyorama_ui_egui::AppearancePreference::Light,
                contrast: polyorama_ui_egui::ContrastPreference::High,
                font_scale: 1.5,
                width: GalleryWidth::Wide,
                ..GalleryConfiguration::default()
            },
        ];
        for configuration in representative {
            let context = egui::Context::default();
            apply_design_system(&context, configuration.preferences());
            let tokens = configuration
                .preferences()
                .tokens(configuration.appearance == polyorama_ui_egui::AppearancePreference::Dark);
            for story in StoryId::ALL {
                let mut dock_state = DockSceneState::new(story);
                let mut observations = Vec::new();
                let mut semantic_nodes = Vec::new();
                let mut focus_story = None;
                let size = egui::vec2(
                    configuration.width.points(),
                    f32::from(story_definition(story).recommended_viewport.height),
                );
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        render_story(
                            ui,
                            story,
                            &mut dock_state,
                            &tokens,
                            configuration.font_scale,
                            &mut observations,
                            &mut semantic_nodes,
                            &mut focus_story,
                        )
                    },
                );
                output.textures_delta.clear();
                assert!(
                    !observations.is_empty(),
                    "story {story} emitted no measured text"
                );
                let findings = audit_text_layouts(&observations);
                assert!(
                    findings.is_empty(),
                    "story {story} at {configuration:?} failed text audit: {findings:#?}"
                );
            }
        }
    }
}
