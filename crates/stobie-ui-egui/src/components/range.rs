use std::ops::RangeInclusive;

use super::SemanticControlOutput;
use crate::{ActionKey, DesignTokens, SemanticActionId, SemanticUiId, UiNode, UiRole};

/// Present a stable, token-sized range control with matching AccessKit and
/// augmented snapshot semantics. A hosted application observes the returned
/// node automatically.
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
        // egui bounds only the track; the control, as observed and targeted,
        // includes its label.
        node.set_bounds(egui::accesskit::Rect {
            x0: f64::from(response.rect.min.x),
            y0: f64::from(response.rect.min.y),
            x1: f64::from(response.rect.max.x),
            y1: f64::from(response.rect.max.y),
        });
    });
    let output = SemanticControlOutput {
        node: UiNode {
            argument: None,
            value: None,
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
    };
    crate::observation::observe_recipe_node(ui, &output.node);
    output
}
