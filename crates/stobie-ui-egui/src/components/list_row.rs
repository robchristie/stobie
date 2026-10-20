use egui::{Rect, Response, Sense, Stroke};

use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, PresentationId,
    SemanticActionId, SemanticUiId, TextComponentId, TextComponentKind, TextLayoutObservation,
    TextOverflow, TextRole, TextRoleStyles, TextSpec, UiNode, UiRole, measure_component_text,
    paint_measured_text,
};

/// One row of a list whose whole surface performs its action for that row,
/// such as opening a person's details. The title names the row, visibly and
/// for assistive technology; the detail is one secondary line.
#[derive(Clone, Copy, Debug)]
pub struct ListRowSpec<'a> {
    /// The row's name: one line in the button label style, as navigation
    /// items' labels, elided when narrow and complete in semantics.
    pub title: &'a str,
    /// One secondary line, such as a role or the latest message; elided.
    pub detail: Option<&'a str>,
}

impl ListRowSpec<'_> {
    /// The detail, then the action's description.
    fn description(&self, action: &str) -> String {
        match self.detail {
            Some(detail) => format!("{detail}; {action}"),
            None => action.to_owned(),
        }
    }
}

/// The height of a list row: its title and optional detail lines within block
/// padding, and at least the token hit size.
pub fn list_row_height(spec: &ListRowSpec<'_>, tokens: &DesignTokens, font_scale: f32) -> f32 {
    let title = TextRole::ButtonLabel.style(tokens, font_scale).line_height;
    let detail = spec.detail.map_or(0.0, |_| {
        TextRole::Secondary.style(tokens, font_scale).line_height
    });
    tokens
        .geometry
        .minimum_hit_size
        .0
        .max(title + detail + tokens.spacing.block.0 * 2.0)
}

/// A full-width list row whose surface activates `target`, named by its
/// title. Hosted applications use
/// [`Frame::list_row`](crate::Frame::list_row) with a
/// [`BoundAction`](crate::BoundAction) carrying the row's ID; use
/// [`list_row_semantic_node`] for low-level publication. Hidden availability
/// must be filtered before calling this recipe. Each enabled row is a Tab
/// stop; Enter, Space and a pointer release share `clicked()`.
#[allow(clippy::too_many_arguments)]
pub fn list_row<A: ActionKey>(
    ui: &mut egui::Ui,
    spec: &ListRowSpec<'_>,
    target: ActionTarget<A>,
    availability: &Availability,
    identity: PresentationId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> Response {
    assert!(
        availability.visible(),
        "filter hidden list rows before presentation"
    );
    let action = target.action.specification();
    let enabled = availability.enabled() && ui.is_enabled();
    let width = ui
        .available_width()
        .max(tokens.geometry.minimum_hit_size.0 * 2.0);
    let (_, rect) = ui.allocate_space(egui::vec2(width, list_row_height(spec, tokens, font_scale)));
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
            use egui::accesskit::{Action, Role};
            node.set_role(Role::Button);
            node.set_label(spec.title);
            node.set_author_id(identity.semantic_id().0);
            node.clear_toggled();
            node.clear_selected();
            node.set_description(availability.disabled_reason().map_or_else(
                || description.clone(),
                |reason| format!("{description}; unavailable: {reason}"),
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
    let mut foreground = *tokens;
    if !enabled {
        foreground.colours.text_primary = tokens.colours.text_muted;
    }
    let inner = rect.shrink2(egui::vec2(
        tokens.geometry.control_padding_x.0,
        tokens.spacing.block.0,
    ));
    let parent = TextComponentId::new(TextComponentKind::ListRow, identity.text_instance());
    let title_height = TextRole::ButtonLabel.style(tokens, font_scale).line_height;
    let detail_height = spec.detail.map_or(0.0, |_| {
        TextRole::Secondary.style(tokens, font_scale).line_height
    });
    // Centre the lines in a row taller than they need, such as at the hit size.
    let top = inner.top() + ((inner.height() - title_height - detail_height) * 0.5).max(0.0);
    let title_rect = Rect::from_min_max(
        egui::pos2(inner.left(), top),
        egui::pos2(inner.right(), top + title_height),
    );
    let mut truncated = false;
    let mut paint = |text: &str, role: TextRole, area: Rect, kind: TextComponentKind| {
        let measured = measure_component_text(
            ui.painter(),
            text,
            TextSpec::single_line(role, TextOverflow::Ellipsis),
            &foreground,
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
    paint(
        spec.title,
        TextRole::ButtonLabel,
        title_rect,
        TextComponentKind::ListRowTitle,
    );
    if let Some(detail) = spec.detail {
        let detail_rect = Rect::from_min_max(
            egui::pos2(inner.left(), title_rect.bottom()),
            egui::pos2(inner.right(), title_rect.bottom() + detail_height),
        );
        paint(
            detail,
            TextRole::Secondary,
            detail_rect,
            TextComponentKind::ListRowDetail,
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

/// The snapshot owner matching [`list_row`]: a button named by the row's
/// title, presenting `target`'s action with the argument it carries.
pub fn list_row_semantic_node<A: ActionKey>(
    response: &Response,
    spec: &ListRowSpec<'_>,
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
        UiRole::Button,
        response.interact_rect.into(),
    );
    node.name = spec.title.to_owned();
    node.description = Some(spec.description(action.description));
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
    use crate::{DensityVariant, PresentationScope, ThemeVariant, test_actions::TestAction};

    /// Present one row `width` points wide and return its node and text.
    fn present(
        spec: ListRowSpec<'_>,
        availability: Availability,
        width: f32,
    ) -> (UiNode, Vec<TextLayoutObservation>, f32) {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let identity = PresentationScope::new("rows").instance(("row", 7));
        let mut result = None;
        for _ in 0..2 {
            let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                ui.set_max_width(width);
                let mut observations = Vec::new();
                let target = ActionTarget::application(TestAction::Undo);
                let response = list_row(
                    ui,
                    &spec,
                    target,
                    &availability,
                    identity,
                    &tokens,
                    1.0,
                    &mut observations,
                );
                let node = list_row_semantic_node(
                    &response,
                    &spec,
                    target,
                    &availability,
                    Some(serde_json::json!(7)),
                    identity,
                    SemanticUiId::root(),
                );
                result = Some((node, observations, response.rect.height()));
            });
            output.textures_delta.clear();
        }
        result.expect("presented")
    }

    #[test]
    fn a_rows_title_and_detail_elide_with_complete_semantics() {
        let title = "Grace Brewster Murray Hopper, Rear Admiral";
        let spec = ListRowSpec {
            title,
            detail: Some("Computer scientist and United States Navy rear admiral"),
        };
        let (node, text, height) = present(spec, Availability::Enabled, 120.0);
        assert_eq!(node.name, title);
        assert_eq!(node.role, UiRole::Button);
        assert_eq!(node.argument, Some(serde_json::json!(7)));
        assert!(
            node.description
                .as_deref()
                .unwrap()
                .starts_with("Computer scientist")
        );
        assert_eq!(
            text.iter()
                .map(|text| (text.component_id.kind, text.truncated, text.line_count))
                .collect::<Vec<_>>(),
            [
                (TextComponentKind::ListRowTitle, true, 1),
                (TextComponentKind::ListRowDetail, true, 1),
            ]
        );
        assert!(text.iter().all(|text| text.layout_error.is_none()));
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        assert_eq!(height, list_row_height(&spec, &tokens, 1.0));
        assert!(height >= tokens.geometry.minimum_hit_size.0);
        // Without a detail, the row keeps at least the hit size.
        let (_, text, height) = present(
            ListRowSpec {
                title: "Ada",
                detail: None,
            },
            Availability::Enabled,
            400.0,
        );
        assert_eq!(text.len(), 1);
        assert!(!text[0].truncated);
        assert!(height >= tokens.geometry.minimum_hit_size.0);
    }

    #[test]
    fn a_disabled_row_keeps_its_reason() {
        let (node, _, _) = present(
            ListRowSpec {
                title: "Ada",
                detail: None,
            },
            Availability::Disabled {
                reason: "Ada is away".into(),
            },
            400.0,
        );
        assert!(!node.enabled);
        assert_eq!(node.disabled_reason.as_deref(), Some("Ada is away"));
        assert!(node.description.is_some());
    }
}
