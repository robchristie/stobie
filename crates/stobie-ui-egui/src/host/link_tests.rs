#![cfg(test)]
//! Links through the host: published as `stobie.open-link` capabilities and
//! opened in a new tab when an agent invokes one.

use std::{cell::RefCell, rc::Rc};

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, ArgumentContract, CapabilityDescription, InspectionErrorCode, InspectionOperation,
    InspectionResult, InspectionSelector, InvocationReceipt, LinkSpec, LinkTarget,
    OPEN_LINK_CAPABILITY, PresentationScope, ReadingTextSpec, ReceiptState,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "reader")]
enum Act {
    #[action(label = "Next", description = "Show the next source")]
    Next,
}

/// A reply with its source link, and a second link that is disabled.
#[derive(Default)]
struct Reader {
    /// Which source the reply cites; Next moves to another URL.
    source: usize,
}

const SOURCES: [&str; 2] = [
    "https://example.org/papers/first",
    "https://example.org/papers/second",
];

impl Application for Reader {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Reader")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("reader"), SemanticUiId::root());
        p.reading_text(
            ui,
            "reply",
            "Here is the paper I mentioned.",
            ReadingTextSpec::default(),
        );
        p.link(
            ui,
            "source",
            LinkSpec {
                // An application that wants the same tab still gets a new
                // one when an agent opens the link.
                target: LinkTarget::SameTab,
                ..LinkSpec::new("Read the paper", SOURCES[self.source])
            },
        );
        ui.add_enabled_ui(false, |ui| {
            p.link(
                ui,
                "archived",
                LinkSpec::new("Archived copy", "https://example.org/archive"),
            );
        });
        frame.action(ui, &mut p, Act::Next, ActionEmphasis::Normal);
    }

    fn apply(&mut self, _intent: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn perform(&mut self, _target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        self.source = 1 - self.source;
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Reader>>,
}

type Opened = Rc<RefCell<Vec<egui::OpenUrl>>>;

fn harness() -> (Harness<'static, Probe>, Opened) {
    let opened = Opened::default();
    let recorded = opened.clone();
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
                            egui::OutputCommand::OpenUrl(open) => Some(open.clone()),
                            _ => None,
                        }));
                });
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, Reader::default());
    host.enable_inspection(Inspection::new("reader-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    (harness, opened)
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Reader> {
    harness.state().host.as_ref().unwrap()
}

fn request(host: &AppHost<Reader>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn link_selector(key: &str) -> InspectionSelector {
    InspectionSelector {
        id: Some(PresentationScope::new("reader").instance(key).semantic_id()),
        capability: Some(OPEN_LINK_CAPABILITY.into()),
        ..Default::default()
    }
}

fn discover(host: &AppHost<Reader>, selector: InspectionSelector) -> Vec<CapabilityDescription> {
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

#[test]
fn presented_links_publish_an_open_link_capability_with_their_url() {
    let (harness, _) = harness();
    let snapshot = host(&harness).ui_snapshot();
    let source = snapshot
        .node(
            &PresentationScope::new("reader")
                .instance("source")
                .semantic_id(),
        )
        .unwrap();
    assert_eq!(source.role, UiRole::Link);
    assert_eq!(source.name, "Read the paper");
    assert_eq!(source.value.as_deref(), Some(SOURCES[0]));
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);

    let capabilities = discover(host(&harness), link_selector("source"));
    assert_eq!(capabilities.len(), 1);
    let capability = &capabilities[0];
    assert_eq!(capability.capability, OPEN_LINK_CAPABILITY);
    assert_eq!(capability.arguments, ArgumentContract::None);
    assert!(capability.semantic_invocable && capability.availability.enabled());
    assert_eq!(capability.target.meaning, SOURCES[0]);

    let archived = discover(host(&harness), link_selector("archived"));
    assert_eq!(archived.len(), 1);
    assert!(!archived[0].availability.enabled());

    // Only links carry the capability.
    let all = discover(
        host(&harness),
        InspectionSelector {
            capability: Some(OPEN_LINK_CAPABILITY.into()),
            ..Default::default()
        },
    );
    assert_eq!(all.len(), 2);
}

#[test]
fn keyboard_focus_reaches_the_link_node() {
    let (mut harness, opened) = harness();
    let id = PresentationScope::new("reader").instance("source");
    // The reply's selectable text takes no Tab stop; the link is first.
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(
        harness.ctx.memory(|memory| memory.focused()),
        Some(id.egui_id())
    );
    let node = host(&harness)
        .ui_snapshot()
        .node(&id.semantic_id())
        .unwrap()
        .clone();
    assert!(node.focused);
    opened.borrow_mut().clear();
    harness.key_press(egui::Key::Enter);
    harness.run();
    // The application asked for the same tab; a person's activation keeps it.
    assert_eq!(
        std::mem::take(&mut *opened.borrow_mut()),
        [egui::OpenUrl {
            url: SOURCES[0].into(),
            new_tab: false
        }]
    );
}

#[test]
fn invoking_open_link_asks_for_a_new_tab() {
    let (mut harness, opened) = harness();
    let expected = discover(host(&harness), link_selector("source"))[0]
        .target
        .clone();
    opened.borrow_mut().clear();
    let receipt = invoke(&mut harness, "open", link_selector("source"), expected).unwrap();
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(
        std::mem::take(&mut *opened.borrow_mut()),
        [egui::OpenUrl::new_tab(SOURCES[0])]
    );
    // Nothing opens again on later passes.
    harness.run();
    assert!(opened.borrow().is_empty());
}

#[test]
fn a_disabled_or_changed_link_is_not_opened() {
    let (mut harness, opened) = harness();
    let archived = discover(host(&harness), link_selector("archived"))[0]
        .target
        .clone();
    let error = invoke(
        &mut harness,
        "archived",
        link_selector("archived"),
        archived,
    )
    .unwrap_err();
    assert_eq!(error.code, InspectionErrorCode::Unavailable);

    // The URL is the target's meaning: after it changes, the old target is stale.
    let stale = discover(host(&harness), link_selector("source"))[0]
        .target
        .clone();
    let next = discover(
        host(&harness),
        InspectionSelector {
            capability: Some("reader.next".into()),
            ..Default::default()
        },
    )[0]
    .target
    .clone();
    let receipt = invoke(
        &mut harness,
        "next",
        InspectionSelector {
            capability: Some("reader.next".into()),
            ..Default::default()
        },
        next,
    )
    .unwrap();
    assert_eq!(receipt.state, ReceiptState::Completed);
    harness.run();
    opened.borrow_mut().clear();
    let error = invoke(&mut harness, "stale", link_selector("source"), stale).unwrap_err();
    assert_eq!(error.code, InspectionErrorCode::StaleTarget);
    assert!(opened.borrow().is_empty());
}
