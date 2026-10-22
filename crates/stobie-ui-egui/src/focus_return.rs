//! Returning keyboard focus to a control that may have gone.
//!
//! Back and a closing dialog return focus to the control that opened them.
//! That control can be gone by then: a row that left its list, or one the
//! dialog's own confirmation removed. Asking egui to focus a widget this
//! pass did not present makes AccessKit name a focus outside its tree, so
//! focus returns to the opener only while it is presented, and otherwise to
//! the presented control nearest where it was.

use egui::{Context, Id, LayerId, Rect};

/// A control that focus may return to, as it was last presented.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FocusReturn {
    id: Id,
    layer: LayerId,
    rect: Rect,
}

impl FocusReturn {
    /// Remember `id` where it was last presented, if it was.
    pub(crate) fn remember(context: &Context, id: Id) -> Option<Self> {
        context.read_response(id).map(|response| Self {
            id,
            layer: response.layer_id,
            rect: response.rect,
        })
    }

    /// Where focus returns now: the remembered control while this pass
    /// presents it, otherwise the presented control on its layer whose
    /// centre is nearest where its centre was and lies in `within`, or
    /// nowhere when there is none. A row that left a list therefore hands
    /// focus to the row that took its place, or to the row before it when
    /// it was the last.
    pub(crate) fn target(&self, context: &Context, within: Rect) -> Option<Id> {
        if presented(context, self.id) {
            return Some(self.id);
        }
        let centre = self.rect.center();
        context.viewport(|viewport| {
            let tree = viewport.this_pass.accesskit_state.as_ref();
            viewport
                .this_pass
                .widgets
                .get_layer(self.layer)
                .filter(|widget| {
                    takes_focus(widget)
                        && widget.interact_rect.is_positive()
                        && within.contains(widget.interact_rect.center())
                        && tree.is_none_or(|tree| tree.nodes.contains_key(&widget.id))
                })
                .map(|widget| (widget.rect.center().distance_sq(centre), widget.id))
                // The first of equally near controls, in presentation order.
                .fold(
                    None,
                    |nearest: Option<(f32, Id)>, candidate| match nearest {
                        Some(nearest) if nearest.0 <= candidate.0 => Some(nearest),
                        _ => Some(candidate),
                    },
                )
                .map(|(_, id)| id)
        })
    }
}

/// Whether a widget can take keyboard focus: an enabled control, not a
/// focusable label.
fn takes_focus(widget: &egui::WidgetRect) -> bool {
    widget.enabled && widget.sense.is_focusable() && widget.sense.interactive()
}

/// Whether this pass presented `id` as a control that can take focus, with
/// a node in this pass's AccessKit tree.
pub(crate) fn presented(context: &Context, id: Id) -> bool {
    context.viewport(|viewport| {
        viewport.this_pass.widgets.get(id).is_some_and(takes_focus)
            && viewport
                .this_pass
                .accesskit_state
                .as_ref()
                .is_none_or(|tree| tree.nodes.contains_key(&id))
    })
}
