//! Thin, pass-local access to the existing presentation recipes.

use std::hash::Hash;

use egui::{Response, Ui};

use crate::{
    ActionArgument, ActionButtonContent, ActionButtonIdentity, ActionButtonSpec, ActionKey,
    DesignTokens, DomainReference, IconId, NativeTextControlKind, SemanticUiId, TextAuditCoverage,
    TextInteraction, TextLayoutObservation, TextOverflow, TextRole, UiNode,
    action_semantic_node_with_identity, record_native_text_control, text_audit_coverage,
};

/// Logical hierarchy, independent of egui's layout hierarchy. Keys must describe
/// stable controls or domain objects, never row positions, labels or counters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationScope(egui::Id);

impl PresentationScope {
    pub fn new(key: impl Hash + std::fmt::Debug) -> Self {
        Self(egui::Id::new(("stobie.presentation", key)))
    }

    pub fn child(self, key: impl Hash + std::fmt::Debug) -> Self {
        Self(self.0.with(key))
    }

    pub fn instance(self, key: impl Hash + std::fmt::Debug) -> PresentationId {
        PresentationId(self.0.with(key))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationId(egui::Id);

impl PresentationId {
    pub fn egui_id(self) -> egui::Id {
        self.0
    }

    pub fn semantic_id(self) -> SemanticUiId {
        SemanticUiId::new(format!("presentation.{:016x}", self.0.value()))
    }

    pub fn text_instance(self) -> u64 {
        // Snapshot IDs must round-trip through JavaScript without rounding.
        self.0.value() & ((1_u64 << 53) - 1)
    }

    pub fn action_identity(self) -> ActionButtonIdentity {
        ActionButtonIdentity {
            widget_id: self.egui_id(),
            semantic_id: self.semantic_id(),
            text_instance: self.text_instance(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContentTextSpec {
    /// Measured typography role.
    pub role: TextRole,
    /// Explicit wrapping or elision policy.
    pub overflow: TextOverflow,
    /// Bounded line count accepted by the selected recipe (1–8).
    pub max_lines: u8,
    /// Selectable text or inert text for controls with an owning gesture.
    pub interaction: TextInteraction,
}

/// The source and justification for presentation outside the adapter's recipes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawPresentation {
    pub id: SemanticUiId,
    pub reason: &'static str,
}

/// Published inside the same pass. Keep the latest publication per viewport;
/// replace it on a repeated layout pass, rather than extending prior results.
/// Coverage's measured count covers this context's retained layouts; attempted,
/// failed and native counts use the shared viewport-pass inventory. Recompute
/// once over merged layouts at the viewport boundary; never sum partial coverage.
pub struct PresentationObservations {
    /// Viewport whose latest publication this replaces.
    pub viewport: egui::ViewportId,
    /// Originating egui layout pass; repeated passes produce replacements.
    pub pass: u64,
    /// This context's retained measured text, not text from earlier passes.
    pub text_layouts: Vec<TextLayoutObservation>,
    /// Current application/control semantics recorded by this context.
    pub semantic_nodes: Vec<UiNode>,
    /// Local measured count plus shared viewport-pass inventory counts; do not sum.
    pub coverage: TextAuditCoverage,
    /// Explicitly recorded presentations outside the measured recipes.
    pub raw_presentations: Vec<RawPresentation>,
}

/// Owns only resolved appearance and current observations. The caller retains
/// explicit egui layout, state, command delivery and collection virtualisation.
/// Construct inside each pass and finish before leaving it. Reuse in another
/// context, viewport or pass is rejected, including publication after a retry.
///
/// In a hosted application ([`AppHost`](crate::AppHost)) the context registers
/// its observations with the pass's collector when it is finished or dropped;
/// no merging is needed. A context created inside
/// [`observe_visible_only`](crate::observe_visible_only), such as one for a
/// dock pane, publishes only what is visible; any other publishes as presented.
pub struct PresentationContext {
    context: egui::Context,
    viewport: egui::ViewportId,
    pass: u64,
    tokens: DesignTokens,
    font_scale: f32,
    scope: PresentationScope,
    parent: SemanticUiId,
    domain_reference: Option<DomainReference>,
    pane: Option<stobie_core::PaneId>,
    text_layouts: Vec<TextLayoutObservation>,
    semantic_nodes: Vec<UiNode>,
    /// The clip rectangle each semantic node was presented under.
    node_clips: Vec<egui::Rect>,
    /// Publish only visible content, from the policy in force at creation.
    visible_only: bool,
    raw_presentations: Vec<RawPresentation>,
}

impl Drop for PresentationContext {
    fn drop(&mut self) {
        // An unfinished context still reaches a hosted collector.
        let pending = !self.semantic_nodes.is_empty() || !self.text_layouts.is_empty();
        if pending && crate::observation::is_enabled(&self.context, self.viewport) {
            let nodes = std::mem::take(&mut self.semantic_nodes);
            let text = std::mem::take(&mut self.text_layouts);
            self.register(nodes, text);
        }
    }
}

impl PresentationContext {
    /// Begin a context inside the current egui viewport/pass with resolved
    /// appearance and stable logical identity. This owns observations only;
    /// application data, layout, intents and repaint scheduling stay with the caller.
    pub fn new(
        ui: &mut Ui,
        tokens: DesignTokens,
        font_scale: f32,
        scope: PresentationScope,
        parent: SemanticUiId,
    ) -> Self {
        Self {
            context: ui.ctx().clone(),
            viewport: ui.ctx().viewport_id(),
            pass: ui.ctx().cumulative_pass_nr(),
            tokens,
            font_scale,
            scope,
            parent,
            domain_reference: None,
            pane: None,
            text_layouts: Vec::new(),
            semantic_nodes: Vec::new(),
            node_clips: Vec::new(),
            visible_only: crate::observation::is_visible_only(ui.ctx()),
            raw_presentations: Vec::new(),
        }
    }

    fn push_node(&mut self, ui: &Ui, mut node: UiNode) {
        if node.pane.is_none() {
            node.pane = self.pane;
        }
        self.semantic_nodes.push(node);
        self.node_clips.push(ui.clip_rect());
    }

    /// The pane this context's nodes are associated with, if any.
    pub fn pane(&self) -> Option<stobie_core::PaneId> {
        self.pane
    }

    /// Associate this context's nodes with a pane, so pane queries find them.
    /// Nodes that already name a pane keep it.
    pub fn with_pane(mut self, pane: stobie_core::PaneId) -> Self {
        self.pane = Some(pane);
        self
    }

    /// Hand observations to the enabled hosted collector under this
    /// context's visibility policy.
    fn register(&mut self, nodes: Vec<UiNode>, text: Vec<TextLayoutObservation>) {
        use crate::observation::Visibility;
        let clips = std::mem::take(&mut self.node_clips);
        let visible_only = self.visible_only;
        crate::observation::record(
            &self.context,
            self.viewport,
            self.pass,
            nodes.into_iter().zip(clips).map(|(node, clip)| {
                let visibility = if visible_only {
                    Visibility::VisibleWithin(clip)
                } else {
                    Visibility::AsPresented
                };
                (node, visibility)
            }),
            text.into_iter().map(|text| (text, visible_only)),
        );
    }

    /// Domain metadata is separate from capability routing. Include the stable
    /// domain ID in the logical scope when controls repeat for several objects.
    pub fn with_domain_reference(mut self, reference: DomainReference) -> Self {
        self.domain_reference = Some(reference);
        self
    }

    /// Resolved appearance for application-owned layout and native composition.
    pub fn tokens(&self) -> &DesignTokens {
        &self.tokens
    }

    pub fn font_scale(&self) -> f32 {
        self.font_scale
    }

    /// Give a repeated domain object a stable logical child scope while keeping
    /// its observations in this publication. Layout remains application-owned.
    pub fn scoped<R>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        render: impl FnOnce(&mut Ui, &mut Self) -> R,
    ) -> R {
        self.check_pass(ui);
        let mut child = Self::new(
            ui,
            self.tokens,
            self.font_scale,
            self.scope.child(key),
            self.parent.clone(),
        );
        child.domain_reference = self.domain_reference.clone();
        child.pane = self.pane;
        child.visible_only = self.visible_only;
        let result = render(ui, &mut child);
        child.check_pass(ui);
        // The parent publishes the child's observations; the child registers nothing.
        self.text_layouts.append(&mut child.text_layouts);
        self.semantic_nodes.append(&mut child.semantic_nodes);
        self.node_clips.append(&mut child.node_clips);
        self.raw_presentations.append(&mut child.raw_presentations);
        result
    }

    /// Retain application-owned semantics for a custom row or evidence surface.
    /// The caller supplies its complete stable identity, parent and domain data.
    pub fn observe_node(&mut self, ui: &Ui, node: UiNode) {
        self.check_pass(ui);
        self.push_node(ui, node);
    }

    /// Retain application-owned measured text, such as a custom row's label.
    pub fn observe_text(&mut self, ui: &Ui, observation: TextLayoutObservation) {
        self.check_pass(ui);
        self.text_layouts.push(observation);
    }

    fn check_pass(&self, ui: &Ui) {
        assert!(
            self.context == *ui.ctx(),
            "presentation context changed egui context"
        );
        assert_eq!(
            self.viewport,
            ui.ctx().viewport_id(),
            "presentation context changed viewport"
        );
        assert_eq!(
            self.pass,
            ui.ctx().cumulative_pass_nr(),
            "presentation context outlived its pass"
        );
    }

    /// Present one typed capability instance and record its semantics/layout.
    /// Handle the response through the application's intent route; this method
    /// never applies model changes. Use a stable logical key independent of label.
    pub fn action<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: ActionButtonSpec<A>,
    ) -> Response {
        self.action_with_content(ui, key, spec, ActionButtonContent::Text)
    }

    /// Icon-only action with its full action name, description and tooltip.
    /// Records semantics without inventing measured text for decorative artwork.
    ///
    /// ```
    /// use stobie_ui_egui::{
    ///     ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionTarget,
    ///     Availability, DesignTokens, IconId, PresentationContext, PresentationObservations,
    ///     PresentationScope, SemanticUiId,
    /// };
    ///
    /// // A is the application's own ActionKey enum; undo/save are application actions.
    /// fn toolbar<A: ActionKey>(
    ///     ui: &mut egui::Ui, tokens: DesignTokens, font_scale: f32, undo: A, save: A,
    /// ) -> (bool, PresentationObservations) {
    ///     let mut p = PresentationContext::new(
    ///         ui, tokens, font_scale, PresentationScope::new("document-toolbar"),
    ///         SemanticUiId::root(),
    ///     );
    ///     let mut save_clicked = false;
    ///     ui.horizontal(|ui| {
    ///         p.icon_action(ui, "undo", ActionButtonSpec {
    ///             target: ActionTarget::application(undo),
    ///             availability: Availability::Disabled { reason: "History is empty".into() },
    ///             state: ActionButtonState::Momentary,
    ///             emphasis: ActionEmphasis::QuietBorderless,
    ///             compact: false,
    ///         }, IconId::Undo);
    ///         save_clicked = p.action_with_icon(ui, "save", ActionButtonSpec {
    ///             target: ActionTarget::application(save),
    ///             availability: Availability::Enabled,
    ///             state: ActionButtonState::Momentary,
    ///             emphasis: ActionEmphasis::Primary,
    ///             compact: false,
    ///         }, IconId::Save).clicked();
    ///     });
    ///     (save_clicked, p.finish(ui))
    /// }
    /// ```
    pub fn icon_action<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: ActionButtonSpec<A>,
        icon: IconId,
    ) -> Response {
        self.action_with_content(ui, key, spec, ActionButtonContent::IconOnly(icon))
    }

    /// Leading icon and measured label; narrow labels elide with complete semantics.
    pub fn action_with_icon<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: ActionButtonSpec<A>,
        icon: IconId,
    ) -> Response {
        self.action_with_content(ui, key, spec, ActionButtonContent::IconLabel(icon))
    }

    /// Explicit presentation with matching automatic semantic/text observations.
    pub fn action_with_content<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: ActionButtonSpec<A>,
        content: ActionButtonContent,
    ) -> Response {
        self.check_pass(ui);
        let identity = self.scope.instance(key).action_identity();
        self.present_action(ui, &identity, spec, content, None, None)
    }

    /// Present one control of an action repeated per row, carrying
    /// `argument`, such as the row's ID. Its identity combines `key` with the
    /// argument, so each row's control is distinct; its semantic node
    /// publishes the argument; and `name`, such as "Open Ada", is its
    /// accessible name and tooltip while the visible label stays the
    /// action's. Hosted applications pass a [`BoundAction`](crate::BoundAction)
    /// to [`Frame::action`](crate::Frame::action) instead, which also
    /// triggers the action with its argument when clicked.
    pub fn bound_action<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: ActionButtonSpec<A>,
        content: ActionButtonContent,
        argument: &ActionArgument,
        name: Option<&str>,
    ) -> Response {
        self.check_pass(ui);
        let value = argument.to_value();
        let identity = match &value {
            Some(value) => self
                .scope
                .instance((key, "stobie.argument", value.to_string()))
                .action_identity(),
            None => self.scope.instance(key).action_identity(),
        };
        self.present_action(ui, &identity, spec, content, name, value)
    }

    fn present_action<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        identity: &ActionButtonIdentity,
        spec: ActionButtonSpec<A>,
        content: ActionButtonContent,
        name: Option<&str>,
        argument: Option<serde_json::Value>,
    ) -> Response {
        let target = spec.target;
        let state = spec.state;
        let availability = spec.availability.clone();
        let response = crate::components::present_action_button(
            ui,
            spec,
            identity,
            content,
            name,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        let mut node = action_semantic_node_with_identity(
            &response,
            target,
            &availability,
            state,
            self.parent.clone(),
            identity,
        );
        if let Some(name) = name {
            node.name = name.to_owned();
        }
        node.argument = argument;
        if self.domain_reference.is_some() {
            node.domain_reference = self.domain_reference.clone();
        }
        if response.interact_rect.is_positive() {
            self.push_node(ui, node);
        }
        response
    }

    /// Present a destination row with automatic semantic and measured-text ownership.
    /// Supply a stable destination key; handle clicked() through your own intent route.
    /// Hidden rows return an empty hover response without observations.
    ///
    /// ```
    /// use stobie_ui_egui::{ActionKey, ActionScope, ActionSpec, ActionTarget,
    ///     Availability, DesignTokens, IconId, NavigationBadge, NavigationItemSpec,
    ///     PresentationContext, PresentationObservations, PresentationScope, SemanticUiId};
    /// use serde::Serialize;
    ///
    /// #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
    /// enum Destination { Home, Tasks }
    /// impl ActionKey for Destination {
    ///     fn stable_id(self) -> &'static str {
    ///         match self { Self::Home => "app.home", Self::Tasks => "app.tasks" }
    ///     }
    ///     fn specification(self) -> ActionSpec<Self> {
    ///         ActionSpec { id: self, label: match self { Self::Home => "Home", Self::Tasks => "Tasks" },
    ///             description: "Open this destination", compact_label: None,
    ///             shortcut: None, scope: ActionScope::Application }
    ///     }
    /// }
    /// fn sidebar(ui: &mut egui::Ui, tokens: DesignTokens, scale: f32,
    ///     selected: &mut Destination, tasks: u64) -> PresentationObservations {
    ///     let mut p = PresentationContext::new(ui, tokens, scale,
    ///         PresentationScope::new("main-navigation"), SemanticUiId::root());
    ///     let mut intent = None;
    ///     for destination in [Destination::Home, Destination::Tasks] {
    ///         let action = destination.specification();
    ///         let response = p.navigation_item(ui, destination, NavigationItemSpec {
    ///             target: ActionTarget::application(destination),
    ///             icon: match destination { Destination::Home => IconId::Home, Destination::Tasks => IconId::Tasks },
    ///             label: action.label, description: action.description,
    ///             selected: *selected == destination, availability: Availability::Enabled,
    ///             badge: if destination == Destination::Tasks {
    ///                 Some(NavigationBadge::Count { value: tasks, meaning: "outstanding tasks" })
    ///             } else { None },
    ///         });
    ///         if response.clicked() {
    ///             intent = Some(destination);
    ///         }
    ///     }
    ///     let observed = p.finish(ui);
    ///     if let Some(destination) = intent {
    ///         *selected = destination; // Apply the application navigation intent.
    ///         ui.ctx().request_repaint();
    ///     }
    ///     observed
    /// }
    /// ```
    pub fn navigation_item<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: crate::NavigationItemSpec<'_, A>,
    ) -> Response {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        if !spec.availability.visible() {
            return ui.interact(
                egui::Rect::NOTHING,
                identity.egui_id(),
                egui::Sense::hover(),
            );
        }
        let response = crate::navigation_item(
            ui,
            &spec,
            identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if response.interact_rect.is_positive() {
            let mut node = crate::navigation_item_semantic_node(
                &response,
                &spec,
                identity,
                self.parent.clone(),
            );
            if self.domain_reference.is_some() {
                node.domain_reference = self.domain_reference.clone();
            }
            self.push_node(ui, node);
        }
        response
    }

    /// Present one row of a list whose whole surface performs `target` for
    /// that row, carrying `argument`, such as the row's ID, and named by its
    /// title. Its identity combines `key` with the argument. Handle
    /// `clicked()` through the application's action route; hosted
    /// applications use [`Frame::list_row`](crate::Frame::list_row), which
    /// does. Hidden availability returns an empty hover response without
    /// observations.
    pub fn list_row<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: crate::ListRowSpec<'_>,
        target: crate::ActionTarget<A>,
        availability: &crate::Availability,
        argument: &ActionArgument,
    ) -> Response {
        self.check_pass(ui);
        let value = argument.to_value();
        let identity = match &value {
            Some(value) => self
                .scope
                .instance((key, "stobie.argument", value.to_string())),
            None => self.scope.instance(key),
        };
        if !availability.visible() {
            return ui.interact(
                egui::Rect::NOTHING,
                identity.egui_id(),
                egui::Sense::hover(),
            );
        }
        let response = crate::list_row(
            ui,
            &spec,
            target,
            availability,
            identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if response.interact_rect.is_positive() {
            let mut node = crate::list_row_semantic_node(
                &response,
                &spec,
                target,
                availability,
                value,
                identity,
                self.parent.clone(),
            );
            if self.domain_reference.is_some() {
                node.domain_reference = self.domain_reference.clone();
            }
            self.push_node(ui, node);
        }
        response
    }

    pub fn heading(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
    ) -> Response {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let response = crate::pane_content::scoped_section_heading(
            ui,
            identity.text_instance(),
            text,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
            identity.egui_id(),
        );
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(identity.semantic_id().0);
        });
        response
    }

    pub fn content(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
        spec: ContentTextSpec,
    ) -> Response {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let response = crate::pane_content::scoped_content_label(
            ui,
            identity.text_instance(),
            text,
            spec,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(identity.semantic_id().0);
        });
        response
    }

    /// Text of any length, such as a message or a document: measured, wrapped
    /// at the available width and selectable. Unlike [`Self::content`] it has
    /// no line limit and is never truncated; put long text in a scroll area.
    ///
    /// ```
    /// use stobie_ui_egui::{DesignTokens, LinkSpec, PresentationContext,
    ///     PresentationObservations, PresentationScope, ReadingTextSpec, SemanticUiId};
    ///
    /// // A reply of any length, followed by the page it cites.
    /// fn reply(ui: &mut egui::Ui, tokens: DesignTokens, scale: f32, message_id: u64,
    ///          text: &str, source: &str) -> PresentationObservations {
    ///     let mut p = PresentationContext::new(ui, tokens, scale,
    ///         PresentationScope::new("conversation").child(message_id), SemanticUiId::root());
    ///     egui::ScrollArea::vertical().show(ui, |ui| {
    ///         p.reading_text(ui, "body", text, ReadingTextSpec::default());
    ///         // Opens in a new browser tab, so the conversation stays open.
    ///         p.link(ui, "source", LinkSpec::new("Read the source", source));
    ///     });
    ///     p.finish(ui)
    /// }
    /// ```
    pub fn reading_text(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
        spec: crate::ReadingTextSpec,
    ) -> Response {
        self.check_pass(ui);
        crate::reading_text(
            ui,
            self.scope.instance(key),
            text,
            spec,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        )
    }

    /// A link to an external URL, opened in a new browser tab by default,
    /// with its measured label and semantic `Link` node recorded. Hosts offer
    /// the link to agents as the `stobie.open-link` capability.
    pub fn link(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: crate::LinkSpec<'_>,
    ) -> Response {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let response = crate::link(
            ui,
            spec,
            identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if let Some(mut node) =
            crate::link_semantic_node(&response, spec, identity, self.parent.clone())
        {
            node.domain_reference = self.domain_reference.clone();
            self.push_node(ui, node);
        }
        response
    }

    /// Reserve the existing measured line slot for a fixed-height row.
    pub fn fixed_slot(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
        spec: ContentTextSpec,
    ) -> Response {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let response = crate::pane_content::scoped_fixed_slot_label(
            ui,
            identity.text_instance(),
            text,
            spec,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(identity.semantic_id().0);
        });
        response
    }

    /// Use the production responsive property recipe and its selectable value.
    pub fn property_row(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        label: &str,
        value: &str,
    ) {
        self.check_pass(ui);
        // The existing recipe derives two child IDs as root * 2 and root * 2 + 1.
        // Reserve one bit so every emitted ID remains exactly representable in JS.
        let instance = self.scope.instance(key).text_instance() >> 1;
        crate::property_row(
            ui,
            instance,
            label,
            value,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
    }

    /// Use the production status recipe, including its measured selectable text.
    pub fn badge(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
        tone: crate::StatusTone,
    ) {
        self.check_pass(ui);
        crate::status_badge(
            ui,
            self.scope.instance(key).text_instance(),
            text,
            tone,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
    }

    /// Compact, informational status with automatic measured text and one
    /// matching semantic owner. The consumer chooses label, meaning and tone.
    /// Default text is selectable; choose Inert inside a pointer-owned row.
    ///
    /// ```
    /// use stobie_ui_egui::{DesignTokens, IconId, PresentationContext,
    ///     PresentationObservations, PresentationScope, SemanticUiId,
    ///     StatusChipSpec, StatusTone};
    ///
    /// // Workflow states and this mapping belong to the application.
    /// enum TaskState { Active, Scheduled, NeedsReview }
    /// fn task_status(ui: &mut egui::Ui, tokens: DesignTokens, scale: f32,
    ///                task_id: u64, state: TaskState) -> PresentationObservations {
    ///     let (label, tone, icon) = match state {
    ///         TaskState::Active => ("Active", StatusTone::Neutral, IconId::Clock),
    ///         TaskState::Scheduled => ("Scheduled", StatusTone::Neutral, IconId::Calendar),
    ///         TaskState::NeedsReview => ("Needs review", StatusTone::Warning, IconId::Warning),
    ///     };
    ///     let mut p = PresentationContext::new(ui, tokens, scale,
    ///         PresentationScope::new("task-detail").child(task_id), SemanticUiId::root());
    ///     p.status_chip(ui, "status", StatusChipSpec {
    ///         icon: Some(icon), ..StatusChipSpec::new(label, tone)
    ///     });
    ///     p.finish(ui)
    /// }
    /// ```
    pub fn status_chip(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: crate::StatusChipSpec<'_>,
    ) -> crate::StatusChipResponse {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let chip = crate::status_chip(
            ui,
            spec,
            identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if let Some(node) = crate::status_chip_semantic_node(
            &chip,
            spec,
            identity,
            self.parent.clone(),
            self.domain_reference.clone(),
        ) {
            self.push_node(ui, node);
        }
        chip
    }

    /// A status or error message with a semantic `status` node named by its
    /// full text, so inspection clients can wait for it. Use it for an
    /// application's status line; [`Self::content`] publishes no node.
    pub fn status(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
        error: bool,
    ) -> Response {
        let identity = self.scope.instance(&key);
        let spec = ContentTextSpec {
            role: if error {
                TextRole::Error
            } else {
                TextRole::Status
            },
            overflow: TextOverflow::Wrap,
            max_lines: 3,
            interaction: TextInteraction::Selectable,
        };
        let response = self.content(ui, key, text, spec);
        // A polite live region, so assistive technology announces changes.
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_role(egui::accesskit::Role::Status);
            node.set_label(text);
            node.clear_value();
            node.set_live(egui::accesskit::Live::Polite);
        });
        if response.rect.is_positive() {
            let mut node = UiNode::container(
                identity.semantic_id(),
                Some(self.parent.clone()),
                crate::UiRole::Status,
                response.rect.into(),
            );
            node.name = text.to_owned();
            node.text_selectable = true;
            node.domain_reference = self.domain_reference.clone();
            self.push_node(ui, node);
        }
        response
    }

    /// A native control remains explicitly unmeasured in the text denominator.
    pub fn native<R>(
        &mut self,
        ui: &mut Ui,
        kind: NativeTextControlKind,
        render: impl FnOnce(&mut Ui) -> (Response, R),
    ) -> (Response, R) {
        self.check_pass(ui);
        let result = render(ui);
        record_native_text_control(&result.0, kind);
        result
    }

    /// Annotate raw presentation, not ordinary layout such as rows or scrolling.
    /// The closure still owns any native-control recording or measured evidence.
    pub fn raw<R>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        reason: &'static str,
        render: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        self.check_pass(ui);
        assert!(
            !reason.trim().is_empty(),
            "raw presentation requires a reason"
        );
        self.raw_presentations.push(RawPresentation {
            id: self.scope.instance(key).semantic_id(),
            reason,
        });
        render(ui)
    }

    /// Geometry filters may remove clipped successes, but never failed attempts.
    pub fn retain_visible_text(
        &mut self,
        ui: &mut Ui,
        mut visible: impl FnMut(&TextLayoutObservation) -> bool,
    ) {
        self.check_pass(ui);
        self.text_layouts
            .retain(|item| item.layout_error.is_some() || visible(item));
    }

    /// Publish and consume this context in its original pass. Panics if used
    /// with another context, viewport or pass. Keep only the latest publication
    /// after layout retries; see [`PresentationObservations`] for coverage merging.
    pub fn finish(mut self, ui: &mut Ui) -> PresentationObservations {
        self.check_pass(ui);
        let text_layouts = std::mem::take(&mut self.text_layouts);
        let semantic_nodes = std::mem::take(&mut self.semantic_nodes);
        if crate::observation::is_enabled(&self.context, self.viewport) {
            self.register(semantic_nodes.clone(), text_layouts.clone());
        }
        self.node_clips.clear();
        let coverage = text_audit_coverage(ui.ctx(), &text_layouts);
        PresentationObservations {
            viewport: self.viewport,
            pass: self.pass,
            text_layouts,
            semantic_nodes,
            coverage,
            raw_presentations: std::mem::take(&mut self.raw_presentations),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionButtonState, ActionEmphasis, ActionTarget, Availability, DensityVariant,
        ThemeVariant, UiRole, UiSnapshot, audit_accesskit, audit_text_layouts,
        test_actions::TestAction,
    };

    fn tokens() -> DesignTokens {
        DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
    }

    fn action() -> ActionButtonSpec<TestAction> {
        ActionButtonSpec {
            target: ActionTarget::application(TestAction::Undo),
            availability: Availability::Enabled,
            state: ActionButtonState::Momentary,
            emphasis: ActionEmphasis::Normal,
            compact: false,
        }
    }

    fn content(max_lines: u8) -> ContentTextSpec {
        ContentTextSpec {
            role: TextRole::Body,
            overflow: TextOverflow::Wrap,
            max_lines,
            interaction: TextInteraction::Selectable,
        }
    }

    #[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
    enum Control {
        HeaderUndo,
        FooterUndo,
    }

    #[test]
    fn repeated_capabilities_keep_identity_across_reordering_and_incidental_layout() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let root = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 300.0));
        let scope = PresentationScope::new("detail").child("obligation-17");
        let domain = DomainReference::External {
            namespace: "test.obligation".into(),
            id: "obligation-17".into(),
        };
        let mut previous = Vec::new();
        for (layout, controls) in [
            ("desktop", [Control::HeaderUndo, Control::FooterUndo]),
            ("narrow", [Control::FooterUndo, Control::HeaderUndo]),
        ] {
            let mut published = None;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(root),
                    ..Default::default()
                },
                |ui| {
                    let mut presentation =
                        PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root())
                            .with_domain_reference(domain.clone());
                    ui.push_id(layout, |ui| {
                        ui.horizontal(|ui| {
                            for control in controls {
                                let response = presentation.action(ui, control, action());
                                assert_eq!(response.id, scope.instance(control).egui_id());
                            }
                        });
                    });
                    published = Some(presentation.finish(ui));
                },
            );
            let published = published.unwrap();
            assert_eq!(published.semantic_nodes.len(), 2);
            assert_ne!(
                published.semantic_nodes[0].id,
                published.semantic_nodes[1].id
            );
            assert_ne!(
                published.text_layouts[0].component_id,
                published.text_layouts[1].component_id
            );
            assert!(
                published
                    .semantic_nodes
                    .iter()
                    .all(|node| node.domain_reference == Some(domain.clone()))
            );
            let ids: Vec<_> = published
                .semantic_nodes
                .iter()
                .map(|node| node.id.clone())
                .collect();
            if !previous.is_empty() {
                assert_eq!(ids, previous.into_iter().rev().collect::<Vec<_>>());
            }
            previous = ids;
            let mut snapshot = UiSnapshot::default();
            snapshot.nodes.push(UiNode::container(
                SemanticUiId::root(),
                None,
                UiRole::Application,
                root.into(),
            ));
            snapshot.nodes.extend(published.semantic_nodes);
            let update = output.platform_output.accesskit_update.take().unwrap();
            assert!(audit_accesskit(&snapshot, &update).is_empty());
            output.textures_delta.clear();
        }
        assert_ne!(
            scope.instance(Control::HeaderUndo),
            PresentationScope::new("detail")
                .child("obligation-18")
                .instance(Control::HeaderUndo),
        );
    }

    #[test]
    fn text_responses_and_author_ids_survive_layout_changes_without_changing_paint() {
        let mut paints = Vec::new();
        let scope = PresentationScope::new("text-identity");
        for scoped in [false, true] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            context.enable_accesskit();
            let mut previous_ids = None;
            for layout in ["wide", "narrow"] {
                let mut output = context.run_ui(Default::default(), |ui| {
                    ui.push_id(layout, |ui| {
                        if scoped {
                            let mut presentation = PresentationContext::new(
                                ui,
                                tokens(),
                                1.0,
                                scope,
                                SemanticUiId::root(),
                            );
                            let heading = presentation.heading(ui, "heading", "Current obligation");
                            let content = presentation.content(
                                ui,
                                "content",
                                "Supporting evidence",
                                content(2),
                            );
                            let ids = (heading.id, content.id);
                            if let Some(previous) = previous_ids {
                                assert_eq!(ids, previous);
                            }
                            previous_ids = Some(ids);
                            let _ = presentation.finish(ui);
                        } else {
                            crate::section_heading(
                                ui,
                                scope.instance("heading").text_instance(),
                                "Current obligation",
                                &tokens(),
                                1.0,
                                &mut Vec::new(),
                            );
                            crate::measured_content_label(
                                ui,
                                scope.instance("content").text_instance(),
                                "Supporting evidence",
                                TextRole::Body,
                                TextOverflow::Wrap,
                                2,
                                TextInteraction::Selectable,
                                &tokens(),
                                1.0,
                                &mut Vec::new(),
                            );
                        }
                    });
                });
                if scoped {
                    let update = output.platform_output.accesskit_update.take().unwrap();
                    for key in ["heading", "content"] {
                        let author_id = scope.instance(key).semantic_id().0;
                        assert_eq!(
                            update
                                .nodes
                                .iter()
                                .filter(|(_, node)| node.author_id() == Some(author_id.as_str()))
                                .count(),
                            1
                        );
                    }
                }
                output.textures_delta.clear();
                paints.push(output.shapes);
            }
        }
        assert_eq!(paints[0], paints[2]);
        assert_eq!(paints[1], paints[3]);
    }

    #[test]
    fn repeated_rows_and_properties_keep_text_identity_when_reordered() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let scope = PresentationScope::new("rows");
        let mut previous = None;
        for (layout, rows) in [
            ("wide", ["object-a", "object-b"]),
            ("narrow", ["object-b", "object-a"]),
        ] {
            let mut identities = std::collections::BTreeMap::new();
            let mut output = context.run_ui(Default::default(), |ui| {
                let mut presentation =
                    PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root());
                ui.push_id(layout, |ui| {
                    for row in rows {
                        presentation.scoped(ui, row, |ui, presentation| {
                            // Equal labels cannot supply the repeated object's identity.
                            let response =
                                presentation.fixed_slot(ui, "title", "Same title", content(2));
                            identities.insert((row, "response"), response.id.value());
                            presentation.property_row(ui, "owner", "Owner", "Same owner");
                            presentation.property_row(ui, "state", "State", "Same state");
                        });
                    }
                });
                let publication = presentation.finish(ui);
                assert_eq!(publication.text_layouts.len(), 10);
                assert_eq!(publication.coverage.attempted_components, 10);
                assert_eq!(publication.coverage.measured_components, 10);
                let components: std::collections::HashSet<_> = publication
                    .text_layouts
                    .iter()
                    .map(|item| item.component_id)
                    .collect();
                assert_eq!(components.len(), 10);
                assert!(publication.text_layouts.iter().all(|item| {
                    item.component_id.instance < (1_u64 << 53)
                        && item
                            .parent_id
                            .is_none_or(|parent| parent.instance < (1_u64 << 53))
                }));
                for (row, observations) in rows
                    .into_iter()
                    .zip(publication.text_layouts.as_chunks::<5>().0)
                {
                    for (key, observation) in [
                        "title",
                        "owner-label",
                        "owner-value",
                        "state-label",
                        "state-value",
                    ]
                    .into_iter()
                    .zip(observations)
                    {
                        identities.insert((row, key), observation.component_id.instance);
                    }
                }
            });
            let update = output.platform_output.accesskit_update.take().unwrap();
            for row in rows {
                let author = scope.child(row).instance("title").semantic_id().0;
                assert_eq!(
                    update
                        .nodes
                        .iter()
                        .filter(|(_, node)| node.author_id() == Some(author.as_str()))
                        .count(),
                    1
                );
            }
            if let Some(previous) = previous {
                assert_eq!(identities, previous);
            }
            previous = Some(identities);
            output.textures_delta.clear();
        }
    }

    #[test]
    fn fixed_slots_properties_and_badges_preserve_existing_recipe_paint() {
        let scope = PresentationScope::new("recipe-equivalence");
        let mut rendered = Vec::new();
        for width in [280.0, 640.0] {
            for scoped in [false, true] {
                let context = egui::Context::default();
                crate::install_typography_fonts(&context);
                let mut layouts = Vec::new();
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 600.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        if scoped {
                            let mut presentation = PresentationContext::new(
                                ui,
                                tokens(),
                                1.0,
                                scope,
                                SemanticUiId::root(),
                            );
                            presentation.fixed_slot(ui, "slot", "A bounded row title", content(2));
                            presentation.property_row(
                                ui,
                                "property",
                                "Owner",
                                "A long property value that wraps in a narrow pane",
                            );
                            presentation.badge(
                                ui,
                                "badge",
                                "Unable to retrieve evidence",
                                crate::StatusTone::Error,
                            );
                            layouts = presentation.finish(ui).text_layouts;
                        } else {
                            crate::measured_fixed_slot_label(
                                ui,
                                scope.instance("slot").text_instance(),
                                "A bounded row title",
                                TextRole::Body,
                                TextOverflow::Wrap,
                                2,
                                TextInteraction::Selectable,
                                &tokens(),
                                1.0,
                                &mut layouts,
                            );
                            crate::property_row(
                                ui,
                                scope.instance("property").text_instance() >> 1,
                                "Owner",
                                "A long property value that wraps in a narrow pane",
                                &tokens(),
                                1.0,
                                &mut layouts,
                            );
                            crate::status_badge(
                                ui,
                                scope.instance("badge").text_instance(),
                                "Unable to retrieve evidence",
                                crate::StatusTone::Error,
                                &tokens(),
                                1.0,
                                &mut layouts,
                            );
                        }
                    },
                );
                assert!(audit_text_layouts(&layouts).is_empty());
                output.textures_delta.clear();
                rendered.push((output.shapes, layouts));
            }
        }
        assert_eq!(rendered[0], rendered[1]);
        assert_eq!(rendered[2], rendered[3]);
    }

    #[test]
    fn scoped_raw_rows_preserve_metadata_and_do_not_claim_native_text_measurement() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let domain = DomainReference::External {
            namespace: "test.obligation".into(),
            id: "object-a".into(),
        };
        let scope = PresentationScope::new("evidence");
        let mut output = context.run_ui(Default::default(), |ui| {
            let mut presentation =
                PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root())
                    .with_domain_reference(domain.clone());
            let result = presentation.scoped(ui, "object-a", |ui, child| {
                assert_eq!(child.tokens().spacing.inline, tokens().spacing.inline);
                assert_eq!(child.font_scale(), 1.0);
                child.action(ui, "undo", action());
                child.fixed_slot(ui, "bad-slot", "Invalid line limit", content(24));
                let response = child.raw(ui, "reader", "Native selectable evidence reader", |ui| {
                    ui.label("Evidence")
                });
                let mut node = UiNode::container(
                    SemanticUiId::new("app.evidence.object-a"),
                    Some(SemanticUiId::root()),
                    UiRole::Section,
                    response.rect.into(),
                );
                node.domain_reference = Some(domain.clone());
                child.observe_node(ui, node);
                child.native(ui, NativeTextControlKind::Button, |ui| {
                    (ui.button("Native option"), ())
                });
                17
            });
            assert_eq!(result, 17);
            // Leaving the child must restore the outer logical scope.
            let response = presentation.action(ui, "undo", action());
            assert_eq!(response.id, scope.instance("undo").egui_id());
            presentation.retain_visible_text(ui, |_| false);
            let publication = presentation.finish(ui);
            assert_eq!(publication.text_layouts.len(), 1);
            assert_eq!(publication.coverage.attempted_components, 3);
            assert_eq!(publication.coverage.failed_components, 1);
            assert_eq!(publication.coverage.native_text_controls, 1);
            assert_eq!(publication.coverage.observed_native_controls, 0);
            assert_eq!(publication.raw_presentations.len(), 1);
            assert_eq!(
                publication.raw_presentations[0].id,
                scope.child("object-a").instance("reader").semantic_id()
            );
            assert_eq!(publication.semantic_nodes.len(), 3);
            assert_eq!(
                publication.semantic_nodes[1].id,
                SemanticUiId::new("app.evidence.object-a")
            );
            assert!(
                publication
                    .semantic_nodes
                    .iter()
                    .all(|node| node.domain_reference == Some(domain.clone()))
            );
        });
        output.textures_delta.clear();
    }

    #[test]
    fn geometry_filter_preserves_failed_attempts_and_native_controls_stay_unmeasured() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut output = context.run_ui(Default::default(), |ui| {
            let mut presentation = PresentationContext::new(
                ui,
                tokens(),
                1.0,
                PresentationScope::new("coverage"),
                SemanticUiId::root(),
            );
            presentation.heading(ui, "title", "Current obligation");
            presentation.content(ui, "valid", "Available evidence", content(2));
            presentation.content(ui, "invalid", "Bad request", content(24));
            presentation.native(ui, NativeTextControlKind::Button, |ui| {
                (ui.button("Native"), ())
            });
            presentation.raw(ui, "reader", "Native selectable evidence reader", |ui| {
                ui.label("Unmeasured evidence");
            });
            presentation.retain_visible_text(ui, |_| false);
            let publication = presentation.finish(ui);
            assert_eq!(publication.text_layouts.len(), 1);
            assert_eq!(publication.coverage.attempted_components, 3);
            assert_eq!(publication.coverage.successful_components, 2);
            assert_eq!(publication.coverage.failed_components, 1);
            assert_eq!(publication.coverage.native_text_controls, 1);
            assert_eq!(publication.coverage.observed_native_controls, 0);
            assert_eq!(publication.raw_presentations.len(), 1);
            assert!(!audit_text_layouts(&publication.text_layouts).is_empty());
        });
        output.textures_delta.clear();
    }

    #[test]
    fn multiple_contexts_merge_layouts_and_recompute_coverage_without_summing_inventories() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context
            .run_ui(Default::default(), |ui| {
                let mut first = PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("first"),
                    SemanticUiId::root(),
                );
                first.content(ui, "text", "First region", content(1));
                first.native(ui, NativeTextControlKind::Button, |ui| {
                    (ui.button("Native"), ())
                });
                let mut first = first.finish(ui);
                let mut second = PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("second"),
                    SemanticUiId::root(),
                );
                second.content(ui, "text", "Second region", content(1));
                let second = second.finish(ui);
                assert_eq!(second.coverage.measured_components, 1);
                assert_eq!(second.coverage.attempted_components, 2);
                assert_eq!(second.coverage.native_text_controls, 1);
                first.text_layouts.extend(second.text_layouts);
                let combined = text_audit_coverage(ui.ctx(), &first.text_layouts);
                assert_eq!(combined.measured_components, 2);
                assert_eq!(combined.attempted_components, 2);
                assert_eq!(combined.native_text_controls, 1);
            })
            .textures_delta
            .clear();
    }

    #[test]
    fn repeated_passes_replace_publication_and_reset_the_inventory() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let mut publication = None;
        let mut pass_count = 0;
        let mut output = context.run_ui(Default::default(), |ui| {
            pass_count += 1;
            let mut presentation = PresentationContext::new(
                ui,
                tokens(),
                1.0,
                PresentationScope::new("retry"),
                SemanticUiId::root(),
            );
            if pass_count == 1 {
                presentation.content(ui, "provisional", "First pass", content(24));
                presentation.native(ui, NativeTextControlKind::Button, |ui| {
                    (ui.button("Provisional"), ())
                });
                ui.ctx().request_discard("exercise a repeated layout pass");
            } else {
                presentation.content(ui, "final", "Final pass", content(2));
            }
            publication = Some(presentation.finish(ui));
        });
        output.textures_delta.clear();
        assert_eq!(pass_count, 2);
        let publication = publication.unwrap();
        assert_eq!(publication.pass, 1);
        assert_eq!(publication.text_layouts.len(), 1);
        assert_eq!(publication.coverage.attempted_components, 1);
        assert_eq!(publication.coverage.failed_components, 0);
        assert_eq!(publication.coverage.native_text_controls, 0);
    }

    #[test]
    fn viewport_inventories_and_publications_are_independent() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let secondary = egui::ViewportId::from_hash_of("secondary");
        for (viewport, count) in [(egui::ViewportId::ROOT, 2), (secondary, 1)] {
            let mut output = context.run_ui(
                egui::RawInput {
                    viewport_id: viewport,
                    viewports: [(viewport, egui::ViewportInfo::default())]
                        .into_iter()
                        .collect(),
                    ..Default::default()
                },
                |ui| {
                    assert_eq!(text_audit_coverage(ui.ctx(), &[]).attempted_components, 0);
                    let mut presentation = PresentationContext::new(
                        ui,
                        tokens(),
                        1.0,
                        PresentationScope::new("viewport"),
                        SemanticUiId::root(),
                    );
                    presentation.content(ui, "title", "Viewport", content(1));
                    if count == 2 {
                        presentation.content(ui, "detail", "Root detail", content(1));
                    }
                    let publication = presentation.finish(ui);
                    assert_eq!(publication.viewport, viewport);
                    assert_eq!(publication.coverage.attempted_components, count);
                },
            );
            output.textures_delta.clear();
        }
    }

    #[test]
    fn publishing_a_context_from_an_earlier_pass_is_rejected() {
        let context = egui::Context::default();
        let mut stale = None;
        context
            .run_ui(Default::default(), |ui| {
                stale = Some(PresentationContext::new(
                    ui,
                    tokens(),
                    1.0,
                    PresentationScope::new("stale"),
                    SemanticUiId::root(),
                ));
            })
            .textures_delta
            .clear();
        context
            .run_ui(Default::default(), |ui| {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    stale.take().unwrap().finish(ui)
                }));
                assert!(result.is_err());
            })
            .textures_delta
            .clear();
    }

    #[test]
    fn scoped_actions_preserve_recipe_geometry_and_painted_text() {
        let mut rendered = Vec::new();
        for scoped in [false, true] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let mut bounds = None;
            let mut output = context.run_ui(Default::default(), |ui| {
                bounds = Some(if scoped {
                    let mut presentation = PresentationContext::new(
                        ui,
                        tokens(),
                        1.0,
                        PresentationScope::new("equivalence"),
                        SemanticUiId::root(),
                    );
                    presentation.action(ui, "undo", action()).rect
                } else {
                    crate::action_button(ui, action(), &tokens(), 1.0, &mut Vec::new()).rect
                });
            });
            output.textures_delta.clear();
            rendered.push((bounds, output.shapes));
        }
        assert_eq!(rendered[0], rendered[1]);
    }
}
