use std::borrow::Cow;

use egui::{Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, HorizontalTextAlignment,
    IconId, PresentationId, SemanticActionId, SemanticUiId, TextComponentId, TextComponentKind,
    TextLayoutObservation, TextOverflow, TextRole, TextSpec, UiNode, UiRole, icon_size,
    measure_component_text, measure_text, paint_icon, paint_measured_text,
};

/// Compact, inert information owned by the destination row's accessible description.
/// Omit the badge with `None`; `Count { value: 0, .. }` deliberately displays zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationBadge<'a> {
    /// Values above 99 paint as “99+”; the description retains the exact value
    /// followed by `meaning` (for example, “125 outstanding tasks”).
    Count { value: u64, meaning: &'a str },
    /// One caption line, elided at the bounded badge width. Supply a complete
    /// explanation independently of the abbreviated visible label.
    Label { text: &'a str, description: &'a str },
}

impl NavigationBadge<'_> {
    fn text(self) -> Cow<'static, str> {
        match self {
            Self::Count { value, .. } if value > 99 => Cow::Borrowed("99+"),
            Self::Count { value, .. } => Cow::Owned(value.to_string()),
            Self::Label { text, .. } => Cow::Owned(text.to_owned()),
        }
    }

    fn description(self) -> String {
        match self {
            Self::Count { value, meaning } => format!("{value} {meaning}"),
            Self::Label { description, .. } => description.to_owned(),
        }
    }
}

/// One application-owned destination. Identity comes from the logical scope/key,
/// independently of these presentation values and the activation capability.
#[derive(Clone, Debug)]
pub struct NavigationItemSpec<'a, A: ActionKey> {
    pub target: ActionTarget<A>,
    pub icon: IconId,
    pub label: &'a str,
    pub description: &'a str,
    pub selected: bool,
    pub availability: Availability,
    pub badge: Option<NavigationBadge<'a>>,
}

impl<A: ActionKey> NavigationItemSpec<'_, A> {
    fn description(&self) -> String {
        let mut description = self.description.to_owned();
        if self.selected {
            description.push_str("; Current destination");
        }
        if let Some(badge) = self.badge {
            description.push_str("; ");
            description.push_str(&badge.description());
        }
        description
    }
}

fn intrinsic_width(
    ui: &egui::Ui,
    text: &str,
    role: TextRole,
    tokens: &DesignTokens,
    scale: f32,
) -> f32 {
    measure_text(
        ui.painter(),
        text,
        TextSpec::single_line(role, TextOverflow::Expand),
        tokens,
        scale,
        4096.0,
    )
    .map_or(tokens.geometry.minimum_hit_size.0, |text| text.size().x)
}

fn badge_width(
    ui: &egui::Ui,
    badge: NavigationBadge<'_>,
    tokens: &DesignTokens,
    scale: f32,
) -> f32 {
    let cap = tokens.geometry.minimum_hit_size.0 * 2.0 * scale.clamp(1.0, 1.5);
    let text = badge.text();
    let width = intrinsic_width(ui, &text, TextRole::Caption, tokens, scale);
    let width = if matches!(badge, NavigationBadge::Count { .. }) {
        width.max(intrinsic_width(ui, "99+", TextRole::Caption, tokens, scale))
    } else {
        width.max(intrinsic_width(ui, "…", TextRole::Caption, tokens, scale))
    };
    width.min(cap) + tokens.spacing.inline.0 * 2.0
}

/// Minimum parent width: padding, icon, gaps, a label slot of two token hit
/// widths (scaled with text), and the measured bounded badge when present.
/// Provide this width, scroll horizontally or relocate the sidebar. Smaller
/// parents receive an overflowing minimum allocation; nothing silently vanishes.
pub fn navigation_item_minimum_width(
    ui: &egui::Ui,
    badge: Option<NavigationBadge<'_>>,
    tokens: &DesignTokens,
    font_scale: f32,
) -> f32 {
    tokens.geometry.control_padding_x.0 * 2.0
        + icon_size(tokens, font_scale)
        + tokens.spacing.inline.0
        + (tokens.geometry.minimum_hit_size.0 * 2.0 * font_scale.clamp(1.0, 1.5)).max(
            intrinsic_width(ui, "…", TextRole::ButtonLabel, tokens, font_scale),
        )
        + badge.map_or(0.0, |badge| {
            tokens.spacing.inline.0 + badge_width(ui, badge, tokens, font_scale)
        })
}

/// Production full-width navigation row. Use the matching semantic helper for
/// low-level publication, or PresentationContext::navigation_item for automatic
/// observations. Hidden availability must be filtered before calling this recipe.
/// Each enabled row is a Tab stop; Enter/Space and pointer release share clicked().
/// Focus never changes caller selection. There is no composite arrow-key policy.
pub fn navigation_item<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: &NavigationItemSpec<'_, A>,
    identity: PresentationId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    assert!(
        spec.availability.visible(),
        "filter hidden navigation items before presentation"
    );
    let enabled = spec.availability.enabled() && ui.is_enabled();
    let scale = font_scale.clamp(1.0, 1.5);
    let height = tokens
        .geometry
        .minimum_hit_size
        .0
        .max(tokens.geometry.control_height.0 * scale)
        .max(
            TextRole::ButtonLabel.style(tokens, font_scale).line_height
                + tokens.spacing.block.0 * 2.0,
        );
    let width = ui.available_width().max(navigation_item_minimum_width(
        ui, spec.badge, tokens, font_scale,
    ));
    let (_, rect) = ui.allocate_space(egui::vec2(width, height));
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
    if response.interact_rect.is_positive() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, spec.label)
        });
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::{Action, AriaCurrent, Role};
            node.set_role(Role::Button);
            node.set_label(spec.label);
            node.set_author_id(identity.semantic_id().0);
            node.clear_toggled();
            node.clear_selected();
            node.set_aria_current(if spec.selected {
                AriaCurrent::True
            } else {
                AriaCurrent::False
            });
            let description = spec.description();
            node.set_description(
                spec.availability
                    .disabled_reason()
                    .map_or(description.clone(), |reason| {
                        format!("{description}; unavailable: {reason}")
                    }),
            );
            if !enabled {
                node.set_disabled();
                node.remove_action(Action::Click);
            } else {
                node.add_action(Action::Click);
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
    if spec.selected || pointer_down {
        // The validated selection pair keeps press feedback visible even when
        // a theme aliases quiet-hover fill and the selection indicator.
        ui.painter()
            .rect_filled(rect, radius, tokens.colours.selection_background);
    } else if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, radius, tokens.colours.action_quiet_hover);
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
    let mut foreground = *tokens;
    if !enabled {
        foreground.colours.text_primary = tokens.colours.text_muted;
    }
    let inner = rect.shrink2(egui::vec2(tokens.geometry.control_padding_x.0, 0.0));
    let size = icon_size(tokens, font_scale);
    paint_icon(
        ui.painter(),
        spec.icon,
        Rect::from_center_size(
            egui::pos2(inner.left() + size * 0.5, rect.center().y),
            egui::Vec2::splat(size),
        ),
        foreground.colours.text_primary.into(),
    );
    let parent = TextComponentId::new(TextComponentKind::NavigationItem, identity.text_instance());
    let mut label_rect = inner;
    label_rect.min.x += size + tokens.spacing.inline.0;
    let mut truncated = false;
    if let Some(badge) = spec.badge {
        let badge_rect = Rect::from_min_max(
            egui::pos2(
                inner.right() - badge_width(ui, badge, tokens, font_scale),
                rect.top(),
            ),
            inner.max,
        );
        label_rect.max.x = badge_rect.left() - tokens.spacing.inline.0;
        let text = badge.text();
        let measured = measure_component_text(
            ui.painter(),
            &text,
            TextSpec {
                horizontal_alignment: if matches!(badge, NavigationBadge::Count { .. }) {
                    HorizontalTextAlignment::End
                } else {
                    HorizontalTextAlignment::Centre
                },
                ..TextSpec::single_line(TextRole::Caption, TextOverflow::Ellipsis)
            },
            &foreground,
            font_scale,
            badge_rect.width() - tokens.spacing.inline.0 * 2.0,
        );
        truncated |= measured.truncated();
        let visual = Rect::from_center_size(
            badge_rect.center(),
            egui::vec2(
                badge_rect.width(),
                measured.size().y + tokens.spacing.block.0,
            ),
        );
        ui.painter()
            .rect_filled(visual, radius, tokens.colours.surface_raised);
        let observation = paint_measured_text(
            ui.painter(),
            &measured,
            badge_rect.shrink2(egui::vec2(tokens.spacing.inline.0, 0.0)),
            TextComponentId::new(TextComponentKind::NavigationBadge, identity.text_instance()),
            Some(parent),
        );
        if ui.clip_rect().intersects(badge_rect) || observation.layout_error.is_some() {
            observations.push(observation);
        }
    }
    let measured = measure_component_text(
        ui.painter(),
        spec.label,
        TextSpec::single_line(TextRole::ButtonLabel, TextOverflow::Ellipsis),
        &foreground,
        font_scale,
        label_rect.width(),
    );
    truncated |= measured.truncated();
    let observation = paint_measured_text(
        ui.painter(),
        &measured,
        label_rect,
        TextComponentId::new(TextComponentKind::NavigationLabel, identity.text_instance()),
        Some(parent),
    );
    if ui.clip_rect().intersects(label_rect) || observation.layout_error.is_some() {
        observations.push(observation);
    }
    if truncated || spec.badge.is_some() || !enabled {
        let mut tooltip = format!("{}\n{}", spec.label, spec.description());
        if let Some(reason) = spec.availability.disabled_reason() {
            tooltip.push_str(&format!("\nUnavailable: {reason}"));
        }
        response.clone().on_hover_text(tooltip);
    }
    response
}

/// Matching snapshot owner; selected means current destination, not a toggle.
/// AccessKit uses Button + aria_current, and neither selected nor toggled.
pub fn navigation_item_semantic_node<A: ActionKey>(
    response: &Response,
    spec: &NavigationItemSpec<'_, A>,
    identity: PresentationId,
    parent: SemanticUiId,
) -> UiNode {
    UiNode {
        id: identity.semantic_id(),
        parent: Some(parent),
        role: UiRole::NavigationItem,
        name: spec.label.to_owned(),
        description: Some(spec.description()),
        rect: response.interact_rect.into(),
        enabled: spec.availability.enabled() && response.enabled(),
        focused: response.has_focus(),
        selected: spec.selected,
        checked: None,
        expanded: None,
        pane: spec.target.pane,
        domain_reference: spec.target.pane.map(DomainReference::Pane),
        actions: vec![SemanticActionId::from_action(spec.target.action)],
        text_selectable: false,
        disabled_reason: spec.availability.disabled_reason().map(ToOwned::to_owned),
    }
}

#[cfg(test)]
mod tests;
