use egui::{Color32, Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, HorizontalTextAlignment,
    SemanticActionId, SemanticUiId, TextComponentId, TextOverflow, TextRole, TextSpec, UiNode,
    UiRole, measure_text, paint_measured_text,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionEmphasis {
    Quiet,
    Normal,
    Primary,
}

pub struct ActionButtonSpec<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub availability: Availability,
    pub selected: bool,
    pub emphasis: ActionEmphasis,
    pub compact: bool,
}

/// Token-derived action control shared by production screens and gallery
/// stories. The label is measured, elided deliberately and retained in full
/// for widget and accessibility semantics.
pub fn action_button<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: ActionButtonSpec<A>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    debug_assert!(spec.availability.visible());
    let action = spec.target.action.specification();
    let visible_label = if spec.compact {
        action.compact_label.unwrap_or(action.label)
    } else {
        action.label
    };
    let enabled = spec.availability.enabled();
    let text_spec = TextSpec {
        horizontal_alignment: HorizontalTextAlignment::Centre,
        ..TextSpec::single_line(TextRole::ButtonLabel, TextOverflow::Ellipsis)
    };
    let intrinsic = measure_text(
        ui.painter(),
        visible_label,
        TextSpec {
            overflow: TextOverflow::Expand,
            ..text_spec
        },
        tokens,
        font_scale,
        4_096.0,
    )
    .ok()
    .map_or(tokens.geometry.minimum_hit_size.0, |text| {
        text.size().x + tokens.geometry.control_padding_x.0 * 2.0
    });
    let width = intrinsic
        .max(tokens.geometry.minimum_hit_size.0)
        .min(ui.available_width().max(tokens.geometry.minimum_hit_size.0));
    let hit_height = tokens
        .geometry
        .minimum_hit_size
        .0
        .max(tokens.geometry.control_height.0 * font_scale.clamp(1.0, 1.5));
    let (_, hit_rect) = ui.allocate_space(egui::vec2(width, hit_height));
    let response = ui.interact(
        hit_rect,
        egui::Id::new((
            "polyorama.action-button",
            spec.target.action.stable_id(),
            spec.target.pane,
        )),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if response.clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            enabled,
            spec.selected,
            action.label,
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::Button);
        node.set_label(action.label);
        node.set_author_id(spec.target.semantic_id());
        let description = spec.availability.disabled_reason().map_or_else(
            || action.description.to_owned(),
            |reason| format!("{}; unavailable: {reason}", action.description),
        );
        node.set_description(description);
        if !enabled {
            node.set_disabled();
        }
        node.set_selected(spec.selected);
        if enabled {
            node.add_action(Action::Click);
        }
    });
    let visual_height =
        (tokens.geometry.control_height.0 * font_scale.clamp(1.0, 1.5)).min(hit_rect.height());
    let visual = Rect::from_center_size(hit_rect.center(), egui::vec2(width, visual_height));
    let fill = if !enabled {
        Color32::from(tokens.colours.surface_raised).linear_multiply(0.55)
    } else if spec.emphasis == ActionEmphasis::Primary || response.is_pointer_button_down_on() {
        tokens.colours.accent_primary.into()
    } else if spec.selected || response.hovered() {
        tokens.colours.selection_background.into()
    } else if spec.emphasis == ActionEmphasis::Quiet {
        Color32::TRANSPARENT
    } else {
        tokens.colours.surface_raised.into()
    };
    ui.painter().rect(
        visual,
        tokens.geometry.control_radius.0,
        fill,
        Stroke::new(1.0, tokens.colours.border_subtle),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            visual,
            tokens.geometry.control_radius.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let label_rect = visual.shrink2(egui::vec2(tokens.geometry.control_padding_x.0, 0.0));
    if let Ok(mut measured) = measure_text(
        ui.painter(),
        visible_label,
        text_spec,
        tokens,
        font_scale,
        label_rect.width().max(0.5),
    ) {
        if spec.emphasis == ActionEmphasis::Primary || response.is_pointer_button_down_on() {
            measured.colour = tokens.colours.accent_on_accent.into();
        } else if !enabled {
            measured.colour = tokens.colours.text_muted.into();
        }
        let truncated = measured.truncated();
        observations.push(paint_measured_text(
            &ui.painter_at(label_rect),
            &measured,
            label_rect,
            TextComponentId::new(
                crate::TextComponentKind::ActionButton,
                crate::actions::stable_action_hash(spec.target.action, spec.target.pane),
            ),
            None,
        ));
        if truncated || spec.compact || !enabled {
            let mut tooltip = format!("{}\n{}", action.label, action.description);
            if let Some(reason) = spec.availability.disabled_reason() {
                tooltip.push_str(&format!("\nUnavailable: {reason}"));
            }
            response.clone().on_hover_text(tooltip);
        }
    }
    response
}

pub fn action_semantic_node<A: ActionKey>(
    response: &Response,
    target: ActionTarget<A>,
    availability: &Availability,
    selected: bool,
    parent: SemanticUiId,
) -> UiNode {
    let action = target.action.specification();
    UiNode {
        id: SemanticUiId::new(target.semantic_id()),
        parent: Some(parent),
        role: UiRole::Button,
        name: action.label.to_owned(),
        description: Some(action.description.to_owned()),
        rect: response.rect.into(),
        enabled: availability.enabled(),
        focused: response.has_focus(),
        selected,
        checked: None,
        expanded: None,
        pane: target.pane,
        domain_reference: target.pane.map(DomainReference::Pane),
        actions: vec![SemanticActionId::from_action(target.action)],
        text_selectable: false,
        disabled_reason: availability.disabled_reason().map(ToOwned::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccessKitMismatch, DensityVariant, ThemeVariant, UiSnapshot, audit_accesskit,
        test_actions::TestAction,
    };

    #[test]
    fn action_snapshot_and_accesskit_semantics_cannot_disagree_silently() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let root_rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 120.0));
        let availability = Availability::Disabled {
            reason: "History is empty".into(),
        };
        let target = ActionTarget::application(TestAction::Undo);
        let mut semantic = None;
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root_rect),
                ..Default::default()
            },
            |ui| {
                let response = action_button(
                    ui,
                    ActionButtonSpec {
                        target,
                        availability: availability.clone(),
                        selected: false,
                        emphasis: ActionEmphasis::Normal,
                        compact: false,
                    },
                    &tokens,
                    1.0,
                    &mut Vec::new(),
                );
                semantic = Some(action_semantic_node(
                    &response,
                    target,
                    &availability,
                    false,
                    SemanticUiId::root(),
                ));
            },
        );
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        output.textures_delta.clear();
        let root = SemanticUiId::root();
        let mut snapshot = UiSnapshot {
            root: root.clone(),
            nodes: vec![
                UiNode::container(root, None, UiRole::Application, root_rect.into()),
                semantic.expect("semantic action node"),
            ],
            ..Default::default()
        };
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        snapshot.nodes[1].name = "Wrong name".into();
        assert!(matches!(
            audit_accesskit(&snapshot, &update).as_slice(),
            [AccessKitMismatch::Name { .. }]
        ));
        snapshot.nodes[1].name = "Undo".into();
        snapshot.nodes[1].description = Some("Wrong description".into());
        assert!(matches!(
            audit_accesskit(&snapshot, &update).as_slice(),
            [AccessKitMismatch::Description { .. }]
        ));
    }

    #[test]
    fn released_egui_kittest_queries_and_activates_registry_actions() {
        use std::{cell::Cell, rc::Rc};

        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        let activated = Rc::new(Cell::new(false));
        let observed = Rc::clone(&activated);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(420.0, 120.0))
            .build_ui(move |ui| {
                let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
                let fit = action_button(
                    ui,
                    ActionButtonSpec {
                        target: ActionTarget::pane(TestAction::FitView, polyorama_core::PaneId(1)),
                        availability: Availability::Enabled,
                        selected: false,
                        emphasis: ActionEmphasis::Normal,
                        compact: false,
                    },
                    &tokens,
                    1.0,
                    &mut Vec::new(),
                );
                observed.set(observed.get() || fit.clicked());
                action_button(
                    ui,
                    ActionButtonSpec {
                        target: ActionTarget::application(TestAction::Undo),
                        availability: Availability::Disabled {
                            reason: "History is empty".into(),
                        },
                        selected: false,
                        emphasis: ActionEmphasis::Normal,
                        compact: false,
                    },
                    &tokens,
                    1.0,
                    &mut Vec::new(),
                );
            });
        let fit = harness.get_by_role_and_label(egui::accesskit::Role::Button, "Fit view");
        assert!(!fit.accesskit_node().is_disabled());
        fit.click_accesskit();
        harness.run();
        assert!(activated.get());
        let undo = harness.get_by_role_and_label(egui::accesskit::Role::Button, "Undo");
        assert!(undo.accesskit_node().is_disabled());
    }
}
