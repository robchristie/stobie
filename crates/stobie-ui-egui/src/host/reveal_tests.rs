#![cfg(test)]
//! Controls out of view in a hosted application: a long account screen
//! scrolls inside a visible-only region, as a dock pane or a navigation body
//! does, with rows of members, a field for a new member's name and notes
//! below the fold. Agents and host tests reveal what they cannot see, then
//! act on it.

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, AnchoredScrollArea, BoundAction, CapabilityDescription, DialogSpec, INSPECTION_VERSION,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    PresentationScope, REVEAL_CAPABILITY, ReadingTextSpec, ReceiptState, SET_TEXT_CAPABILITY,
    TextFieldSpec,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "account")]
enum Act {
    #[action(
        label = "Remove",
        description = "Remove the member with this ID",
        argument = "number",
        minimum = 0,
        maximum = 1000,
        integer = true
    )]
    Remove,
    #[action(label = "Close", description = "Close the member editor")]
    Close,
}

enum Intent {
    Name(String),
    Note(String),
}

/// The members listed above the field.
const MEMBERS: i64 = 40;

/// The scrolled view's height.
const VIEW: f32 = 300.0;

/// The field below the fold.
const NAME: &str = "account.new-name";

/// The field below the fold of the editor dialog.
const NOTE: &str = "account.editor-note";

/// The words of the notes under the field.
const NOTES: &str = "Members pay their dues at the start of each season.";

#[derive(Default)]
struct Account {
    name: String,
    note: String,
    removed: Vec<i64>,
    /// Present the screen outside a visible-only region, as a plain
    /// scrolled page is.
    plain: bool,
    /// Leave the field out, as a section collapsed by the user would,
    /// without an intent.
    hidden: bool,
    /// Present the rows in an anchored scroll area that sticks to its end,
    /// as a conversation does.
    anchored: bool,
    /// Present the member editor, a modal dialog, over the screen.
    editing: bool,
}

impl Account {
    fn screen(&self, ui: &mut Ui, frame: &mut Frame<'_, Act, Intent>) {
        let area = egui::ScrollArea::vertical()
            .max_height(VIEW)
            .stick_to_bottom(self.anchored)
            .auto_shrink(false);
        let mut content = |ui: &mut Ui, mut rows: Option<&mut crate::ScrollRows>| {
            let mut p =
                frame.presentation(ui, PresentationScope::new("account"), SemanticUiId::root());
            for member in 0..MEMBERS {
                let mut row = |ui: &mut Ui| {
                    frame.action(
                        ui,
                        &mut p,
                        BoundAction::new(Act::Remove, member)
                            .with_subject(format!("member {member}")),
                        ActionEmphasis::Normal,
                    );
                };
                match rows.as_deref_mut() {
                    Some(rows) => rows.row(ui, member, row),
                    None => row(ui),
                }
            }
            if !self.hidden && !self.anchored {
                frame.text_field(
                    ui,
                    &mut p,
                    "new-name",
                    &TextFieldSpec::new("New member's name").id(NAME),
                    &self.name,
                    Intent::Name,
                );
                p.reading_text(ui, "notes", NOTES, ReadingTextSpec::default());
            }
            if self.editing {
                frame.dialog(
                    ui,
                    &mut p,
                    "editor",
                    &DialogSpec::new("Edit member", Act::Close),
                    |ui, frame, p| {
                        ui.add_space(2000.0);
                        frame.text_field(
                            ui,
                            p,
                            "note",
                            &TextFieldSpec::new("Note").id(NOTE),
                            &self.note,
                            Intent::Note,
                        );
                    },
                );
            }
            p.finish(ui);
        };
        if self.anchored {
            AnchoredScrollArea::new("members", area).show(ui, |ui, rows| content(ui, Some(rows)));
        } else {
            area.show(ui, |ui| content(ui, None));
        }
    }
}

impl Application for Account {
    type Action = Act;
    type Intent = Intent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Account")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, Intent>) {
        if self.plain {
            self.screen(ui, frame);
        } else {
            crate::observe_visible_only(ui, |ui| self.screen(ui, frame));
        }
    }

    fn apply(&mut self, intent: Intent, _: &mut ()) -> Result<bool, String> {
        match intent {
            Intent::Name(text) => self.name = text,
            Intent::Note(text) => self.note = text,
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        if target.action == Act::Close {
            self.editing = false;
        }
        Ok(true)
    }

    fn perform_with(
        &mut self,
        _: ActionTarget<Act>,
        argument: ActionArgument,
        _: &mut (),
    ) -> Result<bool, String> {
        if let ActionArgument::Number(member) = argument {
            self.removed.push(member as i64);
        }
        Ok(true)
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Account>>,
}

fn harness(account: Account) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(390.0, 400.0))
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
    let mut host = AppHost::new(&context, account);
    host.enable_inspection(Inspection::new("account-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Account> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Account> {
    harness.state_mut().host.as_mut().unwrap()
}

fn request(host: &AppHost<Account>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: INSPECTION_VERSION,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn capability(capability: &str) -> InspectionSelector {
    InspectionSelector {
        capability: Some(capability.into()),
        ..Default::default()
    }
}

fn set_text_on(id: &str) -> InspectionSelector {
    InspectionSelector {
        id: Some(SemanticUiId::new(id)),
        ..capability(SET_TEXT_CAPABILITY)
    }
}

/// Discover the capabilities `selector` matches.
fn discover(host: &AppHost<Account>, selector: InspectionSelector) -> Vec<CapabilityDescription> {
    let reply = host.inspection().unwrap().handle(request(
        host,
        "discover",
        InspectionOperation::Discover {
            selector,
            limit: 8,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities
}

/// Discover `selector`'s one target and invoke it through the inspection
/// interface, as an agent does, run the host and read the receipt.
fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    selector: InspectionSelector,
    value: &str,
) -> InvocationReceipt {
    let found = discover(host(harness), selector.clone());
    let [capability] = found.as_slice() else {
        panic!("{selector:?} discovered {found:?}");
    };
    let host = host(harness);
    let reply = host.inspection().unwrap().handle(request(
        host,
        id,
        InspectionOperation::Invoke {
            selector,
            expected: capability.target.clone(),
            arguments: serde_json::json!({ "value": value }),
        },
    ));
    assert!(reply.error.is_none(), "{reply:?}");
    harness.run();
    receipt(harness, id)
}

fn receipt(harness: &Harness<'_, Probe>, id: &str) -> InvocationReceipt {
    let host = host(harness);
    let reply = host.inspection().unwrap().handle(request(
        host,
        &format!("{id}-receipt"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    ));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("not a receipt: {reply:?}");
    };
    receipt
}

/// The latest published observation's ID.
fn observation(harness: &Harness<'_, Probe>) -> u64 {
    let host = host(harness);
    let reply =
        host.inspection()
            .unwrap()
            .handle(request(host, "observe", InspectionOperation::Observe));
    let Some(InspectionResult::Observe { observation }) = reply.result else {
        panic!("not an observation: {reply:?}");
    };
    observation.id
}

fn presented(harness: &Harness<'_, Probe>, id: &str) -> Option<egui::Rect> {
    host(harness)
        .ui_snapshot()
        .node(&SemanticUiId::new(id))
        .map(|node| node.rect.into())
}

/// A member row's control, by its accessible name, where presented.
fn row(harness: &Harness<'_, Probe>, member: i64) -> Option<UiNode> {
    let name = format!("Remove member {member}");
    host(harness)
        .ui_snapshot()
        .nodes
        .iter()
        .find(|node| node.name == name)
        .cloned()
}

fn click(harness: &mut Harness<'_, Probe>, rect: egui::Rect) {
    let centre = rect.center();
    harness.event(egui::Event::PointerMoved(centre));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: centre,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.run();
}

/// The scrolled view on screen: the harness frames its page in 8 points.
fn view() -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(8.0, 8.0), egui::vec2(374.0, VIEW))
}

#[test]
fn an_agent_reveals_a_field_below_the_fold_then_sets_its_text() {
    let mut harness = harness(Account::default());
    // Below the fold of a visible-only region, the field is not observed,
    // so set text finds nothing to bind.
    assert!(presented(&harness, NAME).is_none());
    assert!(discover(host(&harness), set_text_on(NAME)).is_empty());
    // The application's reveal capability is published, and takes the ID.
    let [reveal] = discover(host(&harness), capability(REVEAL_CAPABILITY))
        .try_into()
        .expect("one reveal capability");
    assert_eq!(reveal.id.0, REVEAL_CAPABILITY);
    assert!(reveal.availability.enabled() && reveal.semantic_invocable && !reveal.rendered);
    let receipt = invoke(&mut harness, "reveal", capability(REVEAL_CAPABILITY), NAME);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    // The observation the receipt names presents the field wholly in view.
    let after = receipt
        .after_observation
        .expect("completed after an observation");
    assert!(observation(&harness) >= after);
    let field = presented(&harness, NAME).expect("the field is observed");
    assert!(view().contains_rect(field), "{field:?}");
    let receipt = invoke(&mut harness, "set-name", set_text_on(NAME), "Robin");
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(host(&harness).app().name, "Robin");
}

#[test]
fn a_host_test_reveals_a_far_off_control_and_clicks_it() {
    let mut harness = harness(Account::default());
    let first = row(&harness, 1).expect("the second row starts in view");
    host_mut(&mut harness).reveal(SemanticUiId::new(NAME));
    harness.run();
    assert_eq!(
        host(&harness).reveal_outcome(),
        Some((&SemanticUiId::new(NAME), Ok(())))
    );
    // Scrolled to the foot, the row is no longer observed.
    assert!(row(&harness, 1).is_none());
    // Revealed by its ID, it is in view again and takes a click.
    host_mut(&mut harness).reveal(first.id.clone());
    harness.run();
    assert_eq!(host(&harness).reveal_outcome(), Some((&first.id, Ok(()))));
    let shown = row(&harness, 1).expect("the row is observed again");
    assert!(view().contains_rect(shown.rect.into()));
    click(&mut harness, shown.rect.into());
    assert_eq!(host(&harness).app().removed, [1]);
}

#[test]
fn a_far_off_field_in_a_plain_scrolled_page_takes_a_click_once_revealed() {
    let mut harness = harness(Account {
        plain: true,
        ..Account::default()
    });
    // Outside a visible-only region the field is observed where it lies,
    // below the view, where a click cannot reach it.
    let field = presented(&harness, NAME).expect("observed as presented");
    assert!(!view().intersects(field));
    host_mut(&mut harness).reveal(SemanticUiId::new(NAME));
    harness.run();
    assert_eq!(host(&harness).reveal_outcome().unwrap().1, Ok(()));
    let field = presented(&harness, NAME).unwrap();
    assert!(view().contains_rect(field), "{field:?}");
    click(&mut harness, field);
    let node = host(&harness).ui_snapshot().node(&SemanticUiId::new(NAME));
    assert!(node.unwrap().focused);
}

#[test]
fn reading_text_out_of_view_is_revealed_by_its_semantic_id() {
    let mut harness = harness(Account::default());
    let observed = |harness: &Harness<'_, Probe>| {
        serde_json::to_string(&host(harness).ui_snapshot().text)
            .unwrap()
            .contains("Members pay")
    };
    assert!(!observed(&harness));
    let notes = PresentationScope::new("account")
        .instance("notes")
        .semantic_id();
    host_mut(&mut harness).reveal(notes.clone());
    harness.run();
    assert_eq!(host(&harness).reveal_outcome(), Some((&notes, Ok(()))));
    assert!(observed(&harness));
}

#[test]
fn a_reveal_of_a_node_no_pass_presents_fails_loudly() {
    let mut harness = harness(Account::default());
    let receipt = invoke(
        &mut harness,
        "reveal-missing",
        capability(REVEAL_CAPABILITY),
        "account.missing",
    );
    assert_eq!(receipt.state, ReceiptState::Rejected);
    assert_eq!(
        receipt.error.unwrap().message,
        "account.missing is not presented"
    );
    host_mut(&mut harness).reveal(SemanticUiId::new("account.missing"));
    harness.run();
    assert_eq!(
        host(&harness).reveal_outcome().unwrap().1,
        Err("account.missing is not presented")
    );
}

#[test]
fn a_reveal_behind_a_dialog_is_refused_and_one_inside_it_scrolls_the_dialog() {
    let mut harness = harness(Account::default());
    host_mut(&mut harness).app_mut().editing = true;
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    let receipt = invoke(&mut harness, "behind", capability(REVEAL_CAPABILITY), NAME);
    assert_eq!(receipt.state, ReceiptState::Rejected);
    assert_eq!(
        receipt.error.unwrap().message,
        "The dialog \"Edit member\" is open"
    );
    assert!(presented(&harness, NOTE).is_none());
    let receipt = invoke(&mut harness, "inside", capability(REVEAL_CAPABILITY), NOTE);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert!(presented(&harness, NOTE).is_some());
    let receipt = invoke(&mut harness, "note", set_text_on(NOTE), "Treasurer");
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(host(&harness).app().note, "Treasurer");
}

#[test]
fn a_reveal_scrolls_an_anchored_area_stuck_to_its_end() {
    // The first row's ID, from a screen that shows it at the top.
    let first = row(&harness(Account::default()), 0)
        .expect("the first row starts in view")
        .id;
    let mut harness = harness(Account {
        anchored: true,
        ..Account::default()
    });
    // Stuck to its end, the area shows the last rows.
    assert!(row(&harness, MEMBERS - 1).is_some());
    assert!(row(&harness, 0).is_none());
    host_mut(&mut harness).reveal(first.clone());
    harness.run();
    assert_eq!(host(&harness).reveal_outcome(), Some((&first, Ok(()))));
    // The area stays where the reveal left it, rather than returning to its
    // end, until the reader scrolls back there.
    for _ in 0..3 {
        host_mut(&mut harness).request_repaint("test");
        harness.run();
    }
    let shown = row(&harness, 0).expect("the first row stays in view");
    assert!(view().contains_rect(shown.rect.into()));
}

#[test]
fn set_text_on_a_field_gone_before_the_edit_applies_fails_loudly() {
    let mut harness = harness(Account {
        plain: true,
        ..Account::default()
    });
    let [field] = discover(host(&harness), set_text_on(NAME))
        .try_into()
        .expect("the field is bound");
    // The field leaves the screen between discovery and the pass that
    // applies the edit, without an intent the host could see.
    host_mut(&mut harness).app_mut().hidden = true;
    let hosted = host(&harness);
    let reply = hosted.inspection().unwrap().handle(request(
        hosted,
        "gone",
        InspectionOperation::Invoke {
            selector: set_text_on(NAME),
            expected: field.target,
            arguments: serde_json::json!({ "value": "Robin" }),
        },
    ));
    assert!(reply.error.is_none(), "{reply:?}");
    harness.run();
    let receipt = receipt(&harness, "gone");
    assert_eq!(receipt.state, ReceiptState::Rejected, "{receipt:?}");
    assert_eq!(
        receipt.error.unwrap().message,
        "account.new-name is no longer presented"
    );
    assert_eq!(host(&harness).app().name, "");
}
