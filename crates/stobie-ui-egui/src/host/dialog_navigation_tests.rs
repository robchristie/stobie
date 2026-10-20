#![cfg(test)]
//! A dialog opened from a pane of the navigation recipe: it is modal over
//! every layout, its stacked Back and the list behind it included, and focus
//! returns to its opener in the pane.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, DestinationSpec, DialogSpec, InspectionOperation, InspectionResult,
    InspectionSelector, NavigationContent, NavigationPane, NavigationSpec, PresentationScope,
    ReadingTextSpec,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "people")]
enum Act {
    #[action(
        label = "Open",
        description = "Open this person",
        argument = "text",
        max_length = 8
    )]
    Open,
    #[action(label = "Back", description = "Return to the list")]
    Back,
    #[action(label = "Rename", description = "Rename the person")]
    Rename,
    #[action(label = "Close", description = "Close the editor")]
    Close,
}

#[derive(Default)]
struct People {
    open: Option<&'static str>,
    renaming: bool,
    performed: Vec<Act>,
}

const PEOPLE: [(&str, &str); 2] = [("p1", "Ada"), ("p2", "Grace")];

impl Application for People {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("People")
    }

    fn availability(&self, target: ActionTarget<Act>) -> Availability {
        match target.action {
            Act::Back | Act::Rename if self.open.is_none() => Availability::Hidden,
            _ => Availability::Enabled,
        }
    }

    fn availability_with(&self, _: ActionTarget<Act>, argument: &ActionArgument) -> Availability {
        match argument {
            ActionArgument::Text(id) if PEOPLE.iter().any(|(person, _)| person == id) => {
                Availability::Enabled
            }
            _ => Availability::Hidden,
        }
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("people"), SemanticUiId::root());
        let open = self
            .open
            .and_then(|id| PEOPLE.iter().find(|(person, _)| *person == id));
        let spec = NavigationSpec {
            list_title: "People",
            content: open.map(|(_, name)| NavigationContent {
                title: name,
                reference: None,
                detail: None,
            }),
            back: Act::Back,
        };
        frame.navigation(ui, &mut p, "navigation", &spec, |ui, p, frame, pane| {
            match pane {
                NavigationPane::List => {
                    for (id, name) in PEOPLE {
                        frame.navigation_item(
                            ui,
                            p,
                            BoundAction::new(Act::Open, id),
                            DestinationSpec {
                                icon: IconId::User,
                                label: name,
                                description: "Open this person",
                                selected: self.open == Some(id),
                                badge: None,
                            },
                        );
                    }
                }
                NavigationPane::Content => {
                    frame.action(ui, p, Act::Rename, ActionEmphasis::Normal);
                    // The dialog is presented from inside the pane.
                    if self.renaming {
                        frame.dialog(
                            ui,
                            p,
                            "rename",
                            &DialogSpec::new("Rename", Act::Close).sheet(),
                            |ui, _, p| {
                                p.reading_text(
                                    ui,
                                    "hint",
                                    "Names appear in every conversation.",
                                    ReadingTextSpec::default(),
                                );
                            },
                        );
                    }
                }
                NavigationPane::Detail | NavigationPane::Empty => {
                    p.reading_text(ui, "empty", "Choose a person", ReadingTextSpec::default());
                }
            }
        });
    }

    fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        self.performed.push(target.action);
        match target.action {
            Act::Back => self.open = None,
            Act::Rename => self.renaming = true,
            Act::Close => self.renaming = false,
            Act::Open => return Err("Open needs a person".into()),
        }
        Ok(true)
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<Act>,
        argument: ActionArgument,
        view: &mut (),
    ) -> Result<bool, String> {
        let ActionArgument::Text(id) = &argument else {
            return self.perform(target, view);
        };
        self.performed.push(target.action);
        self.open = PEOPLE
            .iter()
            .find(|(person, _)| person == id)
            .map(|(person, _)| *person);
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<People>>,
}

fn harness(width: f32) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 760.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(
        &context,
        People {
            open: Some("p1"),
            ..People::default()
        },
    );
    host.enable_inspection(Inspection::new("people-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn app<'a>(harness: &'a Harness<'_, Probe>) -> &'a People {
    harness.state().host.as_ref().unwrap().app()
}

fn focused(harness: &Harness<'_, Probe>) -> Option<String> {
    let widget = harness.ctx.memory(|memory| memory.focused())?;
    let node = widget.accesskit_id();
    harness
        .query_by(|candidate| candidate.locate().0 == node)
        .map(|found| found.accesskit_node().label().unwrap_or_default())
}

fn press(harness: &mut Harness<'_, Probe>, modifiers: Modifiers, key: Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
}

fn disabled_reason(harness: &Harness<'_, Probe>, action: Act) -> Option<String> {
    let host = harness.state().host.as_ref().unwrap();
    let inspection = host.inspection().unwrap();
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: "discover".into(),
        instance: Some(inspection.instance()),
        operation: InspectionOperation::Discover {
            selector: InspectionSelector {
                capability: Some(action.stable_id().into()),
                ..Default::default()
            },
            limit: 16,
            cursor: None,
        },
    });
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities
        .first()
        .and_then(|capability| capability.availability.disabled_reason())
        .map(ToOwned::to_owned)
}

#[test]
fn a_dialog_over_every_navigation_layout_contains_focus_and_returns_it() {
    // The stack with Back, the split and the columns.
    for width in [390.0, 820.0, 1280.0] {
        let mut harness = harness(width);
        harness.get_by_label("Rename").click();
        harness.run();
        assert!(app(&harness).renaming, "{width}");
        // Only the close control takes focus: the list, Back and Rename
        // are behind the dialog.
        for _ in 0..4 {
            press(&mut harness, Modifiers::NONE, Key::Tab);
            assert_eq!(focused(&harness).as_deref(), Some("Close"), "{width}");
        }
        press(&mut harness, Modifiers::SHIFT, Key::Tab);
        assert_eq!(focused(&harness).as_deref(), Some("Close"), "{width}");
        // Agents reach neither Back nor the list.
        for action in [Act::Back, Act::Open] {
            assert_eq!(
                disabled_reason(&harness, action).as_deref(),
                Some("The dialog \"Rename\" is open"),
                "{width}: {action:?}"
            );
        }
        let snapshot = harness.state().host.as_ref().unwrap().ui_snapshot();
        assert!(
            snapshot
                .nodes
                .iter()
                .filter(|node| node.role != UiRole::Dialog)
                .all(|node| !matches!(node.name.as_str(), "Ada" | "Grace" | "Back" | "Rename")),
            "{width}: {:?}",
            snapshot.nodes
        );
        assert!(
            snapshot.semantic_audit.is_empty(),
            "{width}: {:?}",
            snapshot.semantic_audit
        );
        press(&mut harness, Modifiers::NONE, Key::Escape);
        harness.run();
        assert!(!app(&harness).renaming, "{width}");
        assert_eq!(focused(&harness).as_deref(), Some("Rename"), "{width}");
        assert_eq!(
            app(&harness).performed,
            [Act::Rename, Act::Close],
            "{width}"
        );
    }
}
