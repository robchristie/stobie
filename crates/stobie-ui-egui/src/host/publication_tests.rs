#![cfg(test)]
//! What an agent reads when a pass cannot be published, and the pass a
//! completed invocation promises, on the browser-style dispatch route.

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, CapabilityDescription, CompletedObservation, InspectionOperation, InspectionResult,
    InspectionSelector, InvocationReceipt, PUBLICATION_FACT, PresentationScope, ReceiptState,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "feed")]
enum Act {
    #[action(label = "Refresh", description = "Refresh the feed")]
    Refresh,
}

#[derive(Default)]
struct Feed {
    /// How many rows the feed observes, each its own node.
    rows: usize,
    refreshed: usize,
    errors: Vec<String>,
}

impl Application for Feed {
    type Action = Act;
    type Intent = ();
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Feed")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, ()>) {
        let mut p = frame.presentation(ui, PresentationScope::new("feed"), SemanticUiId::root());
        let refresh = frame.action(ui, &mut p, Act::Refresh, ActionEmphasis::Normal);
        for row in 0..self.rows {
            crate::observe_node(
                ui,
                UiNode::container(
                    SemanticUiId::new(format!("row-{row}")),
                    Some(SemanticUiId::root()),
                    UiRole::Status,
                    refresh.rect.into(),
                ),
            );
        }
    }

    fn apply(&mut self, _: (), _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    /// Refreshing an up-to-date feed changes nothing.
    fn perform(&mut self, _: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        self.refreshed += 1;
        Ok(false)
    }

    fn report_error(&mut self, message: String) {
        self.errors.push(message);
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Feed>>,
}

fn harness(rows: usize) -> Harness<'static, Probe> {
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
    let mut host = AppHost::new(
        &context,
        Feed {
            rows,
            ..Feed::default()
        },
    );
    host.enable_inspection(Inspection::new("publication-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Feed> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Feed> {
    harness.state_mut().host.as_mut().unwrap()
}

fn request(host: &AppHost<Feed>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn observe(harness: &Harness<'_, Probe>) -> CompletedObservation {
    let host = host(harness);
    let reply =
        host.inspection()
            .unwrap()
            .handle(request(host, "observe", InspectionOperation::Observe));
    let Some(InspectionResult::Observe { observation }) = reply.result else {
        panic!("observe failed: {:?}", reply.error);
    };
    observation
}

fn refresh(harness: &Harness<'_, Probe>) -> Option<CapabilityDescription> {
    let host = host(harness);
    let reply = host.inspection().unwrap().handle(request(
        host,
        "discover",
        InspectionOperation::Discover {
            selector: InspectionSelector {
                capability: Some(Act::Refresh.stable_id().into()),
                ..Default::default()
            },
            limit: 8,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities.into_iter().next()
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

fn set_rows(harness: &mut Harness<'_, Probe>, rows: usize) {
    host_mut(harness).app_mut().rows = rows;
    harness.ctx.request_repaint();
    harness.run();
}

#[test]
fn a_pass_over_the_node_limit_publishes_why_instead_of_the_previous_pass() {
    let mut harness = harness(10);
    let before = observe(&harness);
    assert!(!before.facts.contains_key(PUBLICATION_FACT));
    assert!(refresh(&harness).is_some());

    set_rows(&mut harness, 5000);
    let failed = observe(&harness);
    assert!(
        failed.id > before.id,
        "the previous pass is not served again"
    );
    let ObservationValue::Text(reason) = &failed.facts[PUBLICATION_FACT] else {
        panic!("{:?}", failed.facts);
    };
    assert!(
        reason.starts_with("too_large: 50") && reason.ends_with(" nodes, over the limit of 4096"),
        "{reason}"
    );
    // Only the root and the framework's facts are presented, and nothing
    // the application offers can be invoked against it.
    assert_eq!(failed.snapshot.nodes.len(), 1);
    assert_eq!(failed.snapshot.nodes[0].id, SemanticUiId::root());
    assert_eq!(failed.facts.len(), 3, "{:?}", failed.facts);
    assert!(refresh(&harness).is_none());
    assert_eq!(
        host(&harness).app().errors,
        [format!("Automation could not publish this pass: {reason}")]
    );

    // A failure is reported once, however many passes it lasts.
    set_rows(&mut harness, 5001);
    assert_eq!(host(&harness).app().errors.len(), 1);
    assert!(observe(&harness).facts.contains_key(PUBLICATION_FACT));

    set_rows(&mut harness, 10);
    let recovered = observe(&harness);
    assert!(!recovered.facts.contains_key(PUBLICATION_FACT));
    assert_eq!(recovered.snapshot.nodes.len(), before.snapshot.nodes.len());
    assert!(refresh(&harness).is_some());
    assert_eq!(host(&harness).app().errors.len(), 1);
}

#[test]
fn a_dispatched_invocation_that_changes_nothing_still_publishes_its_observation() {
    let mut harness = harness(0);
    let expected = refresh(&harness).unwrap().target;
    assert!(!harness.ctx.has_requested_repaint());
    let host = host_mut(&mut harness);
    let invoke = request(
        host,
        "refresh",
        InspectionOperation::Invoke {
            selector: InspectionSelector {
                capability: Some(Act::Refresh.stable_id().into()),
                ..Default::default()
            },
            expected,
            arguments: serde_json::Value::Null,
        },
    );
    let completed = receipt(host.dispatch(invoke.clone()).unwrap());
    assert_eq!(completed.state, ReceiptState::Completed);
    assert_eq!(host.app().refreshed, 1);
    let after = completed.after_observation.expect("a promised observation");
    assert_eq!(
        host.repaint_reasons(),
        [HostRepaint::Application("invocation receipt")]
    );
    assert!(harness.ctx.has_requested_repaint());

    harness.run();
    assert!(observe(&harness).id >= after);

    // A replay reads the receipt again, whose observation is published, so
    // it does not repaint.
    let host = host_mut(&mut harness);
    assert_eq!(receipt(host.dispatch(invoke).unwrap()), completed);
    assert_eq!(host.app().refreshed, 1);
    assert!(!harness.ctx.has_requested_repaint());
}
