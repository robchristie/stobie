//! Per-pass observation collection for hosted applications.
//!
//! An [`AppHost`](crate::AppHost) enables a collector in egui context data for
//! each viewport. [`PresentationContext`](crate::PresentationContext), the dock
//! and the choice and range recipes then register their semantic nodes and
//! measured text as they present. A new UI pass replaces the previous pass's
//! observations, so a discarded layout pass never accumulates. Without a host
//! the collector is disabled and recording does nothing.
//!
//! Observations are published as presented, so the text and semantic audits
//! report content that is clipped or overflows. Content presented inside
//! [`observe_visible_only`], which the host applies to each dock pane, is
//! published only where it is visible.

use egui::{Context, Id, Rect, Ui, ViewportId};

use crate::{TextLayoutObservation, UiNode};

/// How the host treats an observation's visibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Visibility {
    /// Publish as presented, so the audits report clipping and overflow.
    AsPresented,
    /// Scrolling content: clip a node to the bounds it was drawn within, omit
    /// it when scrolled out of view, and audit successful text only when it is
    /// fully visible.
    VisibleWithin(Rect),
}

/// Who registered a node. An application may observe a recipe's node again to
/// add context such as its pane or domain; its observation replaces the
/// recipe's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Origin {
    Recipe,
    Application,
}

#[derive(Clone, Default)]
struct Collector {
    enabled: bool,
    pass: u64,
    /// Depth of nested [`observe_visible_only`] regions being presented.
    visible_only: u32,
    nodes: Vec<(UiNode, Visibility, Origin)>,
    text: Vec<(TextLayoutObservation, bool)>,
    copies: Vec<CopyRegistration>,
}

/// A copy control presented this pass: its semantic node's ID, its egui
/// widget and the revision of its text. The text itself stays with the
/// control.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CopyRegistration {
    pub id: crate::SemanticUiId,
    pub widget: Id,
    pub revision: u64,
}

/// Observations registered during one UI pass.
#[derive(Default)]
pub(crate) struct PassObservations {
    pub nodes: Vec<(UiNode, Visibility, Origin)>,
    /// Text, and whether only its fully visible successes are audited.
    pub text: Vec<(TextLayoutObservation, bool)>,
    /// Copy controls, bound only where their nodes are published.
    pub copies: Vec<CopyRegistration>,
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

/// Whether observations made now are inside an [`observe_visible_only`] region.
pub(crate) fn is_visible_only(context: &Context) -> bool {
    let id = collector_id(context.viewport_id());
    context.data(|data| {
        data.get_temp::<Collector>(id)
            .is_some_and(|collector| collector.enabled && collector.visible_only > 0)
    })
}

/// The visibility policy for a node observed now under `ui`'s clip.
pub(crate) fn visibility(ui: &Ui) -> Visibility {
    if is_visible_only(ui.ctx()) {
        Visibility::VisibleWithin(ui.clip_rect())
    } else {
        Visibility::AsPresented
    }
}

/// Publish only the visible part of what `add_contents` presents, for content
/// that scrolls: each node is clipped to the region it was drawn in and
/// omitted once scrolled out of view, and measured text is audited only when
/// fully visible or failed. Everything else is published as presented, so the
/// audits report clipping and overflow. The host already applies this to each
/// dock pane; presentation contexts take the policy in force when created.
pub fn observe_visible_only<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    struct Region(Context, Id);
    impl Drop for Region {
        fn drop(&mut self) {
            self.0.data_mut(|data| {
                let collector = data.get_temp_mut_or_default::<Collector>(self.1);
                collector.visible_only = collector.visible_only.saturating_sub(1);
            });
        }
    }
    let context = ui.ctx().clone();
    let id = collector_id(context.viewport_id());
    context.data_mut(|data| data.get_temp_mut_or_default::<Collector>(id).visible_only += 1);
    let _region = Region(context, id);
    add_contents(ui)
}

fn withheld_id(viewport: ViewportId) -> Id {
    Id::new(("stobie.observations.withheld-content", viewport))
}

/// Withhold the words of the content text that `add_contents` presents from
/// observation, for sensitive text such as a secret shown to its owner. Its
/// text observations carry [`TextContent::Withheld`](crate::TextContent)
/// instead of an excerpt, so neither the text nor its length reaches a
/// snapshot, an agent or a host test; its layout is still measured, observed
/// and audited, and its accessibility node is unchanged. Regions nest. A
/// password field needs no region: its text is never published. A region
/// governs content text only; a control's name or value is its semantic
/// node's, and is published as before.
pub fn withhold_content_text<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    struct Region(Context, Id);
    impl Drop for Region {
        fn drop(&mut self) {
            self.0.data_mut(|data| {
                let depth = data.get_temp_mut_or_default::<u32>(self.1);
                *depth = depth.saturating_sub(1);
            });
        }
    }
    let context = ui.ctx().clone();
    let id = withheld_id(context.viewport_id());
    context.data_mut(|data| *data.get_temp_mut_or_default::<u32>(id) += 1);
    let _region = Region(context, id);
    add_contents(ui)
}

/// Whether content text presented now is inside a [`withhold_content_text`]
/// region.
pub(crate) fn is_content_withheld(context: &Context) -> bool {
    let id = withheld_id(context.viewport_id());
    context.data(|data| data.get_temp::<u32>(id).is_some_and(|depth| depth > 0))
}

/// Register observations made during `pass` of `viewport`. Observations from
/// an earlier pass than the collector's current one are stale and ignored.
pub(crate) fn record(
    context: &Context,
    viewport: ViewportId,
    pass: u64,
    nodes: impl IntoIterator<Item = (UiNode, Visibility)>,
    text: impl IntoIterator<Item = (TextLayoutObservation, bool)>,
) {
    let nodes = nodes
        .into_iter()
        .map(|(node, visibility)| (node, visibility, Origin::Application));
    record_with_origin(context, viewport, pass, nodes, text);
}

fn record_with_origin(
    context: &Context,
    viewport: ViewportId,
    pass: u64,
    nodes: impl IntoIterator<Item = (UiNode, Visibility, Origin)>,
    text: impl IntoIterator<Item = (TextLayoutObservation, bool)>,
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
            collector.copies.clear();
        }
        collector.nodes.extend(nodes);
        collector.text.extend(text);
    });
}

/// Register a copy control presented in the current pass.
pub(crate) fn record_copy(context: &Context, copy: CopyRegistration) {
    let viewport = context.viewport_id();
    let pass = context.cumulative_pass_nr();
    record_with_origin(context, viewport, pass, [], []);
    context.data_mut(|data| {
        let collector = data.get_temp_mut_or_default::<Collector>(collector_id(viewport));
        if collector.enabled && collector.pass == pass {
            collector.copies.push(copy);
        }
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
            collector.copies.clear();
            return PassObservations::default();
        }
        PassObservations {
            nodes: std::mem::take(&mut collector.nodes),
            text: std::mem::take(&mut collector.text),
            copies: std::mem::take(&mut collector.copies),
        }
    })
}

/// Register a recipe's own node without cloning it when no host collects.
pub(crate) fn observe_recipe_node(ui: &Ui, node: &UiNode) {
    let context = ui.ctx();
    if is_enabled(context, context.viewport_id()) {
        record_with_origin(
            context,
            context.viewport_id(),
            context.cumulative_pass_nr(),
            [(node.clone(), visibility(ui), Origin::Recipe)],
            [],
        );
    }
}

/// Observe an application-owned semantic node, such as a custom row, in a
/// hosted application. Inside [`observe_visible_only`] it is published only
/// where visible within `ui`'s clip rectangle. Recipes and presentation
/// contexts already observe their own nodes. Observe a recipe's node again only
/// to add context such as its pane or domain: that observation replaces the
/// recipe's.
pub fn observe_node(ui: &Ui, node: UiNode) {
    let context = ui.ctx();
    record(
        context,
        context.viewport_id(),
        context.cumulative_pass_nr(),
        [(node, visibility(ui))],
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
        [(observation, is_visible_only(context))],
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
                .map(|(node, _, _)| node.id.0.as_str())
                .collect();
            assert_eq!(ids, ["second-pass"]);
            assert!(take(ui.ctx()).nodes.is_empty());
        });
        output.textures_delta.clear();
    }

    #[test]
    fn only_observations_inside_a_visible_only_region_are_filtered() {
        let context = Context::default();
        let mut output = context.run_ui(Default::default(), |ui| {
            enable(ui.ctx());
            let clip = ui.clip_rect();
            let text = |instance| {
                use crate::{TextComponentId, TextComponentKind, TextRect};
                let rect = TextRect::from(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(10.0, 10.0),
                ));
                crate::TextLayoutObservation {
                    component_id: TextComponentId::new(TextComponentKind::ResultRow, instance),
                    parent_id: None,
                    role: crate::TextRole::Body,
                    horizontal_alignment: crate::HorizontalTextAlignment::Start,
                    vertical_alignment: crate::VerticalTextAlignment::Top,
                    allocated_rect: rect,
                    painted_rect: rect,
                    clip_rect: rect,
                    baseline: None,
                    overflow: crate::TextOverflow::Ellipsis,
                    interaction: Default::default(),
                    declared_max_lines: 1,
                    line_count: 1,
                    truncated: false,
                    layout_error: None,
                    content: None,
                }
            };
            observe_node(ui, node("presented"));
            observe_text(ui, text(1));
            observe_visible_only(ui, |ui| {
                observe_visible_only(ui, |ui| observe_node(ui, node("nested")));
                observe_node(ui, node("visible"));
                observe_text(ui, text(2));
            });
            observe_node(ui, node("after"));
            let taken = take(ui.ctx());
            let nodes: Vec<_> = taken
                .nodes
                .iter()
                .map(|(node, visibility, _)| (node.id.0.as_str(), *visibility))
                .collect();
            assert_eq!(
                nodes,
                [
                    ("presented", Visibility::AsPresented),
                    ("nested", Visibility::VisibleWithin(clip)),
                    ("visible", Visibility::VisibleWithin(clip)),
                    ("after", Visibility::AsPresented),
                ]
            );
            let text: Vec<_> = taken
                .text
                .iter()
                .map(|(text, visible_only)| (text.component_id.instance, *visible_only))
                .collect();
            assert_eq!(text, [(1, false), (2, true)]);
        });
        output.textures_delta.clear();
    }
}
