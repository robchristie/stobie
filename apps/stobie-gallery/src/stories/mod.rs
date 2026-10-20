mod buttons;
mod data;
mod dock;
mod icons;
mod navigation;
mod reading;
mod reference;
mod rows;
mod status_chips;
mod toolbars;
mod typography;

use eframe::egui;
use stobie_ui_egui::{
    DesignTokens, PresentationObservations, TextInteraction, TextLayoutObservation, observe_node,
    observe_text,
};

use crate::catalogue::{StoryId, story_definition};

pub(crate) use dock::DockSceneState;
pub use icons::{IconControl, IconFixtureState};
pub use navigation::{Destination, NavigationFixtureState};
pub use status_chips::StatusChipFixtureState;

/// Per-story fixture state. Selecting another story resets it.
#[derive(Default)]
pub(crate) struct StoryState {
    dock: Option<DockSceneState>,
    focus_story: Option<StoryId>,
    pub(crate) icon: IconFixtureState,
    pub(crate) navigation: NavigationFixtureState,
    pub(crate) status_chip: StatusChipFixtureState,
    pub(crate) rows: rows::RowFixtureState,
}

/// What a story reports besides the observations it registers.
#[derive(Default)]
pub(crate) struct StoryReport {
    /// Whether the story presented selectable text.
    pub(crate) selectable: bool,
    /// Why a fixture change needs another pass.
    pub(crate) repaint: Option<&'static str>,
}

impl StoryReport {
    /// Note a presentation context's text; the context registers it itself.
    fn presented(&mut self, observations: &PresentationObservations) {
        self.selectable |= observations.text_layouts.iter().any(selectable);
    }
}

fn selectable(text: &TextLayoutObservation) -> bool {
    text.interaction == TextInteraction::Selectable
}

/// Present a story's heading and body.
pub(crate) fn render_story(
    ui: &mut egui::Ui,
    story: StoryId,
    state: &mut StoryState,
    tokens: &DesignTokens,
    font_scale: f32,
) -> StoryReport {
    let definition = story_definition(story);
    ui.strong(definition.id.as_str());
    ui.label(definition.description);
    ui.separator();
    present_story(ui, story, state, tokens, font_scale)
}

/// Present a story's body and register the observations its low-level
/// recipes return; presentation contexts and recipes register their own.
fn present_story(
    ui: &mut egui::Ui,
    story: StoryId,
    state: &mut StoryState,
    tokens: &DesignTokens,
    font_scale: f32,
) -> StoryReport {
    let mut report = StoryReport::default();
    let mut observations = Vec::new();
    let mut semantic_nodes = Vec::new();
    let dock_state = state.dock.get_or_insert_with(|| DockSceneState::new(story));
    match story {
        StoryId::StatusChipTasks
        | StoryId::StatusChipTreatments
        | StoryId::StatusChipLongNarrow => status_chips::story(
            ui,
            story,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
            &mut state.status_chip,
            &mut report,
        ),
        StoryId::NavigationSidebar | StoryId::NavigationStates | StoryId::NavigationLongNarrow => {
            navigation::story(
                ui,
                story,
                tokens,
                font_scale,
                &mut state.navigation,
                &mut report,
            )
        }
        StoryId::ReadingConversation | StoryId::ReadingLongNarrow => {
            reading::story(ui, story, tokens, font_scale, &mut report)
        }
        StoryId::RowsActions | StoryId::RowsLongNarrow => {
            rows::story(ui, story, tokens, font_scale, &mut state.rows, &mut report)
        }
        StoryId::IconsVocabulary => icons::vocabulary(ui, tokens, font_scale, &mut observations),
        StoryId::IconsActionPresentations | StoryId::IconsToolbar | StoryId::IconsLongNarrow => {
            icons::actions(ui, story, tokens, font_scale, &mut state.icon, &mut report)
        }
        StoryId::TypographyDense | StoryId::TypographyReading => typography::story(
            ui,
            if story == StoryId::TypographyReading {
                stobie_ui_egui::TypographyProfile::Reading
            } else {
                stobie_ui_egui::TypographyProfile::Dense
            },
            tokens,
            font_scale,
            &mut observations,
        ),
        StoryId::ButtonDefault => {
            let mut focused = false;
            buttons::button_story(
                ui,
                false,
                tokens,
                font_scale,
                &mut observations,
                &mut semantic_nodes,
                &mut focused,
            )
        }
        StoryId::ButtonDisabled => buttons::disabled_story(ui, tokens, font_scale, &mut report),
        StoryId::ButtonKeyboardFocus => {
            let mut focused = state.focus_story == Some(story);
            buttons::button_story(
                ui,
                true,
                tokens,
                font_scale,
                &mut observations,
                &mut semantic_nodes,
                &mut focused,
            );
            if focused {
                state.focus_story = Some(story);
            }
        }
        StoryId::TabsManyLongLabels => dock::dock_story(
            ui,
            false,
            dock_state,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
        ),
        StoryId::TabsNarrow => dock::dock_story(
            ui,
            true,
            dock_state,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
        ),
        StoryId::SplitterHoverActive => {
            dock::splitter_story(ui, tokens, font_scale, &mut observations)
        }
        StoryId::ToolbarNarrow => toolbars::toolbar_story(
            ui,
            true,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
        ),
        StoryId::PropertyRowLongValue => {
            data::property_story(ui, tokens, font_scale, &mut observations)
        }
        StoryId::StatusErrorLongMessage => {
            data::status_story(ui, tokens, font_scale, &mut observations)
        }
        StoryId::VirtualGridLoading => {
            data::thumbnail_grid(ui, false, tokens, font_scale, &mut observations)
        }
        StoryId::VirtualGridPartial => {
            data::thumbnail_grid(ui, true, tokens, font_scale, &mut observations)
        }
        StoryId::ReferenceApplicationShell => reference::application_shell(
            ui,
            dock_state,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
        ),
        StoryId::ReferenceImageToolbarNarrow => reference::image_toolbar(
            ui,
            true,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
        ),
        StoryId::ReferenceImageToolbarWide => reference::image_toolbar(
            ui,
            false,
            tokens,
            font_scale,
            &mut observations,
            &mut semantic_nodes,
        ),
        StoryId::ReferenceInspector => {
            reference::inspector_story(ui, tokens, font_scale, &mut observations)
        }
        StoryId::ReferenceResults => {
            reference::results_story(ui, tokens, font_scale, &mut observations)
        }
        StoryId::ReferenceThumbnails => {
            reference::thumbnails(ui, tokens, font_scale, &mut observations)
        }
        StoryId::ReferenceDiagnostics => {
            reference::diagnostics_story(ui, tokens, font_scale, &mut observations)
        }
    }
    report.selectable |= observations.iter().any(selectable);
    for node in semantic_nodes {
        observe_node(ui, node);
    }
    for text in observations {
        observe_text(ui, text);
    }
    report
}

/// Hosts one story body so tests see the observations it registers.
#[cfg(test)]
pub(crate) mod probe {
    use serde::Serialize;
    use stobie_ui_egui::{
        Action, ActionTarget, AppHost, Application, ApplicationIdentity, ApplicationTheme,
        Availability, Frame, UiPreferences, UiSnapshot, application_identity,
    };

    use super::*;

    #[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
    #[action(prefix = "probe")]
    pub(crate) enum ProbeAction {
        #[action(label = "Unused", description = "The probe performs no actions")]
        Unused,
    }

    pub(crate) struct StoryProbe {
        pub(crate) story: StoryId,
        pub(crate) tokens: DesignTokens,
        pub(crate) scale: f32,
        pub(crate) clip: Option<egui::Rect>,
        pub(crate) theme: ApplicationTheme,
        pub(crate) preferences: UiPreferences,
        /// Present the story's heading and description, as the Gallery does.
        pub(crate) heading: bool,
    }

    impl Application for StoryProbe {
        type Action = ProbeAction;
        type Intent = ();
        type UiState = StoryState;

        fn identity(&self) -> ApplicationIdentity {
            application_identity!("Story probe")
        }

        fn availability(&self, _target: ActionTarget<ProbeAction>) -> Availability {
            Availability::Enabled
        }

        fn theme(&self) -> ApplicationTheme {
            self.theme.clone()
        }

        fn preferences(&self) -> UiPreferences {
            self.preferences
        }

        fn ui(
            &self,
            ui: &mut egui::Ui,
            state: &mut StoryState,
            _: &mut Frame<'_, ProbeAction, ()>,
        ) {
            if let Some(clip) = self.clip {
                ui.set_clip_rect(clip);
            }
            if self.heading {
                render_story(ui, self.story, state, &self.tokens, self.scale);
            } else {
                present_story(ui, self.story, state, &self.tokens, self.scale);
            }
        }

        fn apply(&mut self, _intent: (), _: &mut StoryState) -> Result<bool, String> {
            Ok(false)
        }

        fn perform(
            &mut self,
            _: ActionTarget<ProbeAction>,
            _: &mut StoryState,
        ) -> Result<bool, String> {
            Ok(false)
        }

        fn report_error(&mut self, message: String) {
            panic!("{message}");
        }
    }

    /// Present a story once at `size`; returns its snapshot and egui output.
    pub(crate) fn render(
        context: &egui::Context,
        probe: StoryProbe,
        size: egui::Vec2,
    ) -> (UiSnapshot, egui::FullOutput) {
        let mut host = AppHost::new(context, probe);
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ui| host.present(ui),
        );
        output.textures_delta.clear();
        (host.ui_snapshot().clone(), output)
    }
}

#[cfg(test)]
mod tests {
    use super::probe::{self, StoryProbe};
    use crate::app::{GalleryConfiguration, GalleryWidth};
    use crate::catalogue::{StoryId, story_definition};
    use eframe::egui;
    use stobie_ui_egui::ApplicationTheme;

    #[test]
    fn every_story_renders_through_production_components_without_text_audit_findings() {
        let representative = [
            GalleryConfiguration::default(),
            GalleryConfiguration {
                appearance: stobie_ui_egui::AppearancePreference::Light,
                density: stobie_ui_egui::DensityPreference::Compact,
                width: GalleryWidth::Narrow,
                ..GalleryConfiguration::default()
            },
            GalleryConfiguration {
                contrast: stobie_ui_egui::ContrastPreference::High,
                font_scale: 1.25,
                ..GalleryConfiguration::default()
            },
            GalleryConfiguration {
                appearance: stobie_ui_egui::AppearancePreference::Light,
                contrast: stobie_ui_egui::ContrastPreference::High,
                font_scale: 1.5,
                width: GalleryWidth::Wide,
                ..GalleryConfiguration::default()
            },
        ];
        for configuration in representative {
            let preferences = configuration.preferences();
            let tokens = preferences
                .tokens(configuration.appearance == stobie_ui_egui::AppearancePreference::Dark);
            for story in StoryId::ALL {
                let context = egui::Context::default();
                let probe = StoryProbe {
                    story,
                    tokens,
                    scale: configuration.font_scale,
                    clip: None,
                    theme: ApplicationTheme::analytical(),
                    preferences,
                    heading: true,
                };
                // The story alone at the configured width, as a reviewer sees it.
                let size = egui::vec2(
                    configuration.width.points(),
                    f32::from(story_definition(story).recommended_viewport.height),
                );
                let (snapshot, _) = probe::render(&context, probe, size);
                assert!(
                    !snapshot.text.is_empty(),
                    "story {story} emitted no measured text"
                );
                assert!(
                    snapshot.text_audit.is_empty(),
                    "story {story} at {configuration:?} failed text audit: {:#?}",
                    snapshot.text_audit
                );
            }
        }
    }
}
