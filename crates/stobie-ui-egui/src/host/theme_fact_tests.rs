#![cfg(test)]
//! The theme a hosted application's pass was drawn in, published with its
//! observation as the framework's `stobie.theme` and `stobie.contrast`
//! facts, whatever facts the application publishes itself, and the
//! framework's `stobie.publication`, which only a refused pass carries.

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, AppearancePreference, CONTRAST_FACT, ContrastPreference, InspectionOperation,
    InspectionResult, PUBLICATION_FACT, PresentationScope, ReadingTextSpec, THEME_FACT,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "notes")]
enum Act {
    #[action(label = "Refresh", description = "Refresh the notes")]
    Refresh,
}

struct Notes {
    preferences: UiPreferences,
    /// How many facts of its own the application publishes.
    facts: usize,
}

impl Application for Notes {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Notes")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn preferences(&self) -> UiPreferences {
        self.preferences
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        // An application fact of the same name does not stand in for the
        // framework's.
        observation.fact(THEME_FACT, "sepia");
        // Nor does one claiming that the pass was refused.
        observation.fact(PUBLICATION_FACT, "invalid: the application says so");
        for index in 1..self.facts {
            observation.fact(format!("fact.{index}"), index);
        }
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("notes"), SemanticUiId::root());
        p.reading_text(ui, "body", "No notes yet", ReadingTextSpec::default());
    }

    fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, _: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Notes>>,
}

fn harness(preferences: UiPreferences, facts: usize) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(480.0, 320.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
            },
            Probe::default(),
        );
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, Notes { preferences, facts });
    host.enable_inspection(Inspection::new("theme-fact-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

/// The facts of the latest observation.
fn facts(harness: &Harness<'_, Probe>) -> BTreeMap<String, ObservationValue> {
    let inspection = harness.state().host.as_ref().unwrap().inspection().unwrap();
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: "observe".into(),
        instance: Some(inspection.instance()),
        operation: InspectionOperation::Observe,
    });
    let Some(InspectionResult::Observe { observation }) = reply.result else {
        panic!("observe failed: {:?}", reply.error);
    };
    observation.facts
}

/// The drawn theme's facts in the latest observation.
fn theme(harness: &Harness<'_, Probe>) -> (ObservationValue, ObservationValue) {
    let facts = facts(harness);
    (facts[THEME_FACT].clone(), facts[CONTRAST_FACT].clone())
}

fn preferences(appearance: AppearancePreference, contrast: ContrastPreference) -> UiPreferences {
    UiPreferences {
        appearance,
        contrast,
        ..UiPreferences::default()
    }
}

#[test]
fn observation_publishes_the_theme_each_pass_is_drawn_in() {
    let mut harness = harness(
        preferences(AppearancePreference::Light, ContrastPreference::Standard),
        1,
    );
    assert_eq!(theme(&harness), ("light".into(), "standard".into()));
    for (appearance, contrast, expected) in [
        (
            AppearancePreference::Dark,
            ContrastPreference::Standard,
            ("dark", "standard"),
        ),
        (
            AppearancePreference::Dark,
            ContrastPreference::High,
            ("dark", "high"),
        ),
        (
            AppearancePreference::Light,
            ContrastPreference::High,
            ("light", "high"),
        ),
    ] {
        let host = harness.state_mut().host.as_mut().unwrap();
        host.app_mut().preferences = preferences(appearance, contrast);
        harness.run();
        assert_eq!(theme(&harness), (expected.0.into(), expected.1.into()));
    }
}

#[test]
fn a_system_appearance_publishes_the_theme_the_system_resolved() {
    let mut harness = harness(
        preferences(AppearancePreference::System, ContrastPreference::Standard),
        1,
    );
    for system in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
        harness.input_mut().system_theme = Some(system);
        harness.run();
        let expected = if system == egui::Theme::Dark {
            "dark"
        } else {
            "light"
        };
        assert_eq!(theme(&harness), (expected.into(), "standard".into()));
    }
}

#[test]
fn the_theme_facts_leave_the_application_its_own_fact_limit() {
    // The application publishes the most facts the limit allows.
    let harness = harness(UiPreferences::default(), 256);
    assert_eq!(theme(&harness), ("dark".into(), "standard".into()));
}

#[test]
fn a_published_pass_carries_no_publication_fact_the_application_set() {
    let harness = harness(UiPreferences::default(), 1);
    let facts = facts(&harness);
    assert!(!facts.contains_key(PUBLICATION_FACT), "{facts:?}");
    // The application's own fact is not published either, so it takes
    // none of the framework's allowance.
    assert_eq!(
        facts.keys().map(String::as_str).collect::<Vec<_>>(),
        [CONTRAST_FACT, THEME_FACT]
    );
}
