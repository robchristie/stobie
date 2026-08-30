use egui::{Response, RichText, Slider, SliderClamping, TextStyle};

use crate::{
    ActionKey, AppearancePreference, ContrastPreference, DensityPreference, DesignTokens,
    MAX_FONT_SCALE, MIN_FONT_SCALE, MotionPreference, SemanticActionId, SemanticUiId, UiNode,
    UiPreferences, UiRect, UiRole,
};

/// The result of presenting [`preferences_control`] for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PreferencesControlOutput {
    /// Whether validation or an interaction changed the supplied preferences.
    pub changed: bool,
    /// The complete current-frame allocation of the preference rows.
    pub rect: UiRect,
    /// Current bounded geometry and state for every interactive preference.
    pub nodes: Vec<UiNode>,
}

/// Present every orthogonal appearance preference without owning persistence.
///
/// Text contract: field labels use the token-backed label role and one line;
/// option labels use the body role and one line. Options retain their complete
/// visible and semantic text and wrap only as whole hit targets. At narrow
/// widths the rows therefore stack rather than truncate or introduce a second
/// scroll owner. Four token minimum-hit widths are the minimum useful width.
///
/// Semantic IDs derive from `parent`, the preference field and the value. The
/// returned nodes use exact current-frame response bounds and remain direct
/// children of `parent`, ready to append to the caller-owned [`crate::UiSnapshot`].
pub fn preferences_control<A: ActionKey>(
    ui: &mut egui::Ui,
    preferences: &mut UiPreferences,
    parent: &SemanticUiId,
    action: A,
) -> PreferencesControlOutput {
    let initial = *preferences;
    *preferences = preferences.validated();

    let system_dark = ui.ctx().theme() == egui::Theme::Dark;
    let tokens = preferences.tokens(system_dark);
    let font_scale = preferences.font_scale;
    let mut nodes = Vec::new();

    let response = ui
        .push_id(("polyorama.preferences", parent.0.as_str()), |ui| {
            ui.spacing_mut().item_spacing =
                egui::vec2(tokens.spacing.inline.0, tokens.spacing.block.0);
            ui.spacing_mut().interact_size.y = preference_hit_height(&tokens, font_scale);
            let mut semantics = PreferenceSemantics {
                parent,
                action,
                nodes: &mut nodes,
            };

            ui.vertical(|ui| {
                preference_radio_group(
                    ui,
                    "Appearance",
                    "appearance",
                    &mut preferences.appearance,
                    &[
                        (AppearancePreference::Light, "Light", "light"),
                        (AppearancePreference::Dark, "Dark", "dark"),
                        (AppearancePreference::System, "System", "system"),
                    ],
                    &mut semantics,
                );
                ui.add_space(tokens.spacing.section.0);
                preference_radio_group(
                    ui,
                    "Contrast",
                    "contrast",
                    &mut preferences.contrast,
                    &[
                        (ContrastPreference::Standard, "Standard", "standard"),
                        (ContrastPreference::High, "High", "high"),
                    ],
                    &mut semantics,
                );
                ui.add_space(tokens.spacing.section.0);
                preference_radio_group(
                    ui,
                    "Density",
                    "density",
                    &mut preferences.density,
                    &[
                        (DensityPreference::Compact, "Compact", "compact"),
                        (DensityPreference::Comfortable, "Comfortable", "comfortable"),
                    ],
                    &mut semantics,
                );
                ui.add_space(tokens.spacing.section.0);
                preference_slider(ui, &mut preferences.font_scale, &tokens, &mut semantics);
                ui.add_space(tokens.spacing.section.0);
                preference_radio_group(
                    ui,
                    "Motion",
                    "motion",
                    &mut preferences.motion,
                    &[
                        (MotionPreference::Full, "Full", "full"),
                        (MotionPreference::Reduced, "Reduced", "reduced"),
                    ],
                    &mut semantics,
                );
            })
            .response
        })
        .inner;

    *preferences = preferences.validated();
    PreferencesControlOutput {
        changed: initial != *preferences,
        rect: response.rect.into(),
        nodes,
    }
}

fn preference_hit_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens
        .geometry
        .minimum_hit_size
        .0
        .max(tokens.geometry.control_height.0 * font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE))
}

fn preference_label(ui: &mut egui::Ui, text: &'static str) {
    ui.label(RichText::new(text).text_style(TextStyle::Button));
}

struct PreferenceSemantics<'a, A: ActionKey> {
    parent: &'a SemanticUiId,
    action: A,
    nodes: &'a mut Vec<UiNode>,
}

fn preference_radio_group<T: Copy + PartialEq, A: ActionKey>(
    ui: &mut egui::Ui,
    field_label: &'static str,
    field_id: &'static str,
    current: &mut T,
    options: &[(T, &'static str, &'static str)],
    semantics: &mut PreferenceSemantics<'_, A>,
) {
    preference_label(ui, field_label);
    ui.horizontal_wrapped(|ui| {
        for &(value, label, value_id) in options {
            let semantic_id = preference_semantic_id(semantics.parent, field_id, value_id);
            let response = ui
                .push_id((field_id, value_id), |ui| {
                    ui.radio_value(current, value, label)
                })
                .inner;
            let selected = *current == value;
            complete_radio_semantics(&response, &semantic_id, field_label, label, selected);
            semantics.nodes.push(preference_radio_node(
                &response,
                semantic_id,
                semantics.parent,
                field_label,
                label,
                selected,
                semantics.action,
            ));
        }
    });
}

fn preference_slider<A: ActionKey>(
    ui: &mut egui::Ui,
    font_scale: &mut f32,
    tokens: &DesignTokens,
    semantics: &mut PreferenceSemantics<'_, A>,
) {
    preference_label(ui, "Font scale");
    ui.spacing_mut().slider_width = (ui.available_width()
        - tokens.geometry.minimum_hit_size.0 * 2.0)
        .max(tokens.geometry.minimum_hit_size.0);
    let semantic_id = preference_semantic_id(semantics.parent, "font_scale", "value");
    let response = ui
        .push_id(("font_scale", "value"), |ui| {
            ui.add_sized(
                egui::vec2(ui.available_width(), tokens.geometry.minimum_hit_size.0),
                Slider::new(font_scale, MIN_FONT_SCALE..=MAX_FONT_SCALE)
                    .clamping(SliderClamping::Always)
                    .step_by(0.05)
                    .custom_formatter(|value, _| format!("{:.0}%", value * 100.0))
                    .show_value(true),
            )
        })
        .inner;
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::Slider);
        node.set_label("Font scale");
        node.set_description("Scale interface text from 100% to 150%");
        node.set_author_id(semantic_id.0.clone());
        node.set_numeric_value(f64::from(*font_scale));
        node.set_min_numeric_value(f64::from(MIN_FONT_SCALE));
        node.set_max_numeric_value(f64::from(MAX_FONT_SCALE));
        node.set_numeric_value_step(0.05);
        node.set_bounds(egui::accesskit::Rect {
            x0: f64::from(response.rect.min.x),
            y0: f64::from(response.rect.min.y),
            x1: f64::from(response.rect.max.x),
            y1: f64::from(response.rect.max.y),
        });
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    });
    semantics.nodes.push(UiNode {
        id: semantic_id,
        parent: Some(semantics.parent.clone()),
        role: UiRole::Slider,
        name: "Font scale".to_owned(),
        description: Some("Scale interface text from 100% to 150%".to_owned()),
        rect: response.rect.into(),
        enabled: response.enabled(),
        focused: response.has_focus(),
        selected: false,
        checked: None,
        expanded: None,
        pane: None,
        domain_reference: None,
        actions: vec![SemanticActionId::from_action(semantics.action)],
        disabled_reason: None,
    });
}

fn preference_semantic_id(parent: &SemanticUiId, field: &str, value: &str) -> SemanticUiId {
    SemanticUiId::new(format!("{}.preferences.{field}.{value}", parent.0))
}

fn complete_radio_semantics(
    response: &Response,
    semantic_id: &SemanticUiId,
    field: &str,
    label: &str,
    selected: bool,
) {
    response.ctx.accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role, Toggled};
        node.set_role(Role::RadioButton);
        node.set_label(label);
        node.set_description(format!("{field}: {label}"));
        node.set_author_id(semantic_id.0.clone());
        node.set_toggled(if selected {
            Toggled::True
        } else {
            Toggled::False
        });
        node.add_action(Action::Click);
    });
}

fn preference_radio_node<A: ActionKey>(
    response: &Response,
    id: SemanticUiId,
    parent: &SemanticUiId,
    field: &str,
    label: &str,
    selected: bool,
    action: A,
) -> UiNode {
    UiNode {
        id,
        parent: Some(parent.clone()),
        role: UiRole::RadioButton,
        name: label.to_owned(),
        description: Some(format!("{field}: {label}")),
        rect: response.rect.into(),
        enabled: response.enabled(),
        focused: response.has_focus(),
        selected: false,
        checked: Some(selected),
        expanded: None,
        pane: None,
        domain_reference: None,
        actions: vec![SemanticActionId::from_action(action)],
        disabled_reason: None,
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use egui::{Rect, accesskit::Toggled};
    use egui_kittest::{Harness, kittest::Queryable};

    use super::*;
    use crate::{
        AccessKitMismatch, DensityVariant, ThemeVariant, UiSnapshot,
        actions::test_support::TestAction, audit_accesskit,
    };

    #[test]
    fn narrow_control_returns_stable_bounded_nodes_and_matching_accesskit_semantics() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let root_rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(220.0, 560.0));
        let parent = SemanticUiId::root();
        let mut preferences = UiPreferences::default();
        let mut control = None;
        let mut frame = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root_rect),
                ..Default::default()
            },
            |ui| {
                control = Some(preferences_control(
                    ui,
                    &mut preferences,
                    &parent,
                    TestAction::AppearanceSettings,
                ));
            },
        );
        let update = frame
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        frame.textures_delta.clear();
        let control = control.expect("preference control output");

        assert!(!control.changed);
        assert!(control.rect.is_positive());
        assert_eq!(control.nodes.len(), 10);
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        for node in &control.nodes {
            assert_eq!(node.parent.as_ref(), Some(&parent), "{}", node.id.0);
            assert!(node.rect.is_positive(), "{}: {:?}", node.id.0, node.rect);
            assert!(
                UiRect::from(root_rect).contains(node.rect, 0.0),
                "{}: {:?}",
                node.id.0,
                node.rect
            );
            assert!(
                node.rect.max_y - node.rect.min_y >= tokens.geometry.minimum_hit_size.0,
                "{}: {:?}",
                node.id.0,
                node.rect
            );
            assert_eq!(
                node.actions,
                vec![SemanticActionId::from_action(
                    TestAction::AppearanceSettings
                )]
            );
        }

        let snapshot = UiSnapshot {
            root: parent.clone(),
            nodes: std::iter::once(UiNode::container(
                parent,
                None,
                UiRole::Application,
                root_rect.into(),
            ))
            .chain(control.nodes)
            .collect(),
            ..Default::default()
        };
        assert!(snapshot.audit().is_empty());
        assert_eq!(
            audit_accesskit(&snapshot, &update),
            Vec::<AccessKitMismatch>::new()
        );

        let dark = update
            .nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| node.author_id() == Some("application.preferences.appearance.dark"))
            .expect("dark appearance radio");
        assert_eq!(dark.role(), egui::accesskit::Role::RadioButton);
        assert_eq!(dark.label(), Some("Dark"));
        assert_eq!(dark.toggled(), Some(Toggled::True));
        assert!(dark.supports_action(egui::accesskit::Action::Focus));
        assert!(dark.supports_action(egui::accesskit::Action::Click));

        let slider = update
            .nodes
            .iter()
            .map(|(_, node)| node)
            .find(|node| node.author_id() == Some("application.preferences.font_scale.value"))
            .expect("font scale slider");
        assert_eq!(slider.role(), egui::accesskit::Role::Slider);
        assert_eq!(slider.label(), Some("Font scale"));
        assert_eq!(slider.numeric_value(), Some(1.0));
        assert_eq!(slider.min_numeric_value(), Some(f64::from(MIN_FONT_SCALE)));
        assert_eq!(slider.max_numeric_value(), Some(f64::from(MAX_FONT_SCALE)));
        assert!(slider.supports_action(egui::accesskit::Action::Increment));
        assert!(slider.supports_action(egui::accesskit::Action::Decrement));
    }

    #[test]
    fn invalid_preferences_are_repaired_independently_before_presentation() {
        let context = egui::Context::default();
        let root = SemanticUiId::root();
        let mut preferences = UiPreferences {
            appearance: AppearancePreference::Unknown,
            contrast: ContrastPreference::Unknown,
            density: DensityPreference::Unknown,
            font_scale: f32::INFINITY,
            motion: MotionPreference::Unknown,
            ..UiPreferences::default()
        };
        let mut output = None;
        let mut frame = context.run_ui(Default::default(), |ui| {
            output = Some(preferences_control(
                ui,
                &mut preferences,
                &root,
                TestAction::AppearanceSettings,
            ));
        });
        frame.textures_delta.clear();

        assert_eq!(preferences, UiPreferences::default());
        assert!(output.expect("preference output").changed);
    }

    #[test]
    fn keyboard_focus_and_activation_change_only_the_selected_preference() {
        let preferences = Rc::new(RefCell::new(UiPreferences::default()));
        let observed = Rc::clone(&preferences);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(260.0, 560.0))
            .build_ui(move |ui| {
                let mut preferences = observed.borrow_mut();
                preferences_control(
                    ui,
                    &mut preferences,
                    &SemanticUiId::root(),
                    TestAction::AppearanceSettings,
                );
            });

        let light = harness.get_by_role_and_label(egui::accesskit::Role::RadioButton, "Light");
        light.focus();
        harness.run();
        assert!(
            harness
                .get_by_role_and_label(egui::accesskit::Role::RadioButton, "Light")
                .is_focused()
        );

        harness.key_press(egui::Key::Enter);
        harness.run();
        let preferences = *preferences.borrow();
        assert_eq!(preferences.appearance, AppearancePreference::Light);
        assert_eq!(preferences.contrast, ContrastPreference::Standard);
        assert_eq!(preferences.density, DensityPreference::Comfortable);
        assert_eq!(preferences.font_scale, MIN_FONT_SCALE);
        assert_eq!(preferences.motion, MotionPreference::Full);
    }
}
