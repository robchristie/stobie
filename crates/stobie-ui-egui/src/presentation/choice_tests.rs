//! The segmented choice's faces: roles and states for AccessKit and ARIA,
//! one Tab stop in a single choice, and options that fit or wrap legibly at
//! narrow widths and large text.

use egui::Rect;

use super::*;
use crate::{
    DensityVariant, DesignTokens, PresentationScope, SemanticUiId, TextLayoutObservation,
    ThemeVariant, UiSnapshot, audit_accesskit, audit_text_layouts, test_actions::TestAction,
};

const DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

fn weekdays(on: [bool; 7]) -> Vec<ChoiceOption<'static>> {
    DAYS.iter()
        .zip(on)
        .map(|(day, on)| ChoiceOption::new(*day, day, on).with_short_label(&day[..3]))
        .collect()
}

fn repeats() -> Vec<ChoiceOption<'static>> {
    [
        ("daily", "Daily"),
        ("weekdays", "Weekdays"),
        ("weekly", "Weekly"),
        ("hourly", "Hourly"),
    ]
    .into_iter()
    .map(|(id, label)| ChoiceOption::new(id, label, id == "daily"))
    .collect()
}

/// Hourly is unavailable; anything else is enabled.
fn hourly_disabled(argument: &ActionArgument) -> Availability {
    if *argument == ActionArgument::from("hourly") {
        Availability::Disabled {
            reason: "Hourly runs are turned off".into(),
        }
    } else {
        Availability::Enabled
    }
}

struct Presented {
    snapshot: UiSnapshot,
    update: egui::accesskit::TreeUpdate,
    text: Vec<TextLayoutObservation>,
    group: Rect,
    options: Vec<Response>,
}

fn present(
    spec: &SegmentedChoiceSpec<'_>,
    width: f32,
    tokens: DesignTokens,
    font_scale: f32,
    availability: &dyn Fn(&ActionArgument) -> Availability,
) -> Presented {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    context.enable_accesskit();
    let root = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_000.0, 600.0));
    let mut result = None;
    let mut update = None;
    // The second pass sees the fonts the first one loaded.
    for _ in 0..2 {
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |ui| {
                ui.set_max_width(width);
                let mut p = PresentationContext::new(
                    ui,
                    tokens,
                    font_scale,
                    PresentationScope::new("routine"),
                    SemanticUiId::root(),
                );
                let target = ActionTarget::application(TestAction::AppearanceSettings);
                let response = p.segmented_choice(ui, "choice", target, spec, |_, argument| {
                    availability(argument)
                });
                let published = p.finish(ui);
                let mut snapshot = UiSnapshot::default();
                snapshot.nodes.push(crate::UiNode::container(
                    SemanticUiId::root(),
                    None,
                    UiRole::Application,
                    root.into(),
                ));
                snapshot.nodes.extend(published.semantic_nodes);
                result = Some((
                    snapshot,
                    published.text_layouts,
                    response.response.rect,
                    response.options,
                ));
            },
        );
        output.textures_delta.clear();
        update = output.platform_output.accesskit_update.take();
    }
    let (snapshot, text, group, options) = result.unwrap();
    Presented {
        snapshot,
        update: update.unwrap(),
        text,
        group,
        options,
    }
}

fn comfortable() -> DesignTokens {
    DesignTokens::resolve(ThemeVariant::Light, DensityVariant::Comfortable)
}

fn group_id() -> SemanticUiId {
    PresentationScope::new("routine")
        .instance("choice")
        .semantic_id()
}

#[test]
fn a_single_choice_is_a_radio_group_with_the_chosen_option_checked() {
    let options = repeats();
    let spec = SegmentedChoiceSpec::single("Repeats", &options);
    let presented = present(&spec, 400.0, comfortable(), 1.0, &hourly_disabled);
    assert!(
        presented.snapshot.audit().is_empty(),
        "{:?}",
        presented.snapshot.audit()
    );
    let mismatches = audit_accesskit(&presented.snapshot, &presented.update);
    assert!(mismatches.is_empty(), "{mismatches:?}");

    let group = presented
        .snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Section)
        .expect("the group's node");
    assert_eq!(group.id, group_id());
    assert_eq!(group.name, "Repeats");
    assert_eq!(group.parent, Some(SemanticUiId::root()));
    let radios: Vec<_> = presented
        .snapshot
        .nodes
        .iter()
        .filter(|node| node.role == UiRole::RadioButton)
        .collect();
    assert_eq!(
        radios
            .iter()
            .map(|node| (
                node.name.as_str(),
                node.checked,
                node.enabled,
                node.argument.clone()
            ))
            .collect::<Vec<_>>(),
        [
            ("Daily", Some(true), true, Some(serde_json::json!("daily"))),
            (
                "Weekdays",
                Some(false),
                true,
                Some(serde_json::json!("weekdays"))
            ),
            (
                "Weekly",
                Some(false),
                true,
                Some(serde_json::json!("weekly"))
            ),
            (
                "Hourly",
                Some(false),
                false,
                Some(serde_json::json!("hourly"))
            ),
        ]
    );
    assert!(radios.iter().all(|node| node.parent == Some(group_id())));
    assert_eq!(
        radios[3].disabled_reason.as_deref(),
        Some("Hourly runs are turned off")
    );

    // AccessKit: a radio group named by the choice, holding radio buttons
    // that know their place in it.
    let nodes = &presented.update.nodes;
    let (_, accesskit_group) = nodes
        .iter()
        .find(|(_, node)| node.author_id() == Some(group_id().0.as_str()))
        .expect("the group's AccessKit node");
    assert_eq!(accesskit_group.role(), egui::accesskit::Role::RadioGroup);
    assert_eq!(accesskit_group.label(), Some("Repeats"));
    let mut accesskit_radios: Vec<_> = nodes
        .iter()
        .filter(|(_, node)| node.role() == egui::accesskit::Role::RadioButton)
        .map(|(_, node)| {
            (
                node.label().unwrap_or_default().to_owned(),
                node.toggled(),
                node.position_in_set(),
                node.size_of_set(),
            )
        })
        .collect();
    accesskit_radios.sort_by_key(|(.., position, _)| *position);
    use egui::accesskit::Toggled;
    assert_eq!(
        accesskit_radios,
        [
            ("Daily", Toggled::True, 1),
            ("Weekdays", Toggled::False, 2),
            ("Weekly", Toggled::False, 3),
            ("Hourly", Toggled::False, 4),
        ]
        .map(|(label, toggled, position)| (
            label.to_owned(),
            Some(toggled),
            Some(position),
            Some(4)
        ))
    );

    // ARIA: a labelled radiogroup whose radios carry aria-checked.
    let tree = crate::aria::project(&presented.update, 1.0);
    let (group_key, aria_group) = tree
        .elements
        .iter()
        .find(|(_, element)| {
            element.attributes.get("role").map(String::as_str) == Some("radiogroup")
        })
        .expect("a radiogroup element");
    assert_eq!(
        aria_group.attributes.get("aria-label").map(String::as_str),
        Some("Repeats")
    );
    let aria_radios: Vec<_> = tree
        .elements
        .values()
        .filter(|element| element.attributes.get("role").map(String::as_str) == Some("radio"))
        .collect();
    assert_eq!(aria_radios.len(), 4);
    assert!(
        aria_radios
            .iter()
            .all(|radio| radio.parent.as_ref() == Some(group_key))
    );
    let checked: Vec<_> = aria_radios
        .iter()
        .map(|radio| radio.attributes.get("aria-checked").map(String::as_str))
        .collect();
    assert_eq!(checked.iter().filter(|c| **c == Some("true")).count(), 1);
    assert_eq!(checked.iter().filter(|c| **c == Some("false")).count(), 3);
    let mut positions: Vec<_> = aria_radios
        .iter()
        .map(|radio| {
            (
                radio.attributes.get("aria-posinset").cloned(),
                radio.attributes.get("aria-setsize").cloned(),
            )
        })
        .collect();
    positions.sort();
    let four = Some("4".to_owned());
    assert_eq!(
        positions,
        (1..=4)
            .map(|position| (Some(position.to_string()), four.clone()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_multiple_choice_is_a_group_of_toggles() {
    let options = weekdays([true, false, true, false, false, false, false]);
    let spec = SegmentedChoiceSpec::multiple("Runs on", &options);
    let presented = present(&spec, 400.0, comfortable(), 1.0, &|_| Availability::Enabled);
    let mismatches = audit_accesskit(&presented.snapshot, &presented.update);
    assert!(mismatches.is_empty(), "{mismatches:?}");
    let toggles: Vec<_> = presented
        .snapshot
        .nodes
        .iter()
        .filter(|node| node.parent == Some(group_id()))
        .collect();
    assert!(toggles.iter().all(|node| node.role == UiRole::Button));
    assert_eq!(
        toggles
            .iter()
            .map(|node| (node.name.as_str(), node.checked))
            .collect::<Vec<_>>(),
        DAYS.iter()
            .zip([true, false, true, false, false, false, false])
            .map(|(day, on)| (*day, Some(on)))
            .collect::<Vec<_>>()
    );
    // The short labels show, so each option is narrower than the full
    // labels need.
    let long: Vec<_> = DAYS
        .iter()
        .map(|day| ChoiceOption::new(*day, day, false))
        .collect();
    let without = present(
        &SegmentedChoiceSpec::multiple("Runs on", &long),
        400.0,
        comfortable(),
        1.0,
        &|_| Availability::Enabled,
    );
    assert!(presented.options[0].rect.width() < without.options[0].rect.width() * 0.75);
    let (_, group) = presented
        .update
        .nodes
        .iter()
        .find(|(_, node)| node.author_id() == Some(group_id().0.as_str()))
        .expect("the group's AccessKit node");
    assert_eq!(group.role(), egui::accesskit::Role::Group);
    assert_eq!(group.label(), Some("Runs on"));
    let tree = crate::aria::project(&presented.update, 1.0);
    let mut pressed: Vec<_> = tree
        .elements
        .values()
        .filter_map(|element| element.attributes.get("aria-pressed"))
        .map(String::as_str)
        .collect();
    pressed.sort_unstable();
    assert_eq!(
        pressed,
        ["false", "false", "false", "false", "false", "true", "true"]
    );
    // Every option of a multiple choice is a Tab stop.
    assert!(
        presented
            .options
            .iter()
            .all(|option| option.sense.is_focusable())
    );
}

#[test]
fn a_single_choice_is_one_tab_stop_at_its_chosen_or_first_enabled_option() {
    let mut options = repeats();
    let spec = SegmentedChoiceSpec::single("Repeats", &options);
    let focusable = |presented: &Presented| -> Vec<bool> {
        presented
            .options
            .iter()
            .map(|option| option.sense.is_focusable())
            .collect()
    };
    let presented = present(&spec, 400.0, comfortable(), 1.0, &hourly_disabled);
    assert_eq!(focusable(&presented), [true, false, false, false]);
    options[0].selected = false;
    options[2].selected = true;
    let spec = SegmentedChoiceSpec::single("Repeats", &options);
    let presented = present(&spec, 400.0, comfortable(), 1.0, &hourly_disabled);
    assert_eq!(focusable(&presented), [false, false, true, false]);
    // Nothing chosen, or only a disabled option: the first enabled option.
    options[2].selected = false;
    options[3].selected = true;
    let spec = SegmentedChoiceSpec::single("Repeats", &options);
    let presented = present(&spec, 400.0, comfortable(), 1.0, &hourly_disabled);
    assert_eq!(focusable(&presented), [true, false, false, false]);
}

#[test]
fn a_hidden_option_presents_nothing_and_leaves_the_set() {
    let options = weekdays([false; 7]);
    let spec = SegmentedChoiceSpec::multiple("Runs on", &options);
    let presented = present(&spec, 400.0, comfortable(), 1.0, &|argument| {
        if *argument == ActionArgument::from("Sunday") {
            Availability::Hidden
        } else {
            Availability::Enabled
        }
    });
    assert!(!presented.options[6].rect.is_positive());
    assert!(
        presented
            .snapshot
            .nodes
            .iter()
            .all(|node| node.name != "Sunday")
    );
    let sizes: Vec<_> = presented
        .update
        .nodes
        .iter()
        .filter_map(|(_, node)| node.size_of_set())
        .collect();
    assert_eq!(sizes, [6; 6]);
}

/// Each option's rect, row by row.
fn rows_of(options: &[Response]) -> Vec<Vec<Rect>> {
    let mut rows: Vec<Vec<Rect>> = Vec::new();
    for rect in options.iter().map(|option| option.rect) {
        match rows.last_mut() {
            Some(row) if (row[0].top() - rect.top()).abs() < 0.5 => row.push(rect),
            _ => rows.push(vec![rect]),
        }
    }
    rows
}

#[test]
fn seven_weekday_options_fit_or_wrap_legibly_at_narrow_widths() {
    // A phone 390 points wide less 16-point margins leaves 358; the narrowest
    // supported layout leaves 288.
    for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
        let tokens = DesignTokens::resolve(ThemeVariant::Light, density);
        for font_scale in [1.0, 1.5] {
            for width in [358.0, 288.0] {
                let options = weekdays([false, true, false, false, true, false, false]);
                let spec = SegmentedChoiceSpec::multiple("Runs on", &options);
                let presented =
                    present(&spec, width, tokens, font_scale, &|_| Availability::Enabled);
                let case = format!("{density:?} at {font_scale} in {width}");
                assert!(
                    audit_text_layouts(&presented.text).is_empty(),
                    "{case}: {:?}",
                    audit_text_layouts(&presented.text)
                );
                assert_eq!(presented.text.len(), 7, "{case}");
                assert!(
                    presented.text.iter().all(|text| !text.truncated),
                    "{case}: a label is cut short"
                );
                let rows = rows_of(&presented.options);
                let counts: Vec<_> = rows.iter().map(Vec::len).collect();
                eprintln!("{case}: rows {counts:?}, option {:?}", rows[0][0].size());
                // Balanced rows: none more than one option longer than another.
                assert!(
                    counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1,
                    "{case}: {counts:?}"
                );
                let first = presented.options[0].rect;
                for option in &presented.options {
                    let rect = option.rect;
                    assert!(rect.left() >= -0.5 && rect.right() <= width + 0.5, "{case}");
                    assert!((rect.width() - first.width()).abs() < 0.5, "{case}");
                    assert!(rect.width() >= tokens.geometry.minimum_hit_size.0, "{case}");
                    assert!(
                        rect.height() >= tokens.geometry.minimum_hit_size.0,
                        "{case}"
                    );
                    assert!(
                        presented
                            .options
                            .iter()
                            .filter(|other| other.rect.intersects(rect.shrink(0.5)))
                            .count()
                            == 1,
                        "{case}: options overlap"
                    );
                }
                assert!(presented.group.right() <= width + 0.5, "{case}");
            }
        }
    }
}

#[test]
fn long_labels_take_rows_of_their_own_rather_than_truncating() {
    let options = [
        ChoiceOption::new("daily", "Every day", true),
        ChoiceOption::new("weekdays", "Every weekday", false),
        ChoiceOption::new("hours", "Every few hours", false),
        ChoiceOption::new("once", "Once, at a set time", false),
    ];
    let spec = SegmentedChoiceSpec::single("Repeats", &options);
    for width in [900.0, 288.0] {
        let presented = present(&spec, width, comfortable(), 1.5, &|_| Availability::Enabled);
        assert!(presented.text.iter().all(|text| !text.truncated), "{width}");
        assert!(presented.group.right() <= width + 0.5, "{width}");
        let rows = rows_of(&presented.options);
        if width == 900.0 {
            assert_eq!(rows.len(), 1);
        } else {
            assert!(rows.len() > 1, "{width}");
        }
    }
}

#[test]
fn a_label_wider_than_the_choice_elides_and_keeps_its_name() {
    let options = [
        ChoiceOption::new("long", "A label far wider than the narrow space", true),
        ChoiceOption::new("short", "Short", false),
    ];
    let spec = SegmentedChoiceSpec::single("Repeats", &options);
    let presented = present(&spec, 120.0, comfortable(), 1.0, &|_| Availability::Enabled);
    assert!(presented.options[0].rect.right() <= 120.5);
    assert!(presented.text[0].truncated);
    assert!(audit_text_layouts(&presented.text).is_empty());
    assert!(
        presented
            .snapshot
            .nodes
            .iter()
            .any(|node| node.name == "A label far wider than the narrow space")
    );
}

#[test]
fn short_labels_on_bound_buttons_meet_the_minimum_hit_size() {
    for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
        let tokens = DesignTokens::resolve(ThemeVariant::Light, density);
        for font_scale in [1.0, 1.5] {
            let context = egui::Context::default();
            crate::install_typography_fonts(&context);
            let mut result = None;
            for _ in 0..2 {
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    let mut p = PresentationContext::new(
                        ui,
                        tokens,
                        font_scale,
                        PresentationScope::new("labels"),
                        SemanticUiId::root(),
                    );
                    let spec = |state| crate::ActionButtonSpec {
                        target: ActionTarget::application(TestAction::AppearanceSettings),
                        availability: Availability::Enabled,
                        state,
                        emphasis: ActionEmphasis::Normal,
                        compact: false,
                    };
                    let rects = ui
                        .horizontal(|ui| {
                            [("Tue", "Tuesday"), ("Edit", "Edit Ada")].map(|(label, name)| {
                                p.labelled_bound_action(
                                    ui,
                                    label,
                                    spec(ActionButtonState::Momentary),
                                    ActionButtonContent::Text,
                                    &ActionArgument::from(1_u32),
                                    Some(name),
                                    Some(label),
                                )
                                .rect
                            })
                        })
                        .inner;
                    let published = p.finish(ui);
                    result = Some((rects, published.text_layouts, published.semantic_nodes));
                });
                output.textures_delta.clear();
            }
            let (rects, text, nodes) = result.unwrap();
            let case = format!("{density:?} at {font_scale}");
            for rect in rects {
                eprintln!("{case}: {:?}", rect.size());
                assert!(rect.width() >= tokens.geometry.minimum_hit_size.0, "{case}");
                assert!(
                    rect.height() >= tokens.geometry.minimum_hit_size.0,
                    "{case}"
                );
            }
            assert_eq!(text.len(), 2, "{case}");
            assert!(text.iter().all(|text| !text.truncated), "{case}");
            assert!(audit_text_layouts(&text).is_empty(), "{case}");
            assert_eq!(
                nodes
                    .iter()
                    .map(|node| node.name.as_str())
                    .collect::<Vec<_>>(),
                ["Tuesday", "Edit Ada"]
            );
        }
    }
}

#[test]
fn options_wrap_into_balanced_rows_that_fit() {
    // Seven options of 46 points with 4 between them take 346 points in
    // one row; 300 points fit six to a row, so two balanced rows hold four
    // and three.
    assert_eq!(rows(7, 46.0, 4.0, 346.0), 7);
    assert_eq!(rows(7, 46.0, 4.0, 345.0), 4);
    assert_eq!(rows(7, 46.0, 4.0, 300.0), 4);
    assert_eq!(rows(7, 46.0, 4.0, 140.0), 2);
    assert_eq!(rows(3, 400.0, 4.0, 300.0), 1);
    assert_eq!(rows(0, 46.0, 4.0, 300.0), 0);
}

#[test]
fn arrow_keys_skip_disabled_options_and_wrap() {
    let enabled = [true, false, true, true];
    assert_eq!(step_enabled(&enabled, 0, true), Some(2));
    assert_eq!(step_enabled(&enabled, 3, true), Some(0));
    assert_eq!(step_enabled(&enabled, 0, false), Some(3));
    assert_eq!(step_enabled(&enabled, 2, false), Some(0));
    assert_eq!(step_enabled(&[false, true], 1, true), Some(1));
}
