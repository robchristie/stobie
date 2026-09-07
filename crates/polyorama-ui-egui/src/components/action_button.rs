use egui::{Color32, Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, HorizontalTextAlignment,
    SemanticActionId, SemanticUiId, TextComponentId, TextOverflow, TextRole, TextSpec, UiNode,
    UiRole, measure_text, paint_measured_text,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionEmphasis {
    Quiet,
    /// Transparent resting chrome with a hover fill and independent focus ring.
    QuietBorderless,
    Normal,
    Primary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionButtonState {
    Momentary,
    Toggle { pressed: bool },
}

impl ActionButtonState {
    fn pressed(self) -> bool {
        matches!(self, Self::Toggle { pressed: true })
    }

    fn toggled(self) -> Option<bool> {
        match self {
            Self::Momentary => None,
            Self::Toggle { pressed } => Some(pressed),
        }
    }
}

pub struct ActionButtonSpec<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub availability: Availability,
    pub state: ActionButtonState,
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
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, action.label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role, Toggled};
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
        node.clear_selected();
        node.clear_toggled();
        if let Some(pressed) = spec.state.toggled() {
            node.set_toggled(if pressed {
                Toggled::True
            } else {
                Toggled::False
            });
        }
        if enabled {
            node.add_action(Action::Click);
        }
    });
    let visual_height =
        (tokens.geometry.control_height.0 * font_scale.clamp(1.0, 1.5)).min(hit_rect.height());
    let visual = Rect::from_center_size(hit_rect.center(), egui::vec2(width, visual_height));
    let fill = if !enabled {
        Color32::from(tokens.colours.surface_raised).linear_multiply(0.55)
    } else if spec.emphasis == ActionEmphasis::Primary {
        tokens.colours.action_primary_background.into()
    } else if spec.state.pressed() || response.is_pointer_button_down_on() {
        tokens.colours.selection_background.into()
    } else if response.hovered() {
        if matches!(
            spec.emphasis,
            ActionEmphasis::Quiet | ActionEmphasis::QuietBorderless
        ) {
            tokens.colours.action_quiet_hover.into()
        } else {
            tokens.colours.surface_hover.into()
        }
    } else if matches!(
        spec.emphasis,
        ActionEmphasis::Quiet | ActionEmphasis::QuietBorderless
    ) {
        Color32::TRANSPARENT
    } else {
        tokens.colours.surface_raised.into()
    };
    ui.painter().rect(
        visual,
        tokens.geometry.control_radius.0,
        fill,
        if spec.emphasis == ActionEmphasis::QuietBorderless {
            Stroke::NONE
        } else {
            Stroke::new(1.0, tokens.colours.border_control)
        },
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
    {
        // Resolve the single-colour label before layout: a galley's explicit
        // vertex colours take precedence over Painter::galley's fallback.
        let mut label_tokens = *tokens;
        label_tokens.colours.text_primary = if !enabled {
            tokens.colours.text_muted
        } else if spec.emphasis == ActionEmphasis::Primary {
            tokens.colours.action_primary_foreground
        } else {
            tokens.colours.text_primary
        };
        let measured = crate::measure_component_text(
            ui.painter(),
            visible_label,
            text_spec,
            &label_tokens,
            font_scale,
            label_rect.width().max(0.5),
        );
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
    state: ActionButtonState,
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
        selected: false,
        checked: state.toggled(),
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
    fn emitted_action_label_vertices_use_primary_pressed_and_disabled_foregrounds() {
        for (emphasis, disabled, pointer_down) in [
            (ActionEmphasis::Primary, false, false),
            (ActionEmphasis::Primary, false, true),
            (ActionEmphasis::Normal, false, true),
            (ActionEmphasis::Primary, true, false),
        ] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let mut centre = egui::Pos2::ZERO;
            let mut paint = |input| {
                context.run_ui(input, |ui| {
                    let response = action_button(
                        ui,
                        ActionButtonSpec {
                            target: ActionTarget::application(TestAction::Undo),
                            availability: if disabled {
                                Availability::Disabled {
                                    reason: "Unavailable".into(),
                                }
                            } else {
                                Availability::Enabled
                            },
                            state: ActionButtonState::Momentary,
                            emphasis,
                            compact: false,
                        },
                        &tokens,
                        1.0,
                        &mut Vec::new(),
                    );
                    centre = response.rect.center();
                })
            };
            paint(egui::RawInput::default()).textures_delta.clear();
            let events = if pointer_down {
                vec![
                    egui::Event::PointerMoved(centre),
                    egui::Event::PointerButton {
                        pos: centre,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]
            } else {
                Vec::new()
            };
            let mut output = context.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let response = action_button(
                        ui,
                        ActionButtonSpec {
                            target: ActionTarget::application(TestAction::Undo),
                            availability: if disabled {
                                Availability::Disabled {
                                    reason: "Unavailable".into(),
                                }
                            } else {
                                Availability::Enabled
                            },
                            state: ActionButtonState::Momentary,
                            emphasis,
                            compact: false,
                        },
                        &tokens,
                        1.0,
                        &mut Vec::new(),
                    );
                    assert_eq!(response.is_pointer_button_down_on(), pointer_down);
                },
            );
            output.textures_delta.clear();
            let expected: Color32 = if disabled {
                tokens.colours.text_muted
            } else if emphasis == ActionEmphasis::Primary {
                tokens.colours.action_primary_foreground
            } else {
                tokens.colours.text_primary
            }
            .into();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) => Some(text),
                    _ => None,
                })
                .collect();
            assert!(!labels.is_empty());
            for text in labels {
                assert_eq!(text.galley.job.text, "Undo");
                let vertices: Vec<_> = text
                    .galley
                    .rows
                    .iter()
                    .flat_map(|row| &row.visuals.mesh.vertices)
                    .collect();
                assert!(!vertices.is_empty());
                assert!(
                    vertices.iter().all(|vertex| vertex.color == expected),
                    "{emphasis:?}, disabled={disabled}, pressed={pointer_down}"
                );
            }
        }
    }

    #[test]
    fn action_snapshot_and_accesskit_semantics_cannot_disagree_silently() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
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
                        state: ActionButtonState::Momentary,
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
                    ActionButtonState::Momentary,
                    SemanticUiId::root(),
                ));
            },
        );
        let update = output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        let action_node = update
            .nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| node.author_id() == Some("action.undo"))
            .expect("momentary action node");
        assert_eq!(action_node.toggled(), None);
        assert_eq!(action_node.is_selected(), None);
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
                        state: ActionButtonState::Momentary,
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
                        state: ActionButtonState::Momentary,
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
