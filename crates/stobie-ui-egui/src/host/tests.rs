#![cfg(test)]

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;
use stobie_core::{PaneId, Workspace, WorkspaceChange};

use super::*;
use crate::{
    Action, CapabilityDescription, ContentTextSpec, DensityVariant, DesignTokens,
    InspectionErrorCode, InspectionOperation, InspectionResult, InspectionSelector,
    InvocationReceipt, PresentationScope, ReceiptState, ResolvedTarget, TextInteraction,
    TextOverflow, TextRole, ThemeVariant,
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
    #[action(label = "Recenter", description = "Recentre the view", shortcut = "R")]
    Recenter,
    #[action(
        label = "Confirm",
        description = "Confirm the edit",
        shortcut = "Enter"
    )]
    Confirm,
    #[action(
        label = "Level",
        description = "Choose a level",
        physical_only = "Chooses a value"
    )]
    Level,
}

enum Intent {
    Layout(WorkspaceChange),
    Theme(Box<ApplicationTheme>),
    /// Add to the count, as an event from outside a pass might.
    Add(u64),
    Fail(&'static str),
}

impl From<WorkspaceChange> for Intent {
    fn from(change: WorkspaceChange) -> Self {
        Self::Layout(change)
    }
}

#[derive(Default)]
struct View {
    passes: u64,
    /// Frame tokens resolved for the latest pass.
    tokens: Option<DesignTokens>,
    trigger_reset: bool,
    scroll_viewport: Option<Rect>,
    /// Text typed into the focus probe's text field.
    draft: String,
    focus_probe: Option<Rect>,
}

/// A focusable control presented to check which focus blocks plain shortcuts.
#[derive(Clone, Copy, Debug)]
enum FocusProbe {
    /// A clickable viewport that takes focus, as image viewports do.
    Viewport,
    /// An egui text edit observed as a text input.
    TextEdit,
    /// An egui text edit the application does not observe.
    UnobservedTextEdit,
    /// A custom widget the application observes as a focused text input.
    CustomTextInput,
}

#[derive(Default)]
struct Counter {
    count: u64,
    reset_enabled: bool,
    active_pane: Option<PaneId>,
    fitted: Vec<PaneId>,
    recentred: u64,
    confirmed: u64,
    focus_probe: Option<FocusProbe>,
    errors: Vec<String>,
    discard_first_pass: bool,
    rows: usize,
    /// Present the rows inside an `observe_visible_only` region.
    rows_visible_only: bool,
    duplicate: bool,
    non_finite: bool,
    /// Present a node beyond the bottom of the window.
    overflow: bool,
    /// Present a range recipe and observe its node again with a pane.
    refine_recipe: bool,
    /// Observe a different control under the recipe node's ID.
    clash_with_recipe: bool,
    animate: bool,
    /// Schedule a pass after an idle delay.
    idle_timeout: bool,
    workspace: Option<Workspace>,
    /// The count each completed pass presented.
    presented: Vec<u64>,
    /// A context that availability reads, as some applications do.
    context: Option<egui::Context>,
    /// UI passes counted by `UiState`, as `completed` saw them.
    presented_passes: u64,
    theme: Option<ApplicationTheme>,
    /// The sender the host gave `started`.
    sender: Option<IntentSender<Intent>>,
    /// Sent intents each completed pass presented.
    sent_presented: Vec<usize>,
    /// Schedule a pass from `prepare`.
    prepare_timeout: bool,
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
        if self.theme.is_none() {
            assert_eq!(frame.tokens(), tokens());
        }
        view.tokens = Some(frame.tokens());
        for action in [Act::Increment, Act::Reset, Act::Explode] {
            frame.action(ui, &mut p, action, ActionEmphasis::Normal);
        }
        let marker = ui.label(format!("Pass {pass}"));
        crate::observe_node(
            ui,
            node(format!("pass-{pass}"), UiRole::Status, marker.rect),
        );
        if let Some(probe) = self.focus_probe {
            let response = match probe {
                FocusProbe::Viewport | FocusProbe::CustomTextInput => {
                    let (_, response) =
                        ui.allocate_exact_size(egui::vec2(200.0, 60.0), egui::Sense::click());
                    if response.clicked() {
                        response.request_focus();
                    }
                    response
                }
                FocusProbe::TextEdit | FocusProbe::UnobservedTextEdit => {
                    ui.add(egui::TextEdit::singleline(&mut view.draft))
                }
            };
            view.focus_probe = Some(response.rect);
            let role = match probe {
                FocusProbe::Viewport => Some(UiRole::Viewport),
                FocusProbe::TextEdit | FocusProbe::CustomTextInput => Some(UiRole::TextInput),
                FocusProbe::UnobservedTextEdit => None,
            };
            if let Some(role) = role {
                let mut node = node("focus-probe", role, response.rect);
                node.focused = response.has_focus();
                crate::observe_node(ui, node);
            }
        }
        if self.non_finite {
            let rect = Rect::from_min_size(egui::pos2(f32::NAN, 0.0), egui::vec2(4.0, 4.0));
            crate::observe_node(ui, node("broken", UiRole::Status, rect));
        }
        if self.duplicate {
            crate::observe_node(ui, node("twice", UiRole::Status, marker.rect));
            crate::observe_node(ui, node("twice", UiRole::Status, marker.rect));
        }
        if self.refine_recipe {
            let mut level = 0.5;
            let mut node = crate::range_control(
                ui,
                SemanticUiId::new("level"),
                SemanticUiId::root(),
                "Level",
                &mut level,
                0.0..=1.0,
                Act::Level,
                &tokens(),
            )
            .node;
            node.pane = Some(PaneId(3));
            if self.clash_with_recipe {
                node.role = UiRole::Status;
            }
            crate::observe_node(ui, node);
        }
        if self.overflow {
            ui.add_space(2_000.0);
            let below = ui.label("Below the window");
            crate::observe_node(ui, node("below", UiRole::Status, below.rect));
        }
        if self.rows > 0 {
            let rows = |ui: &mut Ui| {
                egui::ScrollArea::vertical()
                    .max_height(60.0)
                    .show(ui, |ui| {
                        let mut p = frame.presentation(
                            ui,
                            PresentationScope::new("rows"),
                            SemanticUiId::root(),
                        );
                        for row in 0..self.rows {
                            let label = p.content(
                                ui,
                                ("row", row),
                                &format!("Row {row}"),
                                ContentTextSpec {
                                    role: TextRole::Body,
                                    overflow: TextOverflow::Ellipsis,
                                    max_lines: 1,
                                    interaction: TextInteraction::Selectable,
                                },
                            );
                            crate::observe_node(
                                ui,
                                node(format!("row-{row}"), UiRole::ResultRow, label.rect),
                            );
                        }
                    })
                    .inner_rect
            };
            view.scroll_viewport = Some(if self.rows_visible_only {
                crate::observe_visible_only(ui, rows)
            } else {
                rows(ui)
            });
        }
        if let Some(workspace) = &self.workspace {
            frame.dock(ui, workspace, &mut TestPanes);
        }
        if self.animate {
            frame.request_repaint("animation");
        }
        if self.idle_timeout {
            frame.request_repaint_after("idle gesture", std::time::Duration::from_millis(250));
        }
    }

    fn apply(&mut self, intent: Intent, _: &mut View) -> Result<bool, String> {
        match intent {
            Intent::Layout(change) => change.apply(self.workspace.as_mut().expect("dock")),
            Intent::Theme(theme) => {
                self.theme = Some(*theme);
                Ok(true)
            }
            Intent::Add(amount) => {
                self.count += amount;
                Ok(amount > 0)
            }
            Intent::Fail(message) => Err(message.into()),
        }
    }

    fn started(&mut self, intents: IntentSender<Intent>) {
        assert!(self.sender.is_none(), "the host starts an application once");
        self.sender = Some(intents);
    }

    fn prepare(&mut self, pass: &mut PassContext<'_>, _: &mut View) {
        if self.prepare_timeout {
            pass.request_repaint_after("timed work", std::time::Duration::from_millis(80));
        }
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut View) -> Result<bool, String> {
        match target.action {
            Act::Increment => self.count += 1,
            Act::Reset => self.count = 0,
            Act::Explode => return Err("Exploded".into()),
            Act::Fit => self.fitted.push(target.pane.expect("pane-scoped")),
            Act::Recenter => self.recentred += 1,
            Act::Confirm => self.confirmed += 1,
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

    fn completed(&mut self, pass: &CompletedPass<'_>, view: &View) {
        self.sent_presented.push(pass.sent_intents);
        self.presented.push(self.count);
        self.presented_passes = view.passes;
    }

    fn theme(&self) -> ApplicationTheme {
        self.theme
            .clone()
            .unwrap_or_else(ApplicationTheme::analytical)
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Counter>>,
    /// I-key presses left in the input after the host's pass.
    unconsumed: usize,
    /// The region the host was given in the latest pass.
    root_rect: Option<Rect>,
}

fn harness(counter: Counter) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(900.0, 700.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    probe.root_rect = Some(ui.max_rect());
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
fn every_host_opens_new_tabs_without_an_opener() {
    // `AppHost::new` installs the protection, so it holds for an application
    // that runs eframe's web runner itself as much as for `WebHost`.
    let context = egui::Context::default();
    assert!(!super::new_tab_links_installed(&context));
    let _host = AppHost::new(&context, Counter::default());
    assert!(super::new_tab_links_installed(&context));
}

#[test]
fn enabled_inspection_reports_the_renderer_the_host_started_on() {
    let mut harness = harness(Counter::default());
    let renderer = crate::RendererIdentity {
        backend: "webgl2".into(),
        adapter: "ANGLE".into(),
        selection: crate::RendererSelection::Forced,
        reason: Some("forced by ?renderer=webgl2".into()),
    };
    super::renderer::record_renderer(&harness.ctx, renderer.clone());
    with_inspection(&mut harness);
    let host = host(&harness);
    let reply =
        host.inspection()
            .unwrap()
            .handle(request(host, "hello", InspectionOperation::Hello));
    assert!(matches!(
        reply.result,
        Some(InspectionResult::Hello { renderer: Some(reported), .. }) if reported == renderer
    ));
}

#[test]
fn observations_replace_discarded_passes_and_clip_to_their_region() {
    let mut harness = harness(Counter {
        discard_first_pass: true,
        rows: 30,
        rows_visible_only: true,
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
fn observations_outside_a_visible_only_region_are_published_as_presented() {
    let harness = harness(Counter {
        rows: 30,
        ..Default::default()
    });
    let host = host(&harness);
    let snapshot = host.ui_snapshot();
    let viewport = host.ui_state.scroll_viewport.unwrap();
    let rows: Vec<_> = snapshot
        .nodes
        .iter()
        .filter(|node| node.id.0.starts_with("row-"))
        .collect();
    assert_eq!(rows.len(), 30, "every row is published");
    assert!(
        rows.iter()
            .any(|row| !viewport.contains_rect(Rect::from(row.rect))),
        "rows keep their presented bounds"
    );
    // Text is audited as presented, but a row's text wholly scrolled out of
    // view is not observed, so it raises no finding for its empty clip.
    let row_text: Vec<_> = snapshot
        .text
        .iter()
        .filter(|text| text.component_id.kind == crate::TextComponentKind::ContentLabel)
        .collect();
    let in_view = rows
        .iter()
        .filter(|row| viewport.intersects(Rect::from(row.rect)))
        .count();
    assert!(in_view > 0 && in_view < 30);
    assert_eq!(
        row_text.len(),
        in_view,
        "only rows in view observe their text"
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
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
fn an_application_observation_refines_a_recipe_node() {
    let refined = harness(Counter {
        refine_recipe: true,
        ..Default::default()
    });
    let snapshot = host(&refined).ui_snapshot();
    let levels: Vec<_> = snapshot
        .nodes
        .iter()
        .filter(|node| node.id.0 == "level")
        .collect();
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].pane, Some(PaneId(3)));
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );

    // A different control under the same ID is still a duplicate.
    let clashing = harness(Counter {
        refine_recipe: true,
        clash_with_recipe: true,
        ..Default::default()
    });
    assert!(host(&clashing).ui_snapshot().semantic_audit.contains(
        &SemanticAuditFinding::DuplicateId {
            id: SemanticUiId::new("level"),
        }
    ));
}

#[test]
fn the_root_keeps_the_window_bounds_when_content_overflows() {
    let harness = harness(Counter {
        overflow: true,
        ..Default::default()
    });
    let snapshot = host(&harness).ui_snapshot();
    let window = harness.state().root_rect.unwrap();
    assert_eq!(Rect::from(snapshot.nodes[0].rect), window);
    // The overflowing node is published as presented and audited.
    let below = snapshot
        .nodes
        .iter()
        .find(|node| node.id.0 == "below")
        .expect("overflowing node is published");
    assert!(Rect::from(below.rect).min.y > window.max.y);
    assert!(
        snapshot.semantic_audit.iter().any(
            |finding| matches!(finding, SemanticAuditFinding::OutsideRoot { id } if id.0 == "below")
        ),
        "{:?}",
        snapshot.semantic_audit
    );
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

fn click(harness: &mut Harness<'_, Probe>, position: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(position));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();
}

/// Press R as a platform does: a key event followed by its text.
fn type_r(harness: &mut Harness<'_, Probe>) {
    harness.key_press(Key::R);
    harness.event(egui::Event::Text("r".into()));
    harness.run();
}

fn focus_probe(harness: &mut Harness<'_, Probe>) {
    let rect = host(harness)
        .ui_state
        .focus_probe
        .expect("probe is presented");
    click(harness, rect.center());
    assert!(harness.ctx.egui_wants_keyboard_input());
}

#[test]
fn plain_shortcuts_run_while_a_viewport_or_button_has_focus() {
    let mut harness = harness(Counter {
        focus_probe: Some(FocusProbe::Viewport),
        ..Default::default()
    });
    focus_probe(&mut harness);
    let probe = host(&harness)
        .ui_snapshot()
        .node(&SemanticUiId::new("focus-probe"));
    assert!(probe.expect("probe is observed").focused);
    type_r(&mut harness);
    assert_eq!(host(&harness).app().recentred, 1);

    harness.get_by_label("Increment").click();
    harness.run();
    assert_eq!(host(&harness).app().count, 1);
    assert!(harness.ctx.egui_wants_keyboard_input());
    type_r(&mut harness);
    assert_eq!(host(&harness).app().recentred, 2);
}

#[test]
fn enter_reaches_a_focused_viewport_but_activates_a_focused_button() {
    let mut harness = harness(Counter {
        focus_probe: Some(FocusProbe::Viewport),
        ..Default::default()
    });
    focus_probe(&mut harness);
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(host(&harness).app().confirmed, 1);

    // A focused button activates with Enter, so the plain shortcut yields.
    harness.get_by_label("Increment").click();
    harness.run();
    assert_eq!(host(&harness).app().count, 1);
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(host(&harness).app().count, 2);
    assert_eq!(host(&harness).app().confirmed, 1);
    // Other plain keys still reach the shortcut.
    type_r(&mut harness);
    assert_eq!(host(&harness).app().recentred, 1);
}

#[test]
fn text_entry_keeps_plain_keys_but_not_command_chords() {
    for probe in [
        FocusProbe::TextEdit,
        FocusProbe::UnobservedTextEdit,
        FocusProbe::CustomTextInput,
    ] {
        let mut harness = harness(Counter {
            focus_probe: Some(probe),
            ..Default::default()
        });
        focus_probe(&mut harness);
        type_r(&mut harness);
        let host = host(&harness);
        assert_eq!(host.app().recentred, 0, "{probe:?}");
        let typed = match probe {
            FocusProbe::CustomTextInput => "",
            _ => "r",
        };
        assert_eq!(host.ui_state.draft, typed, "{probe:?}");
        harness.key_press_modifiers(Modifiers::COMMAND, Key::I);
        harness.run();
        assert_eq!(
            harness.state().host.as_ref().unwrap().app().count,
            1,
            "{probe:?}"
        );
    }
}

/// The browser ARIA layer activates a control with an AccessKit click and
/// moves focus with an AccessKit focus request, as native assistive technology
/// does: an enabled action runs once through the click route, while a disabled
/// one neither runs nor takes focus.
#[test]
fn accesskit_click_and_focus_take_the_physical_route() {
    let mut harness = harness(Counter::default());
    let focused = |harness: &Harness<'_, Probe>, action| {
        host(harness)
            .ui_snapshot()
            .by_action(action)
            .next()
            .is_some_and(|node| node.focused)
    };
    harness.get_by_label("Increment").click_accesskit();
    harness.run();
    assert_eq!(host(&harness).app().count, 1);

    harness.get_by_label("Explode").focus();
    harness.run();
    assert!(focused(&harness, Act::Explode), "focus reaches the control");
    assert!(
        host(&harness).app().errors.is_empty(),
        "focus does not activate"
    );

    harness.get_by_label("Reset").click_accesskit();
    harness.get_by_label("Reset").focus();
    harness.run();
    assert!(
        host(&harness).app().errors.is_empty(),
        "a disabled control does nothing"
    );
    assert_eq!(host(&harness).app().count, 1);
    assert!(!focused(&harness, Act::Reset));
    assert!(focused(&harness, Act::Explode));
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
fn submitted_intents_and_application_themes_reach_the_pass() {
    let mut harness = harness(Counter::default());
    let geometry = crate::ApplicationGeometry {
        application_bar_height: 40.0,
        control_height: 36.0,
        control_radius: 6.0,
    };
    let theme = ApplicationTheme::analytical()
        .with_geometry(geometry)
        .unwrap();
    // An intent from outside a pass, as a page hook sends, applies and repaints.
    assert!(host_mut(&mut harness).submit(Intent::Theme(Box::new(theme))));
    assert_eq!(
        host(&harness).repaint_reasons(),
        [HostRepaint::StateChanged]
    );
    harness.step();
    assert!(
        host(&harness)
            .repaint_reasons()
            .contains(&HostRepaint::Appearance)
    );
    harness.run();
    let tokens = host(&harness).ui_state().tokens.unwrap();
    assert_eq!(tokens.geometry.control_height.0, 36.0);
    // `completed` sees the UI state the pass presented.
    let host = host(&harness);
    assert_eq!(host.app().presented_passes, host.ui_state().passes);
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

    // A scheduled reason asks for a later pass, not an immediate one.
    let app = host_mut(&mut harness).app_mut();
    app.animate = false;
    app.idle_timeout = true;
    harness.step();
    let reasons = host(&harness).repaint_reasons();
    assert_eq!(
        reasons,
        [HostRepaint::Scheduled {
            reason: "idle gesture",
            delay: std::time::Duration::from_millis(250),
        }]
    );
    assert!(!reasons[0].is_immediate());
}

#[test]
fn the_dock_observes_panes_and_emits_layout_intents() {
    let mut harness = harness(Counter {
        workspace: Some(crate::test_actions::analytical_workspace()),
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
    // Dock nodes name their pane or dock node as a domain reference.
    for node in snapshot
        .by_role(UiRole::Pane)
        .chain(snapshot.by_role(UiRole::Tab))
    {
        assert_eq!(
            node.domain_reference,
            Some(crate::DomainReference::Pane(node.pane.unwrap()))
        );
    }
    for splitter in snapshot.by_role(UiRole::Splitter) {
        assert!(matches!(
            splitter.domain_reference,
            Some(crate::DomainReference::DockNode(_))
        ));
    }
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

fn sender(harness: &Harness<'_, Probe>) -> IntentSender<Intent> {
    host(harness).app().sender.clone().expect("started")
}

/// Run the passes egui asks for and return how many ran. egui follows each
/// immediate request with one settling pass of its own.
fn settle(harness: &mut Harness<'_, Probe>) -> u64 {
    let before = host(harness).ui_state().passes;
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());
    host(harness).ui_state().passes - before
}

#[test]
fn sent_intents_wake_an_idle_host_and_present_in_the_pass_that_applies_them() {
    let mut harness = harness(Counter::default());
    harness.run();
    assert!(
        !harness.ctx.has_requested_repaint(),
        "an idle application asks for no pass"
    );

    // An event arrives on another thread, as a native socket's would.
    let events = sender(&harness);
    std::thread::spawn(move || events.send(Intent::Add(2)).unwrap())
        .join()
        .unwrap();
    assert!(
        harness.ctx.has_requested_repaint(),
        "the event wakes the host"
    );
    assert_eq!(
        host(&harness).app().count,
        0,
        "nothing applies outside a pass"
    );

    harness.step();
    let host_ref = host(&harness);
    // The pass that applied the event presented it, so it asks for no other.
    assert_eq!(host_ref.app().presented.last(), Some(&2));
    assert_eq!(host_ref.app().sent_presented.last(), Some(&1));
    assert!(host_ref.repaint_reasons().is_empty());
    assert_eq!(settle(&mut harness), 1, "only egui's settling pass follows");

    // Intents sent before the pass runs join it, in the order sent.
    let events = sender(&harness);
    events.send(Intent::Add(3)).unwrap();
    events.send(Intent::Fail("Feed closed")).unwrap();
    events.send(Intent::Add(5)).unwrap();
    harness.step();
    let app = host(&harness).app();
    assert_eq!(app.presented.last(), Some(&10));
    assert_eq!(app.sent_presented.last(), Some(&3));
    // The error is shown in the same pass, so it asks for no other either.
    assert_eq!(app.errors, ["Feed closed"]);
    assert!(host(&harness).repaint_reasons().is_empty());
    assert_eq!(settle(&mut harness), 1);

    // An event that changes nothing costs the same passes and no more.
    events.send(Intent::Add(0)).unwrap();
    assert!(harness.ctx.has_requested_repaint());
    assert_eq!(settle(&mut harness), 2);
    assert_eq!(host(&harness).app().count, 10);
}

#[test]
fn intent_senders_fail_once_the_host_stops() {
    let harness = harness(Counter::default());
    let events = host(&harness).intent_sender();
    assert!(!events.is_stopped());
    drop(harness);
    assert!(events.is_stopped());
    assert!(matches!(
        events.send(Intent::Add(1)),
        Err(HostStopped(Intent::Add(1)))
    ));
}

#[test]
fn prepare_can_schedule_a_later_pass() {
    let mut harness = harness(Counter::default());
    host_mut(&mut harness).app_mut().prepare_timeout = true;
    harness.step();
    assert_eq!(
        host(&harness).repaint_reasons(),
        [HostRepaint::Scheduled {
            reason: "timed work",
            delay: std::time::Duration::from_millis(80),
        }]
    );
}

#[test]
fn intents_applied_while_hidden_are_presented_once_visible() {
    let mut harness = harness(Counter::default());
    harness.run();
    let events = sender(&harness);
    events.send(Intent::Add(4)).unwrap();
    // eframe's `logic` while the viewport is hidden: the intent applies, but
    // no pass presents it, so the host checks again later.
    host_mut(&mut harness).apply_sent_intents_before_pass(true);
    let host_ref = host(&harness);
    assert_eq!(host_ref.app().count, 4);
    assert_eq!(host_ref.app().presented.last(), Some(&0));
    let recheck = HostRepaint::Scheduled {
        reason: "sent intents await a visible pass",
        delay: HIDDEN_RECHECK,
    };
    assert_eq!(host_ref.repaint_reasons(), [recheck]);
    assert!(harness.ctx.has_requested_repaint());
    // Still hidden and nothing new: it keeps checking.
    host_mut(&mut harness).apply_sent_intents_before_pass(true);
    assert_eq!(host(&harness).repaint_reasons(), [recheck]);
    assert!(harness.ctx.has_requested_repaint());
    // A visible pass presents it and stops the checks.
    harness.step();
    assert_eq!(host(&harness).app().presented.last(), Some(&4));
    host_mut(&mut harness).apply_sent_intents_before_pass(true);
    assert!(host(&harness).repaint_reasons().is_empty());
    harness.run();
    // While hidden with nothing applied, the host asks for nothing.
    host_mut(&mut harness).apply_sent_intents_before_pass(true);
    assert!(!harness.ctx.has_requested_repaint());
    // A visible viewport's pass follows `logic`, so it needs no check.
    events.send(Intent::Add(1)).unwrap();
    host_mut(&mut harness).apply_sent_intents_before_pass(false);
    assert!(host(&harness).repaint_reasons().is_empty());
}

#[test]
fn web_push_reports_unsupported_natively_through_the_sender() {
    use crate::web_push::{PushEvent, PushOperation, PushSupport, WebPush};
    let mut harness = harness(Counter::default());
    harness.run();
    let heard = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let record = heard.clone();
    let push = WebPush::start(sender(&harness), move |event| {
        record.borrow_mut().push(event);
        Intent::Add(1)
    });
    assert!(!push.support().is_supported());
    assert_eq!(push.permission(), None);
    assert!(!push.set_app_badge(3));
    push.subscribe(
        "BEl62iUYgUivxIkv69yViEuiBIa-Ib9-SkvMeAtA3LFgDzkrxZJjSgSnfckjBJuBkr3qBUYIHBQFLXYp5Nksh8U",
    );
    push.subscribe("not a key");
    push.unsubscribe();
    let heard = heard.borrow();
    let PushEvent::State(state) = &heard[0] else {
        panic!("{heard:?}")
    };
    assert!(
        matches!(&state.support, PushSupport::Unsupported(reason) if reason.contains("browser"))
    );
    assert_eq!(
        (state.permission, state.subscription.as_ref()),
        (None, None)
    );
    let failed: Vec<_> = heard[1..]
        .iter()
        .map(|event| match event {
            PushEvent::Failed { operation, message } => (*operation, message.as_str()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(failed.len(), 3);
    assert_eq!(failed[0].0, PushOperation::Subscribe);
    assert!(failed[0].1.contains("only in the browser"));
    assert!(
        failed[1].1.contains("base64url"),
        "the key is checked first"
    );
    assert_eq!(failed[2].0, PushOperation::Unsubscribe);
    // Each event reached the host as the application's own intent.
    harness.step();
    assert_eq!(host(&harness).app().count, 4);
}
