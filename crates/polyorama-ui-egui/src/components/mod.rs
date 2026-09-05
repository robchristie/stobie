mod action_button;
mod choice;
mod property;
mod range;
mod result_row;
mod status;
mod thumbnail;
mod viewport_status;

pub use action_button::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, action_button, action_semantic_node,
};
pub use choice::choice_control;
pub use property::property_row;
pub use range::range_control;
pub use result_row::{ResultRowSpec, result_row, result_row_height};
pub use status::{StatusTone, status_badge};
pub use thumbnail::{ThumbnailCellSpec, ThumbnailState, thumbnail_cell, thumbnail_cell_side};
pub use viewport_status::{ImageStatusSpec, image_status_height, paint_image_status};

use egui::{Color32, Frame, Margin, Painter, Rect, Response, Sense, Stroke};

use crate::{
    DesignTokens, HorizontalTextAlignment, TextComponentId, TextOverflow, TextRole, TextSpec,
    UiNode, measure_text, paint_measured_text, present_accessible_measured_text,
    present_measured_text,
};

pub const SPLITTER_VISUAL_WIDTH: f32 = 5.0;

/// Visual interaction state shared by the live dock and deterministic
/// splitter stories.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SplitterVisualState {
    pub hovered: bool,
    pub active: bool,
    pub focused: bool,
}

pub fn paint_splitter(
    painter: &Painter,
    rect: Rect,
    state: SplitterVisualState,
    tokens: &DesignTokens,
) {
    let fill = if state.active {
        Color32::from(tokens.colours.accent_primary)
    } else if state.hovered {
        Color32::from(tokens.colours.selection_background)
    } else {
        Color32::from(tokens.colours.surface_raised)
    };
    painter.rect_filled(rect, 0.0, fill);
    if state.focused {
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
}

pub struct SemanticControlOutput {
    pub response: Response,
    pub node: UiNode,
}

pub(super) struct ComponentTextSpec<'a> {
    text: &'a str,
    rect: Rect,
    spec: TextSpec,
    component_id: TextComponentId,
    parent_id: Option<TextComponentId>,
    accessible: bool,
}

pub(super) fn paint_text_observation(
    ui: &mut egui::Ui,
    text: ComponentTextSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Option<Response> {
    if let Ok(measured) = measure_text(
        ui.painter(),
        text.text,
        text.spec,
        tokens,
        font_scale,
        text.rect.width().max(0.5),
    ) {
        let (response, observation) = if text.accessible {
            let (response, observation) = present_accessible_measured_text(
                ui,
                &measured,
                text.rect,
                text.component_id,
                text.parent_id,
            );
            (Some(response), observation)
        } else {
            present_measured_text(ui, &measured, text.rect, text.component_id, text.parent_id)
        };
        observations.push(observation);
        response
    } else {
        None
    }
}

/// The shell keeps visual and hit geometry distinct. This is deliberately a
/// small component boundary rather than a general widget framework.
pub fn minimum_hit_rect(visual: Rect, minimum: f32, bounds: Rect) -> Rect {
    let size = egui::vec2(visual.width().max(minimum), visual.height().max(minimum));
    Rect::from_center_size(visual.center(), size).intersect(bounds)
}

/// Present one measured dock tab. The caller owns strip allocation and maps
/// the returned response to the canonical dock command.
pub struct DockTabSpec {
    pub selected: bool,
    pub visual_rect: Rect,
    pub font_scale: f32,
    pub component_id: TextComponentId,
    pub parent_id: TextComponentId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TabStripAllocation {
    pub visible: Vec<usize>,
    pub widths: Vec<f32>,
    pub overflow: bool,
}

/// Allocate whole, ordered tab targets. `available_width` excludes strip
/// padding; widths include neither gaps nor the overflow trigger.
pub fn allocate_tab_strip(
    desired_widths: &[f32],
    active: usize,
    available_width: f32,
    minimum_hit: f32,
    gap: f32,
) -> TabStripAllocation {
    let minimum = minimum_hit.max(1.0);
    let gap = gap.max(0.0);
    let count = desired_widths.len();
    if count == 0 || !available_width.is_finite() {
        return TabStripAllocation {
            visible: Vec::new(),
            widths: Vec::new(),
            overflow: count != 0,
        };
    }
    let minimum_total = count as f32 * minimum + count.saturating_sub(1) as f32 * gap;
    let overflow = minimum_total > available_width;
    let tab_capacity = if overflow {
        (available_width - minimum - gap).max(0.0)
    } else {
        available_width
    };
    let maximum_visible = ((tab_capacity + gap) / (minimum + gap)).floor().max(0.0) as usize;
    if maximum_visible == 0 {
        return TabStripAllocation {
            visible: Vec::new(),
            widths: Vec::new(),
            overflow,
        };
    }
    let active = active.min(count - 1);
    let mut visible = vec![active];
    let mut right = active + 1;
    let mut left = active;
    while visible.len() < maximum_visible && (right < count || left > 0) {
        if right < count {
            visible.push(right);
            right += 1;
        }
        if visible.len() < maximum_visible && left > 0 {
            left -= 1;
            visible.insert(0, left);
        }
    }
    let visible_count = visible.len();
    let gaps = gap * visible_count.saturating_sub(1) as f32;
    let spare = (tab_capacity - gaps - visible_count as f32 * minimum).max(0.0);
    let extras: Vec<_> = visible
        .iter()
        .map(|&index| (desired_widths[index].max(minimum) - minimum).max(0.0))
        .collect();
    let extra_total: f32 = extras.iter().sum();
    let growth = spare.min(extra_total);
    let widths = extras
        .into_iter()
        .map(|extra| {
            minimum
                + if extra_total > 0.0 {
                    growth * extra / extra_total
                } else {
                    0.0
                }
        })
        .collect();
    TabStripAllocation {
        visible,
        widths,
        overflow,
    }
}

pub fn dock_tab_interaction(ui: &mut egui::Ui, id: egui::Id, hit_rect: Rect) -> Response {
    let response = ui.interact(hit_rect, id, Sense::click_and_drag());
    if response.clicked() {
        response.request_focus();
    }
    response
}

pub fn paint_dock_tab(
    ui: &mut egui::Ui,
    response: &Response,
    title: &str,
    spec: DockTabSpec,
    tokens: &DesignTokens,
) -> Option<crate::TextLayoutObservation> {
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, ui.is_enabled(), title)
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::Tab);
        node.set_label(title);
        node.set_author_id(format!("polyorama.dock.tab.{}", spec.component_id.instance));
        node.clear_toggled();
        node.set_selected(spec.selected);
        node.add_action(Action::Click);
    });
    let fill = if spec.selected {
        ui.visuals().extreme_bg_color
    } else if response.hovered() {
        ui.visuals().widgets.hovered.bg_fill
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    ui.painter().rect_filled(spec.visual_rect, 4.0, fill);
    if response.has_focus() {
        ui.painter().rect_stroke(
            spec.visual_rect,
            4.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let padding = tokens
        .geometry
        .control_padding_x
        .0
        .min((spec.visual_rect.width() - 0.5).max(0.0) * 0.25);
    let label_rect = spec.visual_rect.shrink2(egui::vec2(padding, 0.0));
    let text_spec = TextSpec {
        horizontal_alignment: HorizontalTextAlignment::Centre,
        ..TextSpec::single_line(TextRole::TabLabel, TextOverflow::Ellipsis)
    };
    let observation = measure_text(
        ui.painter(),
        title,
        text_spec,
        tokens,
        spec.font_scale,
        label_rect.width().max(0.5),
    )
    .ok()
    .map(|measured| {
        paint_measured_text(
            &ui.painter_at(label_rect),
            &measured,
            label_rect,
            spec.component_id,
            Some(spec.parent_id),
        )
    });
    if observation
        .as_ref()
        .is_some_and(|observation| observation.truncated)
    {
        response.clone().on_hover_text(title);
    }
    observation
}

/// Present the explicit overflow trigger using a project-painted primitive,
/// avoiding an untyped icon glyph dependency before the icon increment.
pub fn dock_overflow_trigger(
    ui: &mut egui::Ui,
    id: egui::Id,
    instance: u64,
    hit_rect: Rect,
    tokens: &DesignTokens,
) -> Response {
    let response = ui.interact(hit_rect, id, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "More tabs")
    });
    ui.ctx().accesskit_node_builder(id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::Button);
        node.set_label("More tabs");
        node.set_author_id(format!("polyorama.dock.tabs.overflow.{instance}"));
        node.add_action(Action::Click);
    });
    ui.painter()
        .rect_filled(hit_rect, 3.0, ui.visuals().widgets.inactive.bg_fill);
    let centre = hit_rect.center();
    for offset in [-5.0, 0.0, 5.0] {
        ui.painter().circle_filled(
            centre + egui::vec2(offset, 0.0),
            1.35,
            tokens.colours.text_primary,
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            hit_rect,
            3.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// The isolated application-bar recipe is the first production token
/// consumer. Further component migration belongs to later campaign increments.
pub fn application_bar_frame(tokens: &DesignTokens) -> Frame {
    Frame::new()
        .fill(tokens.colours.surface_panel.into())
        .stroke(Stroke::new(1.0, tokens.colours.border_subtle))
        .inner_margin(Margin::symmetric(
            bounded_margin(tokens.spacing.inline.0),
            0,
        ))
}

fn bounded_margin(points: f32) -> i8 {
    if points.is_finite() {
        points.round().clamp(0.0, f32::from(i8::MAX)) as i8
    } else {
        0
    }
}

pub fn application_bar_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    (tokens.geometry.application_bar_height.0 * font_scale).max(tokens.geometry.minimum_hit_size.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActionKey, ActionTarget, Availability, DensityVariant, SemanticActionId, SemanticUiId,
        TextInteraction, ThemeVariant, UiRole, test_actions::TestAction,
    };

    #[test]
    fn application_bar_recipe_uses_visual_geometry_without_shrinking_hit_geometry() {
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
        assert_eq!(tokens.geometry.application_bar_height.0, 32.0);
        assert_eq!(application_bar_height(&tokens, 1.0), 32.0);
        assert_eq!(application_bar_height(&tokens, 1.5), 48.0);
    }

    #[test]
    fn application_bar_margin_conversion_is_explicitly_bounded() {
        assert_eq!(bounded_margin(-2.0), 0);
        assert_eq!(bounded_margin(7.4), 7);
        assert_eq!(bounded_margin(200.0), i8::MAX);
        assert_eq!(bounded_margin(f32::NAN), 0);
    }

    #[test]
    fn tab_allocation_preserves_minimum_targets_and_active_visibility() {
        let allocation = allocate_tab_strip(&[40.0, 180.0, 40.0, 40.0], 1, 130.0, 32.0, 3.0);
        assert!(allocation.overflow);
        assert!(allocation.visible.contains(&1));
        assert!(allocation.widths.iter().all(|width| *width >= 32.0));
        assert!(
            allocation
                .visible
                .iter()
                .zip(&allocation.widths)
                .all(|(&index, &width)| width <= [40.0, 180.0, 40.0, 40.0][index])
        );
        assert_eq!(allocation.visible.len(), allocation.widths.len());
        let all_fit = allocate_tab_strip(&[1.0, 2.0, 3.0], 1, 102.0, 32.0, 3.0);
        assert!(!all_fit.overflow);
        assert_eq!(all_fit.visible, vec![0, 1, 2]);
        assert_eq!(all_fit.widths, vec![32.0, 32.0, 32.0]);
        let minimum = allocate_tab_strip(&[200.0, 200.0], 1, 32.0, 32.0, 3.0);
        assert!(minimum.overflow);
        assert!(minimum.visible.is_empty());
    }

    #[test]
    fn overflow_trigger_exposes_full_button_semantics_and_minimum_bounds() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let root = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(80.0, 40.0));
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |ui| {
                let _ = dock_overflow_trigger(
                    ui,
                    egui::Id::new("overflow-test"),
                    7,
                    Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(32.0, 32.0)),
                    &DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable),
                );
            },
        );
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        let overflow = update
            .nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| node.author_id() == Some("polyorama.dock.tabs.overflow.7"))
            .expect("overflow semantic node");
        assert_eq!(overflow.role(), egui::accesskit::Role::Button);
        assert_eq!(overflow.label(), Some("More tabs"));
        assert!(overflow.supports_action(egui::accesskit::Action::Click));
        let bounds = overflow.bounds().expect("overflow bounds");
        assert_eq!(bounds.width(), 32.0);
        assert_eq!(bounds.height(), 32.0);
    }

    #[test]
    fn gallery_components_expose_stable_roles_names_states_actions_and_hit_bounds() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
        let mut observations = Vec::new();
        let mut control_nodes = Vec::new();
        let root = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(720.0, 420.0));
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |ui| {
                let button = action_button(
                    ui,
                    ActionButtonSpec {
                        target: ActionTarget::application(TestAction::Undo),
                        availability: Availability::Disabled {
                            reason: "History is empty".into(),
                        },
                        state: ActionButtonState::Momentary,
                        emphasis: ActionEmphasis::Normal,
                        compact: true,
                    },
                    &tokens,
                    1.0,
                    &mut observations,
                );
                assert_eq!(
                    button.id,
                    egui::Id::new((
                        "polyorama.action-button",
                        TestAction::Undo.stable_id(),
                        Option::<polyorama_core::PaneId>::None,
                    ))
                );
                let result = result_row(
                    ui,
                    ResultRowSpec {
                        instance: 12,
                        identifier: "#12",
                        position: "−1.0, 2.0",
                        confidence: "99.5 %",
                        category: "Selected target",
                        selected: true,
                    },
                    &tokens,
                    1.0,
                    &mut observations,
                );
                assert_eq!(result.id, egui::Id::new(("polyorama.result-row", 12_u64)));
                property_row(
                    ui,
                    14,
                    "Result identifier",
                    "#12",
                    &tokens,
                    1.0,
                    &mut observations,
                );
                status_badge(
                    ui,
                    15,
                    "Worker decode failed for tile 12",
                    StatusTone::Error,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                let thumbnail = thumbnail_cell(
                    ui,
                    ThumbnailCellSpec {
                        instance: 13,
                        label: "Tile 13",
                        state: ThumbnailState::Resident,
                        selected: true,
                        texture: None,
                    },
                    &tokens,
                    1.0,
                    &mut observations,
                );
                assert_eq!(
                    thumbnail.id,
                    egui::Id::new(("polyorama.thumbnail-cell", 13_u64))
                );
                let parent = SemanticUiId::root();
                let mut choice = 1_u8;
                control_nodes.push(
                    choice_control(
                        ui,
                        SemanticUiId::new("test.display-map"),
                        parent.clone(),
                        "Display map",
                        &mut choice,
                        &[(0, "Viridis"), (1, "Greyscale")],
                        TestAction::DisplaySettings,
                        &tokens,
                    )
                    .node,
                );
                let mut low = 0.2;
                control_nodes.push(
                    range_control(
                        ui,
                        SemanticUiId::new("test.display-low"),
                        parent,
                        "Low",
                        &mut low,
                        0.0..=0.8,
                        TestAction::DisplaySettings,
                        &tokens,
                    )
                    .node,
                );
            },
        );
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        let node = |author_id: &str| {
            update
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| node.author_id() == Some(author_id))
                .unwrap_or_else(|| panic!("missing node {author_id}"))
        };
        let button = node("action.undo");
        assert_eq!(button.role(), egui::accesskit::Role::Button);
        assert_eq!(button.label(), Some("Undo"));
        assert!(
            button
                .description()
                .is_some_and(|description| description.contains("History is empty"))
        );
        assert!(button.is_disabled());
        assert!(!button.supports_action(egui::accesskit::Action::Click));
        assert!(
            button
                .bounds()
                .is_some_and(|bounds| bounds.height() >= 32.0)
        );

        let result = node("polyorama.result-row.12");
        assert_eq!(result.role(), egui::accesskit::Role::ListBoxOption);
        assert_eq!(result.is_selected(), Some(true));
        assert_eq!(result.toggled(), None);
        assert!(result.supports_action(egui::accesskit::Action::Click));

        let label_count = |label: &str| {
            update
                .nodes
                .iter()
                .map(|(_, node)| node)
                .filter(|node| {
                    node.role() == egui::accesskit::Role::Label && node.value() == Some(label)
                })
                .count()
        };
        assert_eq!(label_count("Result identifier"), 1);
        assert_eq!(label_count("#12"), 1);
        assert_eq!(label_count("Worker decode failed for tile 12"), 1);
        assert_eq!(label_count("Result identifier: #12"), 0);
        let (property_label_id, _) = update
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == egui::accesskit::Role::Label
                    && node.value() == Some("Result identifier")
            })
            .expect("property label node");
        let (_, property_value) = update
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == egui::accesskit::Role::Label && node.value() == Some("#12")
            })
            .expect("property value node");
        assert_eq!(property_value.labelled_by(), &[*property_label_id]);

        let thumbnail = node("polyorama.thumbnail-cell.13");
        assert_eq!(thumbnail.role(), egui::accesskit::Role::ListBoxOption);
        assert_eq!(thumbnail.label(), Some("Tile 13; Resident"));
        assert_eq!(thumbnail.is_selected(), Some(true));
        assert_eq!(thumbnail.toggled(), None);
        assert!(thumbnail.supports_action(egui::accesskit::Action::Click));
        let choice = node("test.display-map");
        assert_eq!(choice.role(), egui::accesskit::Role::ComboBox);
        assert_eq!(choice.label(), Some("Display map"));
        assert!(choice.supports_action(egui::accesskit::Action::Click));
        let range = node("test.display-low");
        assert_eq!(range.role(), egui::accesskit::Role::Slider);
        assert_eq!(range.label(), Some("Low"));
        assert!(range.supports_action(egui::accesskit::Action::Increment));
        assert!(range.supports_action(egui::accesskit::Action::Decrement));
        assert_eq!(control_nodes[0].role, UiRole::ComboBox);
        assert!(observations.iter().any(|text| {
            text.component_id.kind == crate::TextComponentKind::PropertyRow
                && text.role == TextRole::Body
                && text.interaction == TextInteraction::Selectable
        }));
        assert!(observations.iter().any(|text| {
            text.component_id.kind == crate::TextComponentKind::StatusBadge
                && text.interaction == TextInteraction::Selectable
        }));
        assert!(
            observations
                .iter()
                .filter(|text| {
                    matches!(
                        text.component_id.kind,
                        crate::TextComponentKind::ActionButton
                            | crate::TextComponentKind::ResultRow
                            | crate::TextComponentKind::ThumbnailCell
                    )
                })
                .all(|text| text.interaction == TextInteraction::Inert)
        );
        assert_eq!(control_nodes[1].role, UiRole::Slider);
        assert!(control_nodes.iter().all(|node| {
            node.actions == vec![SemanticActionId::from_action(TestAction::DisplaySettings)]
        }));
        assert!(observations.len() >= 6);
    }
}
