use egui::{Color32, Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, HorizontalTextAlignment,
    IconId, SemanticActionId, SemanticUiId, TextComponentId, TextOverflow, TextRole, TextSpec,
    UiNode, UiRole, icon_size, measure_text, paint_icon, paint_measured_text,
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

/// Explicit artwork/label presentation; capability and semantic names stay in the action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionButtonContent {
    Text,
    IconOnly(IconId),
    /// Leading decorative icon and one measured, start-aligned label.
    IconLabel(IconId),
}

/// Widget, semantic and text identities for one displayed capability instance.
/// Existing callers retain their historical target-derived identity.
#[derive(Clone, Debug)]
pub struct ActionButtonIdentity {
    pub widget_id: egui::Id,
    pub semantic_id: SemanticUiId,
    pub text_instance: u64,
}

impl ActionButtonIdentity {
    pub fn for_target<A: ActionKey>(target: ActionTarget<A>) -> Self {
        Self {
            widget_id: egui::Id::new((
                "stobie.action-button",
                target.action.stable_id(),
                target.pane,
            )),
            semantic_id: SemanticUiId::new(target.semantic_id()),
            text_instance: crate::actions::stable_action_hash(target.action, target.pane),
        }
    }
}

/// Presentation of a typed capability; callers own response handling and mutation.
/// A control whose availability is Hidden is neither drawn nor observed: it
/// allocates no space and returns an empty hover response.
pub struct ActionButtonSpec<A: ActionKey> {
    /// Typed capability and applicable pane identity.
    pub target: ActionTarget<A>,
    /// Enabled, disabled with an observable reason, or hidden (not drawn).
    pub availability: Availability,
    /// Momentary action or current toggle state, also retained in semantics.
    pub state: ActionButtonState,
    /// Token-defined visual prominence independent of capability identity.
    pub emphasis: ActionEmphasis,
    /// Select the compact label when supplied by the action specification.
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
    let identity = ActionButtonIdentity::for_target(spec.target);
    action_button_with_identity(ui, spec, &identity, tokens, font_scale, observations)
}

/// Render a distinct logical instance of a capability without changing its target.
pub fn action_button_with_identity<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: ActionButtonSpec<A>,
    identity: &ActionButtonIdentity,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    action_button_with_identity_and_content(
        ui,
        spec,
        identity,
        ActionButtonContent::Text,
        tokens,
        font_scale,
        observations,
    )
}

/// Render an explicit icon/text presentation using the historical target identity.
/// Labelled actions reserve horizontal padding, artwork, inline spacing and a
/// measured ellipsis as their minimum useful width. Parents must provide that
/// width or move the action into overflow. Labels never collapse implicitly.
pub fn action_button_with_content<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: ActionButtonSpec<A>,
    content: ActionButtonContent,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let identity = ActionButtonIdentity::for_target(spec.target);
    action_button_with_identity_and_content(
        ui,
        spec,
        &identity,
        content,
        tokens,
        font_scale,
        observations,
    )
}

/// Common action implementation for all presentations and logical identities.
pub fn action_button_with_identity_and_content<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: ActionButtonSpec<A>,
    identity: &ActionButtonIdentity,
    content: ActionButtonContent,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    present_action_button(
        ui,
        spec,
        identity,
        content,
        ControlLabels::default(),
        tokens,
        font_scale,
        observations,
    )
}

fn button_text_spec(content: ActionButtonContent) -> TextSpec {
    TextSpec {
        horizontal_alignment: if matches!(content, ActionButtonContent::IconLabel(_)) {
            HorizontalTextAlignment::Start
        } else {
            HorizontalTextAlignment::Centre
        },
        ..TextSpec::single_line(TextRole::ButtonLabel, TextOverflow::Ellipsis)
    }
}

/// An action button's height and widths before it is placed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ButtonSize {
    /// The hit target's height, at least the minimum hit size.
    pub hit_height: f32,
    /// The width that shows the whole label.
    pub intrinsic: f32,
    /// The narrowest width it takes, eliding its label.
    pub minimum_width: f32,
}

/// The size an action button showing `visible_label` as `content` takes.
pub(crate) fn action_button_size(
    ui: &egui::Ui,
    visible_label: &str,
    content: ActionButtonContent,
    tokens: &DesignTokens,
    font_scale: f32,
) -> ButtonSize {
    let text_spec = button_text_spec(content);
    let padding = tokens.geometry.control_padding_x.0 * 2.0;
    let artwork_size = icon_size(tokens, font_scale);
    let icon_reservation = if matches!(content, ActionButtonContent::IconLabel(_)) {
        artwork_size + tokens.spacing.inline.0
    } else {
        0.0
    };
    let hit_height = action_hit_height(tokens, font_scale);
    let (intrinsic, minimum_width) = if matches!(content, ActionButtonContent::IconOnly(_)) {
        let side = hit_height.max(artwork_size);
        (side, side)
    } else {
        let text_width = |label| {
            measure_text(
                ui.painter(),
                label,
                TextSpec {
                    overflow: TextOverflow::Expand,
                    ..text_spec
                },
                tokens,
                font_scale,
                4_096.0,
            )
            .map_or(tokens.geometry.minimum_hit_size.0, |text| text.size().x)
        };
        let minimum = if matches!(content, ActionButtonContent::IconLabel(_)) {
            (padding + icon_reservation + text_width("…")).max(tokens.geometry.minimum_hit_size.0)
        } else {
            tokens.geometry.minimum_hit_size.0
        };
        (
            text_width(visible_label) + padding + icon_reservation,
            minimum,
        )
    };
    ButtonSize {
        hit_height,
        intrinsic,
        minimum_width,
    }
}

/// An action button's hit height: the control height at the font scale, and
/// never less than the minimum hit size.
pub(crate) fn action_hit_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens
        .geometry
        .minimum_hit_size
        .0
        .max(tokens.geometry.control_height.0 * font_scale.clamp(1.0, 1.5))
}

/// A bound control's own name and label, in place of its action's.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ControlLabels<'a> {
    /// The accessible name and the tooltip's first line, such as "Open Ada"
    /// for one row's Open.
    pub name: Option<&'a str>,
    /// The visible label, such as "Tue" for one option of a weekday choice.
    pub visible: Option<&'a str>,
}

/// The action button, named and labelled by `labels` where they are given,
/// and otherwise by its action. A visible label that differs from the name
/// shows the name in a tooltip.
#[allow(clippy::too_many_arguments)]
pub(crate) fn present_action_button<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: ActionButtonSpec<A>,
    identity: &ActionButtonIdentity,
    content: ActionButtonContent,
    labels: ControlLabels<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    if !spec.availability.visible() {
        // Hidden: no space, paint, focus, semantic node or text observation.
        return ui.interact(Rect::NOTHING, identity.widget_id, Sense::hover());
    }
    let action = spec.target.action.specification();
    let name = labels.name.unwrap_or(action.label);
    let visible_label = labels.visible.unwrap_or(if spec.compact {
        action.compact_label.unwrap_or(action.label)
    } else {
        action.label
    });
    let face = ButtonFace {
        name,
        visible_label,
        description: Some(action.description),
        shortcut: action.shortcut.map(|shortcut| shortcut.aria_keyshortcuts()),
        availability: &spec.availability,
        state: spec.state,
        emphasis: spec.emphasis,
        content,
        also_fits: &[],
        tooltip: spec.compact || labels.visible.is_some_and(|visible| visible != name),
        tab_stop: true,
    };
    present_button(ui, &face, identity, tokens, font_scale, observations)
}

/// What a button shows and how it is named and described, apart from any
/// action: the action button's and the copy recipe's shared face.
pub(crate) struct ButtonFace<'a> {
    /// The accessible name, and the tooltip's first line.
    pub name: &'a str,
    pub visible_label: &'a str,
    pub description: Option<&'a str>,
    /// The `aria-keyshortcuts` form of the button's shortcut.
    pub shortcut: Option<String>,
    pub availability: &'a Availability,
    pub state: ActionButtonState,
    pub emphasis: ActionEmphasis,
    pub content: ActionButtonContent,
    /// Other labels the button shows in turn, whose width it keeps, so that
    /// changing between them moves nothing.
    pub also_fits: &'a [&'a str],
    /// Show the tooltip even when the whole label shows.
    pub tooltip: bool,
    /// Whether Tab stops on the enabled button. A roving group, such as a
    /// single choice, keeps one option a Tab stop and moves between the
    /// others with arrow keys; the rest still take clicks.
    pub tab_stop: bool,
}

/// Present a visible button's face: allocate it, sense clicks when enabled,
/// set its AccessKit node and paint it.
pub(crate) fn present_button(
    ui: &mut egui::Ui,
    face: &ButtonFace<'_>,
    identity: &ActionButtonIdentity,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let content = face.content;
    let name = face.name;
    let visible_label = face.visible_label;
    let enabled = face.availability.enabled();
    let text_spec = button_text_spec(content);
    let artwork_size = icon_size(tokens, font_scale);
    let icon_reservation = if matches!(content, ActionButtonContent::IconLabel(_)) {
        artwork_size + tokens.spacing.inline.0
    } else {
        0.0
    };
    let ButtonSize {
        hit_height,
        mut intrinsic,
        minimum_width,
    } = action_button_size(ui, visible_label, content, tokens, font_scale);
    for label in face.also_fits {
        intrinsic =
            intrinsic.max(action_button_size(ui, label, content, tokens, font_scale).intrinsic);
    }
    let width = intrinsic
        .max(minimum_width)
        .min(ui.available_width().max(minimum_width));
    let (_, hit_rect) = ui.allocate_space(egui::vec2(width, hit_height));
    let response = ui.interact(
        hit_rect,
        identity.widget_id,
        match (enabled, face.tab_stop) {
            (true, true) => Sense::click(),
            (true, false) => Sense::CLICK,
            (false, _) => Sense::hover(),
        },
    );
    if response.clicked() {
        response.request_focus();
    }
    if response.interact_rect.is_positive() {
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::{Action, Role, Toggled};
            node.set_role(Role::Button);
            node.set_label(name);
            node.set_author_id(identity.semantic_id.0.clone());
            if let Some(shortcut) = &face.shortcut {
                node.set_keyboard_shortcut(shortcut.clone());
            }
            // Match the current pointer target when a scroll area clips the control.
            if response.interact_rect.is_positive() {
                node.set_bounds(egui::accesskit::Rect {
                    x0: f64::from(response.interact_rect.min.x),
                    y0: f64::from(response.interact_rect.min.y),
                    x1: f64::from(response.interact_rect.max.x),
                    y1: f64::from(response.interact_rect.max.y),
                });
            }
            match (face.description, face.availability.disabled_reason()) {
                (description, Some(reason)) => node.set_description(format!(
                    "{}; unavailable: {reason}",
                    description.unwrap_or_default()
                )),
                (Some(description), None) => node.set_description(description),
                (None, None) => node.clear_description(),
            }
            if !enabled {
                node.set_disabled();
            }
            node.clear_selected();
            node.clear_toggled();
            if let Some(pressed) = face.state.toggled() {
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
    }
    let visual_height =
        (tokens.geometry.control_height.0 * font_scale.clamp(1.0, 1.5)).min(hit_rect.height());
    let visual_width = if matches!(content, ActionButtonContent::IconOnly(_)) {
        visual_height
    } else {
        width
    };
    let visual = Rect::from_center_size(hit_rect.center(), egui::vec2(visual_width, visual_height));
    let fill = if !enabled {
        Color32::from(tokens.colours.surface_raised).linear_multiply(0.55)
    } else if face.emphasis == ActionEmphasis::Primary {
        tokens.colours.action_primary_background.into()
    } else if face.state.pressed() || response.is_pointer_button_down_on() {
        tokens.colours.selection_background.into()
    } else if response.hovered() {
        if matches!(
            face.emphasis,
            ActionEmphasis::Quiet | ActionEmphasis::QuietBorderless
        ) {
            tokens.colours.action_quiet_hover.into()
        } else {
            tokens.colours.surface_hover.into()
        }
    } else if matches!(
        face.emphasis,
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
        if face.emphasis == ActionEmphasis::QuietBorderless {
            Stroke::NONE
        } else if enabled && face.emphasis == ActionEmphasis::Primary && response.hovered() {
            Stroke::new(1.0, tokens.colours.action_primary_foreground)
        } else {
            Stroke::new(1.0, tokens.colours.border_control)
        },
        egui::StrokeKind::Inside,
    );
    if enabled && (face.state.pressed() || response.is_pointer_button_down_on()) {
        // The reference theme aliases hover and selection fills. Keep active
        // feedback explicit, with geometry distinct from the outer focus ring.
        let inset = tokens.spacing.unit.0 * 0.5;
        ui.painter().rect_stroke(
            visual.shrink(inset),
            (tokens.geometry.control_radius.0 - inset).max(0.0),
            Stroke::new(
                1.0,
                if face.emphasis == ActionEmphasis::Primary {
                    tokens.colours.action_primary_foreground
                } else {
                    tokens.colours.selection_indicator
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            visual,
            tokens.geometry.control_radius.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    // Resolve artwork and label through exactly the same foreground rule.
    let mut label_tokens = *tokens;
    label_tokens.colours.text_primary = if !enabled {
        tokens.colours.text_muted
    } else if face.emphasis == ActionEmphasis::Primary {
        tokens.colours.action_primary_foreground
    } else {
        tokens.colours.text_primary
    };
    let foreground = label_tokens.colours.text_primary.into();
    let mut label_rect = visual.shrink2(egui::vec2(tokens.geometry.control_padding_x.0, 0.0));
    match content {
        ActionButtonContent::Text => {}
        ActionButtonContent::IconOnly(icon) => {
            paint_icon(
                ui.painter(),
                icon,
                Rect::from_center_size(visual.center(), egui::Vec2::splat(artwork_size)),
                foreground,
            );
        }
        ActionButtonContent::IconLabel(icon) => {
            let centre = egui::pos2(label_rect.left() + artwork_size * 0.5, visual.center().y);
            paint_icon(
                ui.painter(),
                icon,
                Rect::from_center_size(centre, egui::Vec2::splat(artwork_size)),
                foreground,
            );
            label_rect.min.x += icon_reservation;
        }
    }
    let mut truncated = false;
    if !matches!(content, ActionButtonContent::IconOnly(_)) {
        let measured = crate::measure_component_text(
            ui.painter(),
            visible_label,
            text_spec,
            &label_tokens,
            font_scale,
            label_rect.width().max(0.5),
        );
        truncated = measured.truncated();
        let observation = paint_measured_text(
            &ui.painter_at(label_rect),
            &measured,
            label_rect,
            TextComponentId::new(
                crate::TextComponentKind::ActionButton,
                identity.text_instance,
            ),
            None,
        );
        if response.interact_rect.is_positive() || observation.layout_error.is_some() {
            observations.push(observation);
        }
    }
    if matches!(content, ActionButtonContent::IconOnly(_)) || truncated || face.tooltip || !enabled
    {
        let mut tooltip = name.to_owned();
        if let Some(description) = face.description {
            tooltip.push_str(&format!("\n{description}"));
        }
        if let Some(reason) = face.availability.disabled_reason() {
            tooltip.push_str(&format!("\nUnavailable: {reason}"));
        }
        response.clone().on_hover_text(tooltip);
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
    action_semantic_node_with_identity(
        response,
        target,
        availability,
        state,
        parent,
        &ActionButtonIdentity::for_target(target),
    )
}

pub fn action_semantic_node_with_identity<A: ActionKey>(
    response: &Response,
    target: ActionTarget<A>,
    availability: &Availability,
    state: ActionButtonState,
    parent: SemanticUiId,
    identity: &ActionButtonIdentity,
) -> UiNode {
    let action = target.action.specification();
    UiNode {
        argument: None,
        value: None,
        id: identity.semantic_id.clone(),
        parent: Some(parent),
        role: UiRole::Button,
        name: action.label.to_owned(),
        description: Some(action.description.to_owned()),
        rect: response.interact_rect.into(),
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
        for (theme, (emphasis, disabled, pointer_down)) in [
            ThemeVariant::Light,
            ThemeVariant::Dark,
            ThemeVariant::LightHighContrast,
            ThemeVariant::DarkHighContrast,
        ]
        .into_iter()
        .flat_map(|theme| {
            [
                (ActionEmphasis::Primary, false, false),
                (ActionEmphasis::Primary, false, true),
                (ActionEmphasis::Normal, false, true),
                (ActionEmphasis::Primary, true, false),
            ]
            .map(|state| (theme, state))
        }) {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
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
                    "{theme:?}, {emphasis:?}, disabled={disabled}, pressed={pointer_down}"
                );
            }
        }
    }

    #[test]
    fn clipped_action_targets_match_accesskit_bounds() {
        for availability in [
            Availability::Enabled,
            Availability::Disabled {
                reason: "History is empty".into(),
            },
        ] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            context.enable_accesskit();
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let root_rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 120.0));
            let clip = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(360.0, 16.0));
            let target = ActionTarget::application(TestAction::Undo);
            let mut semantic = None;
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(root_rect),
                    ..Default::default()
                },
                |ui| {
                    ui.set_clip_rect(clip);
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
                    assert!(response.rect.height() > response.interact_rect.height());
                    assert_eq!(response.interact_rect.height(), 16.0);
                    semantic = Some(action_semantic_node(
                        &response,
                        target,
                        &availability,
                        ActionButtonState::Momentary,
                        SemanticUiId::root(),
                    ));
                },
            );
            let root = SemanticUiId::root();
            let snapshot = UiSnapshot {
                root: root.clone(),
                nodes: vec![
                    UiNode::container(root, None, UiRole::Application, root_rect.into()),
                    semantic.unwrap(),
                ],
                ..Default::default()
            };
            output.textures_delta.clear();
            let update = output.platform_output.accesskit_update.unwrap();
            assert!(snapshot.audit().is_empty());
            assert!(audit_accesskit(&snapshot, &update).is_empty());
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
                        target: ActionTarget::pane(TestAction::FitView, stobie_core::PaneId(1)),
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
