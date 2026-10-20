#![cfg(test)]
//! The copy recipe through the host: a press, a key or an agent copies the
//! text, the control confirms for a while, and the text never reaches
//! observation.

use std::{cell::RefCell, rc::Rc};

use egui::Vec2;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, ArgumentContract, COPY_CONFIRMATION, COPY_TEXT_CAPABILITY, CapabilityDescription,
    CopySpec, InspectionErrorCode, InspectionOperation, InspectionResult, InspectionSelector,
    InvocationReceipt, PresentationScope, ReceiptState,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "invite")]
enum Act {
    #[action(label = "New code", description = "Issue a new invitation code")]
    Renew,
}

const CODES: [&str; 2] = ["QX7-SECRET-4410", "ZK2-SECRET-9031"];

/// An invitation code with its copy control, a disabled copy control and a
/// hidden one.
#[derive(Default)]
struct Invitation {
    code: usize,
}

impl Application for Invitation {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Invitation")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("invite"), SemanticUiId::root());
        p.copy(
            ui,
            "code",
            &CopySpec::new(CODES[self.code], "Copy code", "Copied", "Couldn't copy"),
        );
        p.copy(
            ui,
            "expired",
            &CopySpec {
                availability: Availability::Disabled {
                    reason: "The code has expired".into(),
                },
                ..CopySpec::new(CODES[1], "Copy old code", "Copied", "Couldn't copy")
            },
        );
        p.copy(
            ui,
            "hidden",
            &CopySpec {
                availability: Availability::Hidden,
                ..CopySpec::new(CODES[1], "Copy hidden code", "Copied", "Couldn't copy")
            },
        );
        frame.action(ui, &mut p, Act::Renew, ActionEmphasis::Normal);
    }

    fn apply(&mut self, _intent: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, _target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        self.code = 1 - self.code;
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Invitation>>,
}

type Copied = Rc<RefCell<Vec<String>>>;

fn harness() -> (Harness<'static, Probe>, Copied) {
    let copied = Copied::default();
    let recorded = copied.clone();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            move |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
                ui.ctx().output(|output| {
                    recorded
                        .borrow_mut()
                        .extend(output.commands.iter().filter_map(|command| match command {
                            egui::OutputCommand::CopyText(text) => Some(text.clone()),
                            _ => None,
                        }));
                });
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, Invitation::default());
    host.enable_inspection(Inspection::new("invite-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    (harness, copied)
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Invitation> {
    harness.state().host.as_ref().unwrap()
}

fn id(key: &str) -> crate::PresentationId {
    PresentationScope::new("invite").instance(key)
}

fn status(key: &str) -> SemanticUiId {
    id(key).child("status").semantic_id()
}

fn request(
    host: &AppHost<Invitation>,
    id: &str,
    operation: InspectionOperation,
) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn copy_selector(key: &str) -> InspectionSelector {
    InspectionSelector {
        id: Some(id(key).semantic_id()),
        capability: Some(COPY_TEXT_CAPABILITY.into()),
        ..Default::default()
    }
}

fn discover(
    host: &AppHost<Invitation>,
    selector: InspectionSelector,
) -> Vec<CapabilityDescription> {
    let reply = host.inspection().unwrap().handle(request(
        host,
        "discover",
        InspectionOperation::Discover {
            selector,
            limit: 16,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities
}

fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    selector: InspectionSelector,
    expected: crate::ResolvedTarget,
) -> Result<InvocationReceipt, crate::InspectionError> {
    let current = host(harness);
    let reply = current.inspection().unwrap().handle(request(
        current,
        id,
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments: serde_json::Value::Null,
        },
    ));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        return Err(reply.error.expect("a reply without a receipt has an error"));
    };
    if receipt.state == ReceiptState::Rejected {
        return Err(receipt.error.expect("a rejected receipt has an error"));
    }
    if receipt.state != ReceiptState::Queued {
        return Ok(receipt);
    }
    harness.run();
    let host = host(harness);
    let reply = host.inspection().unwrap().handle(request(
        host,
        &format!("{id}-receipt"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    ));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    match receipt.state {
        ReceiptState::Rejected => Err(receipt.error.expect("a rejected receipt has an error")),
        _ => Ok(receipt),
    }
}

fn status_name(harness: &Harness<'_, Probe>, key: &str) -> Option<String> {
    host(harness)
        .ui_snapshot()
        .node(&status(key))
        .map(|node| node.name.clone())
}

#[test]
fn a_press_copies_the_text_and_confirms_with_a_status() {
    let (mut harness, copied) = harness();
    let snapshot = host(&harness).ui_snapshot();
    let button = snapshot.node(&id("code").semantic_id()).unwrap();
    assert_eq!(button.role, UiRole::Button);
    assert_eq!(button.name, "Copy code");
    assert!(button.enabled);
    // No status before an outcome, and no audit finding.
    assert!(snapshot.node(&status("code")).is_none());
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.audit().is_empty(), "{:?}", snapshot.audit());

    harness.get_by_label("Copy code").click();
    harness.run();
    assert_eq!(std::mem::take(&mut *copied.borrow_mut()), [CODES[0]]);
    let snapshot = host(&harness).ui_snapshot();
    let confirmation = snapshot.node(&status("code")).unwrap();
    assert_eq!(confirmation.role, UiRole::Status);
    assert_eq!(confirmation.name, "Copied");
    // The button keeps its name while its face shows the confirmation.
    assert_eq!(
        snapshot.node(&id("code").semantic_id()).unwrap().name,
        "Copy code"
    );
    assert!(snapshot.audit().is_empty(), "{:?}", snapshot.audit());
    // Assistive technology sees a polite status with the confirmation.
    let announced = harness.get_by_label("Copied");
    assert_eq!(
        announced.accesskit_node().role(),
        egui::accesskit::Role::Status
    );
    assert_eq!(
        announced.accesskit_node().live(),
        egui::accesskit::Live::Polite
    );
    // Copying once copies once.
    harness.run();
    assert!(copied.borrow().is_empty());
}

#[test]
fn the_confirmation_returns_to_the_label_after_its_time() {
    let (mut harness, _) = harness();
    harness.get_by_label("Copy code").click();
    harness.step();
    harness.step();
    assert_eq!(status_name(&harness, "code").as_deref(), Some("Copied"));
    let shown = harness.ctx.input(|input| input.time);
    let mut hidden = None;
    for _ in 0..16 {
        harness.step();
        if status_name(&harness, "code").is_none() {
            hidden = Some(harness.ctx.input(|input| input.time));
            break;
        }
    }
    let elapsed = hidden.expect("the confirmation never ended") - shown;
    let confirmation = COPY_CONFIRMATION.as_secs_f64();
    assert!(
        (confirmation..=confirmation + 0.25).contains(&elapsed),
        "{elapsed}"
    );
    // The live region stays, empty, for the next outcome.
    let code = status("code");
    let region = harness
        .query_by(|node| node.author_id() == Some(code.0.as_str()))
        .expect("the status region stays");
    assert_eq!(
        region.accesskit_node().role(),
        egui::accesskit::Role::Status
    );
    assert_eq!(region.accesskit_node().label().unwrap_or_default(), "");
}

#[test]
fn the_keyboard_copies_from_the_focused_control() {
    let (mut harness, copied) = harness();
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(
        harness.ctx.memory(|memory| memory.focused()),
        Some(id("code").egui_id())
    );
    assert!(
        host(&harness)
            .ui_snapshot()
            .node(&id("code").semantic_id())
            .unwrap()
            .focused
    );
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(std::mem::take(&mut *copied.borrow_mut()), [CODES[0]]);
    assert_eq!(status_name(&harness, "code").as_deref(), Some("Copied"));
}

#[test]
fn agents_discover_and_invoke_copy_text() {
    let (mut harness, copied) = harness();
    let capabilities = discover(host(&harness), copy_selector("code"));
    assert_eq!(capabilities.len(), 1);
    let capability = &capabilities[0];
    assert_eq!(capability.capability, COPY_TEXT_CAPABILITY);
    assert_eq!(capability.arguments, ArgumentContract::None);
    assert!(capability.semantic_invocable && capability.availability.enabled());
    // The meaning names a revision of the text, never the text.
    assert_eq!(capability.target.meaning, "text-revision:1");

    let receipt = invoke(
        &mut harness,
        "copy",
        copy_selector("code"),
        capability.target.clone(),
    )
    .unwrap();
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(std::mem::take(&mut *copied.borrow_mut()), [CODES[0]]);
    assert_eq!(status_name(&harness, "code").as_deref(), Some("Copied"));
    harness.run();
    assert!(copied.borrow().is_empty());

    // Only copy controls that are presented carry the capability.
    let all = discover(
        host(&harness),
        InspectionSelector {
            capability: Some(COPY_TEXT_CAPABILITY.into()),
            ..Default::default()
        },
    );
    assert_eq!(all.len(), 2);
    assert!(discover(host(&harness), copy_selector("hidden")).is_empty());
}

#[test]
fn a_disabled_or_changed_copy_target_copies_nothing() {
    let (mut harness, copied) = harness();
    let expired = discover(host(&harness), copy_selector("expired"));
    assert_eq!(expired.len(), 1);
    assert_eq!(
        expired[0].availability.disabled_reason(),
        Some("The code has expired")
    );
    let error = invoke(
        &mut harness,
        "expired",
        copy_selector("expired"),
        expired[0].target.clone(),
    )
    .unwrap_err();
    assert_eq!(error.code, InspectionErrorCode::Unavailable);

    // A new code makes the earlier target stale.
    let stale = discover(host(&harness), copy_selector("code"))[0]
        .target
        .clone();
    let renew = InspectionSelector {
        capability: Some("invite.renew".into()),
        ..Default::default()
    };
    let target = discover(host(&harness), renew.clone())[0].target.clone();
    invoke(&mut harness, "renew", renew, target).unwrap();
    harness.run();
    let error = invoke(&mut harness, "stale", copy_selector("code"), stale).unwrap_err();
    assert_eq!(error.code, InspectionErrorCode::StaleTarget);
    assert!(copied.borrow().is_empty());

    let current = discover(host(&harness), copy_selector("code"))[0]
        .target
        .clone();
    assert_eq!(current.meaning, "text-revision:2");
    invoke(&mut harness, "fresh", copy_selector("code"), current).unwrap();
    assert_eq!(std::mem::take(&mut *copied.borrow_mut()), [CODES[1]]);
}

#[test]
fn a_new_text_drops_an_earlier_confirmation() {
    let (mut harness, _) = harness();
    harness.get_by_label("Copy code").click();
    harness.run();
    assert_eq!(status_name(&harness, "code").as_deref(), Some("Copied"));
    harness.get_by_label("New code").click();
    harness.run();
    assert_eq!(status_name(&harness, "code"), None);
}

#[test]
fn the_copied_text_never_reaches_observation() {
    let (mut harness, copied) = harness();
    harness.get_by_label("Copy code").click();
    harness.run();
    assert_eq!(copied.borrow().as_slice(), [CODES[0]]);
    let snapshot = host(&harness).ui_snapshot();
    let published = serde_json::to_string(&snapshot).unwrap();
    let described =
        serde_json::to_string(&discover(host(&harness), InspectionSelector::default())).unwrap();
    for text in [published, described] {
        assert!(!text.contains("SECRET"), "{text}");
    }
    assert!(snapshot.node(&id("hidden").semantic_id()).is_none());
    assert!(harness.query_by_label("Copy hidden code").is_none());
}
