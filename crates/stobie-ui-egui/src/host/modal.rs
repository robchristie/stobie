//! Modal dialogs in a hosted application: where keyboard focus goes while one
//! is presented and after it closes, and what people and agents can reach.
//!
//! egui's modal layer already keeps the pointer, and Tab traversal, from what
//! is behind a dialog. The host adds what egui leaves open:
//!
//! - Focus moves into a dialog when it opens, and back into it if it lands
//!   behind it (an arrow key can move egui's focus to any widget); a focused
//!   control behind a dialog would still take Space and Return.
//! - Focus returns to the control that opened the dialog when it closes,
//!   or, when that control has gone, to the presented control nearest
//!   where it was.
//! - AccessKit's focus is always a node of its tree, the root when the
//!   focused widget was not presented.
//! - Assistive technology sees only the dialog: every other top-level layer
//!   below it is hidden in the AccessKit tree, and requests aimed at nodes
//!   outside what is shown are dropped before the pass sees them.
//! - The host observes and binds only the dialog's content (see
//!   `AppHost::collect` and `AppHost::bindings`), so agents can reach
//!   nothing behind it, and shortcuts trigger only what it presents.

use std::collections::HashSet;
use std::sync::Arc;

use egui::accesskit::{NodeId, TreeId};

use super::HostRepaint;
use crate::focus_return::FocusReturn;
use crate::observation::DialogRegistration;

/// The modal dialog the latest pass presented, and the focus around it.
struct ModalState {
    dialog: DialogRegistration,
    /// The control focus returns to when the dialog closes.
    opener: Option<FocusReturn>,
    /// The widget inside the dialog that last had focus.
    last_inside: Option<egui::Id>,
    /// Whether focus has been moved into the dialog.
    placed: bool,
}

/// The host's modal bookkeeping across passes.
#[derive(Default)]
pub(super) struct Modal {
    state: Option<ModalState>,
    /// The widget focused when the current pass began.
    focus_at_start: Option<egui::Id>,
    /// The widget the previous pass's pointer clicked.
    previous_click: Option<egui::Id>,
}

/// The AccessKit nodes shown while a dialog is presented: the latest output's
/// tree without the hidden layers.
#[derive(Clone, Debug, Default)]
struct Reachable(Arc<HashSet<NodeId>>);

fn reachable_id() -> egui::Id {
    egui::Id::new("stobie.modal.reachable")
}

/// Install the plugin that hides what is behind a dialog from assistive
/// technology. Install it before any plugin that reads the AccessKit update,
/// such as the browser's ARIA layer; installing it again does nothing.
pub(crate) fn install(context: &egui::Context) {
    context.add_plugin(ModalScreen);
}

/// The layer `widget` was last drawn on, if it was drawn.
fn layer_of(context: &egui::Context, widget: egui::Id) -> Option<egui::LayerId> {
    context
        .read_response(widget)
        .map(|response| response.layer_id)
}

impl Modal {
    /// Whether the latest pass presented a modal dialog.
    pub(super) fn presented(&self) -> bool {
        self.state.is_some()
    }

    /// At the start of a pass, before invocations and the application's UI:
    /// drop AccessKit requests for what a dialog hides, and keep keyboard
    /// focus inside the dialog.
    pub(super) fn begin(&mut self, root: &mut egui::Ui) {
        let context = root.ctx().clone();
        self.focus_at_start = context.memory(|memory| memory.focused());
        if let Some(Reachable(reachable)) =
            context.data(|data| data.get_temp::<Reachable>(reachable_id()))
        {
            root.input_mut(|input| {
                input.events.retain(|event| match event {
                    egui::Event::AccessKitActionRequest(request) => {
                        request.target_tree != TreeId::ROOT
                            || reachable.contains(&request.target_node)
                    }
                    _ => true,
                })
            });
        }
        let Some(state) = &mut self.state else {
            return;
        };
        let inside =
            |layer: egui::LayerId| context.memory(|memory| memory.is_above_modal_layer(layer));
        match self.focus_at_start {
            Some(widget) => match layer_of(&context, widget) {
                Some(layer) if inside(layer) => {
                    state.placed = true;
                    state.last_inside = Some(widget);
                }
                // Focus behind the dialog goes back to where it was in it.
                Some(_) => {
                    let back = state
                        .last_inside
                        .filter(|widget| layer_of(&context, *widget).is_some_and(inside));
                    context.memory_mut(|memory| {
                        memory.surrender_focus(widget);
                        match back {
                            Some(back) => memory.request_focus(back),
                            None => memory.move_focus(egui::FocusDirection::Next),
                        }
                    });
                    state.placed = true;
                }
                None => {}
            },
            // Only the dialog's widgets take focus now, so the first of them
            // does.
            None if !state.placed => {
                context.memory_mut(|memory| memory.move_focus(egui::FocusDirection::Next));
                state.placed = true;
            }
            None => {}
        }
    }

    /// After a pass's collection: note a dialog that opened, with the
    /// control that opened it, and return focus to that control when its
    /// dialog closed.
    pub(super) fn end(
        &mut self,
        context: &egui::Context,
        dialogs: &[DialogRegistration],
        repaint: &mut Vec<HostRepaint>,
    ) {
        let clicked = context.interaction_snapshot(|snapshot| snapshot.clicked);
        let previous_click = std::mem::replace(&mut self.previous_click, clicked);
        let dialog = dialogs.last();
        match (self.state.take(), dialog) {
            (None, Some(dialog)) => {
                // A click this pass or the previous one opened it, or the
                // focused control did.
                let opener = [clicked, self.focus_at_start, previous_click]
                    .into_iter()
                    .flatten()
                    .find(|widget| layer_of(context, *widget) != Some(dialog.layer))
                    .and_then(|widget| FocusReturn::remember(context, widget));
                self.state = Some(ModalState {
                    dialog: dialog.clone(),
                    opener,
                    last_inside: None,
                    placed: false,
                });
                // The next pass moves focus into it.
                repaint.push(HostRepaint::Application("dialog focus"));
            }
            (Some(mut state), Some(dialog)) => {
                if state.dialog.id != dialog.id {
                    // Another dialog took its place, such as a confirm after
                    // a form; focus returns to the first one's opener.
                    state.dialog = dialog.clone();
                    state.last_inside = None;
                    state.placed = false;
                    repaint.push(HostRepaint::Application("dialog focus"));
                } else {
                    state.dialog = dialog.clone();
                }
                self.state = Some(state);
            }
            (Some(state), None) => {
                // The opener, or the control nearest where it was when it
                // has gone, such as the next row's when the dialog's own
                // confirmation removed the opener's row.
                if let Some(target) = state
                    .opener
                    .and_then(|opener| opener.target(context, egui::Rect::EVERYTHING))
                {
                    context.memory_mut(|memory| memory.request_focus(target));
                    repaint.push(HostRepaint::Application("dialog focus"));
                }
            }
            (None, None) => {}
        }
    }
}

/// Hides every top-level layer below the modal layer from assistive
/// technology, and records which nodes stay shown. It also keeps the
/// update's focus inside its tree.
struct ModalScreen;

impl egui::Plugin for ModalScreen {
    fn debug_name(&self) -> &'static str {
        "stobie.modal-screen"
    }

    fn output_hook(&mut self, context: &egui::Context, output: &mut egui::FullOutput) {
        let Some(update) = output.platform_output.accesskit_update.as_mut() else {
            return;
        };
        // egui sets this pass's modal layer before the output hooks run.
        let shown_layers: Option<HashSet<NodeId>> = context.memory(|memory| {
            memory.top_modal_layer().map(|_| {
                memory
                    .layer_ids()
                    .filter(|layer| memory.is_above_modal_layer(*layer))
                    .map(|layer| layer.id.accesskit_id())
                    .collect()
            })
        });
        let Some(shown_layers) = shown_layers else {
            context.data_mut(|data| data.remove::<Reachable>(reachable_id()));
            // Focus requested for a widget this pass did not present names a
            // node outside the tree, which assistive technology cannot
            // follow; egui drops that focus on the next pass.
            if let Some(tree) = &update.tree
                && !update.nodes.iter().any(|(id, _)| *id == update.focus)
            {
                update.focus = tree.root;
            }
            return;
        };
        let Some(root) = update.tree.as_ref().map(|tree| tree.root) else {
            return;
        };
        let index: std::collections::HashMap<NodeId, usize> = update
            .nodes
            .iter()
            .enumerate()
            .map(|(position, (id, _))| (*id, position))
            .collect();
        let top_level: Vec<NodeId> = index
            .get(&root)
            .map(|&position| update.nodes[position].1.children().to_vec())
            .unwrap_or_default();
        for child in &top_level {
            if !shown_layers.contains(child)
                && let Some(&position) = index.get(child)
            {
                update.nodes[position].1.set_hidden();
            }
        }
        let mut reachable = HashSet::from([root]);
        let mut pending: Vec<NodeId> = top_level
            .into_iter()
            .filter(|child| shown_layers.contains(child))
            .collect();
        while let Some(id) = pending.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(&position) = index.get(&id) {
                pending.extend(update.nodes[position].1.children().iter().copied());
            }
        }
        if !reachable.contains(&update.focus) {
            update.focus = root;
        }
        context.data_mut(|data| data.insert_temp(reachable_id(), Reachable(Arc::new(reachable))));
    }
}
