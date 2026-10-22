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
    CopySpec, InspectionErrorCode, InspectionOperation, InspectionRequest, InspectionResult,
    InspectionSelector, InvocationReceipt, PresentationScope, ReceiptState,
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

/// A reply in a message bubble whose code blocks each carry a copy control.
struct Reply {
    source: String,
}

const REPLY: &str = "Install it, then check it:\n\n```sh\npip install example\n```\n\n\
Tabs become spaces and the final line break goes:\n\n```\nfor x in xs:\n\tprint(x)\n\n```\n";

impl Application for Reply {
    type Action = Act;
    type Intent = ();
    type UiState = Vec<(Rect, Vec<Rect>)>;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Reply")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, layouts: &mut Self::UiState, frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("chat"), SemanticUiId::root());
        let spec = crate::MessageBubbleSpec {
            side: crate::MessageSide::Other,
            sender: "Assistant",
        };
        p.message_bubble(ui, ("message", 7), spec, |ui, p| {
            p.markdown_with_code_copy(
                ui,
                "body",
                &self.source,
                crate::MarkdownSpec::default(),
                &crate::CodeCopySpec::new("Copy code", "Copied", "Couldn't copy"),
            );
        });
        // Where the bubble and each control were, pass by pass.
        let observed = p.finish(ui);
        let bubble = observed
            .semantic_nodes
            .iter()
            .find(|node| node.role == UiRole::Section)
            .map_or(Rect::NOTHING, |node| node.rect.into());
        let controls = observed
            .semantic_nodes
            .iter()
            .filter(|node| node.role == UiRole::Button)
            .map(|node| node.rect.into())
            .collect();
        layouts.push((bubble, controls));
    }

    fn apply(&mut self, _intent: (), _: &mut Self::UiState) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(
        &mut self,
        _target: ActionTarget<Act>,
        _: &mut Self::UiState,
    ) -> Result<bool, String> {
        Ok(false)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

fn reply_harness(width: f32, source: &str) -> (Harness<'static, Option<AppHost<Reply>>>, Copied) {
    let copied = Copied::default();
    let recorded = copied.clone();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 700.0))
        .build_ui_state(
            move |ui, host: &mut Option<AppHost<Reply>>| {
                if let Some(host) = host {
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
            None,
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    *harness.state_mut() = Some(AppHost::new(
        &context,
        Reply {
            source: source.to_owned(),
        },
    ));
    harness.run();
    (harness, copied)
}

fn reply<'a>(harness: &'a Harness<'_, Option<AppHost<Reply>>>) -> &'a AppHost<Reply> {
    harness.state().as_ref().unwrap()
}

/// The bubble and its controls in the last pass, asserting that every pass
/// since `since` put them in the same place.
fn settled(harness: &Harness<'_, Option<AppHost<Reply>>>, since: usize) -> (Rect, Vec<Rect>) {
    let passes = &reply(harness).ui_state()[since..];
    let last = passes.last().unwrap().clone();
    for (bubble, controls) in passes {
        assert!(
            (bubble.min - last.0.min).length() < 0.5
                && (bubble.size() - last.0.size()).length() < 0.5
        );
        assert_eq!(controls.len(), last.1.len());
        for (control, now) in controls.iter().zip(&last.1) {
            assert!(
                (control.min - now.min).length() < 0.5,
                "{control:?} {now:?}"
            );
        }
    }
    last
}

#[test]
fn each_code_block_in_a_message_bubble_copies_its_own_text() {
    for width in [390.0, 900.0] {
        let (mut harness, copied) = reply_harness(width, REPLY);
        let snapshot = reply(&harness).ui_snapshot();
        let controls: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Button)
            .collect();
        let names: Vec<_> = controls.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, ["Copy code", "Copy code 2"]);
        // The controls are inside the bubble, which holds them.
        let bubble = snapshot
            .nodes
            .iter()
            .find(|node| node.role == UiRole::Section)
            .unwrap();
        for control in &controls {
            assert_eq!(control.parent.as_ref(), Some(&bubble.id));
            assert!(Rect::from(bubble.rect).contains_rect(control.rect.into()));
        }
        assert!(snapshot.audit().is_empty(), "{:?}", snapshot.audit());
        let since = reply(&harness).ui_state().len();
        harness.run();
        let before = settled(&harness, since);

        harness.get_by_label("Copy code 2").click();
        harness.run();
        assert_eq!(
            std::mem::take(&mut *copied.borrow_mut()),
            ["for x in xs:\n    print(x)\n"]
        );
        harness.get_by_label("Copy code").click();
        harness.run();
        assert_eq!(
            std::mem::take(&mut *copied.borrow_mut()),
            ["pip install example"]
        );
        // Both confirm; neither the bubble nor a control moves, and the
        // bubble settles while confirming and once the confirmations end.
        assert_eq!(harness.query_all_by_label("Copied").count(), 2);
        let since = reply(&harness).ui_state().len();
        harness.run();
        assert_eq!(settled(&harness, since), before, "{width}");
        for _ in 0..12 {
            harness.step();
        }
        assert!(harness.query_by_label("Copied").is_none());
        let since = reply(&harness).ui_state().len();
        harness.run();
        assert_eq!(settled(&harness, since), before, "{width}");
    }
}

#[test]
fn a_growing_reply_keeps_each_controls_key_and_confirmation() {
    let first = "```sh\npip install example\n```\n";
    let (mut harness, copied) = reply_harness(390.0, first);
    harness.get_by_label("Copy code").click();
    harness.run();
    assert_eq!(copied.borrow().as_slice(), ["pip install example"]);
    let id = |index: usize| {
        PresentationScope::new("chat")
            .child(("message", 7))
            .instance((&"body", "stobie.code-copy", index))
    };
    let status = |harness: &Harness<'_, Option<AppHost<Reply>>>, index: usize| {
        reply(harness)
            .ui_snapshot()
            .node(&id(index).child("status").semantic_id())
            .map(|node| node.name.clone())
    };
    assert_eq!(status(&harness, 0).as_deref(), Some("Copied"));
    // A second block arrives while the first confirms.
    harness.state_mut().as_mut().unwrap().app_mut().source = REPLY.to_owned();
    harness.run();
    // The first control keeps its name as well as its key.
    let snapshot = reply(&harness).ui_snapshot();
    assert_eq!(
        snapshot.node(&id(0).semantic_id()).unwrap().name,
        "Copy code"
    );
    assert_eq!(
        snapshot.node(&id(1).semantic_id()).unwrap().name,
        "Copy code 2"
    );
    assert_eq!(status(&harness, 0).as_deref(), Some("Copied"));
    assert_eq!(status(&harness, 1), None);
    // AccessKit names them as the snapshot does.
    harness.get_by_label("Copy code");
    harness.get_by_label("Copy code 2");
    let mismatches = crate::audit_accesskit(snapshot, &accesskit_tree(&harness));
    assert!(mismatches.is_empty(), "{mismatches:?}");
    // Each is bound under its own ID, and invoking the second copies its block.
    harness
        .state_mut()
        .as_mut()
        .unwrap()
        .enable_inspection(Inspection::new("reply-test"));
    harness.run();
    let handle = |harness: &Harness<'_, Option<AppHost<Reply>>>, id: &str, operation| {
        let inspection = reply(harness).inspection().unwrap();
        let reply = inspection.handle(InspectionRequest {
            version: 1,
            request_id: id.into(),
            instance: Some(inspection.instance()),
            operation,
        });
        reply
            .result
            .unwrap_or_else(|| panic!("{id} failed: {:?}", reply.error))
    };
    let selector = InspectionSelector {
        capability: Some(COPY_TEXT_CAPABILITY.into()),
        ..Default::default()
    };
    let InspectionResult::Discover { capabilities, .. } = handle(
        &harness,
        "discover",
        InspectionOperation::Discover {
            selector: selector.clone(),
            limit: 8,
            cursor: None,
        },
    ) else {
        panic!("not a discovery")
    };
    let ids: Vec<_> = capabilities
        .iter()
        .map(|capability| capability.id.clone())
        .collect();
    assert_eq!(ids, [id(0).semantic_id(), id(1).semantic_id()]);
    copied.borrow_mut().clear();
    let InspectionResult::Receipt { receipt } = handle(
        &harness,
        "copy-second",
        InspectionOperation::Invoke {
            selector: InspectionSelector {
                id: Some(id(1).semantic_id()),
                ..selector
            },
            expected: capabilities[1].target.clone(),
            arguments: serde_json::Value::Null,
        },
    ) else {
        panic!("not a receipt")
    };
    assert_eq!(receipt.state, ReceiptState::Queued, "{receipt:?}");
    harness.run();
    assert_eq!(copied.borrow().as_slice(), ["for x in xs:\n    print(x)\n"]);
}

/// The AccessKit tree as kittest holds it, for the parity audit.
fn accesskit_tree<S>(harness: &Harness<'_, S>) -> egui::accesskit::TreeUpdate {
    fn collect(
        node: egui_kittest::kittest::AccessKitNode<'_>,
        update: &mut egui::accesskit::TreeUpdate,
    ) {
        update.nodes.push((node.locate().0, node.data().clone()));
        for child in node.children() {
            collect(child, update);
        }
    }
    let root = harness.kittest_state().root();
    let mut update = egui::accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: Some(egui::accesskit::Tree::new(root.locate().0)),
        tree_id: egui::accesskit::TreeId::ROOT,
        focus: root.locate().0,
    };
    collect(root, &mut update);
    update
}
