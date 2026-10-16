//! A framework-owned application lifecycle.
//!
//! An application implements [`Application`]: its identity, its derived action
//! set and their availability, a UI function that reads state and emits intents
//! through a [`Frame`], and the handlers that apply intents and perform actions.
//! [`AppHost`] owns everything else in each UI pass: the root semantic node,
//! observation collection and clipping, text and semantic audits, Tab counting,
//! exact-modifier shortcuts, availability gating, inspection startup, publishing
//! and draining, and repaint decisions.
//!
//! One pass runs in this order:
//!
//! 1. [`Application::prepare`] adapts state to the viewport and polls workers.
//! 2. Queued semantic invocations run, each rechecked against current state.
//! 3. Available shortcuts are consumed with exact modifiers.
//! 4. [`Application::ui`] presents state and emits intents and action triggers.
//! 5. The completed observation is published, and [`Application::completed`]
//!    sees it together with the state it presented.
//! 6. Data intents apply in emission order (or as [`Application::order_intents`]
//!    arranges them), then actions in trigger order, each action rechecking its
//!    availability immediately before it runs.
//! 7. A repaint is requested only for a recorded [`HostRepaint`]. Any state
//!    change requests one, so the next pass presents and observes it. A
//!    scheduled reason requests a pass after its delay instead.

use std::collections::{BTreeMap, VecDeque};
use std::hash::Hash;

use egui::{Rect, Response, Ui};
use stobie_core::{PaneId, Workspace, WorkspaceChange};

use crate::observation::{Origin, Visibility};
use crate::{
    ActionArgument, ActionButtonContent, ActionButtonSpec, ActionButtonState, ActionEmphasis,
    ActionKey, ActionScope, ActionSet, ActionTarget, ApplicationIdentity, ApplicationTheme,
    Availability, CollectionMetadata, CollectionObservation, DesignTokens, DockBehaviour,
    DockTextContext, DomainReference, Execution, IconId, Inspection, InspectionBinding,
    InspectionError, InspectionReply, InspectionRequest, IntoActionTarget, Invocation,
    ObservationValue, PanePresenter, PresentationContext, PresentationScope, SemanticAuditFinding,
    SemanticUiId, ShortcutKey, TextFieldBinding, TextLayoutObservation, TypographyProfile, UiNode,
    UiPreferences, UiRole, UiSnapshot, apply_design_system_with_theme, audit_text_layouts,
    text_audit_coverage,
};

#[cfg(feature = "eframe")]
mod entry;
mod renderer;
use crate::ShortcutEgui;
#[cfg(feature = "eframe")]
pub use entry::*;
pub use renderer::renderer_identity;

/// An application whose per-pass plumbing Stobie owns. See the crate documentation
/// and the application composition guide.
///
/// The UI borrows the application immutably, so panes cannot mutate the model:
/// they read state and emit intents. UI-only state such as focus requests lives
/// in [`Self::UiState`].
pub trait Application: 'static {
    /// The derived action set (see `#[derive(Action)]`).
    type Action: ActionSet;
    /// Data changes emitted by the UI, such as an edited draft or a selection.
    type Intent;
    /// UI-only state that persists across passes, such as a pending focus request.
    type UiState: Default;

    /// The name and build reported to inspection clients and used for the root
    /// node. `application_identity!("Name")` fills the build from Cargo.
    fn identity(&self) -> ApplicationIdentity;

    /// Current availability of one action target, used for buttons, shortcuts,
    /// discovery and semantic invocation alike.
    fn availability(&self, target: ActionTarget<Self::Action>) -> Availability;

    /// Present the current state. Emit data changes with [`Frame::emit`] and
    /// actions with [`Frame::action`] or [`Frame::trigger`].
    fn ui(
        &self,
        ui: &mut Ui,
        state: &mut Self::UiState,
        frame: &mut Frame<'_, Self::Action, Self::Intent>,
    );

    /// Apply one data intent. Return whether presented state changed, or an
    /// error for the user.
    fn apply(&mut self, intent: Self::Intent, state: &mut Self::UiState) -> Result<bool, String>;

    /// Perform one available action, whether it came from a click, a shortcut or
    /// a semantic invocation. Return whether presented state changed, or an error.
    fn perform(
        &mut self,
        target: ActionTarget<Self::Action>,
        state: &mut Self::UiState,
    ) -> Result<bool, String>;

    /// Perform an action with its argument, already validated against the
    /// action's declared contract. The default runs [`Self::perform`] for an
    /// action without an argument; an application whose actions declare
    /// arguments implements this and applies the same intent its control
    /// would emit.
    fn perform_with(
        &mut self,
        target: ActionTarget<Self::Action>,
        argument: ActionArgument,
        state: &mut Self::UiState,
    ) -> Result<bool, String> {
        match argument {
            ActionArgument::None => self.perform(target, state),
            _ => Err(format!(
                "{} does not handle its argument",
                target.action.specification().label
            )),
        }
    }

    /// Show an error from an intent, a clicked or shortcut action, or host
    /// startup. A rejected semantic invocation is reported in its receipt instead,
    /// so a failed invocation leaves observed state unchanged.
    fn report_error(&mut self, message: String);

    /// Assumptions whose change makes a previously discovered target stale, for
    /// example a selection, draft revision or history depth. Leave focus, layout
    /// and repaint counters out. The default treats the action as stable.
    fn meaning(&self, _target: ActionTarget<Self::Action>) -> String {
        "stable".into()
    }

    /// Reorder this pass's data intents before they apply. They apply in
    /// emission order by default; an application might apply draft edits
    /// before a selection change that depends on them.
    fn order_intents(&self, _intents: &mut [Self::Intent]) {}

    /// Publish bounded application facts and collection metadata with each
    /// completed observation.
    fn observe(&self, _observation: &mut Observation<'_>) {}

    /// Appearance preferences. The host applies the design system for them at
    /// startup and whenever they change, and resolves [`Frame::tokens`].
    fn preferences(&self) -> UiPreferences {
        UiPreferences::default()
    }

    /// The colour theme the design system and [`Frame::tokens`] resolve,
    /// Stobie's analytical theme by default. The host reapplies it on change.
    fn theme(&self) -> ApplicationTheme {
        ApplicationTheme::analytical()
    }

    /// Adapt state before the UI runs, for example to the viewport size, and
    /// poll workers. UI state is available for UI-owned resources such as
    /// textures made from worker results.
    fn prepare(&mut self, _pass: &mut PassContext<'_>, _state: &mut Self::UiState) {}

    /// Inspect the completed pass with the state it presented, including UI
    /// state, before this pass's intents and actions apply. Geometry and state
    /// therefore agree.
    fn completed(&mut self, _pass: &CompletedPass<'_>, _state: &Self::UiState) {}

    /// Whether the browser host mirrors this application's accessibility tree
    /// into a hidden ARIA layer over its canvas, for screen readers and
    /// accessibility-tree browser agents. On by default; it has no effect
    /// natively, where eframe's AccessKit adapter serves assistive technology.
    fn web_accessibility_layer(&self) -> bool {
        true
    }

    /// The pane that receives pane-scoped shortcuts. Without one, pane and
    /// active-pane shortcuts do not dispatch.
    fn active_pane(&self) -> Option<PaneId> {
        None
    }

    /// Panes that offer a pane-scoped action, so it stays discoverable when its
    /// control is not rendered. Rendered controls are always discoverable.
    fn action_panes(&self, _action: Self::Action) -> Vec<PaneId> {
        Vec::new()
    }

    /// Persist durable state to eframe storage. eframe calls this periodically
    /// and on exit; the host also calls it, and flushes, after a pass in which
    /// [`Self::take_save_request`] returns true.
    #[cfg(feature = "eframe")]
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}

    /// Whether an action asked to save now, for example "Save layout". The host
    /// asks once after each pass and clears nothing itself.
    #[cfg(feature = "eframe")]
    fn take_save_request(&mut self) -> bool {
        false
    }
}

/// Build identity from the calling crate's Cargo version and the optional
/// `STOBIE_BUILD_REVISION` set at build time.
#[macro_export]
macro_rules! build_identity {
    () => {
        $crate::BuildIdentity {
            version: ::std::env!("CARGO_PKG_VERSION").into(),
            source_revision: ::std::option_env!("STOBIE_BUILD_REVISION")
                .map(::std::borrow::ToOwned::to_owned),
        }
    };
}

/// Application identity with the calling crate's build identity.
#[macro_export]
macro_rules! application_identity {
    ($name:expr) => {
        $crate::ApplicationIdentity {
            name: ::std::convert::Into::into($name),
            build: $crate::build_identity!(),
        }
    };
}

/// Why the host asked egui for another pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostRepaint {
    /// An intent or action changed presented state.
    StateChanged,
    /// An intent or action failed and its error is now shown.
    Rejected,
    /// A dock drag or split preview is in progress.
    DockInteraction,
    /// Automation was enabled and needs a first observation.
    AutomationEnabled,
    /// Appearance preferences changed and the design system was reapplied.
    Appearance,
    /// The application asked for a repaint for this reason.
    Application(&'static str),
    /// The application asked for a pass after a delay, for example to end an
    /// idle gesture.
    Scheduled {
        reason: &'static str,
        delay: std::time::Duration,
    },
}

impl HostRepaint {
    /// Whether this reason asks for a pass now rather than after a delay.
    pub fn is_immediate(self) -> bool {
        !matches!(self, Self::Scheduled { .. })
    }
}

/// Ask egui for passes for the recorded reasons: one now if any reason is
/// immediate, and one after each scheduled delay.
fn request_passes(context: &egui::Context, reasons: &[HostRepaint]) {
    if reasons.iter().any(|reason| reason.is_immediate()) {
        context.request_repaint();
    }
    for reason in reasons {
        if let HostRepaint::Scheduled { delay, .. } = reason {
            context.request_repaint_after(*delay);
        }
    }
}

/// The UI's sink for one pass: intents, action triggers and repaint requests.
/// Panes receive it instead of mutable access to the application.
pub struct Frame<'a, A: ActionSet, I> {
    intents: &'a mut Vec<I>,
    triggers: &'a mut Vec<(ActionTarget<A>, ActionArgument)>,
    availability: &'a dyn Fn(ActionTarget<A>) -> Availability,
    repaint: &'a mut Vec<HostRepaint>,
    tokens: DesignTokens,
    font_scale: f32,
}

impl<A: ActionSet, I> Frame<'_, A, I> {
    /// Resolved design tokens for this pass's appearance preferences.
    pub fn tokens(&self) -> DesignTokens {
        self.tokens
    }

    /// Dock chrome appearance matching [`Self::tokens`].
    pub fn dock_text(&self) -> DockTextContext {
        DockTextContext {
            tokens: self.tokens,
            font_scale: self.font_scale,
        }
    }

    /// Begin a presentation context with this pass's appearance. Its
    /// observations reach the host automatically.
    pub fn presentation(
        &self,
        ui: &mut Ui,
        scope: PresentationScope,
        parent: SemanticUiId,
    ) -> PresentationContext {
        PresentationContext::new(ui, self.tokens, self.font_scale, scope, parent)
    }

    /// Begin a presentation context for a pane's content: its nodes take the
    /// pane's semantic node as parent and are associated with the pane.
    pub fn pane_presentation(
        &self,
        ui: &mut Ui,
        scope: PresentationScope,
        pane: PaneId,
    ) -> PresentationContext {
        self.presentation(ui, scope, SemanticUiId::pane(pane))
            .with_pane(pane)
    }

    /// Queue a data intent. Intents apply after the UI, before actions.
    pub fn emit(&mut self, intent: I) {
        self.intents.push(intent);
    }

    /// Queue an action through the same route as a click. It runs after this
    /// pass's intents if it is still available. Pass an [`ActionTarget`] for a
    /// pane-scoped action.
    pub fn trigger(&mut self, target: impl IntoActionTarget<A>) {
        self.triggers
            .push((target.into_target(None), ActionArgument::None));
    }

    /// Queue an action with its argument, as a control that chooses a value
    /// would. The argument is validated against the action's contract before
    /// [`Application::perform_with`] receives it.
    pub fn trigger_with(&mut self, target: impl IntoActionTarget<A>, argument: ActionArgument) {
        self.triggers.push((target.into_target(None), argument));
    }

    /// The application's current availability for a target.
    pub fn availability(&self, target: impl IntoActionTarget<A>) -> Availability {
        (self.availability)(target.into_target(None))
    }

    /// Ask for another pass, for example while an animation runs.
    pub fn request_repaint(&mut self, reason: &'static str) {
        self.repaint.push(HostRepaint::Application(reason));
    }

    /// Ask for a pass after `delay`, for example to end an idle gesture.
    pub fn request_repaint_after(&mut self, reason: &'static str, delay: std::time::Duration) {
        self.repaint.push(HostRepaint::Scheduled { reason, delay });
    }

    /// Present an action button with its current availability, and trigger the
    /// action when clicked. The action is the control's stable logical key. A
    /// pane-scoped action takes its pane from a [`Self::pane_presentation`].
    pub fn action(
        &mut self,
        ui: &mut Ui,
        presentation: &mut PresentationContext,
        target: impl IntoActionTarget<A>,
        emphasis: ActionEmphasis,
    ) -> Response {
        let target = target.into_target(presentation.pane());
        self.button(
            ui,
            presentation,
            target.action,
            target,
            ActionButtonState::Momentary,
            emphasis,
            ActionButtonContent::Text,
        )
    }

    /// Present an icon-only action with its full accessible name.
    pub fn icon_action(
        &mut self,
        ui: &mut Ui,
        presentation: &mut PresentationContext,
        target: impl IntoActionTarget<A>,
        icon: IconId,
        emphasis: ActionEmphasis,
    ) -> Response {
        let target = target.into_target(presentation.pane());
        self.button(
            ui,
            presentation,
            target.action,
            target,
            ActionButtonState::Momentary,
            emphasis,
            ActionButtonContent::IconOnly(icon),
        )
    }

    /// Present a toggle whose pressed state the application owns; a click
    /// triggers the action, which flips that state in [`Application::perform`].
    pub fn toggle(
        &mut self,
        ui: &mut Ui,
        presentation: &mut PresentationContext,
        key: impl Hash + std::fmt::Debug,
        target: impl IntoActionTarget<A>,
        pressed: bool,
    ) -> Response {
        let target = target.into_target(presentation.pane());
        self.button(
            ui,
            presentation,
            key,
            target,
            ActionButtonState::Toggle { pressed },
            ActionEmphasis::Normal,
            ActionButtonContent::Text,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn button(
        &mut self,
        ui: &mut Ui,
        presentation: &mut PresentationContext,
        key: impl Hash + std::fmt::Debug,
        target: ActionTarget<A>,
        state: ActionButtonState,
        emphasis: ActionEmphasis,
        content: ActionButtonContent,
    ) -> Response {
        let spec = ActionButtonSpec {
            target,
            availability: (self.availability)(target),
            state,
            emphasis,
            compact: false,
        };
        let response = presentation.action_with_content(ui, key, spec, content);
        if response.clicked() {
            self.triggers.push((target, ActionArgument::None));
        }
        response
    }

    /// Present the canonical dock tree read-only. Panes receive this frame;
    /// a completed tab, move or resize interaction is emitted as an intent.
    /// The host observes pane, tab and splitter semantics and repaints during
    /// drags. Use one dock per application.
    pub fn dock(&mut self, ui: &mut Ui, workspace: &Workspace, panes: &mut impl Panes<A, I>)
    where
        I: From<WorkspaceChange>,
    {
        let text_context = self.dock_text();
        let context = ui.ctx().clone();
        let id = egui::Id::new(("stobie.host.dock", context.viewport_id()));
        let mut behaviour = context
            .data_mut(|data| data.remove_temp::<DockBehaviour>(id))
            .unwrap_or_default();
        let change = crate::dock_workspace_view(
            ui,
            workspace,
            &mut behaviour,
            &mut HostedPanes {
                panes,
                frame: self,
                context: context.clone(),
            },
            text_context,
        );
        if behaviour.interaction_active() {
            self.repaint.push(HostRepaint::DockInteraction);
        }
        context.data_mut(|data| data.insert_temp(id, behaviour));
        if let Some(change) = change {
            self.intents.push(change.into());
        }
    }
}

/// Pane content inside [`Frame::dock`]. Implement it on a narrow read view
/// (borrowed fields the panes need), not on the application: a pane receives
/// the frame to emit intents and present actions, never the model.
///
/// The host publishes `pane.<id>` for each visible pane, `stobie.dock.tab.<id>`
/// for its tab and `stobie.dock.splitter.<id>` for splitters. Present pane
/// content with [`Frame::pane_presentation`], which parents it to `pane.<id>`.
pub trait Panes<A: ActionSet, I> {
    /// Stable tab title for a pane. Titles are static; show changing counts in
    /// the pane content instead.
    fn title(&self, pane: PaneId) -> &'static str;
    /// Present one pane's content inside its clipped body.
    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, frame: &mut Frame<'_, A, I>);
}

struct HostedPanes<'p, 'f, 'a, A: ActionSet, I, P> {
    panes: &'p mut P,
    frame: &'f mut Frame<'a, A, I>,
    context: egui::Context,
}

impl<A: ActionSet, I, P: Panes<A, I>> HostedPanes<'_, '_, '_, A, I, P> {
    fn observe(&self, node: UiNode) {
        let pass = self.context.cumulative_pass_nr();
        crate::observation::record(
            &self.context,
            self.context.viewport_id(),
            pass,
            [(node, Visibility::AsPresented)],
            [],
        );
    }
}

impl<A: ActionSet, I, P: Panes<A, I>> PanePresenter for HostedPanes<'_, '_, '_, A, I, P> {
    fn title(&self, pane: PaneId) -> &'static str {
        self.panes.title(pane)
    }

    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, pane_rect: Rect) {
        let mut node = UiNode::container(
            SemanticUiId::pane(pane),
            Some(SemanticUiId::root()),
            UiRole::Pane,
            pane_rect.into(),
        );
        node.name = self.panes.title(pane).into();
        node.pane = Some(pane);
        node.domain_reference = Some(DomainReference::Pane(pane));
        // Pane bodies scroll: publish only what each pane shows.
        crate::observe_visible_only(ui, |ui| {
            crate::observe_node(ui, node);
            self.panes.pane_ui(ui, pane, self.frame);
        });
    }

    fn record_tab_rect(&mut self, pane: PaneId, rect: Rect, selected: bool, focused: bool) {
        let mut node = UiNode::container(
            SemanticUiId::tab(pane),
            Some(SemanticUiId::root()),
            UiRole::Tab,
            rect.into(),
        );
        node.name = self.panes.title(pane).into();
        node.pane = Some(pane);
        node.domain_reference = Some(DomainReference::Pane(pane));
        node.selected = selected;
        node.focused = focused;
        self.observe(node);
    }

    fn record_text_layout(&mut self, observation: TextLayoutObservation) {
        let pass = self.context.cumulative_pass_nr();
        crate::observation::record(
            &self.context,
            self.context.viewport_id(),
            pass,
            [],
            [(observation, false)],
        );
    }

    fn record_splitter_rect(
        &mut self,
        id: stobie_core::DockNodeId,
        rect: Rect,
        horizontal: bool,
        focused: bool,
    ) {
        let mut node = UiNode::container(
            SemanticUiId::splitter(id),
            Some(SemanticUiId::root()),
            UiRole::Splitter,
            rect.into(),
        );
        // The divider's orientation is perpendicular to the split axis.
        node.name = if horizontal {
            "Vertical splitter"
        } else {
            "Horizontal splitter"
        }
        .into();
        node.description = Some("Resize adjacent dock panes".into());
        node.domain_reference = Some(DomainReference::DockNode(id));
        node.focused = focused;
        self.observe(node);
    }
}

/// Facts and collection metadata published with one completed observation.
pub struct Observation<'a> {
    snapshot: &'a UiSnapshot,
    facts: BTreeMap<String, ObservationValue>,
    collections: Vec<CollectionObservation>,
}

impl Observation<'_> {
    /// The completed pass's semantic snapshot.
    pub fn snapshot(&self) -> &UiSnapshot {
        self.snapshot
    }

    /// Publish one bounded primitive fact, such as a count or a status message.
    pub fn fact(&mut self, key: impl Into<String>, value: impl Into<ObservationValue>) {
        self.facts.insert(key.into(), value.into());
    }

    /// Describe how much of a collection the observation contains.
    pub fn collection(&mut self, collection: CollectionObservation) {
        self.collections.push(collection);
    }

    /// Describe a collection whose observed items are the snapshot's nodes with
    /// `role` and which is complete when every item is observed.
    pub fn counted_collection(&mut self, id: impl Into<String>, role: UiRole, total: u64) {
        let observed = self.snapshot.by_role(role).count();
        self.collection(CollectionObservation {
            id: id.into(),
            total: Some(total),
            observed,
            offset: None,
            virtualised: false,
            complete: observed as u64 == total,
        });
    }
}

/// The start of a pass, before invocations, shortcuts and the UI.
pub struct PassContext<'a> {
    context: &'a egui::Context,
    available_rect: Rect,
    repaint: &'a mut Vec<HostRepaint>,
}

impl PassContext<'_> {
    pub fn context(&self) -> &egui::Context {
        self.context
    }

    /// The area the application UI will occupy in this pass.
    pub fn available_rect(&self) -> Rect {
        self.available_rect
    }

    /// Ask for another pass, for example after a worker completion.
    pub fn request_repaint(&mut self, reason: &'static str) {
        self.repaint.push(HostRepaint::Application(reason));
    }
}

/// A completed UI pass, before its intents and actions apply.
pub struct CompletedPass<'a> {
    /// The pass's semantic and text observation.
    pub snapshot: &'a UiSnapshot,
    /// Tab key presses observed so far, counted once per input frame.
    pub tab_presses: u64,
    /// The egui context, for example to read this pass's input.
    pub context: &'a egui::Context,
    /// Repaints requested so far in this pass, for application diagnostics.
    /// Intents and actions that change state add [`HostRepaint::StateChanged`]
    /// after this hook.
    pub repaint: &'a [HostRepaint],
}

/// Owns an [`Application`] and runs its per-pass lifecycle.
pub struct AppHost<A: Application> {
    app: A,
    ui_state: A::UiState,
    identity: ApplicationIdentity,
    context: egui::Context,
    inspection: Option<Inspection>,
    #[cfg(not(target_arch = "wasm32"))]
    _native_host: Option<crate::NativeInspectionHost>,
    snapshot: UiSnapshot,
    passes: u64,
    tab_presses: u64,
    tab_frame: Option<u64>,
    repaint: Vec<HostRepaint>,
    preferences: UiPreferences,
    theme: ApplicationTheme,
    /// The latest pass's framework text fields and their text.
    text_fields: Vec<(TextFieldBinding, String)>,
    /// Text to set, applied one field per pass.
    text_edits: VecDeque<TextEdit>,
    /// The field this pass's edit focused and the focus to return to after it.
    edit_focus: Option<(egui::Id, Option<egui::Id>)>,
    /// The set-text invocation this pass applied, completed once its intents
    /// have settled.
    completing: Option<String>,
    /// Whether state changed since the latest pass paired text fields, so a
    /// widget may now present something other than its paired field.
    fields_stale: bool,
    /// Whether the first complete pass was reported to the startup loader.
    #[cfg(target_arch = "wasm32")]
    startup_reported: bool,
}

/// One queued set-text edit.
struct TextEdit {
    widget: egui::Id,
    /// The field's semantic ID when queued; the edit applies only while the
    /// widget still presents it.
    field: SemanticUiId,
    text: String,
    /// The invocation for a semantic edit, which returns focus afterwards;
    /// `None` for assistive technology editing in place.
    request: Option<String>,
}

fn apply_appearance(context: &egui::Context, preferences: UiPreferences, theme: &ApplicationTheme) {
    apply_design_system_with_theme(context, preferences, TypographyProfile::Dense, theme);
}

impl<A: Application> AppHost<A> {
    /// Host `app` in `context`. Native hosts start automation when
    /// `STOBIE_AUTOMATION_SOCKET` names an absolute socket path; a startup
    /// failure is reported through [`Application::report_error`].
    pub fn new(context: &egui::Context, app: A) -> Self {
        crate::observation::enable(context);
        let identity = app.identity();
        let preferences = app.preferences();
        let theme = app.theme();
        apply_appearance(context, preferences, &theme);
        #[allow(unused_mut)]
        let mut host = Self {
            app,
            ui_state: A::UiState::default(),
            identity,
            context: context.clone(),
            inspection: None,
            #[cfg(not(target_arch = "wasm32"))]
            _native_host: None,
            snapshot: UiSnapshot::default(),
            passes: 0,
            tab_presses: 0,
            tab_frame: None,
            repaint: Vec::new(),
            preferences,
            theme,
            text_fields: Vec::new(),
            text_edits: VecDeque::new(),
            edit_focus: None,
            completing: None,
            fields_stale: false,
            #[cfg(target_arch = "wasm32")]
            startup_reported: false,
        };
        #[cfg(not(target_arch = "wasm32"))]
        host.start_native_inspection();
        host
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_native_inspection(&mut self) {
        let inspection = Inspection::with_build_identity(
            self.identity.name.clone(),
            self.identity.build.clone(),
        );
        let wake = self.context.clone();
        if let Some(renderer) = renderer::renderer_identity(&self.context) {
            inspection.set_renderer(Some(renderer));
        }
        match inspection.start_native(move || wake.request_repaint()) {
            Ok(Some(host)) => {
                // Text field values and set text read the AccessKit tree.
                self.context.enable_accesskit();
                inspection.install_completion_hook(&self.context);
                self.inspection = Some(inspection);
                self._native_host = Some(host);
            }
            Ok(None) => {}
            Err(error) => {
                // Headless operators see stderr, not the application's status line.
                let message = format!("Automation could not start: {error}");
                eprintln!("{message}");
                self.app.report_error(message);
            }
        }
    }

    /// Enable inspection with an explicitly constructed service, for example a
    /// browser instance or a test. The host publishes from the next pass.
    pub fn enable_inspection(&mut self, inspection: Inspection) {
        if let Some(renderer) = renderer::renderer_identity(&self.context) {
            inspection.set_renderer(Some(renderer));
        }
        self.context.enable_accesskit();
        inspection.install_completion_hook(&self.context);
        self.inspection = Some(inspection);
        self.repaint.push(HostRepaint::AutomationEnabled);
        self.context.request_repaint();
    }

    pub fn inspection(&self) -> Option<&Inspection> {
        self.inspection.as_ref()
    }

    pub fn app(&self) -> &A {
        &self.app
    }

    /// Mutable access for tests and host integrations. Mutating here bypasses
    /// intents, so request a repaint if presented state changes.
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    /// The latest pass's semantic and text observation.
    pub fn ui_snapshot(&self) -> &UiSnapshot {
        &self.snapshot
    }

    /// Ask for another pass from outside one, for example from a page hook.
    pub fn request_repaint(&mut self, reason: &'static str) {
        self.repaint.push(HostRepaint::Application(reason));
        self.context.request_repaint();
    }

    /// The application's UI-only state, for tests and read-only page hooks.
    pub fn ui_state(&self) -> &A::UiState {
        &self.ui_state
    }

    /// Apply an intent from outside a pass, such as a page hook or a test,
    /// through the same handler as the UI's intents. An error reaches
    /// [`Application::report_error`]. Returns whether it asked for a repaint.
    pub fn submit(&mut self, intent: A::Intent) -> bool {
        let reasons = self.repaint.len();
        let outcome = self.app.apply(intent, &mut self.ui_state);
        self.settle(outcome);
        let repaint = self.repaint.len() > reasons;
        if repaint {
            self.context.request_repaint();
        }
        repaint
    }

    /// Tab key presses observed so far, counted once per input frame.
    pub fn tab_presses(&self) -> u64 {
        self.tab_presses
    }

    /// Why the latest pass (and any later out-of-pass request) asked for a repaint.
    pub fn repaint_reasons(&self) -> &[HostRepaint] {
        &self.repaint
    }

    /// Run one UI pass of the hosted application over `root`.
    pub fn present(&mut self, root: &mut Ui) {
        let context = root.ctx().clone();
        // The root node covers the viewport, not content that overflowed it.
        let root_rect = root.max_rect();
        self.repaint.clear();
        self.count_tab_presses(&context);
        let preferences = self.app.preferences();
        let theme = self.app.theme();
        if preferences != self.preferences || theme != self.theme {
            // New fonts and styles take effect from the next pass.
            apply_appearance(&context, preferences, &theme);
            self.preferences = preferences;
            self.theme = theme;
            self.repaint.push(HostRepaint::Appearance);
        }
        let mut prepare = PassContext {
            context: &context,
            available_rect: root.available_rect_before_wrap(),
            repaint: &mut self.repaint,
        };
        self.app.prepare(&mut prepare, &mut self.ui_state);
        self.drain_invocations();
        let mut triggers: Vec<_> = self
            .consume_shortcuts(root)
            .into_iter()
            .map(|target| (target, ActionArgument::None))
            .collect();
        self.apply_text_edit(root);
        let mut intents = Vec::new();
        {
            let app = &self.app;
            let availability = |target: ActionTarget<A::Action>| app.availability(target);
            let mut frame = Frame {
                intents: &mut intents,
                triggers: &mut triggers,
                availability: &availability,
                repaint: &mut self.repaint,
                tokens: self.theme.resolve(
                    self.preferences
                        .theme_variant(context.theme() == egui::Theme::Dark),
                    self.preferences.density_variant(),
                    TypographyProfile::Dense,
                ),
                font_scale: self.preferences.font_scale,
            };
            app.ui(root, &mut self.ui_state, &mut frame);
        }
        // Set text is a semantic edit: once the field has taken it, focus
        // returns to where it was, so no caret is left blinking.
        if let Some((widget, previous)) = self.edit_focus.take() {
            context.memory_mut(|memory| match previous {
                Some(previous) => memory.request_focus(previous),
                None => memory.surrender_focus(widget),
            });
        }
        self.passes += 1;
        (self.snapshot, self.text_fields) = self.collect(root.ctx(), root_rect);
        self.fields_stale = false;
        self.publish(&context);
        self.app.completed(
            &CompletedPass {
                snapshot: &self.snapshot,
                tab_presses: self.tab_presses,
                context: &context,
                repaint: &self.repaint,
            },
            &self.ui_state,
        );
        self.app.order_intents(&mut intents);
        for intent in intents {
            let outcome = self.app.apply(intent, &mut self.ui_state);
            self.settle(outcome);
        }
        for (target, argument) in triggers {
            let outcome = self.run_action(target, argument);
            self.settle(outcome);
        }
        // The edit's intents have now applied, so the next published pass
        // presents the application's own effect.
        if let (Some(request), Some(inspection)) = (self.completing.take(), &self.inspection) {
            inspection.complete(&request, Ok(()));
        }
        request_passes(&context, &self.repaint);
    }

    /// Dispatch one inspection request on the application thread, running an
    /// admitted invocation through the semantic route immediately.
    pub fn dispatch(&mut self, request: InspectionRequest) -> Option<InspectionReply> {
        let inspection = self.inspection.clone()?;
        let bindings = self.bindings();
        let reasons = self.repaint.len();
        let fields = self.text_field_bindings();
        let reply = inspection.dispatch_invocation(request, &bindings, &fields, |invocation| {
            self.run_invocation(invocation)
        });
        // Only a state change repaints; reads never do.
        if self.repaint.len() > reasons {
            self.context.request_repaint();
        }
        Some(reply)
    }

    /// Handle one JSON inspection request on the application thread, as the
    /// browser automation hook does. Malformed or oversized requests are
    /// answered without admitting a mutation.
    pub fn dispatch_json(&mut self, json: &str) -> Result<String, String> {
        let inspection = self.inspection.clone().ok_or("Automation is disabled")?;
        if json.len() > crate::INSPECTION_REQUEST_BYTES {
            return Ok(inspection.handle_json(json));
        }
        let Ok(request) = serde_json::from_str::<InspectionRequest>(json) else {
            return Ok(inspection.handle_json(json));
        };
        let reply = self.dispatch(request).expect("inspection is enabled");
        serde_json::to_string(&reply).map_err(|error| error.to_string())
    }

    fn count_tab_presses(&mut self, context: &egui::Context) {
        // Discarded layout passes see the same frame; count its input once.
        let frame = context.cumulative_frame_nr();
        if self.tab_frame == Some(frame) {
            return;
        }
        self.tab_frame = Some(frame);
        self.tab_presses += context.input(|input| {
            input
                .events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Tab,
                            pressed: true,
                            ..
                        }
                    )
                })
                .count() as u64
        });
    }

    /// Whether the latest pass observed a focused node with one of `roles`.
    fn observed_focus(&self, roles: &[UiRole]) -> bool {
        self.snapshot
            .nodes
            .iter()
            .any(|node| node.focused && roles.contains(&node.role))
    }

    /// Consume available shortcut presses whose modifiers match exactly. egui's
    /// logical matcher accepts extra Shift/Alt, so an unavailable Shift chord
    /// must not fall through to its plain counterpart; it stays in the input.
    ///
    /// Shortcuts without Cmd yield every key to focused text entry: an egui text
    /// edit, or a node the latest pass observed as a focused text input. They
    /// yield Enter and Space to any other focused control except an observed
    /// viewport, because egui activates a focused control with those keys.
    fn consume_shortcuts(&self, root: &mut Ui) -> Vec<ActionTarget<A::Action>> {
        let context = root.ctx();
        let text_focus = context.text_edit_focused()
            || self.observed_focus(&[UiRole::TextInput, UiRole::MultilineTextInput]);
        let control_focus = context.memory(|memory| memory.focused().is_some())
            && !self.observed_focus(&[UiRole::Viewport]);
        let active_pane = self.app.active_pane();
        let shortcuts: Vec<_> = A::Action::ALL
            .iter()
            .filter_map(|&action| {
                let spec = action.specification();
                let shortcut = spec.shortcut?;
                let activation = matches!(shortcut.key, ShortcutKey::Enter | ShortcutKey::Space);
                if !shortcut.command && (text_focus || control_focus && activation) {
                    return None;
                }
                let target = match spec.scope {
                    ActionScope::Application => ActionTarget::application(action),
                    ActionScope::Pane | ActionScope::ActivePane => {
                        ActionTarget::pane(action, active_pane?)
                    }
                };
                Some((shortcut, target))
            })
            .collect();
        if shortcuts.is_empty() {
            return Vec::new();
        }
        let pressed: Vec<(usize, ActionTarget<A::Action>)> = root.input(|input| {
            input
                .events
                .iter()
                .enumerate()
                .filter_map(|(index, event)| {
                    let egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } = event
                    else {
                        return None;
                    };
                    shortcuts
                        .iter()
                        .find(|(shortcut, _)| {
                            shortcut.key.egui() == *key
                                && modifiers.matches_exact(shortcut.egui().modifiers)
                        })
                        .map(|(_, target)| (index, *target))
                })
                .collect()
        });
        // Check availability outside egui's input lock: the application may
        // read the context, and egui locks are not reentrant.
        let (triggered, consumed): (Vec<_>, Vec<_>) = pressed
            .into_iter()
            .filter(|(_, target)| self.app.availability(*target).enabled())
            .map(|(index, target)| (target, index))
            .unzip();
        if !consumed.is_empty() {
            root.input_mut(|input| {
                let mut index = 0;
                input.events.retain(|_| {
                    let keep = !consumed.contains(&index);
                    index += 1;
                    keep
                });
            });
        }
        triggered
    }

    fn run_action(
        &mut self,
        target: ActionTarget<A::Action>,
        argument: ActionArgument,
    ) -> Result<bool, String> {
        match self.app.availability(target) {
            Availability::Enabled => {
                target
                    .action
                    .argument()
                    .validate(&argument)
                    .map_err(|error| error.message)?;
                self.app.perform_with(target, argument, &mut self.ui_state)
            }
            Availability::Disabled { reason } => Err(reason.into_owned()),
            Availability::Hidden => Err(format!(
                "{} is unavailable",
                target.action.specification().label
            )),
        }
    }

    fn run_invocation(
        &mut self,
        invocation: Invocation<A::Action>,
    ) -> Result<Execution, InspectionError> {
        match invocation {
            Invocation::Action { target, argument } => {
                let changed = self
                    .run_action(target, argument)
                    .map_err(InspectionError::validation)?;
                if changed {
                    self.repaint.push(HostRepaint::StateChanged);
                    self.fields_stale = true;
                }
                Ok(Execution::Completed)
            }
            // The edit applies through egui's own text editing on a pass, and
            // completes once that pass's intents have applied.
            Invocation::SetText {
                request_id,
                field,
                text,
            } => {
                self.text_edits.push_back(TextEdit {
                    widget: field.widget,
                    field: field.id,
                    text,
                    request: Some(request_id),
                });
                self.repaint.push(HostRepaint::Application("set text"));
                Ok(Execution::Deferred)
            }
        }
    }

    /// Finish a semantic edit that cannot apply.
    fn reject_edit(&self, edit: &TextEdit, message: String) {
        if let (Some(request), Some(inspection)) = (&edit.request, &self.inspection) {
            inspection.complete(request, Err(InspectionError::validation(message)));
        }
    }

    fn text_field_bindings(&self) -> Vec<TextFieldBinding> {
        self.text_fields
            .iter()
            .map(|(binding, _)| binding.clone())
            .collect()
    }

    /// Apply one queued set-text edit, from an invocation or an AccessKit
    /// `SetValue` request, as typed input: focus the field, select its text
    /// and paste the new text over it (or delete the selection for empty
    /// text). The field's `TextEdit` handles it as any edit, so the
    /// application sees its ordinary change. After a semantic edit, focus
    /// returns to where it was; assistive technology editing in place keeps
    /// it and goes first, so the field being typed in never waits. Further
    /// edits wait for later passes, because egui edits only the focused
    /// field.
    fn apply_text_edit(&mut self, root: &mut Ui) {
        use egui::accesskit::{Action, ActionData, ActionRequest, TreeId};
        let fields = &self.text_fields;
        let requests: Vec<TextEdit> = root.input_mut(|input| {
            let mut requests = Vec::new();
            input.events.retain(|event| {
                if let egui::Event::AccessKitActionRequest(ActionRequest {
                    action: Action::SetValue,
                    target_node,
                    target_tree,
                    data: Some(ActionData::Value(value)),
                }) = event
                    && *target_tree == TreeId::ROOT
                    && let Some((field, _)) = fields
                        .iter()
                        .find(|(field, _)| field.widget.accesskit_id() == *target_node)
                {
                    // Assistive technology edits in place; focus stays.
                    requests.push(TextEdit {
                        widget: field.widget,
                        field: field.id.clone(),
                        text: value.to_string(),
                        request: None,
                    });
                    false
                } else {
                    true
                }
            });
            requests
        });
        // Each in-place edit carries the field's whole text, so consecutive
        // edits of one field, such as fast typing in a layer field, apply as
        // the latest.
        for edit in requests {
            match self.text_edits.front_mut() {
                Some(first) if first.request.is_none() && first.widget == edit.widget => {
                    first.text = edit.text;
                }
                _ => self.text_edits.push_front(edit),
            }
        }
        // A queued semantic edit names its field as last presented. If state
        // changed since then, in this pass or between passes, the widget may
        // now present something else, so the edit waits for a pass that pairs
        // fields afresh.
        if self
            .text_edits
            .front()
            .is_some_and(|edit| edit.request.is_some())
            && self.fields_stale
        {
            self.repaint.push(HostRepaint::Application("set text"));
            return;
        }
        let Some(edit) = self.text_edits.pop_front() else {
            return;
        };
        if !self.text_edits.is_empty() {
            self.repaint.push(HostRepaint::Application("set text"));
        }
        let widget = edit.widget;
        let Some((field, current)) = self
            .text_fields
            .iter()
            .find(|(field, _)| field.widget == widget && field.id == edit.field)
        else {
            self.reject_edit(&edit, format!("{} is no longer presented", edit.field.0));
            return;
        };
        // A disabled or read-only field takes no typed input either.
        if let Some(reason) = field.availability.disabled_reason() {
            let reason = reason.to_owned();
            self.reject_edit(&edit, reason);
            return;
        }
        if *current == edit.text {
            if let (Some(request), Some(inspection)) = (&edit.request, &self.inspection) {
                inspection.complete(request, Ok(()));
            }
            return;
        }
        let context = root.ctx().clone();
        let previous = context.memory(|memory| memory.focused());
        if edit.request.is_some() && previous != Some(widget) {
            self.edit_focus = Some((widget, previous));
        }
        self.completing = edit.request;
        let text = edit.text;
        context.memory_mut(|memory| memory.request_focus(widget));
        let mut state = egui::text_edit::TextEditState::load(&context, widget).unwrap_or_default();
        // egui clamps the end to the text's length.
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(usize::MAX),
            )));
        state.store(&context, widget);
        let event = if text.is_empty() {
            egui::Event::Key {
                key: egui::Key::Delete,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }
        } else {
            egui::Event::Paste(text)
        };
        root.input_mut(|input| input.events.push(event));
    }

    fn settle(&mut self, outcome: Result<bool, String>) {
        match outcome {
            Ok(true) => {
                self.repaint.push(HostRepaint::StateChanged);
                self.fields_stale = true;
            }
            Ok(false) => {}
            Err(message) => {
                self.app.report_error(message);
                self.repaint.push(HostRepaint::Rejected);
            }
        }
    }

    fn drain_invocations(&mut self) {
        let Some(inspection) = self.inspection.clone() else {
            return;
        };
        // Recalculate bindings from current state before every queued action.
        loop {
            let bindings = self.bindings();
            let fields = self.text_field_bindings();
            if !inspection.drain_invocation(&bindings, &fields, |invocation| {
                self.run_invocation(invocation)
            }) {
                break;
            }
        }
    }

    /// Bind every action to each rendered control carrying it, or to its
    /// default identity when none is rendered, so discovery covers the whole set.
    fn bindings(&self) -> Vec<InspectionBinding<A::Action>> {
        let mut bindings = Vec::new();
        let mut presented = Vec::new();
        for &action in A::Action::ALL {
            let nodes: Vec<&UiNode> = self.snapshot.by_action(action).collect();
            let mut targets: Vec<(ActionTarget<A::Action>, Option<&UiNode>)> =
                match action.specification().scope {
                    ActionScope::Application if nodes.is_empty() => {
                        vec![(ActionTarget::application(action), None)]
                    }
                    ActionScope::Application => nodes
                        .iter()
                        .map(|node| (ActionTarget::application(action), Some(*node)))
                        .collect(),
                    ActionScope::Pane | ActionScope::ActivePane => nodes
                        .iter()
                        .filter_map(|node| {
                            node.pane
                                .map(|pane| (ActionTarget::pane(action, pane), Some(*node)))
                        })
                        .collect(),
                };
            for pane in self.app.action_panes(action) {
                if action.specification().scope != ActionScope::Application
                    && !targets.iter().any(|(target, _)| target.pane == Some(pane))
                {
                    targets.push((ActionTarget::pane(action, pane), None));
                }
            }
            let semantic = action.invocation().semantic();
            for (target, node) in targets {
                let mut meaning = self.app.meaning(target);
                if meaning.is_empty() {
                    meaning = "stable".into();
                }
                let mut binding = InspectionBinding::new(
                    target,
                    self.app.availability(target),
                    semantic,
                    meaning,
                );
                // A pane target belongs to its pane unless its control names a
                // more specific domain object.
                let domain = node
                    .and_then(|node| node.domain_reference.clone())
                    .or_else(|| target.pane.map(DomainReference::Pane));
                // Several controls can present one target, such as a toolbar
                // button and a viewport's custom action; the first is the
                // capability's instance.
                let key = (target, domain.clone());
                if presented.contains(&key) {
                    continue;
                }
                presented.push(key);
                if let Some(node) = node {
                    binding = binding.with_id(node.id.clone());
                }
                if let Some(domain) = domain {
                    binding = binding.with_domain(domain);
                }
                bindings.push(binding);
            }
        }
        bindings
    }

    fn collect(
        &self,
        context: &egui::Context,
        root_rect: Rect,
    ) -> (UiSnapshot, Vec<(TextFieldBinding, String)>) {
        let observed = crate::observation::take(context);
        let mut root_node = UiNode::container(
            SemanticUiId::root(),
            None,
            UiRole::Application,
            root_rect.into(),
        );
        root_node.name = self.identity.name.clone();
        let mut nodes = vec![root_node];
        let mut published = BTreeMap::from([(SemanticUiId::root(), (0, Origin::Application))]);
        let mut duplicates = Vec::new();
        let mut non_finite = Vec::new();
        for (mut node, visibility, origin) in observed.nodes {
            let rect = Rect::from(node.rect);
            // Non-finite geometry would make the whole publication invalid, so
            // report it as an audit finding instead of publishing the node.
            if !rect.is_finite() {
                non_finite.push(node.id);
                continue;
            }
            // Visible-only content that is clipped away is not shown. Keep an
            // empty allocation so the semantic audit reports it.
            if let Visibility::VisibleWithin(clip) = visibility
                && rect.is_positive()
            {
                let visible = rect.intersect(clip);
                if !visible.is_positive() {
                    continue;
                }
                node.rect = visible.into();
            }
            match published.get(&node.id).copied() {
                None => {
                    published.insert(node.id.clone(), (nodes.len(), origin));
                    nodes.push(node);
                }
                // An application's observation of the same control refines the
                // recipe's own node; anything else sharing the ID is a duplicate.
                Some((index, Origin::Recipe))
                    if origin == Origin::Application && nodes[index].role == node.role =>
                {
                    published.insert(node.id.clone(), (index, origin));
                    nodes[index] = node;
                }
                Some((index, Origin::Application))
                    if origin == Origin::Recipe && nodes[index].role == node.role => {}
                Some(_) => duplicates.push(node.id),
            }
        }
        let text_fields = pair_text_fields(context, &mut nodes);
        // Visible-only surfaces scroll, so they submit partially visible text.
        // Audit their measured successes only when complete; keep every failed
        // attempt and all other text as presented.
        let text: Vec<_> = observed
            .text
            .into_iter()
            .filter(|(text, visible_only)| {
                !visible_only
                    || text.layout_error.is_some()
                    || (text.clip_rect.min_x <= text.allocated_rect.min_x + 1.0
                        && text.clip_rect.min_y <= text.allocated_rect.min_y + 1.0
                        && text.clip_rect.max_x >= text.allocated_rect.max_x - 1.0
                        && text.clip_rect.max_y >= text.allocated_rect.max_y - 1.0)
            })
            .map(|(text, _)| text)
            .collect();
        let mut snapshot = UiSnapshot {
            frame: self.passes,
            pixels_per_point: context.pixels_per_point(),
            root: SemanticUiId::root(),
            nodes,
            text_audit: audit_text_layouts(&text),
            text_audit_coverage: Some(text_audit_coverage(context, &text)),
            text,
            semantic_audit: Vec::new(),
        };
        snapshot.semantic_audit = snapshot.audit();
        snapshot.semantic_audit.extend(
            duplicates
                .into_iter()
                .map(|id| SemanticAuditFinding::DuplicateId { id })
                .chain(
                    non_finite
                        .into_iter()
                        .map(|id| SemanticAuditFinding::InvalidRect { id }),
                ),
        );
        (snapshot, text_fields)
    }

    fn publish(&self, context: &egui::Context) {
        let Some(inspection) = &self.inspection else {
            return;
        };
        if context.will_discard() {
            return;
        }
        let mut observation = Observation {
            snapshot: &self.snapshot,
            facts: BTreeMap::new(),
            collections: Vec::new(),
        };
        self.app.observe(&mut observation);
        let metadata = CollectionMetadata {
            collections: observation.collections,
            ..Default::default()
        };
        if let Err(error) = inspection.stage_completed_with_fields(
            context,
            self.snapshot.clone(),
            &self.bindings(),
            &self.text_field_bindings(),
            metadata,
            observation.facts,
        ) {
            eprintln!("Automation publication failed: {error:?}");
        }
    }
}

/// Pair this pass's recorded egui text edits with the observed text input
/// nodes that present them, publish each paired field's text as its node's
/// value and offer AccessKit `SetValue` on it. A field pairs by its AccessKit
/// `author_id`, which Stobie recipes set to the semantic ID, or else by
/// presenting exactly one text input node's rectangle; the host then sets the
/// `author_id`. Without AccessKit, no field pairs.
fn pair_text_fields(
    context: &egui::Context,
    nodes: &mut [UiNode],
) -> Vec<(TextFieldBinding, String)> {
    use egui::accesskit::Action;
    let is_text =
        |node: &UiNode| matches!(node.role, UiRole::TextInput | UiRole::MultilineTextInput);
    let mut fields = Vec::new();
    for (widget, rect) in crate::text_coverage::recorded_text_edits(context) {
        let Some((author, value, disabled, read_only, password)) =
            context.accesskit_node_builder(widget, |node| {
                (
                    node.author_id().map(ToOwned::to_owned),
                    node.value().unwrap_or_default().to_owned(),
                    node.is_disabled(),
                    node.is_read_only(),
                    node.role() == egui::accesskit::Role::PasswordInput,
                )
            })
        else {
            continue;
        };
        // AccessKit carries a password field's masked text, which an edit
        // would write back; set text never touches one.
        if password {
            continue;
        }
        let index = match &author {
            Some(author) => nodes
                .iter()
                .position(|node| is_text(node) && node.id.0 == *author),
            None => {
                let mut matching = nodes.iter().enumerate().filter(|(_, node)| {
                    let presented = Rect::from(node.rect);
                    is_text(node)
                        && (presented.min - rect.min).length() < 0.5
                        && (presented.max - rect.max).length() < 0.5
                });
                match (matching.next(), matching.next()) {
                    (Some((index, _)), None) => Some(index),
                    _ => None,
                }
            }
        };
        let Some(index) = index else {
            continue;
        };
        let node = &mut nodes[index];
        if fields
            .iter()
            .any(|(field, _): &(TextFieldBinding, String)| field.id == node.id)
        {
            continue;
        }
        node.value = Some(value.clone());
        let editable = node.enabled && !disabled && !read_only;
        context.accesskit_node_builder(widget, |builder| {
            if author.is_none() {
                builder.set_author_id(node.id.0.clone());
            }
            if editable {
                builder.add_action(Action::SetValue);
            }
        });
        let availability = if editable {
            Availability::Enabled
        } else {
            Availability::Disabled {
                reason: node
                    .disabled_reason
                    .clone()
                    .unwrap_or_else(|| "The text field is read-only".into())
                    .into(),
            }
        };
        fields.push((
            TextFieldBinding {
                id: node.id.clone(),
                widget,
                pane: node.pane,
                domain: node.domain_reference.clone(),
                availability,
            },
            value,
        ));
    }
    fields
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod text_tests;
