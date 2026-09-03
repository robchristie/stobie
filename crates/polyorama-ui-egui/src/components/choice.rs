use super::SemanticControlOutput;
use crate::{ActionKey, DesignTokens, SemanticActionId, SemanticUiId, UiNode, UiRole};

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
