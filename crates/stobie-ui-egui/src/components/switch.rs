use egui::{Color32, Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, PresentationId,
    SemanticActionId, SemanticUiId, TextComponentId, TextComponentKind, TextLayoutObservation,
    TextOverflow, TextRole, TextRoleStyles, TextSpec, UiNode, UiRole, measure_component_text,
    paint_measured_text,
};

/// One on or off setting. The label names it, visibly and for assistive
/// technology, and stays in view in both states; the track and thumb show
/// the state by position and fill, never by the row's background.
#[derive(Clone, Copy, Debug)]
pub struct SwitchSpec<'a> {
    pub label: &'a str,
    /// Whether the setting is on. The application owns it: activating the
    /// switch performs its action, which turns the setting over.
    pub on: bool,
}

/// The track's width and height, grown with text to 150%.
pub fn switch_track_size(tokens: &DesignTokens, font_scale: f32) -> egui::Vec2 {
    let scale = if font_scale.is_finite() {
        font_scale.clamp(1.0, 1.5)
    } else {
        1.0
    };
    egui::vec2(
        tokens.switch_track_width().0 * scale,
        tokens.switch_track_height().0 * scale,
    )
}

/// The height of a switch row: its label or its track within block padding,
/// whichever is taller, and at least the token hit size.
pub fn switch_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    let label = TextRole::ButtonLabel.style(tokens, font_scale).line_height;
    let track = switch_track_size(tokens, font_scale).y;
    tokens
        .geometry
        .minimum_hit_size
        .0
        .max(label.max(track) + tokens.spacing.block.0 * 2.0)
}

/// The track's fill, outline and thumb colours for a state.
pub(crate) struct SwitchColours {
    pub(crate) track: Option<Color32>,
    pub(crate) outline: Color32,
    pub(crate) thumb: Color32,
}

pub(crate) fn switch_colours(tokens: &DesignTokens, on: bool, enabled: bool) -> SwitchColours {
    let colours = &tokens.colours;
    match (on, enabled) {
        (true, true) => SwitchColours {
            track: Some(colours.accent_primary.into()),
            outline: colours.accent_primary.into(),
            thumb: colours.accent_on_accent.into(),
        },
        (false, true) => SwitchColours {
            track: None,
            outline: colours.text_muted.into(),
            thumb: colours.text_muted.into(),
        },
        // A disabled switch is quieter than an enabled one: a raised track
        // with a decorative outline, its thumb still at the end.
        (true, false) => SwitchColours {
            track: Some(colours.surface_raised.into()),
            outline: colours.border_decorative.into(),
            thumb: colours.border_decorative.into(),
        },
        (false, false) => SwitchColours {
            track: None,
            outline: colours.border_decorative.into(),
            thumb: colours.border_decorative.into(),
        },
    }
}

/// A full-width switch row whose surface performs `target`, named by its
/// label, with the track at the row's end. Hosted applications use
/// [`Frame::switch`](crate::Frame::switch); use [`switch_semantic_node`] for
/// low-level publication. Hidden availability must be filtered before
/// calling this recipe. Each enabled switch is a Tab stop; Enter, Space and
/// a pointer release share `clicked()`.
#[allow(clippy::too_many_arguments)]
pub fn switch<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: &SwitchSpec<'_>,
    target: ActionTarget<A>,
    availability: &Availability,
    identity: PresentationId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    assert!(
        availability.visible(),
        "filter hidden switches before presentation"
    );
    let action = target.action.specification();
    let enabled = availability.enabled() && ui.is_enabled();
    let track_size = switch_track_size(tokens, font_scale);
    let padding = tokens.geometry.control_padding_x.0;
    let width = ui
        .available_width()
        .max(tokens.geometry.minimum_hit_size.0 + track_size.x + padding * 2.0);
    let (_, rect) = ui.allocate_space(egui::vec2(width, switch_height(tokens, font_scale)));
    let response = ui.interact(
        rect,
        identity.egui_id(),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if response.clicked() {
        response.request_focus();
    }
    let description = action.description;
    if response.interact_rect.is_positive() {
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, spec.on, spec.label)
        });
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::{Action, Role, Toggled};
            node.set_role(Role::Switch);
            node.set_label(spec.label);
            node.set_author_id(identity.semantic_id().0);
            node.set_toggled(if spec.on {
                Toggled::True
            } else {
                Toggled::False
            });
            node.clear_selected();
            node.set_description(availability.disabled_reason().map_or_else(
                || description.to_owned(),
                |reason| format!("{description}; unavailable: {reason}"),
            ));
            if enabled {
                node.add_action(Action::Click);
            } else {
                node.set_disabled();
                node.remove_action(Action::Click);
            }
            let bounds = response.interact_rect;
            node.set_bounds(egui::accesskit::Rect {
                x0: f64::from(bounds.min.x),
                y0: f64::from(bounds.min.y),
                x1: f64::from(bounds.max.x),
                y1: f64::from(bounds.max.y),
            });
        });
    }
    let radius = tokens.geometry.control_radius.0;
    let pointer_down = enabled && response.is_pointer_button_down_on();
    // The row's fill shows only hover and press; the state is the track's.
    if pointer_down {
        ui.painter()
            .rect_filled(rect, radius, tokens.colours.selection_background);
        let inset = tokens.spacing.unit.0 * 0.5;
        ui.painter().rect_stroke(
            rect.shrink(inset),
            (radius - inset).max(0.0),
            Stroke::new(1.0, tokens.colours.selection_indicator),
            egui::StrokeKind::Inside,
        );
    } else if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, radius, tokens.colours.action_quiet_hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let track = Rect::from_center_size(
        egui::pos2(rect.right() - padding - track_size.x * 0.5, rect.center().y),
        track_size,
    );
    paint_switch_track(ui.painter(), track, spec.on, enabled, tokens);
    let mut foreground = *tokens;
    if !enabled {
        foreground.colours.text_primary = tokens.colours.text_muted;
    }
    let label_rect = Rect::from_min_max(
        egui::pos2(rect.left() + padding, rect.top()),
        egui::pos2(track.left() - tokens.spacing.inline.0, rect.bottom()),
    );
    let measured = measure_component_text(
        ui.painter(),
        spec.label,
        TextSpec::single_line(TextRole::ButtonLabel, TextOverflow::Ellipsis),
        &foreground,
        font_scale,
        label_rect.width().max(0.5),
    );
    let truncated = measured.truncated();
    let observation = paint_measured_text(
        ui.painter(),
        &measured,
        label_rect,
        TextComponentId::new(TextComponentKind::SwitchLabel, identity.text_instance()),
        None,
    );
    if ui.clip_rect().intersects(label_rect) || observation.layout_error.is_some() {
        observations.push(observation);
    }
    if truncated || !enabled {
        let mut tooltip = format!("{}\n{description}", spec.label);
        if let Some(reason) = availability.disabled_reason() {
            tooltip.push_str(&format!("\nUnavailable: {reason}"));
        }
        response.clone().on_hover_text(tooltip);
    }
    response
}

/// Paint a switch's track and thumb into `track`: on, the thumb sits at the
/// end of an accent track; off, at the start of an outlined one.
pub fn paint_switch_track(
    painter: &egui::Painter,
    track: Rect,
    on: bool,
    enabled: bool,
    tokens: &DesignTokens,
) {
    let colours = switch_colours(tokens, on, enabled);
    let radius = track.height() * 0.5;
    if let Some(fill) = colours.track {
        painter.rect_filled(track, radius, fill);
    }
    painter.rect_stroke(
        track,
        radius,
        Stroke::new(1.0, colours.outline),
        egui::StrokeKind::Inside,
    );
    let inset = (track.height() * 0.15).max(2.0);
    let thumb_radius = radius - inset;
    let thumb_x = if on {
        track.right() - radius
    } else {
        track.left() + radius
    };
    painter.circle_filled(
        egui::pos2(thumb_x, track.center().y),
        thumb_radius.max(1.0),
        colours.thumb,
    );
}

/// The snapshot owner matching [`switch`]: a switch named by its label,
/// whose `checked` is its state, presenting `target`'s action with the
/// argument it carries.
pub fn switch_semantic_node<A: ActionKey>(
    response: &Response,
    spec: &SwitchSpec<'_>,
    target: ActionTarget<A>,
    availability: &Availability,
    argument: Option<serde_json::Value>,
    identity: PresentationId,
    parent: SemanticUiId,
) -> UiNode {
    let action = target.action.specification();
    let mut node = UiNode::container(
        identity.semantic_id(),
        Some(parent),
        UiRole::Switch,
        response.interact_rect.into(),
    );
    node.name = spec.label.to_owned();
    node.description = Some(action.description.to_owned());
    node.checked = Some(spec.on);
    node.enabled = availability.enabled() && response.enabled();
    node.focused = response.has_focus();
    node.pane = target.pane;
    node.domain_reference = target.pane.map(DomainReference::Pane);
    node.actions = vec![SemanticActionId::from_action(target.action)];
    node.disabled_reason = availability.disabled_reason().map(ToOwned::to_owned);
    node.argument = argument;
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DensityVariant, PresentationScope, ThemeVariant, components::reading::contrast_ratio,
        test_actions::TestAction,
    };

    const THEMES: [ThemeVariant; 4] = [
        ThemeVariant::Dark,
        ThemeVariant::Light,
        ThemeVariant::DarkHighContrast,
        ThemeVariant::LightHighContrast,
    ];

    /// Present one switch `width` points wide and return its node, text,
    /// height and AccessKit node.
    fn present(
        spec: SwitchSpec<'_>,
        availability: Availability,
        width: f32,
    ) -> (
        UiNode,
        Vec<TextLayoutObservation>,
        f32,
        egui::accesskit::Node,
    ) {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        context.enable_accesskit();
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let identity = PresentationScope::new("settings").instance(("switch", 3));
        let mut result = None;
        let mut accesskit = None;
        // The second pass sees the fonts the first one loaded.
        for _ in 0..2 {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(width);
                let mut observations = Vec::new();
                let target = ActionTarget::application(TestAction::AppearanceSettings);
                let response = switch(
                    ui,
                    &spec,
                    target,
                    &availability,
                    identity,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                let node = switch_semantic_node(
                    &response,
                    &spec,
                    target,
                    &availability,
                    None,
                    identity,
                    SemanticUiId::root(),
                );
                result = Some((node, observations, response.rect.height()));
            });
            output.textures_delta.clear();
            accesskit = output
                .platform_output
                .accesskit_update
                .take()
                .expect("AccessKit update")
                .nodes
                .into_iter()
                .map(|(_, node)| node)
                .find(|node| node.author_id() == Some(identity.semantic_id().0.as_str()));
        }
        let (node, text, height) = result.expect("presented");
        (
            node,
            text,
            height,
            accesskit.expect("switch AccessKit node"),
        )
    }

    #[test]
    fn a_switch_keeps_its_label_and_states_on_or_off() {
        for on in [true, false] {
            let (node, text, height, accesskit) = present(
                SwitchSpec {
                    label: "Show hidden Bots",
                    on,
                },
                Availability::Enabled,
                360.0,
            );
            assert_eq!(node.role, UiRole::Switch);
            assert_eq!(node.name, "Show hidden Bots");
            assert_eq!(node.checked, Some(on));
            assert!(node.enabled);
            // The label is in view in both states.
            assert_eq!(text.len(), 1);
            assert_eq!(text[0].component_id.kind, TextComponentKind::SwitchLabel);
            assert!(!text[0].truncated);
            assert_eq!(accesskit.role(), egui::accesskit::Role::Switch);
            assert_eq!(accesskit.label(), Some("Show hidden Bots"));
            assert_eq!(
                accesskit.toggled(),
                Some(if on {
                    egui::accesskit::Toggled::True
                } else {
                    egui::accesskit::Toggled::False
                })
            );
            assert!(accesskit.supports_action(egui::accesskit::Action::Click));
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            assert_eq!(height, switch_height(&tokens, 1.0));
            assert!(height >= tokens.geometry.minimum_hit_size.0);
        }
    }

    #[test]
    fn a_disabled_switch_keeps_its_state_and_reason() {
        let (node, _, _, accesskit) = present(
            SwitchSpec {
                label: "Notifications",
                on: true,
            },
            Availability::Disabled {
                reason: "Notifications are blocked in this browser".into(),
            },
            360.0,
        );
        assert!(!node.enabled);
        assert_eq!(node.checked, Some(true));
        assert_eq!(
            node.disabled_reason.as_deref(),
            Some("Notifications are blocked in this browser")
        );
        assert!(accesskit.is_disabled());
        assert!(!accesskit.supports_action(egui::accesskit::Action::Click));
    }

    #[test]
    fn a_narrow_switch_elides_its_label_beside_a_whole_track() {
        let label = "Keep every conversation with this assistant at the top of the list";
        let (node, text, _, _) = present(
            SwitchSpec { label, on: false },
            Availability::Enabled,
            160.0,
        );
        assert_eq!(node.name, label);
        assert!(text[0].truncated);
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let track = switch_track_size(&tokens, 1.0).x;
        assert!(text[0].allocated_rect.max_x <= node.rect.max_x - track);
    }

    #[test]
    fn the_on_state_is_not_the_quiet_hover_fill_and_every_state_is_visible() {
        for theme in THEMES {
            let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
            let on = switch_colours(&tokens, true, true);
            let off = switch_colours(&tokens, false, true);
            let hover: Color32 = tokens.colours.action_quiet_hover.into();
            assert_ne!(on.track, Some(hover), "{theme:?}");
            assert_ne!(on.track, off.track, "{theme:?}");
            for surface in [
                tokens.colours.surface_canvas,
                tokens.colours.surface_panel,
                tokens.colours.surface_raised,
                tokens.colours.action_quiet_hover,
            ] {
                let surface: Color32 = surface.into();
                // Non-text contrast: the on track and the off outline and
                // thumb each stand out from the row at 3:1.
                assert!(
                    contrast_ratio(on.track.unwrap(), surface) >= 3.0,
                    "{theme:?} on track"
                );
                assert!(
                    contrast_ratio(off.outline, surface) >= 3.0,
                    "{theme:?} off outline"
                );
                assert!(
                    contrast_ratio(off.thumb, surface) >= 3.0,
                    "{theme:?} off thumb"
                );
            }
            assert!(
                contrast_ratio(on.thumb, on.track.unwrap()) >= 3.0,
                "{theme:?} on thumb"
            );
            // A disabled switch's track never stands out more than an
            // enabled one's, in either state, and its thumb shows on it.
            let panel: Color32 = tokens.colours.surface_panel.into();
            for state in [true, false] {
                let disabled = switch_colours(&tokens, state, false);
                let enabled = switch_colours(&tokens, state, true);
                let fill = |colours: &SwitchColours| colours.track.unwrap_or(panel);
                assert!(
                    contrast_ratio(fill(&disabled), panel) <= contrast_ratio(fill(&enabled), panel),
                    "{theme:?} disabled track, on {state}"
                );
                assert_ne!(
                    disabled.thumb,
                    fill(&disabled),
                    "{theme:?} disabled thumb, on {state}"
                );
            }
        }
    }
}
