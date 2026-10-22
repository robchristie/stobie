#![cfg(test)]
//! Focus that returns to a control which has gone: Back to a level whose
//! opening row has left the list, and a dialog closing after its opener
//! went, whether the dialog's own confirmation removed it or something else
//! did. Focus moves to the presented control nearest where the opener was,
//! and AccessKit never names a focus outside its tree.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, ConfirmSpec, DestinationSpec, NavigationContent, NavigationPane,
    NavigationSpec, PresentationScope, ReadingTextSpec,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "routines")]
enum Act {
    #[action(
        label = "Open",
        description = "Open this routine",
        argument = "text",
        max_length = 8
    )]
    Open,
    #[action(label = "Back", description = "Return to the list")]
    Back,
    #[action(label = "Archive", description = "Archive this routine")]
    Archive,
    #[action(
        label = "Delete",
        description = "Delete this routine",
        argument = "text",
        max_length = 8
    )]
    Delete,
    #[action(label = "Delete routine", description = "Delete the routine now")]
    ConfirmDelete,
    #[action(label = "Keep routine", description = "Keep the routine")]
    KeepRoutine,
}

const ROUTINES: [(&str, &str); 3] = [("r1", "Morning"), ("r2", "Evening"), ("r3", "Weekend")];

/// Routines in a navigation recipe, or as a plain list whose rows each
/// carry Delete behind a confirm.
struct Routines {
    navigation: bool,
    shown: Vec<&'static str>,
    open: Option<&'static str>,
    deleting: Option<&'static str>,
}

impl Routines {
    fn new(navigation: bool) -> Self {
        Self {
            navigation,
            shown: ROUTINES.iter().map(|(id, _)| *id).collect(),
            open: None,
            deleting: None,
        }
    }

    fn routine(&self, argument: &ActionArgument) -> Option<&'static str> {
        let ActionArgument::Text(id) = argument else {
            return None;
        };
        self.shown
            .iter()
            .copied()
            .find(|shown| *shown == id.as_str())
    }

    fn name(id: &str) -> &'static str {
        ROUTINES
            .iter()
            .find(|(routine, _)| *routine == id)
            .map_or("", |(_, name)| name)
    }

    fn remove(&mut self, id: &str) {
        self.shown.retain(|shown| *shown != id);
    }
}

impl Application for Routines {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Routines")
    }

    fn availability(&self, target: ActionTarget<Act>) -> Availability {
        match target.action {
            Act::Back | Act::Archive if self.open.is_none() => Availability::Hidden,
            _ => Availability::Enabled,
        }
    }

    fn availability_with(&self, _: ActionTarget<Act>, argument: &ActionArgument) -> Availability {
        match self.routine(argument) {
            Some(_) => Availability::Enabled,
            None => Availability::Hidden,
        }
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p =
            frame.presentation(ui, PresentationScope::new("routines"), SemanticUiId::root());
        if !self.navigation {
            for id in &self.shown {
                let name = Self::name(id);
                ui.horizontal(|ui| {
                    ui.label(name);
                    frame.action(
                        ui,
                        &mut p,
                        BoundAction::new(Act::Delete, *id).with_subject(name),
                        ActionEmphasis::Normal,
                    );
                });
            }
            if let Some(id) = self.deleting {
                frame.confirm(
                    ui,
                    &mut p,
                    "delete",
                    &ConfirmSpec::new(
                        "Delete the routine?",
                        &format!("{} will stop running.", Self::name(id)),
                        Act::ConfirmDelete,
                        Act::KeepRoutine,
                    ),
                );
            }
            return;
        }
        let spec = NavigationSpec {
            list_title: "Routines",
            content: self.open.map(|id| NavigationContent {
                title: Self::name(id),
                reference: None,
                detail: None,
            }),
            back: Act::Back,
        };
        frame.navigation(
            ui,
            &mut p,
            "navigation",
            &spec,
            |ui, p, frame, pane| match pane {
                NavigationPane::List => {
                    for id in &self.shown {
                        frame.navigation_item(
                            ui,
                            p,
                            BoundAction::new(Act::Open, *id),
                            DestinationSpec {
                                icon: IconId::Calendar,
                                label: Self::name(id),
                                description: "Open this routine",
                                selected: self.open == Some(*id),
                                badge: None,
                            },
                        );
                    }
                }
                NavigationPane::Content => {
                    frame.action(ui, p, Act::Archive, ActionEmphasis::Normal);
                }
                NavigationPane::Detail | NavigationPane::Empty => {
                    p.reading_text(ui, "empty", "Choose a routine", ReadingTextSpec::default());
                }
            },
        );
    }

    fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        match target.action {
            Act::Back => self.open = None,
            // Archiving leaves the list and closes the routine.
            Act::Archive => {
                if let Some(id) = self.open.take() {
                    self.remove(id);
                }
            }
            // The confirmation removes the row that holds its opener.
            Act::ConfirmDelete => {
                if let Some(id) = self.deleting.take() {
                    self.remove(id);
                }
            }
            Act::KeepRoutine => self.deleting = None,
            Act::Open | Act::Delete => return Err("needs a routine".into()),
        }
        Ok(true)
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<Act>,
        argument: ActionArgument,
        view: &mut (),
    ) -> Result<bool, String> {
        let Some(id) = self.routine(&argument) else {
            return self.perform(target, view);
        };
        match target.action {
            Act::Open => self.open = Some(id),
            Act::Delete => self.deleting = Some(id),
            _ => return self.perform(target, view),
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Routines>>,
    /// Ask egui, once, to focus a widget no pass presents.
    stray: bool,
}

fn harness(routines: Routines) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(390.0, 760.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
                if std::mem::take(&mut probe.stray) {
                    ui.memory_mut(|memory| memory.request_focus(egui::Id::new("gone")));
                }
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    harness.state_mut().host = Some(AppHost::new(&context, routines));
    harness.run();
    harness
}

fn app<'a>(harness: &'a Harness<'_, Probe>) -> &'a Routines {
    harness.state().host.as_ref().unwrap().app()
}

fn app_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut Routines {
    harness.state_mut().host.as_mut().unwrap().app_mut()
}

/// The control with keyboard focus, as assistive technology names it. The
/// harness's own AccessKit tree checks, on every pass, that the focus it is
/// given is in the tree.
fn focused(harness: &Harness<'_, Probe>) -> Option<String> {
    let widget = harness.ctx.memory(|memory| memory.focused())?;
    let node = widget.accesskit_id();
    harness
        .query_by(|candidate| candidate.locate().0 == node)
        .map(|found| found.accesskit_node().label().unwrap_or_default())
}

fn press(harness: &mut Harness<'_, Probe>, key: Key) {
    harness.key_press_modifiers(Modifiers::NONE, key);
    harness.run();
}

/// Open `name` from the keyboard, so its row is the level's opener.
fn open(harness: &mut Harness<'_, Probe>, name: &str) {
    harness.get_by_label(name).focus();
    harness.run();
    press(harness, Key::Enter);
    assert_eq!(app(harness).open.map(Routines::name), Some(name));
    assert_eq!(focused(harness).as_deref(), Some("Back"));
}

#[test]
fn back_moves_focus_to_the_next_row_when_the_opener_has_left_the_list() {
    let mut harness = harness(Routines::new(true));
    open(&mut harness, "Evening");
    // Evening leaves the list while it is open, as a sync might remove it.
    app_mut(&mut harness).remove("r2");
    harness.run();
    press(&mut harness, Key::Enter);
    assert_eq!(app(&harness).open, None);
    assert_eq!(focused(&harness).as_deref(), Some("Weekend"));
}

#[test]
fn back_moves_focus_to_the_previous_row_when_the_last_row_has_gone() {
    let mut harness = harness(Routines::new(true));
    open(&mut harness, "Weekend");
    app_mut(&mut harness).remove("r3");
    harness.run();
    press(&mut harness, Key::Enter);
    assert_eq!(focused(&harness).as_deref(), Some("Evening"));
}

#[test]
fn an_action_that_closes_the_level_and_removes_its_opener_moves_focus_to_the_next_row() {
    let mut harness = harness(Routines::new(true));
    open(&mut harness, "Morning");
    harness.get_by_label("Archive").focus();
    harness.run();
    press(&mut harness, Key::Enter);
    assert_eq!(app(&harness).open, None);
    assert_eq!(app(&harness).shown, ["r2", "r3"]);
    assert_eq!(focused(&harness).as_deref(), Some("Evening"));
}

#[test]
fn back_still_returns_focus_to_an_opener_that_stayed() {
    let mut harness = harness(Routines::new(true));
    open(&mut harness, "Evening");
    press(&mut harness, Key::Enter);
    assert_eq!(focused(&harness).as_deref(), Some("Evening"));
}

/// Open the confirm from `name`'s Delete with the keyboard.
fn ask_to_delete(harness: &mut Harness<'_, Probe>, name: &str) {
    harness.get_by_label(&format!("Delete {name}")).focus();
    harness.run();
    press(harness, Key::Enter);
    harness.run();
    assert!(app(harness).deleting.is_some());
    assert_eq!(focused(harness).as_deref(), Some("Keep routine"));
}

#[test]
fn a_confirm_that_removes_its_opener_moves_focus_to_the_next_rows_control() {
    let mut harness = harness(Routines::new(false));
    ask_to_delete(&mut harness, "Evening");
    harness.get_by_label("Delete routine").click();
    harness.run();
    assert_eq!(app(&harness).shown, ["r1", "r3"]);
    assert_eq!(app(&harness).deleting, None);
    assert_eq!(focused(&harness).as_deref(), Some("Delete Weekend"));
}

#[test]
fn a_confirm_that_removes_the_last_row_moves_focus_to_the_previous_rows_control() {
    let mut harness = harness(Routines::new(false));
    ask_to_delete(&mut harness, "Weekend");
    harness.get_by_label("Delete routine").click();
    harness.run();
    assert_eq!(app(&harness).shown, ["r1", "r2"]);
    assert_eq!(focused(&harness).as_deref(), Some("Delete Evening"));
}

#[test]
fn a_confirm_cancelled_after_its_opener_went_elsewhere_moves_focus_nearby() {
    let mut harness = harness(Routines::new(false));
    ask_to_delete(&mut harness, "Morning");
    // Something else removes Morning while the confirm is open.
    app_mut(&mut harness).remove("r1");
    harness.run();
    harness.run();
    press(&mut harness, Key::Escape);
    harness.run();
    assert_eq!(app(&harness).deleting, None);
    assert_eq!(focused(&harness).as_deref(), Some("Delete Evening"));
}

#[test]
fn a_confirm_cancelled_returns_focus_to_its_opener() {
    let mut harness = harness(Routines::new(false));
    ask_to_delete(&mut harness, "Evening");
    press(&mut harness, Key::Escape);
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("Delete Evening"));
}

#[test]
fn a_confirm_that_removes_the_only_row_returns_focus_nowhere() {
    let mut routines = Routines::new(false);
    routines.shown = vec!["r1"];
    let mut harness = harness(routines);
    ask_to_delete(&mut harness, "Morning");
    harness.get_by_label("Delete routine").click();
    harness.run();
    assert!(app(&harness).shown.is_empty());
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), None);
}

#[test]
fn accesskit_focus_stays_in_the_tree_when_a_focused_widget_is_not_presented() {
    let mut harness = harness(Routines::new(false));
    harness.state_mut().stray = true;
    // Without the host's guard the harness's AccessKit tree refuses the
    // update: "Focused ID ... is not in the node list".
    harness.step();
    assert_eq!(
        harness.ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new("gone"))
    );
    // egui drops the stray focus on the next pass.
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), None);
}

/// Keeps a copy of each pass's AccessKit update, as the browser's ARIA layer
/// receives it after the host's plugins.
struct Capture(std::sync::Arc<std::sync::Mutex<Option<egui::accesskit::TreeUpdate>>>);

impl egui::Plugin for Capture {
    fn debug_name(&self) -> &'static str {
        "focus-return-tests.capture"
    }

    fn output_hook(&mut self, _: &egui::Context, output: &mut egui::FullOutput) {
        if let Some(update) = &output.platform_output.accesskit_update {
            *self.0.lock().unwrap() = Some(update.clone());
        }
    }
}

/// The name of the element the ARIA layer's canvas names as its active
/// descendant after the latest pass.
fn aria_active(capture: &std::sync::Mutex<Option<egui::accesskit::TreeUpdate>>) -> Option<String> {
    let update = capture.lock().unwrap().clone()?;
    let tree = crate::aria::project(&update, 1.0);
    let active = tree.active?;
    tree.elements[&active].attributes.get("aria-label").cloned()
}

#[test]
fn the_aria_layer_follows_focus_to_the_fallback() {
    let capture = std::sync::Arc::new(std::sync::Mutex::new(None));
    let mut list = harness(Routines::new(false));
    list.ctx.add_plugin(Capture(capture.clone()));
    ask_to_delete(&mut list, "Evening");
    assert_eq!(aria_active(&capture).as_deref(), Some("Keep routine"));
    list.get_by_label("Delete routine").click();
    list.run();
    assert_eq!(aria_active(&capture).as_deref(), Some("Delete Weekend"));

    let mut navigation = harness(Routines::new(true));
    navigation.ctx.add_plugin(Capture(capture.clone()));
    open(&mut navigation, "Evening");
    app_mut(&mut navigation).remove("r2");
    navigation.run();
    press(&mut navigation, Key::Enter);
    assert_eq!(aria_active(&capture).as_deref(), Some("Weekend"));
}
