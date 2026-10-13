//! Per-pass observation collection for hosted applications.
//!
//! An [`AppHost`](crate::AppHost) enables a collector in egui context data for
//! each viewport. [`PresentationContext`](crate::PresentationContext), the dock
//! and the choice and range recipes then register their semantic nodes and
//! measured text as they present, together with the clip rectangle each node
//! was drawn under. A new UI pass replaces the previous pass's observations, so
//! a discarded layout pass never accumulates. Without a host the collector is
//! disabled and recording does nothing.

use egui::{Context, Id, Rect, Ui, ViewportId};

use crate::{TextLayoutObservation, UiNode};

#[derive(Clone, Default)]
struct Collector {
    enabled: bool,
    pass: u64,
    nodes: Vec<(UiNode, Rect)>,
    text: Vec<TextLayoutObservation>,
}

/// Observations registered during one UI pass.
#[derive(Default)]
pub(crate) struct PassObservations {
    /// Each node with the clip rectangle it was presented under.
    pub nodes: Vec<(UiNode, Rect)>,
    pub text: Vec<TextLayoutObservation>,
}

fn collector_id(viewport: ViewportId) -> Id {
    Id::new(("stobie.observations", viewport))
}

pub(crate) fn enable(context: &Context) {
    let id = collector_id(context.viewport_id());
    context.data_mut(|data| data.get_temp_mut_or_default::<Collector>(id).enabled = true);
}

pub(crate) fn is_enabled(context: &Context, viewport: ViewportId) -> bool {
    context.data(|data| {
        data.get_temp::<Collector>(collector_id(viewport))
            .is_some_and(|collector| collector.enabled)
    })
}

/// Register observations made during `pass` of `viewport`. Observations from
/// an earlier pass than the collector's current one are stale and ignored.
pub(crate) fn record(
    context: &Context,
    viewport: ViewportId,
    pass: u64,
    nodes: impl IntoIterator<Item = (UiNode, Rect)>,
    text: impl IntoIterator<Item = TextLayoutObservation>,
) {
    context.data_mut(|data| {
        let collector = data.get_temp_mut_or_default::<Collector>(collector_id(viewport));
        if !collector.enabled || pass < collector.pass {
            return;
        }
        if pass > collector.pass {
            collector.pass = pass;
            collector.nodes.clear();
            collector.text.clear();
        }
        collector.nodes.extend(nodes);
        collector.text.extend(text);
    });
}

/// Take the current pass's observations, leaving the collector empty.
pub(crate) fn take(context: &Context) -> PassObservations {
    // Read the context before locking its data; egui locks are not reentrant.
    let pass = context.cumulative_pass_nr();
    let id = collector_id(context.viewport_id());
    context.data_mut(|data| {
        let collector = data.get_temp_mut_or_default::<Collector>(id);
        if collector.pass != pass {
            collector.nodes.clear();
            collector.text.clear();
            return PassObservations::default();
        }
        PassObservations {
            nodes: std::mem::take(&mut collector.nodes),
            text: std::mem::take(&mut collector.text),
        }
    })
}

/// Register a recipe's own node without cloning it when no host collects.
pub(crate) fn observe_recipe_node(ui: &Ui, node: &UiNode) {
    let context = ui.ctx();
    if is_enabled(context, context.viewport_id()) {
        observe_node(ui, node.clone());
    }
}

/// Observe an application-owned semantic node, such as a custom row, in a
/// hosted application. Its visible bounds are clipped to `ui`'s clip rectangle.
/// Recipes and [`PresentationContext`](crate::PresentationContext) already
/// observe their own nodes; do not observe those again.
pub fn observe_node(ui: &Ui, node: UiNode) {
    let context = ui.ctx();
    record(
        context,
        context.viewport_id(),
        context.cumulative_pass_nr(),
        [(node, ui.clip_rect())],
        [],
    );
}

/// Observe application-owned measured text in a hosted application.
pub fn observe_text(ui: &Ui, observation: TextLayoutObservation) {
    let context = ui.ctx();
    record(
        context,
        context.viewport_id(),
        context.cumulative_pass_nr(),
        [],
        [observation],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SemanticUiId, UiRole};

    fn node(id: &str) -> UiNode {
        UiNode::container(
            SemanticUiId::new(id),
            Some(SemanticUiId::root()),
            UiRole::Button,
            Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(10.0, 10.0)).into(),
        )
    }

    #[test]
    fn collection_is_disabled_until_a_host_enables_it_and_resets_each_pass() {
        let context = Context::default();
        let mut output = context.run_ui(Default::default(), |ui| {
            observe_node(ui, node("ignored"));
            assert!(take(ui.ctx()).nodes.is_empty());
            enable(ui.ctx());
            observe_node(ui, node("first-pass"));
        });
        output.textures_delta.clear();
        let mut output = context.run_ui(Default::default(), |ui| {
            observe_node(ui, node("second-pass"));
            let taken = take(ui.ctx());
            let ids: Vec<_> = taken
                .nodes
                .iter()
                .map(|(node, _)| node.id.0.as_str())
                .collect();
            assert_eq!(ids, ["second-pass"]);
            assert!(take(ui.ctx()).nodes.is_empty());
        });
        output.textures_delta.clear();
    }
}
