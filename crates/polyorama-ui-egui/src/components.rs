use std::ops::RangeInclusive;

use egui::{Color32, Frame, Margin, Painter, Rect, Response, Sense, Stroke};

#[cfg(test)]
use crate::{AccessKitMismatch, UiSnapshot, audit_accesskit, test_actions::TestAction};
use crate::{
    ActionKey, ActionTarget, Availability, DesignTokens, DomainReference, HorizontalTextAlignment,
    SemanticActionId, SemanticUiId, TextComponentId, TextInteraction, TextOverflow, TextRole,
    TextSpec, UiNode, UiRole, VerticalTextAlignment, measure_text, paint_measured_text,
    present_accessible_measured_text, present_measured_text,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionEmphasis {
    Quiet,
    Normal,
    Primary,
}

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

pub struct SemanticControlOutput {
    pub response: Response,
    pub node: UiNode,
}

/// Present a bounded labelled choice using egui's native combo-box behaviour
/// while fixing stable Polyorama and AccessKit identity at the recipe boundary.
#[allow(clippy::too_many_arguments)]
pub fn choice_control<T: Copy + Eq, A: ActionKey>(
    ui: &mut egui::Ui,
    semantic_id: SemanticUiId,
    parent: SemanticUiId,
    label: &str,
    value: &mut T,
    options: &[(T, &'static str)],
    action: A,
    tokens: &DesignTokens,
) -> SemanticControlOutput {
    let selected_before = options
        .iter()
        .find_map(|(candidate, name)| (*candidate == *value).then_some(*name))
        .unwrap_or("Unavailable");
    let response = egui::ComboBox::from_id_salt(semantic_id.0.clone())
        .selected_text(selected_before)
        .width(tokens.geometry.minimum_hit_size.0 * 3.0)
        .show_ui(ui, |ui| {
            for (candidate, name) in options {
                let option = ui.selectable_value(value, *candidate, *name);
                crate::record_native_text_control(
                    &option,
                    crate::NativeTextControlKind::Selectable,
                );
            }
        })
        .response;
    crate::record_native_text_control(&response, crate::NativeTextControlKind::ComboBox);
    let selected = options
        .iter()
        .find_map(|(candidate, name)| (*candidate == *value).then_some(*name))
        .unwrap_or("Unavailable");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::ComboBox);
        node.set_label(label);
        node.set_description(format!("Current value: {selected}"));
        node.set_author_id(semantic_id.0.clone());
        node.add_action(Action::Click);
    });
    SemanticControlOutput {
        node: UiNode {
            id: semantic_id,
            parent: Some(parent),
            role: UiRole::ComboBox,
            name: label.to_owned(),
            description: Some(format!("Current value: {selected}")),
            rect: response.rect.into(),
            enabled: response.enabled(),
            focused: response.has_focus(),
            selected: false,
            checked: None,
            expanded: None,
            pane: None,
            domain_reference: None,
            actions: vec![SemanticActionId::from_action(action)],
            text_selectable: false,
            disabled_reason: None,
        },
        response,
    }
}

/// Present a stable, token-sized range control with matching AccessKit and
/// augmented snapshot semantics.
#[allow(clippy::too_many_arguments)]
pub fn range_control<A: ActionKey>(
    ui: &mut egui::Ui,
    semantic_id: SemanticUiId,
    parent: SemanticUiId,
    label: &str,
    value: &mut f32,
    range: RangeInclusive<f32>,
    action: A,
    tokens: &DesignTokens,
) -> SemanticControlOutput {
    let minimum = *range.start();
    let maximum = *range.end();
    let response = ui
        .push_id(semantic_id.0.clone(), |ui| {
            ui.add_sized(
                egui::vec2(
                    tokens.geometry.minimum_hit_size.0 * 3.0,
                    tokens.geometry.minimum_hit_size.0,
                ),
                egui::Slider::new(value, range)
                    .clamping(egui::SliderClamping::Always)
                    .show_value(false)
                    .text(label),
            )
        })
        .inner;
    crate::record_native_text_control(&response, crate::NativeTextControlKind::Slider);
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::Slider);
        node.set_label(label);
        node.set_author_id(semantic_id.0.clone());
        node.set_numeric_value(f64::from(*value));
        node.set_min_numeric_value(f64::from(minimum));
        node.set_max_numeric_value(f64::from(maximum));
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    });
    SemanticControlOutput {
        node: UiNode {
            id: semantic_id,
            parent: Some(parent),
            role: UiRole::Slider,
            name: label.to_owned(),
            description: None,
            rect: response.rect.into(),
            enabled: response.enabled(),
            focused: response.has_focus(),
            selected: false,
            checked: None,
            expanded: None,
            pane: None,
            domain_reference: None,
            actions: vec![SemanticActionId::from_action(action)],
            text_selectable: false,
            disabled_reason: None,
        },
        response,
    }
}

pub fn image_status_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    (tokens.typography.body_size.0 * font_scale.clamp(1.0, 1.5) * tokens.typography.line_height.0
        + tokens.spacing.block.0 * 2.0)
        .max(tokens.geometry.minimum_hit_size.0)
}

pub struct ImageStatusSpec<'a> {
    pub instance: u64,
    pub rect: Rect,
    pub coordinates: &'a str,
    pub detail: &'a str,
}

/// Paint a clipped analytical viewport status strip from semantic tokens.
pub fn paint_image_status(
    ui: &mut egui::Ui,
    spec: ImageStatusSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
) -> Vec<crate::TextLayoutObservation> {
    let painter = ui.painter().clone();
    painter.rect_filled(spec.rect, 0.0, tokens.colours.surface_panel);
    painter.line_segment(
        [spec.rect.left_top(), spec.rect.right_top()],
        Stroke::new(1.0, tokens.colours.border_subtle),
    );
    let padding = tokens.spacing.inline.0;
    let right_width = (tokens.geometry.minimum_hit_size.0 * 3.0).min(spec.rect.width() * 0.35);
    let left = Rect::from_min_max(
        spec.rect.min + egui::vec2(padding, 0.0),
        egui::pos2(
            (spec.rect.right() - right_width - padding).max(spec.rect.left()),
            spec.rect.bottom(),
        ),
    );
    let right = Rect::from_min_max(
        egui::pos2(
            (spec.rect.right() - right_width).max(spec.rect.left()),
            spec.rect.top(),
        ),
        spec.rect.max - egui::vec2(padding, 0.0),
    );
    let parent = TextComponentId::new(crate::TextComponentKind::ImageStatus, spec.instance);
    let mut observations = Vec::new();
    for (index, (text, rect, role, alignment)) in [
        (
            spec.coordinates,
            left,
            TextRole::MonospaceTechnical,
            HorizontalTextAlignment::Start,
        ),
        (
            spec.detail,
            right,
            TextRole::Caption,
            HorizontalTextAlignment::End,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let text_spec = TextSpec {
            horizontal_alignment: alignment,
            interaction: TextInteraction::Selectable,
            ..TextSpec::single_line(role, TextOverflow::Ellipsis)
        };
        if let Ok(measured) = measure_text(
            &painter,
            text,
            text_spec,
            tokens,
            font_scale,
            rect.width().max(0.5),
        ) {
            let (_, observation) = present_measured_text(
                ui,
                &measured,
                rect,
                TextComponentId::new(
                    crate::TextComponentKind::ImageStatus,
                    spec.instance * 4 + index as u64,
                ),
                Some(parent),
            );
            observations.push(observation);
        }
    }
    observations
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusTone {
    Neutral,
    Success,
    Warning,
    Error,
}

pub fn status_badge(
    ui: &mut egui::Ui,
    instance: u64,
    text: &str,
    tone: StatusTone,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let maximum_width = ui.available_width().max(tokens.geometry.minimum_hit_size.0);
    let max_lines = if maximum_width < 360.0 { 3 } else { 2 };
    let spec = TextSpec {
        role: if tone == StatusTone::Error {
            TextRole::Error
        } else {
            TextRole::Status
        },
        overflow: TextOverflow::Wrap,
        interaction: TextInteraction::Selectable,
        horizontal_alignment: HorizontalTextAlignment::Start,
        vertical_alignment: VerticalTextAlignment::Centre,
        max_lines,
    };
    let measured = measure_text(
        ui.painter(),
        text,
        spec,
        tokens,
        font_scale,
        (maximum_width - tokens.spacing.inline.0 * 2.0).max(0.5),
    )
    .ok();
    let height = measured.as_ref().map_or_else(
        || tokens.geometry.control_height.0,
        |text| (text.size().y + tokens.spacing.block.0 * 2.0).max(tokens.geometry.control_height.0),
    );
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(maximum_width, height), Sense::hover());
    let colour: Color32 = match tone {
        StatusTone::Neutral => tokens.colours.text_muted.into(),
        StatusTone::Success => tokens.colours.status_success.into(),
        StatusTone::Warning => tokens.colours.status_warning.into(),
        StatusTone::Error => tokens.colours.status_error.into(),
    };
    ui.painter().rect(
        rect,
        tokens.geometry.control_radius.0,
        colour.linear_multiply(0.14),
        Stroke::new(1.0, colour),
        egui::StrokeKind::Inside,
    );
    if let Some(measured) = measured {
        let text_rect = rect.shrink2(egui::vec2(tokens.spacing.inline.0, tokens.spacing.block.0));
        let (_, observation) = present_measured_text(
            ui,
            &measured,
            text_rect,
            TextComponentId::new(crate::TextComponentKind::StatusBadge, instance),
            None,
        );
        observations.push(observation);
    }
    response
}

/// Present a deterministic property label/value pair. Narrow layouts stack;
/// regular and wide layouts align the two columns.
pub fn property_row(
    ui: &mut egui::Ui,
    instance: u64,
    label: &str,
    value: &str,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) {
    let width = ui.available_width().max(1.0);
    let narrow = crate::PaneWidthClass::from_points(width) == crate::PaneWidthClass::Narrow;
    let line_height = tokens.typography.body_size.0 * font_scale * tokens.typography.line_height.0;
    let height = if narrow {
        line_height * 3.0 + tokens.spacing.block.0
    } else {
        line_height * 2.0
    };
    let (rect, _response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let parent = TextComponentId::new(crate::TextComponentKind::PropertyRow, instance);
    let (label_rect, value_rect) = if narrow {
        let label_rect =
            Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + line_height));
        let value_rect = Rect::from_min_max(
            egui::pos2(rect.min.x, label_rect.max.y + tokens.spacing.block.0),
            rect.max,
        );
        (label_rect, value_rect)
    } else {
        let label_width = (width * 0.34).clamp(96.0, 220.0);
        let label_rect =
            Rect::from_min_max(rect.min, egui::pos2(rect.min.x + label_width, rect.max.y));
        let value_rect = Rect::from_min_max(
            egui::pos2(label_rect.max.x + tokens.spacing.inline.0, rect.min.y),
            rect.max,
        );
        (label_rect, value_rect)
    };
    let label_response = paint_text_observation(
        ui,
        ComponentTextSpec {
            text: label,
            rect: label_rect,
            spec: TextSpec::single_line(TextRole::Secondary, TextOverflow::Ellipsis),
            component_id: TextComponentId::new(crate::TextComponentKind::PropertyRow, instance * 2),
            parent_id: Some(parent),
            accessible: true,
        },
        tokens,
        font_scale,
        observations,
    );
    let value_response = paint_text_observation(
        ui,
        ComponentTextSpec {
            text: value,
            rect: value_rect,
            spec: TextSpec {
                interaction: TextInteraction::Selectable,
                max_lines: if narrow { 2 } else { 1 },
                overflow: if narrow {
                    TextOverflow::Wrap
                } else {
                    TextOverflow::Ellipsis
                },
                ..TextSpec::single_line(TextRole::Body, TextOverflow::Ellipsis)
            },
            component_id: TextComponentId::new(
                crate::TextComponentKind::PropertyRow,
                instance * 2 + 1,
            ),
            parent_id: Some(parent),
            accessible: false,
        },
        tokens,
        font_scale,
        observations,
    );
    if let (Some(label_response), Some(value_response)) = (label_response, value_response) {
        value_response.labelled_by(label_response.id);
    }
}

pub struct ResultRowSpec<'a> {
    pub instance: u64,
    pub identifier: &'a str,
    pub position: &'a str,
    pub confidence: &'a str,
    pub category: &'a str,
    pub selected: bool,
}

pub fn result_row_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens.geometry.control_height.0.max(
        tokens.typography.body_size.0
            * font_scale.clamp(1.0, 1.5)
            * tokens.typography.line_height.0,
    )
}

pub fn result_row(
    ui: &mut egui::Ui,
    spec: ResultRowSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let width = ui.available_width().max(1.0);
    let height = result_row_height(tokens, font_scale);
    let (_, rect) = ui.allocate_space(egui::vec2(width, height));
    let response = ui.interact(
        rect,
        egui::Id::new(("polyorama.result-row", spec.instance)),
        Sense::click(),
    );
    if response.clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            spec.selected,
            format!(
                "{}; {}; {}; {}",
                spec.identifier, spec.position, spec.confidence, spec.category
            ),
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::ListBoxOption);
        node.set_label(format!(
            "{}; {}; {}; {}",
            spec.identifier, spec.position, spec.confidence, spec.category
        ));
        node.set_author_id(format!("polyorama.result-row.{}", spec.instance));
        node.set_selected(spec.selected);
        node.add_action(Action::Click);
    });
    if spec.selected {
        ui.painter()
            .rect_filled(rect, 0.0, tokens.colours.selection_background);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let parent = TextComponentId::new(crate::TextComponentKind::ResultRow, spec.instance);
    let values = [
        (spec.identifier, 0.18, HorizontalTextAlignment::Start),
        (spec.position, 0.42, HorizontalTextAlignment::End),
        (spec.confidence, 0.20, HorizontalTextAlignment::End),
        (spec.category, 0.20, HorizontalTextAlignment::Start),
    ];
    let mut x = rect.min.x;
    for (index, (text, fraction, alignment)) in values.into_iter().enumerate() {
        let next = if index == 3 {
            rect.max.x
        } else {
            x + rect.width() * fraction
        };
        let cell = Rect::from_min_max(
            egui::pos2(x + tokens.spacing.unit.0, rect.min.y),
            egui::pos2(
                (next - tokens.spacing.unit.0).max(x + tokens.spacing.unit.0 + 0.5),
                rect.max.y,
            ),
        );
        paint_text_observation(
            ui,
            ComponentTextSpec {
                text,
                rect: cell,
                spec: TextSpec {
                    horizontal_alignment: alignment,
                    ..TextSpec::single_line(
                        if index == 1 || index == 2 {
                            TextRole::TabularValue
                        } else {
                            TextRole::Body
                        },
                        TextOverflow::Ellipsis,
                    )
                },
                component_id: TextComponentId::new(
                    crate::TextComponentKind::ResultRow,
                    spec.instance * 8 + index as u64,
                ),
                parent_id: Some(parent),
                accessible: false,
            },
            tokens,
            font_scale,
            observations,
        );
        x = next;
    }
    response
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThumbnailState {
    Loading,
    Resident,
    Error,
    Empty,
}

pub struct ThumbnailCellSpec<'a> {
    pub instance: u64,
    pub label: &'a str,
    pub state: ThumbnailState,
    pub selected: bool,
    /// Optional progressively decoded content for the resident state.
    pub texture: Option<egui::TextureId>,
}

pub fn thumbnail_cell_side(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens.geometry.control_height.0 * 3.0 * font_scale.clamp(1.0, 1.5)
}

pub fn thumbnail_cell(
    ui: &mut egui::Ui,
    spec: ThumbnailCellSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let side = thumbnail_cell_side(tokens, font_scale)
        .min(ui.available_width().max(tokens.geometry.minimum_hit_size.0));
    let (_, rect) = ui.allocate_space(egui::vec2(side, side));
    let response = ui.interact(
        rect,
        egui::Id::new(("polyorama.thumbnail-cell", spec.instance)),
        Sense::click(),
    );
    if response.clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            spec.selected,
            format!("{}; {:?}", spec.label, spec.state),
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::ListBoxOption);
        node.set_label(format!("{}; {:?}", spec.label, spec.state));
        node.set_author_id(format!("polyorama.thumbnail-cell.{}", spec.instance));
        node.set_selected(spec.selected);
        node.add_action(Action::Click);
    });
    let border: Color32 = if spec.selected {
        tokens.colours.accent_primary.into()
    } else {
        tokens.colours.border_subtle.into()
    };
    ui.painter().rect(
        rect,
        tokens.geometry.control_radius.0,
        tokens.colours.surface_raised,
        Stroke::new(if spec.selected { 2.0 } else { 1.0 }, border),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            tokens.geometry.control_radius.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let label_height =
        tokens.typography.label_size.0 * font_scale * tokens.typography.line_height.0;
    let image_rect = Rect::from_min_max(
        rect.min + egui::vec2(tokens.spacing.unit.0, tokens.spacing.unit.0),
        egui::pos2(
            rect.max.x - tokens.spacing.unit.0,
            rect.max.y - label_height - tokens.spacing.block.0 * 2.0,
        ),
    );
    match spec.state {
        ThumbnailState::Loading => {
            ui.painter()
                .rect_filled(image_rect, 1.0, tokens.colours.surface_canvas);
            ui.painter().line_segment(
                [image_rect.left_center(), image_rect.right_center()],
                Stroke::new(2.0, tokens.colours.text_muted),
            );
        }
        ThumbnailState::Resident => {
            if let Some(texture) = spec.texture {
                ui.painter().image(
                    texture,
                    image_rect,
                    Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.painter()
                    .rect_filled(image_rect, 1.0, tokens.colours.selection_background);
                let centre = image_rect.center();
                ui.painter().circle_filled(
                    centre,
                    image_rect.width().min(image_rect.height()) * 0.27,
                    tokens.colours.accent_primary,
                );
            }
        }
        ThumbnailState::Error => {
            ui.painter()
                .rect_filled(image_rect, 1.0, tokens.colours.surface_canvas);
            ui.painter().line_segment(
                [image_rect.left_top(), image_rect.right_bottom()],
                Stroke::new(2.0, tokens.colours.status_error),
            );
            ui.painter().line_segment(
                [image_rect.right_top(), image_rect.left_bottom()],
                Stroke::new(2.0, tokens.colours.status_error),
            );
        }
        ThumbnailState::Empty => {
            ui.painter().rect_stroke(
                image_rect,
                1.0,
                Stroke::new(1.0, tokens.colours.border_subtle),
                egui::StrokeKind::Inside,
            );
        }
    }
    let label_rect = Rect::from_min_max(
        egui::pos2(
            rect.min.x + tokens.spacing.unit.0,
            image_rect.max.y + tokens.spacing.block.0,
        ),
        egui::pos2(
            rect.max.x - tokens.spacing.unit.0,
            rect.max.y - tokens.spacing.unit.0,
        ),
    );
    paint_text_observation(
        ui,
        ComponentTextSpec {
            text: spec.label,
            rect: label_rect,
            spec: TextSpec {
                horizontal_alignment: HorizontalTextAlignment::Centre,
                ..TextSpec::single_line(TextRole::Caption, TextOverflow::Ellipsis)
            },
            component_id: TextComponentId::new(
                crate::TextComponentKind::ThumbnailCell,
                spec.instance,
            ),
            parent_id: None,
            accessible: false,
        },
        tokens,
        font_scale,
        observations,
    );
    response
}

struct ComponentTextSpec<'a> {
    text: &'a str,
    rect: Rect,
    spec: TextSpec,
    component_id: TextComponentId,
    parent_id: Option<TextComponentId>,
    accessible: bool,
}

fn paint_text_observation(
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
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            spec.selected,
            title,
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::Tab);
        node.set_label(title);
        node.set_author_id(format!("polyorama.dock.tab.{}", spec.component_id.instance));
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
    use crate::{DensityVariant, ThemeVariant};

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
                        selected: false,
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

    #[test]
    fn image_status_uses_measured_clipped_text_at_all_supported_scales() {
        let context = egui::Context::default();
        let tokens =
            DesignTokens::resolve(ThemeVariant::LightHighContrast, DensityVariant::Compact);
        for font_scale in [1.0, 1.25, 1.5] {
            let root = Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(280.0, image_status_height(&tokens, font_scale)),
            );
            let mut observations = Vec::new();
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(root),
                    ..Default::default()
                },
                |ui| {
                    observations = paint_image_status(
                        ui,
                        ImageStatusSpec {
                            instance: 1,
                            rect: root,
                            coordinates: "image 123456.7, 765432.1 · world 1234567890.1, 9876543210.2",
                            detail: "L12 · 64 tiles",
                        },
                        &tokens,
                        font_scale,
                    );
                },
            );
            output.textures_delta.clear();
            assert_eq!(observations.len(), 2);
            assert!(crate::audit_text_layouts(&observations).is_empty());
            assert!(observations.iter().all(|item| item.allocated_rect.max_x
                >= item.allocated_rect.min_x
                && item.interaction == TextInteraction::Selectable));
        }
    }

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
