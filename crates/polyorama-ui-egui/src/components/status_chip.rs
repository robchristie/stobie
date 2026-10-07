use egui::{Color32, Rect, Response, Sense, Stroke};

use crate::{
    DesignTokens, DomainReference, IconId, PresentationId, SemanticUiId, StatusTone,
    TextComponentId, TextComponentKind, TextInteraction, TextLayoutObservation, TextOverflow,
    TextRole, TextSpec, UiNode, UiRole, icon_size, measure_component_text, paint_icon,
    present_accessible_measured_text, present_measured_text,
};

/// Single-line, content-sized allocation with deliberate ellipsis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum StatusChipWidth {
    /// Cap visual width at the generated status-chip maximum, scaled with text.
    #[default]
    ContentSized,
    /// Cap visual width in logical points, also bounded by the default maximum.
    /// Must be finite and positive. The measured useful minimum takes precedence.
    AtMost(f32),
}

/// Application-owned status information; no workflow, transition or activation.
/// Labels must contain visible text. Complete text survives elision in semantics
/// and the tooltip. Newlines do not create another line in the compact galley.
#[derive(Clone, Copy, Debug)]
pub struct StatusChipSpec<'a> {
    pub label: &'a str,
    pub tone: StatusTone,
    /// Decorative artwork; it never replaces the label or owns semantics.
    pub icon: Option<IconId>,
    /// Additional context, retained in semantics and a tooltip when supplied.
    pub explanation: Option<&'a str>,
    pub width: StatusChipWidth,
    /// Standalone information is selectable. Use Inert in pointer-owned rows.
    pub interaction: TextInteraction,
}

impl<'a> StatusChipSpec<'a> {
    pub const fn new(label: &'a str, tone: StatusTone) -> Self {
        Self {
            label,
            tone,
            icon: None,
            explanation: None,
            width: StatusChipWidth::ContentSized,
            interaction: TextInteraction::Selectable,
        }
    }

    pub const fn text_spec(self) -> TextSpec {
        TextSpec {
            interaction: self.interaction,
            ..TextSpec::single_line(TextRole::Status, TextOverflow::Ellipsis)
        }
    }
}

/// Actual opaque painted pair. Parent hover/selection cannot change contrast.
/// The boundary supplements the full text; it is not the sole status cue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusChipColours {
    pub foreground: Color32,
    pub background: Color32,
    pub boundary: Color32,
}

pub fn status_chip_colours(tokens: &DesignTokens, tone: StatusTone) -> StatusChipColours {
    StatusChipColours {
        foreground: tokens.colours.text_primary.into(),
        background: tokens.colours.surface_panel.into(),
        boundary: match tone {
            StatusTone::Neutral => tokens.colours.border_decorative,
            StatusTone::Success => tokens.colours.status_success,
            StatusTone::Warning => tokens.colours.status_warning,
            StatusTone::Error => tokens.colours.status_error,
        }
        .into(),
    }
}

/// Informational text response, separate from decorative visual geometry.
/// Selectable response sensing supports egui selection, not status activation.
pub struct StatusChipResponse {
    pub response: Response,
    pub visual_rect: Rect,
    /// Selection/semantic owner allocation; parent interactions remain caller-owned.
    pub text_rect: Rect,
    pub icon_rect: Option<Rect>,
    pub truncated: bool,
}

/// Production compact status. Low-level callers publish the matching
/// [`status_chip_semantic_node`]; [`crate::PresentationContext::status_chip`]
/// records both observations automatically.
///
/// Inline/block padding, radius and icon gap come from resolved tokens. Height
/// is one Status line plus block padding (no button minimum hit target). The
/// useful label minimum is the smaller of its intrinsic width and one scaled
/// token minimum-hit width, but never smaller than measured ellipsis. Padding,
/// artwork and gap are reserved before measuring the remaining label width.
/// A smaller parent receives that minimum allocation; wrap, scroll or relocate
/// the chip in the parent rather than expecting implicit icon-only collapse.
pub fn status_chip(
    ui: &mut egui::Ui,
    spec: StatusChipSpec<'_>,
    identity: PresentationId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> StatusChipResponse {
    assert!(
        !spec.label.trim().is_empty(),
        "a status chip needs a visible label"
    );
    let scale = if font_scale.is_finite() {
        font_scale.clamp(1.0, 1.5)
    } else {
        1.0
    };
    let cap = tokens.status_chip_max_width().0 * scale;
    let limit = match spec.width {
        StatusChipWidth::ContentSized => cap,
        StatusChipWidth::AtMost(width) => {
            assert!(
                width.is_finite() && width > 0.0,
                "status-chip width must be finite and positive"
            );
            width.min(cap)
        }
    };
    let colours = status_chip_colours(tokens, spec.tone);
    // Status typography keeps its regular face/line height, with the validated
    // primary foreground rather than an unvalidated status-coloured text pair.
    let text_tokens = DesignTokens {
        colours: crate::ColourTokens {
            text_muted: tokens.colours.text_primary,
            ..tokens.colours
        },
        ..*tokens
    };
    let intrinsic = measure_component_text(
        ui.painter(),
        spec.label,
        TextSpec {
            overflow: TextOverflow::Expand,
            ..spec.text_spec()
        },
        &text_tokens,
        font_scale,
        cap,
    );
    let ellipsis = measure_component_text(
        ui.painter(),
        "…",
        TextSpec::single_line(TextRole::Status, TextOverflow::Expand),
        &text_tokens,
        font_scale,
        cap,
    );
    let padding = egui::vec2(tokens.spacing.inline.0, tokens.spacing.block.0);
    let artwork = spec.icon.map(|_| icon_size(tokens, font_scale));
    let icon_reservation = artwork.map_or(0.0, |size| size + tokens.spacing.inline.0);
    let reservation = padding.x * 2.0 + icon_reservation;
    let minimum_label = intrinsic
        .size()
        .x
        .min(tokens.geometry.minimum_hit_size.0 * scale)
        .max(ellipsis.size().x);
    let width = (intrinsic.size().x.max(ellipsis.size().x) + reservation).min(
        limit
            .min(ui.available_width())
            .max(minimum_label + reservation),
    );
    let measured = measure_component_text(
        ui.painter(),
        spec.label,
        spec.text_spec(),
        &text_tokens,
        font_scale,
        (width - reservation).max(ellipsis.size().x),
    );
    let height = measured.size().y.max(artwork.unwrap_or(0.0)) + padding.y * 2.0;
    let (visual_rect, chrome) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    ui.painter().rect(
        visual_rect,
        (tokens.status_chip_radius().0 * scale).min(height * 0.5),
        colours.background,
        Stroke::new(1.0, colours.boundary),
        egui::StrokeKind::Inside,
    );
    let icon_rect = artwork.map(|size| {
        let rect = Rect::from_center_size(
            egui::pos2(
                visual_rect.left() + padding.x + size * 0.5,
                visual_rect.center().y,
            ),
            egui::vec2(size, size),
        );
        paint_icon(ui.painter(), spec.icon.unwrap(), rect, colours.foreground);
        rect
    });
    let text_rect = Rect::from_min_max(
        visual_rect.min + egui::vec2(padding.x + icon_reservation, padding.y),
        visual_rect.max - padding,
    );
    let component = TextComponentId::new(TextComponentKind::StatusChip, identity.text_instance());
    let (response, observation) = if spec.interaction == TextInteraction::Selectable {
        let (response, observation) =
            present_measured_text(ui, &measured, text_rect, component, None);
        (
            response.expect("selectable text owns its response"),
            observation,
        )
    } else {
        present_accessible_measured_text(ui, &measured, text_rect, component, None)
    };
    if response.interact_rect.is_positive() {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(identity.semantic_id().0);
            node.set_value(spec.label);
            node.clear_label();
            if let Some(explanation) = spec.explanation {
                node.set_description(explanation);
            }
            node.remove_action(egui::accesskit::Action::Click);
            let rect = response.interact_rect;
            node.set_bounds(egui::accesskit::Rect {
                x0: f64::from(rect.min.x),
                y0: f64::from(rect.min.y),
                x1: f64::from(rect.max.x),
                y1: f64::from(rect.max.y),
            });
        });
    }
    if ui.clip_rect().intersect(text_rect).is_positive() || observation.layout_error.is_some() {
        observations.push(observation);
    }
    let truncated = measured.truncated();
    if truncated || spec.explanation.is_some() {
        chrome.on_hover_ui(|ui| {
            ui.label(spec.label);
            if let Some(explanation) = spec.explanation {
                ui.label(explanation);
            }
        });
    }
    StatusChipResponse {
        response,
        visual_rect,
        text_rect,
        icon_rect,
        truncated,
    }
}

/// One informational text owner. Fully hidden text has no visible observation.
pub fn status_chip_semantic_node(
    chip: &StatusChipResponse,
    spec: StatusChipSpec<'_>,
    identity: PresentationId,
    parent: SemanticUiId,
    domain_reference: Option<DomainReference>,
) -> Option<UiNode> {
    let rect = chip.response.interact_rect;
    if !rect.is_positive() {
        return None;
    }
    let mut node = UiNode::container(
        identity.semantic_id(),
        Some(parent),
        UiRole::StatusChip,
        rect.into(),
    );
    node.name = spec.label.to_owned();
    node.description = spec.explanation.map(str::to_owned);
    node.enabled = chip.response.enabled();
    if let Some(DomainReference::Pane(pane)) = &domain_reference {
        node.pane = Some(*pane);
    }
    node.domain_reference = domain_reference;
    node.text_selectable = spec.interaction == TextInteraction::Selectable;
    Some(node)
}
