use egui::{Color32, Painter, Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, AvatarSpec, DesignTokens, DomainReference,
    PresentationId, SemanticActionId, SemanticUiId, TextComponentId, TextComponentKind,
    TextLayoutObservation, TextOverflow, TextRole, TextRoleStyles, TextSpec, UiNode, UiRole,
    avatar_size, components::reading::contrast_ratio, measure_component_text, paint_avatar,
    paint_measured_text,
};

/// The unread dot's diameter, grown with text to 150%.
pub fn unread_dot_size(tokens: &DesignTokens, font_scale: f32) -> f32 {
    let scale = if font_scale.is_finite() {
        font_scale.clamp(1.0, 1.5)
    } else {
        1.0
    };
    tokens.unread_dot_size().0 * scale
}

/// The unread dot's colour: the accent where it stands out at 3:1 from
/// `behind`, or from every surface when `behind` is unknown; otherwise
/// primary text, which always does.
pub fn unread_dot_colour(tokens: &DesignTokens, behind: Option<Color32>) -> Color32 {
    let accent: Color32 = tokens.colours.accent_primary.into();
    let surfaces: Vec<Color32> = match behind {
        Some(colour) => vec![colour],
        None => vec![
            tokens.colours.surface_canvas.into(),
            tokens.colours.surface_panel.into(),
            tokens.colours.surface_raised.into(),
        ],
    };
    if surfaces
        .iter()
        .all(|&surface| contrast_ratio(accent, surface) >= 3.0)
    {
        accent
    } else {
        tokens.colours.text_primary.into()
    }
}

/// A conversation row's fill: the selection fill for the current row (or
/// one being pressed), and the hover surface, which every authored theme
/// keeps apart from selection, for a hovered one.
fn row_fill(tokens: &DesignTokens, current: bool, hovered: bool) -> Option<Color32> {
    if current {
        Some(tokens.colours.selection_background.into())
    } else if hovered {
        Some(tokens.colours.surface_hover.into())
    } else {
        None
    }
}

/// Muted text's colour on `fill`: the muted colour where it reaches 4.5:1
/// on the fill, otherwise primary text. Theme validation already holds
/// muted text to 4.5:1 (7:1 in high contrast) on the hover, selection and
/// raised fills, so every authored theme and `ApplicationTheme` keeps its
/// muted text; this keeps tokens assembled by hand legible too.
pub(crate) fn muted_text_on(tokens: &DesignTokens, fill: Color32) -> crate::Rgba8 {
    let muted = tokens.colours.text_muted;
    if contrast_ratio(muted.into(), fill) >= 4.5 {
        muted
    } else {
        tokens.colours.text_primary
    }
}

/// Paint an unread dot centred in `rect`, at most its token size. Like
/// [`paint_avatar`], it creates no interaction or accessible owner: the
/// control it marks states that it is unread, in the application's words.
pub fn paint_unread_dot(
    painter: &Painter,
    rect: Rect,
    tokens: &DesignTokens,
    font_scale: f32,
    behind: Option<Color32>,
) {
    let diameter = unread_dot_size(tokens, font_scale)
        .min(rect.width())
        .min(rect.height());
    if diameter <= 0.0 || !rect.is_finite() {
        return;
    }
    painter.circle_filled(
        rect.center(),
        diameter * 0.5,
        unread_dot_colour(tokens, behind),
    );
}

/// Allocate and paint an unread dot at its size. The response senses hover
/// only; name the unread state in the control or text beside it.
pub fn unread_dot(ui: &mut egui::Ui, tokens: &DesignTokens, font_scale: f32) -> Response {
    let side = unread_dot_size(tokens, font_scale);
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(side), Sense::hover());
    paint_unread_dot(ui.painter(), rect, tokens, font_scale, None);
    response
}

/// One conversation in a list: an avatar, the name, a one-line preview of
/// the latest message, its time and an unread dot. The whole row performs
/// its action for that conversation, such as opening it.
#[derive(Clone, Copy, Debug)]
pub struct ConversationRowSpec<'a> {
    /// The conversation's name: one line, elided when narrow and complete
    /// in semantics.
    pub title: &'a str,
    /// The latest message, on one secondary line; elided.
    pub preview: Option<&'a str>,
    /// When the latest message arrived, already formatted by the
    /// application, such as with [`local_time`](crate::local_time)'s
    /// `compact`. It keeps its whole width up to half the title line.
    pub time: Option<&'a str>,
    /// The application's words for the unread state, such as "2 unread
    /// messages"; `Some` paints the unread dot and leads the description.
    pub unread: Option<&'a str>,
    /// Whether this is the conversation on show: the current item, not a
    /// toggle.
    pub selected: bool,
    /// A decorative avatar leading the row; the title names it.
    pub avatar: Option<AvatarSpec<'a>>,
}

impl ConversationRowSpec<'_> {
    /// The unread words, the preview and the time, then the action's
    /// description, each present part separated by "; ".
    pub fn description(&self, action: &str) -> String {
        let mut parts: Vec<&str> = [self.unread, self.preview, self.time]
            .into_iter()
            .flatten()
            .collect();
        parts.push(action);
        parts.join("; ")
    }
}

/// The height of a conversation row: its title and preview lines or its
/// avatar within block padding, whichever is taller, and at least the token
/// hit size.
pub fn conversation_row_height(
    spec: &ConversationRowSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
) -> f32 {
    let title = TextRole::ButtonLabel.style(tokens, font_scale).line_height;
    let preview = spec.preview.map_or(0.0, |_| {
        TextRole::Secondary.style(tokens, font_scale).line_height
    });
    let avatar = spec
        .avatar
        .map_or(0.0, |avatar| avatar_size(tokens, avatar.size, font_scale));
    tokens
        .geometry
        .minimum_hit_size
        .0
        .max((title + preview).max(avatar) + tokens.spacing.block.0 * 2.0)
}

/// A full-width conversation row whose surface activates `target`, named
/// by its title. Hosted applications use
/// [`Frame::conversation_row`](crate::Frame::conversation_row) with a
/// [`BoundAction`](crate::BoundAction) carrying the conversation's ID; use
/// [`conversation_row_semantic_node`] for low-level publication. Hidden
/// availability must be filtered before calling this recipe. Each enabled
/// row is a Tab stop; Enter, Space and a pointer release share `clicked()`.
#[allow(clippy::too_many_arguments)]
pub fn conversation_row<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: &ConversationRowSpec<'_>,
    target: ActionTarget<A>,
    availability: &Availability,
    identity: PresentationId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    assert!(
        availability.visible(),
        "filter hidden conversation rows before presentation"
    );
    let action = target.action.specification();
    let enabled = availability.enabled() && ui.is_enabled();
    let width = ui
        .available_width()
        .max(tokens.geometry.minimum_hit_size.0 * 3.0);
    let (_, rect) = ui.allocate_space(egui::vec2(
        width,
        conversation_row_height(spec, tokens, font_scale),
    ));
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
    let description = spec.description(action.description);
    if response.interact_rect.is_positive() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, spec.title)
        });
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::{Action, AriaCurrent, Role};
            node.set_role(Role::Button);
            node.set_label(spec.title);
            node.set_author_id(identity.semantic_id().0);
            node.clear_toggled();
            node.clear_selected();
            node.set_aria_current(if spec.selected {
                AriaCurrent::True
            } else {
                AriaCurrent::False
            });
            node.set_description(availability.disabled_reason().map_or_else(
                || description.clone(),
                |reason| crate::semantics::unavailable_description(Some(&description), reason),
            ));
            if enabled {
                node.add_action(Action::Click);
            } else {
                node.set_disabled();
                node.remove_action(Action::Click);
            }
            // The current pointer target when a scroll area clips the row.
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
    let behind = row_fill(
        tokens,
        spec.selected || pointer_down,
        enabled && response.hovered(),
    );
    if let Some(fill) = behind {
        ui.painter().rect_filled(rect, radius, fill);
    }
    if spec.selected {
        let marker_width = tokens.spacing.unit.0 * 0.5;
        let marker = Rect::from_min_max(
            rect.min + egui::vec2(tokens.spacing.unit.0, tokens.spacing.block.0),
            egui::pos2(
                rect.left() + tokens.spacing.unit.0 + marker_width,
                rect.bottom() - tokens.spacing.block.0,
            ),
        );
        ui.painter()
            .rect_filled(marker, radius, tokens.colours.selection_indicator);
    }
    if enabled && spec.selected && response.hovered() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, tokens.colours.border_control),
            egui::StrokeKind::Inside,
        );
    }
    if pointer_down {
        let inset = tokens.spacing.unit.0 * 0.5;
        ui.painter().rect_stroke(
            rect.shrink(inset),
            (radius - inset).max(0.0),
            Stroke::new(1.0, tokens.colours.selection_indicator),
            egui::StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let inner = rect.shrink2(egui::vec2(
        tokens.geometry.control_padding_x.0,
        tokens.spacing.block.0,
    ));
    let mut left = inner.left();
    if let Some(avatar) = spec.avatar {
        let side = avatar_size(tokens, avatar.size, font_scale);
        let avatar_rect = Rect::from_min_size(
            egui::pos2(left, rect.center().y - side * 0.5),
            egui::Vec2::splat(side),
        );
        paint_avatar(ui.painter(), &avatar, avatar_rect, tokens);
        left = avatar_rect.right() + tokens.spacing.section.0;
    }
    let mut foreground = *tokens;
    if let Some(fill) = behind {
        // The preview and time stay legible on the row's fill.
        foreground.colours.text_muted = muted_text_on(tokens, fill);
    }
    if !enabled {
        foreground.colours.text_primary = foreground.colours.text_muted;
    }
    let parent = TextComponentId::new(TextComponentKind::ConversationRow, identity.text_instance());
    let title_height = TextRole::ButtonLabel.style(tokens, font_scale).line_height;
    let preview_height = spec.preview.map_or(0.0, |_| {
        TextRole::Secondary.style(tokens, font_scale).line_height
    });
    // Centre the lines in a row taller than they need, such as beside an avatar.
    let top = inner.top() + ((inner.height() - title_height - preview_height) * 0.5).max(0.0);
    let mut title_rect = Rect::from_min_max(
        egui::pos2(left, top),
        egui::pos2(inner.right(), top + title_height),
    );
    let mut truncated = false;
    let mut paint =
        |text: &str, spec: TextSpec, tokens: &DesignTokens, area: Rect, kind: TextComponentKind| {
            let measured = measure_component_text(
                ui.painter(),
                text,
                spec,
                tokens,
                font_scale,
                area.width().max(0.5),
            );
            truncated |= measured.truncated();
            let observation = paint_measured_text(
                ui.painter(),
                &measured,
                area,
                TextComponentId::new(kind, identity.text_instance()),
                Some(parent),
            );
            if ui.clip_rect().intersects(area) || observation.layout_error.is_some() {
                observations.push(observation);
            }
        };
    if let Some(time) = spec.time {
        // The time keeps its whole width, up to half the line; the title
        // takes what is left.
        let time_spec = TextSpec {
            horizontal_alignment: crate::HorizontalTextAlignment::End,
            ..TextSpec::single_line(TextRole::Caption, TextOverflow::Ellipsis)
        };
        let time_width = crate::measure_text(
            ui.painter(),
            time,
            TextSpec::single_line(TextRole::Caption, TextOverflow::Expand),
            &foreground,
            font_scale,
            inner.width().max(0.5),
        )
        .map_or(0.0, |measured| measured.size().x)
        .min(title_rect.width() * 0.5);
        let time_rect = Rect::from_min_max(
            egui::pos2(title_rect.right() - time_width, title_rect.top()),
            title_rect.max,
        );
        title_rect.max.x = time_rect.left() - tokens.spacing.inline.0;
        paint(
            time,
            time_spec,
            &foreground,
            time_rect,
            TextComponentKind::ConversationRowTime,
        );
    }
    paint(
        spec.title,
        TextSpec::single_line(TextRole::ButtonLabel, TextOverflow::Ellipsis),
        &foreground,
        title_rect,
        TextComponentKind::ConversationRowTitle,
    );
    let mut preview_rect = Rect::from_min_max(
        egui::pos2(left, title_rect.bottom()),
        egui::pos2(inner.right(), title_rect.bottom() + preview_height),
    );
    let dot = unread_dot_size(tokens, font_scale);
    if spec.unread.is_some() {
        // The dot sits at the end of the preview line, or of the title line
        // when there is no preview.
        let line = if spec.preview.is_some() {
            preview_rect
        } else {
            Rect::from_min_max(
                egui::pos2(left, title_rect.bottom()),
                egui::pos2(inner.right(), title_rect.bottom() + dot * 2.0),
            )
        };
        let dot_rect = Rect::from_center_size(
            egui::pos2(line.right() - dot * 0.5, line.center().y),
            egui::Vec2::splat(dot),
        );
        paint_unread_dot(ui.painter(), dot_rect, tokens, font_scale, behind);
        preview_rect.max.x = dot_rect.left() - tokens.spacing.inline.0;
    }
    if let Some(preview) = spec.preview {
        // An unread preview reads in primary text; a read one is quieter.
        let mut preview_tokens = foreground;
        if spec.unread.is_some() && enabled {
            preview_tokens.colours.text_muted = tokens.colours.text_primary;
        }
        paint(
            preview,
            TextSpec::single_line(TextRole::Secondary, TextOverflow::Ellipsis),
            &preview_tokens,
            preview_rect,
            TextComponentKind::ConversationRowPreview,
        );
    }
    if truncated || !enabled {
        let mut tooltip = format!("{}\n{}", spec.title, description.replace("; ", "\n"));
        if let Some(reason) = availability.disabled_reason() {
            tooltip.push_str(&format!("\nUnavailable: {reason}"));
        }
        response.clone().on_hover_text(tooltip);
    }
    response
}

/// The snapshot owner matching [`conversation_row`]: a navigation item
/// named by the conversation's title, `selected` while it is on show, whose
/// description holds the unread words, preview and time, presenting
/// `target`'s action with the argument it carries.
pub fn conversation_row_semantic_node<A: ActionKey>(
    response: &Response,
    spec: &ConversationRowSpec<'_>,
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
        UiRole::NavigationItem,
        response.interact_rect.into(),
    );
    node.name = spec.title.to_owned();
    node.description = Some(spec.description(action.description));
    node.selected = spec.selected;
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
        AvatarFigure, AvatarSize, DensityVariant, PresentationScope, ThemeVariant,
        test_actions::TestAction,
    };

    const THEMES: [ThemeVariant; 4] = [
        ThemeVariant::Dark,
        ThemeVariant::Light,
        ThemeVariant::DarkHighContrast,
        ThemeVariant::LightHighContrast,
    ];

    /// Present one row `width` points wide and return its node, text,
    /// height and AccessKit node.
    fn present(
        spec: ConversationRowSpec<'_>,
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
        let identity = PresentationScope::new("conversations").instance(("conversation", 12));
        let mut result = None;
        let mut accesskit = None;
        // The second pass sees the fonts the first one loaded.
        for _ in 0..2 {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(width);
                let mut observations = Vec::new();
                let target = ActionTarget::application(TestAction::Undo);
                let response = conversation_row(
                    ui,
                    &spec,
                    target,
                    &availability,
                    identity,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                let node = conversation_row_semantic_node(
                    &response,
                    &spec,
                    target,
                    &availability,
                    Some(serde_json::json!(12)),
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
        (node, text, height, accesskit.expect("row AccessKit node"))
    }

    fn spec(selected: bool) -> ConversationRowSpec<'static> {
        ConversationRowSpec {
            title: "Grace Brewster Murray Hopper",
            preview: Some("The hall is booked for Friday from six until late"),
            time: Some("09:41"),
            unread: Some("2 unread messages"),
            selected,
            avatar: Some(AvatarSpec {
                figure: AvatarFigure::Person {
                    name: "Grace Hopper",
                },
                size: AvatarSize::List,
                image: None,
            }),
        }
    }

    #[test]
    fn a_row_states_its_name_preview_time_and_unread_state() {
        let (node, text, height, accesskit) = present(spec(false), Availability::Enabled, 480.0);
        assert_eq!(node.role, UiRole::NavigationItem);
        assert_eq!(node.name, "Grace Brewster Murray Hopper");
        assert_eq!(
            node.description.as_deref(),
            Some(
                "2 unread messages; The hall is booked for Friday from six until late; \
                 09:41; Undo the most recent change"
            )
        );
        assert!(!node.selected);
        assert_eq!(node.argument, Some(serde_json::json!(12)));
        assert_eq!(
            text.iter()
                .map(|text| (text.component_id.kind, text.line_count))
                .collect::<Vec<_>>(),
            [
                (TextComponentKind::ConversationRowTime, 1),
                (TextComponentKind::ConversationRowTitle, 1),
                (TextComponentKind::ConversationRowPreview, 1),
            ]
        );
        assert!(text.iter().all(|text| {
            text.layout_error.is_none()
                && !text.truncated
                && text.parent_id.map(|parent| parent.kind)
                    == Some(TextComponentKind::ConversationRow)
        }));
        // The time ends the title's line, clear of the title.
        let (time, title) = (&text[0], &text[1]);
        assert!(title.allocated_rect.max_x <= time.allocated_rect.min_x);
        assert_eq!(time.allocated_rect.min_y, title.allocated_rect.min_y);
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        assert_eq!(height, conversation_row_height(&spec(false), &tokens, 1.0));
        assert!(height >= tokens.geometry.minimum_hit_size.0);
        assert_eq!(accesskit.role(), egui::accesskit::Role::Button);
        assert_eq!(accesskit.label(), Some("Grace Brewster Murray Hopper"));
        assert_eq!(
            accesskit.aria_current(),
            Some(egui::accesskit::AriaCurrent::False)
        );
        assert!(accesskit.toggled().is_none());
        assert!(accesskit.supports_action(egui::accesskit::Action::Click));
    }

    #[test]
    fn the_current_row_is_selected_not_toggled() {
        let (node, _, _, accesskit) = present(spec(true), Availability::Enabled, 480.0);
        assert!(node.selected);
        assert_eq!(
            accesskit.aria_current(),
            Some(egui::accesskit::AriaCurrent::True)
        );
        assert!(accesskit.toggled().is_none());
        assert!(accesskit.is_selected().is_none());
    }

    #[test]
    fn a_narrow_row_elides_its_title_and_preview_but_keeps_its_time() {
        let (node, text, _, _) = present(spec(false), Availability::Enabled, 200.0);
        assert_eq!(node.name, "Grace Brewster Murray Hopper");
        let truncated = |kind| {
            text.iter()
                .find(|text| text.component_id.kind == kind)
                .expect("observed")
                .truncated
        };
        assert!(!truncated(TextComponentKind::ConversationRowTime));
        assert!(truncated(TextComponentKind::ConversationRowTitle));
        assert!(truncated(TextComponentKind::ConversationRowPreview));
        assert!(text.iter().all(|text| text.layout_error.is_none()));
        assert!(
            text.iter()
                .all(|text| text.allocated_rect.max_x <= node.rect.max_x)
        );
    }

    #[test]
    fn a_row_without_preview_time_or_avatar_keeps_the_hit_size() {
        let row = ConversationRowSpec {
            title: "Ada",
            preview: None,
            time: None,
            unread: Some("Unread"),
            selected: false,
            avatar: None,
        };
        let (node, text, height, _) = present(row, Availability::Enabled, 320.0);
        assert_eq!(text.len(), 1);
        assert_eq!(
            node.description.as_deref(),
            Some("Unread; Undo the most recent change")
        );
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        assert!(height >= tokens.geometry.minimum_hit_size.0);
    }

    #[test]
    fn a_disabled_row_keeps_its_reason() {
        let (node, _, _, accesskit) = present(
            spec(false),
            Availability::Disabled {
                reason: "This conversation is archived".into(),
            },
            480.0,
        );
        assert!(!node.enabled);
        assert_eq!(
            node.disabled_reason.as_deref(),
            Some("This conversation is archived")
        );
        assert!(accesskit.is_disabled());
        assert!(
            accesskit
                .description()
                .unwrap()
                .ends_with("unavailable: This conversation is archived")
        );
    }

    #[test]
    fn the_unread_dot_stands_out_from_every_row_fill() {
        for theme in THEMES {
            let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
            let colours = &tokens.colours;
            let current = row_fill(&tokens, true, true).unwrap();
            let hovered = row_fill(&tokens, false, true).unwrap();
            assert_eq!(row_fill(&tokens, false, false), None);
            // A hovered row never looks like the current one.
            assert_ne!(current, hovered, "{theme:?}");
            assert_ne!(hovered, Color32::from(colours.surface_panel), "{theme:?}");
            // Hover has its own fill, so the row needs no stand-in surface.
            assert_eq!(hovered, Color32::from(colours.surface_hover), "{theme:?}");
            for behind in [None, Some(current), Some(hovered)] {
                let dot = unread_dot_colour(&tokens, behind);
                let surfaces: Vec<Color32> = match behind {
                    Some(colour) => vec![colour],
                    None => vec![
                        colours.surface_canvas.into(),
                        colours.surface_panel.into(),
                        colours.surface_raised.into(),
                    ],
                };
                for surface in surfaces {
                    assert!(
                        contrast_ratio(dot, surface) >= 3.0,
                        "{theme:?} dot on {surface:?}"
                    );
                }
            }
            // The row's preview and time stay legible on the current and
            // hovered rows' fills.
            for fill in [current, hovered] {
                assert!(
                    contrast_ratio(muted_text_on(&tokens, fill).into(), fill) >= 4.5,
                    "{theme:?} muted text on {fill:?}"
                );
                // The authored themes keep muted text muted on both fills;
                // the primary-text fallback is only a safety net.
                assert_eq!(
                    muted_text_on(&tokens, fill),
                    colours.text_muted,
                    "{theme:?} muted text falls back on {fill:?}"
                );
            }
        }
    }

    #[test]
    fn muted_text_turns_primary_on_a_hand_assembled_fill_too_close_to_it() {
        let mut tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let fill = Color32::from(tokens.colours.selection_background);
        tokens.colours.text_muted = tokens.colours.selection_background;
        assert_eq!(muted_text_on(&tokens, fill), tokens.colours.text_primary);
    }
}
