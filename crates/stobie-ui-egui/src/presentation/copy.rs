//! The copy recipe: a button that copies a given text to the clipboard and
//! confirms it.

use std::hash::{Hash, Hasher};
use std::time::Duration;

use egui::{Response, Ui};

use super::PresentationContext;
use crate::clipboard::{self, CopyState};
use crate::components::{ButtonFace, present_button};
use crate::{
    ActionButtonContent, ActionButtonState, ActionEmphasis, Availability, IconId, UiNode, UiRole,
};

/// How long a copy control shows its confirmation before its label returns.
pub const COPY_CONFIRMATION: Duration = Duration::from_secs(2);

/// How long a copy control shows its failure before its label returns.
pub const COPY_FAILURE: Duration = Duration::from_secs(5);

/// How a copy control shows: its icon, its label, or both. The icon is
/// Copy, then Check while confirming and Warning after a failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CopyContent {
    /// The icon alone; the label names the control and is its tooltip.
    Icon,
    /// The label alone.
    Label,
    /// The icon followed by the label.
    #[default]
    IconLabel,
}

/// A copy control: the text it copies and the words it shows. The
/// application supplies every word, so it owns their language.
#[derive(Clone, Debug)]
pub struct CopySpec<'a> {
    /// The text copied, exactly as given. It is never published to
    /// observation.
    pub text: &'a str,
    /// The control's name and visible label, such as "Copy code".
    pub label: &'a str,
    /// An optional description for assistive technology and the tooltip.
    pub description: Option<&'a str>,
    /// Shown and announced once the text is copied, such as "Copied".
    pub confirmation: &'a str,
    /// Shown and announced when the platform refuses the write, such as
    /// "Couldn't copy".
    pub failure: &'a str,
    pub content: CopyContent,
    pub emphasis: ActionEmphasis,
    /// Enabled, disabled with a reason, or hidden (not drawn).
    pub availability: Availability,
}

impl<'a> CopySpec<'a> {
    /// An enabled, quiet control showing its icon and label.
    pub fn new(text: &'a str, label: &'a str, confirmation: &'a str, failure: &'a str) -> Self {
        Self {
            text,
            label,
            description: None,
            confirmation,
            failure,
            content: CopyContent::IconLabel,
            emphasis: ActionEmphasis::Quiet,
            availability: Availability::Enabled,
        }
    }
}

/// A presented copy control: its button's response and where it is in
/// copying. `state` is the state shown in this pass.
#[derive(Clone, Debug)]
pub struct CopyResponse {
    pub response: Response,
    pub state: CopyState,
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

impl PresentationContext {
    /// A button that copies `spec.text` to the clipboard when pressed,
    /// tapped, activated from the keyboard or by assistive technology, or
    /// invoked by an agent (`stobie.copy-text`), then shows and announces
    /// its confirmation for [`COPY_CONFIRMATION`], or its failure for
    /// [`COPY_FAILURE`]. Copying changes no application state, so it needs
    /// no action or intent.
    ///
    /// In the web host a tap or click writes in the pass that handles it,
    /// inside the browser's `pointerup` handler, and Enter, Space or an
    /// activation through the ARIA layer (such as VoiceOver's double tap)
    /// writes inside its own event, so each copies on iOS too; the browser's
    /// answer decides between confirmation and failure. An agent's
    /// invocation writes on the next pass, outside any gesture, which a
    /// browser may refuse.
    /// Natively the platform clipboard takes the text through egui, and the
    /// control confirms. The text is never published: observation shows the
    /// control as a button named by its label, and while confirming or
    /// failing, a polite status named by the confirmation or failure.
    ///
    /// ```
    /// use stobie_ui_egui::{CopySpec, DesignTokens, PresentationContext,
    ///     PresentationObservations, PresentationScope, SemanticUiId};
    ///
    /// fn invitation(ui: &mut egui::Ui, tokens: DesignTokens, scale: f32,
    ///               code: &str) -> PresentationObservations {
    ///     let mut p = PresentationContext::new(ui, tokens, scale,
    ///         PresentationScope::new("invitation"), SemanticUiId::root());
    ///     p.copy(ui, "copy-code",
    ///         &CopySpec::new(code, "Copy code", "Copied", "Couldn't copy"));
    ///     p.finish(ui)
    /// }
    /// ```
    pub fn copy(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &CopySpec<'_>,
    ) -> CopyResponse {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let widget = identity.egui_id();
        if !spec.availability.visible() {
            // Hidden: no space, paint, focus, semantic node or binding.
            return CopyResponse {
                response: ui.interact(egui::Rect::NOTHING, widget, egui::Sense::hover()),
                state: CopyState::Ready,
            };
        }
        let context = ui.ctx().clone();
        let now = ui.input(|input| input.time);
        let hash = text_hash(spec.text);
        // Settle the state shown: new text starts afresh, and an outcome
        // returns to the label once its time has passed.
        let (state, revision, requested, gestured, remaining) =
            clipboard::update(&context, widget, |control| {
                if control.text_hash != Some(hash) {
                    control.text_hash = Some(hash);
                    control.revision += 1;
                    control.state = CopyState::Ready;
                    control.since = None;
                }
                let shown = match control.state {
                    CopyState::Copied => Some(COPY_CONFIRMATION),
                    CopyState::Failed => Some(COPY_FAILURE),
                    CopyState::Ready | CopyState::Pending => None,
                };
                let mut remaining = None;
                if let Some(shown) = shown {
                    let since = *control.since.get_or_insert(now);
                    let left = shown.as_secs_f64() - (now - since);
                    if left > 0.0 {
                        remaining = Some(Duration::from_secs_f64(left));
                    } else {
                        control.state = CopyState::Ready;
                        control.since = None;
                    }
                }
                (
                    control.state,
                    control.revision,
                    std::mem::take(&mut control.requested),
                    std::mem::take(&mut control.gestured),
                    remaining,
                )
            });
        if let Some(remaining) = remaining {
            context.request_repaint_after(remaining);
        }
        let (icon, visible_label, message) = match state {
            CopyState::Copied => (IconId::Check, spec.confirmation, spec.confirmation),
            CopyState::Failed => (IconId::Warning, spec.failure, spec.failure),
            CopyState::Ready | CopyState::Pending => (IconId::Copy, spec.label, ""),
        };
        let content = match spec.content {
            CopyContent::Icon => ActionButtonContent::IconOnly(icon),
            CopyContent::Label => ActionButtonContent::Text,
            CopyContent::IconLabel => ActionButtonContent::IconLabel(icon),
        };
        let action_identity = identity.action_identity();
        let response = present_button(
            ui,
            &ButtonFace {
                name: spec.label,
                visible_label,
                description: spec.description,
                shortcut: None,
                availability: &spec.availability,
                state: ActionButtonState::Momentary,
                emphasis: spec.emphasis,
                content,
                also_fits: &[spec.label, spec.confirmation, spec.failure],
                tooltip: false,
                tab_stop: true,
            },
            &action_identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        let enabled = spec.availability.enabled();
        // A press the web host already wrote for, inside its event, does not
        // write again.
        if enabled && ((response.clicked() && !gestured) || requested) {
            clipboard::write(&context, widget, spec.text);
            // Show the outcome, which a write without the web route already has.
            context.request_repaint();
        }
        if enabled {
            clipboard::present(&context, response.id, widget, spec.text);
        }
        let rect = response.interact_rect;
        if !rect.is_positive() {
            return CopyResponse { response, state };
        }
        let mut node = UiNode::container(
            identity.semantic_id(),
            Some(self.parent.clone()),
            UiRole::Button,
            rect.into(),
        );
        node.name = spec.label.to_owned();
        node.description = spec.description.map(str::to_owned);
        node.enabled = enabled;
        node.focused = response.has_focus();
        node.disabled_reason = spec.availability.disabled_reason().map(ToOwned::to_owned);
        node.domain_reference = self.domain_reference.clone();
        self.push_node(ui, node);
        crate::observation::record_copy(
            &context,
            crate::observation::CopyRegistration {
                id: identity.semantic_id(),
                widget,
                revision,
            },
        );
        // A polite live region that is present, empty, before any outcome,
        // so assistive technology announces the outcome when it appears.
        let status = identity.child("status");
        context.accesskit_node_builder(status.egui_id(), |accesskit| {
            accesskit.set_role(egui::accesskit::Role::Status);
            accesskit.set_label(message);
            accesskit.set_live(egui::accesskit::Live::Polite);
            accesskit.set_author_id(status.semantic_id().0);
            accesskit.set_bounds(egui::accesskit::Rect {
                x0: f64::from(rect.min.x),
                y0: f64::from(rect.min.y),
                x1: f64::from(rect.max.x),
                y1: f64::from(rect.max.y),
            });
        });
        if !message.is_empty() {
            let mut node = UiNode::container(
                status.semantic_id(),
                Some(self.parent.clone()),
                UiRole::Status,
                rect.into(),
            );
            node.name = message.to_owned();
            node.domain_reference = self.domain_reference.clone();
            self.push_node(ui, node);
        }
        CopyResponse { response, state }
    }
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
