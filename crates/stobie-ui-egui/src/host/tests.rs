#![cfg(test)]

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;
use stobie_core::{PaneId, Workspace, WorkspaceChange};

use super::*;
use crate::{
    Action, CapabilityDescription, DensityVariant, DesignTokens, InspectionErrorCode,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    PresentationScope, ReceiptState, ResolvedTarget, ThemeVariant,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "host-test")]
enum Act {
    #[action(
        label = "Increment",
        description = "Add one to the count",
        shortcut = "Cmd+I"
    )]
    Increment,
    #[action(
        label = "Reset",
        description = "Set the count to zero",
        shortcut = "Cmd+Shift+I"
    )]
    Reset,
    #[action(label = "Explode", description = "Fail visibly", shortcut = "Cmd+E")]
    Explode,
    #[action(
        label = "Fit",
        description = "Fit the active pane",
        scope = "active_pane",
        shortcut = "F"
    )]
    Fit,
    #[action(
        label = "Level",
        description = "Choose a level",
        physical_only = "Chooses a value"
    )]
    Level,
}

enum Intent {
    Layout(WorkspaceChange),
}

impl From<WorkspaceChange> for Intent {
    fn from(change: WorkspaceChange) -> Self {
        Self::Layout(change)
    }
}

#[derive(Default)]
struct View {
    passes: u64,
    trigger_reset: bool,
    scroll_viewport: Option<Rect>,
}

#[derive(Default)]
struct Counter {
    count: u64,
    reset_enabled: bool,
    active_pane: Option<PaneId>,
    fitted: Vec<PaneId>,
    errors: Vec<String>,
    discard_first_pass: bool,
    rows: usize,
    duplicate: bool,
    non_finite: bool,
    animate: bool,
    workspace: Option<Workspace>,
    /// The count each completed pass presented.
    presented: Vec<u64>,
    /// A context that availability reads, as some applications do.
    context: Option<egui::Context>,
}

fn tokens() -> DesignTokens {
    DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
}

fn node(id: impl Into<String>, role: UiRole, rect: Rect) -> UiNode {
    UiNode::container(
        SemanticUiId::new(id),
        Some(SemanticUiId::root()),
        role,
        rect.into(),
    )
}

struct TestPanes;

impl Panes<Act, Intent> for TestPanes {
    fn title(&self, pane: PaneId) -> &'static str {
        [
            "Zero", "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine",
        ]
        .get(pane.0 as usize)
        .copied()
        .unwrap_or("Other")
    }

    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, frame: &mut Frame<'_, Act, Intent>) {
        let mut p = frame.pane_presentation(ui, PresentationScope::new(("pane", pane)), pane);
        p.heading(ui, "title", self.title(pane));
        if pane == PaneId(1) {
            p.status(ui, "status", "Pane ready", false);
            // A pane-scoped action takes its pane from the pane presentation.
            frame.action(ui, &mut p, Act::Fit, ActionEmphasis::Normal);
        }
    }
}

impl Application for Counter {
    type Action = Act;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Counter")
    }

    fn availability(&self, target: ActionTarget<Act>) -> Availability {
        if let Some(context) = &self.context {
            // Must not deadlock while shortcuts are being dispatched.
            let _ = context.input(|input| input.modifiers);
        }
        match target.action {
            Act::Reset if !self.reset_enabled => Availability::Disabled {
                reason: "Nothing to reset".into(),
            },
            _ => Availability::Enabled,
        }
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, Act, Intent>) {
        view.passes += 1;
        let pass = ui.ctx().current_pass_index();
        if self.discard_first_pass && pass == 0 {
            ui.ctx().request_discard("test layout");
        }
        if std::mem::take(&mut view.trigger_reset) {
            frame.trigger(Act::Reset);
        }
        let mut p = frame.presentation(ui, PresentationScope::new("counter"), SemanticUiId::root());
        assert_eq!(frame.tokens(), tokens());
        for action in [Act::Increment, Act::Reset, Act::Explode] {
            frame.action(ui, &mut p, action, ActionEmphasis::Normal);
        }
        let marker = ui.label(format!("Pass {pass}"));
        crate::observe_node(
            ui,
            node(format!("pass-{pass}"), UiRole::Status, marker.rect),
        );
        if self.non_finite {
            let rect = Rect::from_min_size(egui::pos2(f32::NAN, 0.0), egui::vec2(4.0, 4.0));
            crate::observe_node(ui, node("broken", UiRole::Status, rect));
        }
        if self.duplicate {
            crate::observe_node(ui, node("twice", UiRole::Status, marker.rect));
            crate::observe_node(ui, node("twice", UiRole::Status, marker.rect));
        }
        if self.rows > 0 {
            let scroll = egui::ScrollArea::vertical()
                .max_height(60.0)
                .show(ui, |ui| {
                    for row in 0..self.rows {
                        let label = ui.label(format!("Row {row}"));
                        crate::observe_node(
                            ui,
                            node(format!("row-{row}"), UiRole::ResultRow, label.rect),
                        );
                    }
                });
            view.scroll_viewport = Some(scroll.inner_rect);
        }
        if let Some(workspace) = &self.workspace {
            frame.dock(ui, workspace, &mut TestPanes);
        }
        if self.animate {
            frame.request_repaint("animation");
        }
    }

    fn apply(&mut self, intent: Intent, _: &mut View) -> Result<bool, String> {
        match intent {
            Intent::Layout(change) => change.apply(self.workspace.as_mut().expect("dock")),
        }
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut View) -> Result<bool, String> {
        match target.action {
            Act::Increment => self.count += 1,
            Act::Reset => self.count = 0,
            Act::Explode => return Err("Exploded".into()),
            Act::Fit => self.fitted.push(target.pane.expect("pane-scoped")),
            Act::Level => return Ok(false),
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.errors.push(message);
    }

    fn meaning(&self, target: ActionTarget<Act>) -> String {
        match target.action {
            Act::Increment => format!("count:{}", self.count),
            _ => "stable".into(),
        }
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("count", self.count);
        observation.counted_collection("rows", UiRole::ResultRow, self.rows as u64);
    }

    fn active_pane(&self) -> Option<PaneId> {
        self.active_pane
    }

    fn completed(&mut self, _pass: &CompletedPass<'_>) {
        self.presented.push(self.count);
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Counter>>,
    /// I-key presses left in the input after the host's pass.
    unconsumed: usize,
}

fn harness(counter: Counter) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(900.0, 700.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
                probe.unconsumed += ui.input(|input| {
                    input
                        .events
                        .iter()
                        .filter(|event| {
                            matches!(
                                event,
                                egui::Event::Key {
                                    key: Key::I,
                                    pressed: true,
                                    ..
                                }
                            )
                        })
                        .count()
                });
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    harness.state_mut().host = Some(AppHost::new(&context, counter));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Counter> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Counter> {
    harness.state_mut().host.as_mut().unwrap()
}

fn ids(snapshot: &UiSnapshot) -> Vec<&str> {
    snapshot
        .nodes
        .iter()
        .map(|node| node.id.0.as_str())
        .collect()
}

fn request(host: &AppHost<Counter>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn selector(action: Act) -> InspectionSelector {
    InspectionSelector {
        capability: Some(action.stable_id().into()),
        ..Default::default()
    }
}

fn discover(host: &AppHost<Counter>, action: Act) -> CapabilityDescription {
    let reply = host.inspection().unwrap().handle(request(
        host,
        "discover",
        InspectionOperation::Discover {
            selector: selector(action),
            limit: 8,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    capabilities.into_iter().next().unwrap()
}

fn invoke(action: Act, expected: ResolvedTarget) -> InspectionOperation {
    InspectionOperation::Invoke {
        selector: selector(action),
        expected,
        arguments: serde_json::Value::Null,
    }
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

/// Admit an invocation to the native queue; the next pass drains it.
fn queue(harness: &Harness<'_, Probe>, id: &str, operation: InspectionOperation) {
    let host = host(harness);
    let queued = receipt(
        host.inspection()
            .unwrap()
            .handle(request(host, id, operation)),
    );
    assert_eq!(queued.state, ReceiptState::Queued, "{queued:?}");
}

fn lookup(harness: &Harness<'_, Probe>, id: &str) -> InvocationReceipt {
    let host = host(harness);
    receipt(host.inspection().unwrap().handle(request(
        host,
        &format!("lookup-{id}"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    )))
}

fn with_inspection(harness: &mut Harness<'_, Probe>) {
    host_mut(harness).enable_inspection(Inspection::new("host-test"));
    harness.run();
}

#[test]
fn observations_replace_discarded_passes_and_clip_to_their_region() {
    let mut harness = harness(Counter {
        discard_first_pass: true,
        rows: 30,
        ..Default::default()
    });
    with_inspection(&mut harness);
    let passes_before = host(&harness).ui_state.passes;
    harness.key_press(Key::Tab);
    harness.run();
    let host = host(&harness);
    let snapshot = host.ui_snapshot();
    // Every frame ran a discarded pass and then a completed one.
    assert_eq!((host.ui_state.passes - passes_before) % 2, 0);
    assert!(ids(snapshot).contains(&"pass-1"));
    assert!(!ids(snapshot).contains(&"pass-0"), "{:?}", ids(snapshot));
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty());
    assert_eq!(host.tab_presses(), 1, "one press across discarded passes");
    // Rows below the short scroll viewport are omitted; the boundary row keeps
    // only its visible bounds.
    let rows: Vec<_> = snapshot
        .nodes
        .iter()
        .filter(|node| node.id.0.starts_with("row-"))
        .collect();
    assert!(!rows.is_empty() && rows.len() < 30, "{} rows", rows.len());
    // egui's scroll clip extends a few points beyond the viewport.
    let viewport = host.ui_state.scroll_viewport.unwrap().expand(8.0);
    for row in &rows {
        let rect = Rect::from(row.rect);
        assert!(
            viewport.contains_rect(rect),
            "{rect:?} outside {viewport:?}"
        );
    }
    // The published observation is the completed pass, never a discarded one.
    let reply =
        host.inspection()
            .unwrap()
            .handle(request(host, "observe", InspectionOperation::Observe));
    let Some(InspectionResult::Observe { observation }) = reply.result else {
        panic!("observe failed");
    };
    assert_eq!(observation.snapshot.nodes, snapshot.nodes);
    assert_eq!(observation.facts["count"], 0_u64.into());
    let collection = &observation.collection.collections[0];
    assert_eq!(
        (collection.observed, collection.complete),
        (rows.len(), false)
    );
}

#[test]
fn invalid_observations_are_audited_without_blocking_publication() {
    let mut harness = harness(Counter {
        duplicate: true,
        non_finite: true,
        ..Default::default()
    });
    with_inspection(&mut harness);
    let host = host(&harness);
    let snapshot = host.ui_snapshot();
    assert_eq!(ids(snapshot).iter().filter(|id| **id == "twice").count(), 1);
    assert!(!ids(snapshot).contains(&"broken"));
    for finding in [
        SemanticAuditFinding::DuplicateId {
            id: SemanticUiId::new("twice"),
        },
        SemanticAuditFinding::InvalidRect {
            id: SemanticUiId::new("broken"),
        },
    ] {
        assert!(snapshot.semantic_audit.contains(&finding), "{finding:?}");
    }
    // The findings reach inspection clients instead of blocking publication.
    let reply =
        host.inspection()
            .unwrap()
            .handle(request(host, "observe", InspectionOperation::Observe));
    let Some(InspectionResult::Observe { observation }) = reply.result else {
        panic!("observe failed: {:?}", reply.error);
    };
    assert_eq!(observation.snapshot.semantic_audit, snapshot.semantic_audit);
}

#[test]
fn shortcuts_dispatch_only_exact_available_chords() {
    let mut harness = harness(Counter::default());
    let context = harness.ctx.clone();
    host_mut(&mut harness).app_mut().context = Some(context);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::I);
    harness.run();
    assert_eq!(host(&harness).app().count, 1);
    assert_eq!(harness.state().unconsumed, 0);

    // Unavailable Reset must not fall through to the plain Increment chord,
    // and its press stays in the input for other consumers.
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::I);
    harness.run();
    assert_eq!(host(&harness).app().count, 1);
    assert_eq!(harness.state().unconsumed, 1);
    assert!(host(&harness).app().errors.is_empty());

    host_mut(&mut harness).app_mut().reset_enabled = true;
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::I);
    harness.run();
    assert_eq!(host(&harness).app().count, 0);

    // An extra modifier is a different chord.
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::I);
    harness.run();
    assert_eq!(host(&harness).app().count, 0);
    assert_eq!(harness.state().unconsumed, 2);

    // Pane-scoped shortcuts need an active pane.
    harness.key_press(Key::F);
    harness.run();
    assert!(host(&harness).app().fitted.is_empty());
    host_mut(&mut harness).app_mut().active_pane = Some(PaneId(3));
    harness.key_press(Key::F);
    harness.run();
    assert_eq!(host(&harness).app().fitted, [PaneId(3)]);
}

#[test]
fn availability_gates_clicks_triggers_and_semantic_invocation() {
    let mut harness = harness(Counter::default());
    with_inspection(&mut harness);

    harness.get_by_label("Reset").click();
    harness.run();
    assert!(
        host(&harness).app().errors.is_empty(),
        "a disabled button does not click"
    );

    // A trigger is rechecked when it runs, and its reason is shown.
    host_mut(&mut harness).ui_state.trigger_reset = true;
    harness.step();
    assert_eq!(host(&harness).app().errors, ["Nothing to reset"]);
    assert_eq!(host(&harness).repaint_reasons(), [HostRepaint::Rejected]);
    harness.run();

    let reset = discover(host(&harness), Act::Reset);
    assert_eq!(
        reset.availability.disabled_reason(),
        Some("Nothing to reset")
    );
    let host = host_mut(&mut harness);
    let rejected = receipt(
        host.dispatch(request(host, "reset", invoke(Act::Reset, reset.target)))
            .unwrap(),
    );
    assert_eq!(rejected.state, ReceiptState::Rejected);
    assert_eq!(
        rejected.error.unwrap().code,
        InspectionErrorCode::Unavailable
    );

    // A failing semantic action is reported in its receipt, not to the user.
    let explode = discover(host, Act::Explode);
    let failed = receipt(
        host.dispatch(request(
            host,
            "explode",
            invoke(Act::Explode, explode.target),
        ))
        .unwrap(),
    );
    assert_eq!(
        failed.error.unwrap().code,
        InspectionErrorCode::ValidationFailed
    );
    assert_eq!(host.app().errors.len(), 1);

    let level = discover(host, Act::Level);
    assert!(!level.semantic_invocable);
    let physical = receipt(
        host.dispatch(request(host, "level", invoke(Act::Level, level.target)))
            .unwrap(),
    );
    assert_eq!(
        physical.error.unwrap().code,
        InspectionErrorCode::NotInvocable
    );

    harness.get_by_label("Explode").click();
    harness.run();
    let host = harness.state().host.as_ref().unwrap();
    assert_eq!(host.app().errors, ["Nothing to reset", "Exploded"]);
    assert_eq!(host.app().count, 0);
}

#[test]
fn queued_invocations_recheck_meaning_and_availability_when_they_run() {
    let mut harness = harness(Counter {
        reset_enabled: true,
        ..Default::default()
    });
    with_inspection(&mut harness);

    let increment = discover(host(&harness), Act::Increment);
    queue(&harness, "stale", invoke(Act::Increment, increment.target));
    // The count changes before the queue drains, so the target is stale.
    host_mut(&mut harness).app_mut().count = 5;
    harness.run();
    let stale = lookup(&harness, "stale");
    assert_eq!(stale.state, ReceiptState::Rejected);
    assert_eq!(stale.error.unwrap().code, InspectionErrorCode::StaleTarget);
    assert_eq!(host(&harness).app().count, 5);

    let reset = discover(host(&harness), Act::Reset);
    queue(&harness, "unavailable", invoke(Act::Reset, reset.target));
    host_mut(&mut harness).app_mut().reset_enabled = false;
    harness.run();
    assert_eq!(
        lookup(&harness, "unavailable").error.unwrap().code,
        InspectionErrorCode::Unavailable
    );
    assert_eq!(host(&harness).app().count, 5);

    let increment = discover(host(&harness), Act::Increment);
    queue(
        &harness,
        "current",
        invoke(Act::Increment, increment.target),
    );
    harness.step();
    assert_eq!(lookup(&harness, "current").state, ReceiptState::Completed);
    assert_eq!(host(&harness).app().count, 6);
    assert!(
        host(&harness)
            .repaint_reasons()
            .contains(&HostRepaint::StateChanged)
    );
    harness.run();
    assert!(host(&harness).app().errors.is_empty());
}

#[test]
fn completed_sees_the_presented_state_before_intents_apply() {
    let mut harness = harness(Counter::default());
    harness.get_by_label("Increment").click();
    harness.step();
    let counter = host(&harness).app();
    assert_eq!((counter.count, counter.presented.last()), (1, Some(&0)));
    harness.step();
    assert_eq!(host(&harness).app().presented.last(), Some(&1));
}

#[test]
fn repaints_only_for_recorded_reasons() {
    let mut harness = harness(Counter::default());
    harness.step();
    assert!(
        host(&harness).repaint_reasons().is_empty(),
        "idle passes do not repaint"
    );

    harness.get_by_label("Increment").click();
    harness.step();
    assert_eq!(
        host(&harness).repaint_reasons(),
        [HostRepaint::StateChanged]
    );
    harness.step();
    assert!(host(&harness).repaint_reasons().is_empty());

    harness.get_by_label("Explode").click();
    harness.step();
    assert_eq!(host(&harness).repaint_reasons(), [HostRepaint::Rejected]);
    harness.run();

    host_mut(&mut harness).enable_inspection(Inspection::new("repaint-test"));
    assert_eq!(
        host(&harness).repaint_reasons(),
        [HostRepaint::AutomationEnabled]
    );
    harness.run();

    host_mut(&mut harness).app_mut().animate = true;
    harness.step();
    assert_eq!(
        host(&harness).repaint_reasons(),
        [HostRepaint::Application("animation")]
    );
}

#[test]
fn the_dock_observes_panes_and_emits_layout_intents() {
    let mut harness = harness(Counter {
        workspace: Some(Workspace::analytical_default()),
        ..Default::default()
    });
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.by_role(UiRole::Pane).count() >= 2);
    assert!(snapshot.by_role(UiRole::Tab).count() >= 2);
    assert!(snapshot.by_role(UiRole::Splitter).count() >= 1);
    let fit = snapshot
        .by_action(Act::Fit)
        .next()
        .expect("pane 1 presents Fit");
    assert_eq!(fit.parent, Some(SemanticUiId::pane(PaneId(1))));
    // Pane content is parented to and associated with its pane, and a status
    // line publishes a waitable status node named by its text.
    let status = snapshot
        .by_role(UiRole::Status)
        .find(|n| n.name == "Pane ready")
        .unwrap();
    assert_eq!(status.parent, Some(SemanticUiId::pane(PaneId(1))));
    assert_eq!(status.pane, Some(PaneId(1)));
    let inactive = snapshot
        .by_role(UiRole::Tab)
        .find(|tab| !tab.selected)
        .expect("an inactive tab")
        .pane
        .unwrap();
    harness.get_by_label(TestPanes.title(inactive)).click();
    harness.step();
    assert_eq!(
        host(&harness).repaint_reasons(),
        [HostRepaint::StateChanged]
    );
    harness.run();
    let workspace = host(&harness).app().workspace.as_ref().unwrap();
    assert_eq!(workspace.active_pane, inactive);
}
