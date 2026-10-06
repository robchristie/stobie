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
    choice_impl(
        ui,
        semantic_id,
        parent,
        label,
        value,
        options,
        action,
        tokens,
        None,
    )
    .control
}

/// A choice and its currently submitted popup options. Options disappear from
/// the next publication when the popup closes; these are observations, not state.
pub struct ChoiceControlOutput {
    pub control: SemanticControlOutput,
    pub options: Vec<UiNode>,
}

/// Observe stable current geometry for popup choices as well as the closed
/// control. `T` must have a stable Debug representation (typically an application
/// enum); identities derive from the choice value, never its position or label.
/// Merge `options` into the same viewport snapshot as `control.node`. Popup
/// geometry belongs to its floating layer, not the enclosing pane's clip.
#[allow(clippy::too_many_arguments)]
pub fn choice_control_with_options<T: Copy + Eq + std::fmt::Debug, A: ActionKey>(
    ui: &mut egui::Ui,
    semantic_id: SemanticUiId,
    parent: SemanticUiId,
    label: &str,
    value: &mut T,
    options: &[(T, &'static str)],
    action: A,
    tokens: &DesignTokens,
) -> ChoiceControlOutput {
    let identities: Vec<_> = options
        .iter()
        .map(|(value, _)| SemanticUiId::new(format!("{}.option.{value:?}", semantic_id.0)))
        .collect();
    choice_impl(
        ui,
        semantic_id,
        parent,
        label,
        value,
        options,
        action,
        tokens,
        Some(&identities),
    )
}

#[allow(clippy::too_many_arguments)]
fn choice_impl<T: Copy + Eq, A: ActionKey>(
    ui: &mut egui::Ui,
    semantic_id: SemanticUiId,
    parent: SemanticUiId,
    label: &str,
    value: &mut T,
    options: &[(T, &'static str)],
    action: A,
    tokens: &DesignTokens,
    option_ids: Option<&[SemanticUiId]>,
) -> ChoiceControlOutput {
    let mut option_nodes = Vec::new();
    let selected_before = options
        .iter()
        .find_map(|(candidate, name)| (*candidate == *value).then_some(*name))
        .unwrap_or("Unavailable");
    let response = egui::ComboBox::from_id_salt(semantic_id.0.clone())
        .selected_text(selected_before)
        .width(tokens.geometry.minimum_hit_size.0 * 3.0)
        .show_ui(ui, |ui| {
            for (index, (candidate, name)) in options.iter().enumerate() {
                let option = ui.selectable_value(value, *candidate, *name);
                crate::record_native_text_control(
                    &option,
                    crate::NativeTextControlKind::Selectable,
                );
                if let Some(ids) = option_ids {
                    let id = &ids[index];
                    let checked = *candidate == *value;
                    ui.ctx().accesskit_node_builder(option.id, |node| {
                        use egui::accesskit::{Action, Role, Toggled};
                        node.set_role(Role::RadioButton);
                        node.set_label(*name);
                        node.set_author_id(id.0.clone());
                        node.clear_selected();
                        node.set_toggled(if checked {
                            Toggled::True
                        } else {
                            Toggled::False
                        });
                        node.add_action(Action::Click);
                    });
                    let mut node = UiNode::container(
                        id.clone(),
                        Some(semantic_id.clone()),
                        UiRole::RadioButton,
                        option.rect.into(),
                    );
                    node.name = (*name).into();
                    node.enabled = option.enabled();
                    node.checked = Some(checked);
                    node.focused = option.has_focus();
                    node.actions.push(SemanticActionId::from_action(action));
                    option_nodes.push(node);
                }
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
    ChoiceControlOutput {
        options: option_nodes,
        control: SemanticControlOutput {
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
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant, test_actions::TestAction};
    use egui_kittest::{Harness, kittest::Queryable};

    #[derive(Default)]
    struct State {
        value: u8,
        reverse: bool,
        options: Vec<UiNode>,
    }

    #[test]
    fn popup_observations_follow_values_and_disappear_on_close() {
        let mut h = Harness::builder().build_ui_state(
            |ui, state: &mut State| {
                let options = if state.reverse {
                    [(1, "Second label changed"), (0, "First")]
                } else {
                    [(0, "First"), (1, "Second")]
                };
                let output = choice_control_with_options(
                    ui,
                    SemanticUiId::new("test.choice"),
                    SemanticUiId::root(),
                    "Choice",
                    &mut state.value,
                    &options,
                    TestAction::DisplaySettings,
                    &DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable),
                );
                state.options = output.options;
            },
            State::default(),
        );
        h.run();
        assert!(h.state().options.is_empty());
        h.get_by_label("Choice").click();
        h.run();
        let second = h
            .state()
            .options
            .iter()
            .find(|n| n.name == "Second")
            .unwrap()
            .id
            .clone();
        assert_eq!(second.0, "test.choice.option.1");
        assert!(h.state().options.iter().all(|n| n.rect.is_positive()));
        h.get_by_label("Second").click();
        h.run();
        assert_eq!(h.state().value, 1);
        h.key_press(egui::Key::Escape);
        h.run();
        assert!(h.state().options.is_empty());
        h.state_mut().reverse = true;
        h.get_by_label("Choice").click();
        h.run();
        assert_eq!(
            h.state()
                .options
                .iter()
                .find(|n| n.name == "Second label changed")
                .unwrap()
                .id,
            second
        );
    }
}
