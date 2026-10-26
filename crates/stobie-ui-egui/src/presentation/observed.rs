//! The observed route for plain egui widgets: one call presents a widget
//! and publishes its semantic node from the widget's response.

use egui::{Response, Ui};

use super::PresentationContext;
use crate::{
    NativeTextControlKind, SemanticUiId, UiNode, UiRole, record_native_text_control,
    semantics::accesskit_role,
};

impl PresentationContext {
    /// Present a plain egui widget, such as `ui.checkbox`, a `ProgressBar`,
    /// an `Image` or a table inside `ui.scope`, and observe it as `role`
    /// named `name`, with the semantic ID `id`.
    ///
    /// `render` presents the widget and returns its response. The node is
    /// built from that response: its rectangle, whether it is enabled and
    /// focused, and, for a toggle such as a checkbox, whether it is checked.
    /// Its parent, pane and domain reference are this context's. The
    /// widget's AccessKit node takes the same ID, name and role, so the
    /// inspection snapshot, assistive technology and the browser's ARIA
    /// layer agree, and a text input observed this way accepts
    /// `stobie set-text`.
    ///
    /// Derive `id` from stable pane and domain IDs, as for any semantic
    /// node. Prefer a recipe where one exists; for a custom widget whose
    /// node needs more than its response says, build the [`UiNode`] and
    /// pass it to [`Self::observe_node`].
    pub fn observed(
        &mut self,
        ui: &mut Ui,
        id: SemanticUiId,
        role: UiRole,
        name: &str,
        render: impl FnOnce(&mut Ui) -> Response,
    ) -> Response {
        self.check_pass(ui);
        let response = render(ui);
        // Egui lays out a native control's text itself, outside the measured
        // text audit; a text edit recorded here pairs for set text.
        let native = match role {
            UiRole::TextInput | UiRole::MultilineTextInput => Some(NativeTextControlKind::TextEdit),
            UiRole::Button => Some(NativeTextControlKind::Button),
            UiRole::RadioButton => Some(NativeTextControlKind::RadioButton),
            UiRole::ComboBox => Some(NativeTextControlKind::ComboBox),
            UiRole::Slider => Some(NativeTextControlKind::Slider),
            _ => None,
        };
        if let Some(kind) = native {
            record_native_text_control(&response, kind);
        }
        // Egui records a toggle's state on its AccessKit node, which
        // inspection always enables.
        let toggled = ui
            .ctx()
            .accesskit_node_builder(response.id, |node| {
                if let Some(role) = accesskit_role(role) {
                    node.set_role(role);
                }
                node.set_label(name);
                node.set_author_id(id.0.clone());
                // The part a scroll area shows.
                let shown = response.interact_rect;
                if shown.is_positive() {
                    node.set_bounds(egui::accesskit::Rect {
                        x0: f64::from(shown.min.x),
                        y0: f64::from(shown.min.y),
                        x1: f64::from(shown.max.x),
                        y1: f64::from(shown.max.y),
                    });
                }
                node.toggled()
            })
            .flatten();
        let mut node = UiNode::container(id, Some(self.parent.clone()), role, response.rect.into());
        node.name = name.to_owned();
        node.enabled = response.enabled();
        node.focused = response.has_focus();
        node.checked = match toggled {
            Some(egui::accesskit::Toggled::True) => Some(true),
            Some(egui::accesskit::Toggled::False) => Some(false),
            _ => None,
        };
        node.domain_reference = self.domain_reference.clone();
        if response.rect.is_positive() {
            self.push_node(ui, node);
        }
        response
    }
}
