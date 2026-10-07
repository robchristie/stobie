use super::*;
use crate::{
    DensityVariant, ThemeVariant, UiSnapshot, audit_accesskit, audit_text_layouts,
    test_actions::TestAction,
};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{cell::Cell, rc::Rc};

fn spec<'a>(
    label: &'a str,
    selected: bool,
    badge: Option<NavigationBadge<'a>>,
) -> NavigationItemSpec<'a, TestAction> {
    NavigationItemSpec {
        target: ActionTarget::pane(TestAction::CommitPolygon, polyorama_core::PaneId(7)),
        icon: IconId::Tasks,
        label,
        description: "Open the task destination",
        selected,
        availability: Availability::Enabled,
        badge,
    }
}

#[test]
fn pointer_keyboard_and_accesskit_activation_focus_does_not_select() {
    let activations = Rc::new(Cell::new(0));
    let output = activations.clone();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 150.0))
        .build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let mut p = crate::PresentationContext::new(
                ui,
                tokens,
                1.0,
                crate::PresentationScope::new("sidebar"),
                SemanticUiId::root(),
            );
            p.navigation_item(ui, "home", spec("Home", true, None));
            if p.navigation_item(
                ui,
                "tasks",
                spec(
                    "Tasks",
                    false,
                    Some(NavigationBadge::Count {
                        value: 125,
                        meaning: "outstanding tasks",
                    }),
                ),
            )
            .clicked()
            {
                output.set(output.get() + 1);
            }
            p.finish(ui);
        });
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(harness.get_by_label("Tasks").is_focused());
    assert_eq!(activations.get(), 0);
    assert_eq!(
        harness.get_by_label("Home").accesskit_node().aria_current(),
        Some(egui::accesskit::AriaCurrent::True)
    );
    assert_eq!(
        harness
            .get_by_label("Tasks")
            .accesskit_node()
            .aria_current(),
        Some(egui::accesskit::AriaCurrent::False)
    );
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    harness.get_by_label("Tasks").click();
    harness.run();
    harness.get_by_label("Tasks").click_accesskit();
    harness.run();
    assert_eq!(activations.get(), 4);
    let node = harness.get_by_label("Tasks");
    assert_eq!(node.accesskit_node().label(), Some("Tasks".into()));
    assert!(
        node.accesskit_node()
            .description()
            .unwrap()
            .contains("125 outstanding tasks")
    );
    assert_eq!(node.accesskit_node().toggled(), None);
    assert_eq!(node.accesskit_node().is_selected(), None);
}

#[test]
fn disabled_and_parent_disabled_rows_never_activate() {
    for parent_disabled in [false, true] {
        let clicks = Rc::new(Cell::new(0));
        let observed = clicks.clone();
        let mut harness = Harness::builder().build_ui(move |ui| {
            crate::install_typography_fonts(ui.ctx());
            if parent_disabled {
                ui.disable();
            }
            let tokens = DesignTokens::resolve(ThemeVariant::Light, DensityVariant::Compact);
            let mut s = spec("Tasks", true, None);
            if !parent_disabled {
                s.availability = Availability::Disabled {
                    reason: "Account is read-only".into(),
                };
            }
            let response = navigation_item(
                ui,
                &s,
                crate::PresentationScope::new("sidebar").instance("tasks"),
                &tokens,
                1.0,
                &mut Vec::new(),
            );
            if response.clicked() {
                observed.set(observed.get() + 1);
            }
        });
        let node = harness.get_by_label("Tasks");
        assert!(node.accesskit_node().is_disabled());
        assert!(
            !node
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        if !parent_disabled {
            assert!(
                node.accesskit_node()
                    .description()
                    .unwrap()
                    .contains("Account is read-only")
            );
        }
        node.click();
        harness.run();
        harness.key_press(egui::Key::Tab);
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.key_press(egui::Key::Space);
        harness.run();
        assert_eq!(clicks.get(), 0);
    }
}

#[test]
fn identity_focus_and_metadata_survive_label_badge_order_and_layout_changes() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::install_typography_fonts(&context);
    let scope = crate::PresentationScope::new("feature").child(17_u64);
    let id = scope.instance("tasks").egui_id();
    for reverse in [false, true] {
        let mut output = context.run_ui(Default::default(), |ui| {
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let mut p =
                crate::PresentationContext::new(ui, tokens, 1.0, scope, SemanticUiId::root())
                    .with_domain_reference(DomainReference::External {
                        namespace: "fixture".into(),
                        id: "17".into(),
                    });
            ui.push_id(reverse, |ui| {
                for key in if reverse {
                    ["home", "tasks"]
                } else {
                    ["tasks", "home"]
                } {
                    let response = p.navigation_item(
                        ui,
                        key,
                        spec(
                            if reverse {
                                "Updated destination"
                            } else {
                                "Tasks"
                            },
                            false,
                            Some(NavigationBadge::Count {
                                value: if reverse { 1000 } else { 0 },
                                meaning: "tasks",
                            }),
                        ),
                    );
                    if key == "tasks" {
                        assert_eq!(response.id, id);
                        if !reverse {
                            response.request_focus();
                        } else {
                            assert!(response.has_focus());
                        }
                    }
                }
            });
            let observed = p.finish(ui);
            let node = observed
                .semantic_nodes
                .iter()
                .find(|node| node.id == scope.instance("tasks").semantic_id())
                .unwrap();
            assert_eq!(node.pane, Some(polyorama_core::PaneId(7)));
            assert_eq!(
                node.domain_reference,
                Some(DomainReference::External {
                    namespace: "fixture".into(),
                    id: "17".into()
                })
            );
        });
        output.textures_delta.clear();
    }
}

#[test]
fn layout_minimum_and_current_parity_across_appearances() {
    for theme in [
        ThemeVariant::Light,
        ThemeVariant::Dark,
        ThemeVariant::LightHighContrast,
        ThemeVariant::DarkHighContrast,
    ] {
        for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
            for scale in [1.0, 1.5] {
                let context = egui::Context::default();
                context.enable_accesskit();
                crate::install_typography_fonts(&context);
                let tokens = DesignTokens::resolve(theme, density);
                let mut nodes = Vec::new();
                let mut layouts = Vec::new();
                let mut output = context.run_ui(Default::default(), |ui| {
                    ui.set_width(1.0);
                    let mut p = crate::PresentationContext::new(
                        ui,
                        tokens,
                        scale,
                        crate::PresentationScope::new("sidebar"),
                        SemanticUiId::root(),
                    );
                    for (index, badge) in [
                        None,
                        Some(NavigationBadge::Count {
                            value: 0,
                            meaning: "tasks",
                        }),
                        Some(NavigationBadge::Count {
                            value: u64::MAX,
                            meaning: "tasks",
                        }),
                        Some(NavigationBadge::Label {
                            text: "A very long status label that must be elided",
                            description: "Complete status information",
                        }),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let minimum = navigation_item_minimum_width(ui, badge, &tokens, scale);
                        let response = p.navigation_item(
                            ui,
                            index,
                            spec(
                                "A destination with a deliberately long name",
                                index == 0,
                                badge,
                            ),
                        );
                        assert_eq!(response.rect.width(), minimum);
                        assert!(response.rect.height() >= tokens.geometry.minimum_hit_size.0);
                    }
                    let observed = p.finish(ui);
                    nodes = observed.semantic_nodes;
                    layouts = observed.text_layouts;
                });
                output.textures_delta.clear();
                assert_eq!(layouts.len(), 7);
                assert!(
                    audit_text_layouts(&layouts).is_empty(),
                    "{:#?}",
                    audit_text_layouts(&layouts)
                );
                assert!(layouts.iter().all(|text| text.line_count == 1
                    && text.interaction == crate::TextInteraction::Inert));
                assert!(
                    layouts
                        .iter()
                        .filter(|text| text.component_id.kind == TextComponentKind::NavigationLabel)
                        .all(|text| text.truncated)
                );
                assert_eq!(nodes.len(), 4);
                let snapshot = UiSnapshot {
                    nodes,
                    ..Default::default()
                };
                let update = output.platform_output.accesskit_update.take().unwrap();
                assert!(
                    audit_accesskit(&snapshot, &update).is_empty(),
                    "{:#?}",
                    audit_accesskit(&snapshot, &update)
                );
            }
        }
    }
}

#[test]
fn partially_clipped_and_hidden_rows_publish_current_visible_bounds_only() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::install_typography_fonts(&context);
    let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
    let mut nodes = Vec::new();
    let mut layouts = Vec::new();
    let mut output = context.run_ui(Default::default(), |ui| {
        ui.set_width(240.0);
        let start = ui.cursor().min;
        ui.set_clip_rect(Rect::from_min_size(
            start + egui::vec2(0.0, 12.0),
            egui::vec2(240.0, 10.0),
        ));
        let mut p = crate::PresentationContext::new(
            ui,
            tokens,
            1.0,
            crate::PresentationScope::new("clip"),
            SemanticUiId::root(),
        );
        let response = p.navigation_item(
            ui,
            "partial",
            spec(
                "Partly visible",
                true,
                Some(NavigationBadge::Count {
                    value: 12,
                    meaning: "tasks",
                }),
            ),
        );
        assert!(response.rect.height() >= 32.0);
        assert_eq!(response.interact_rect.height(), 10.0);
        p.navigation_item(ui, "hidden", spec("Below the clip", false, None));
        let mut s = spec("Unavailable in this layout", false, None);
        s.availability = Availability::Hidden;
        p.navigation_item(ui, "omitted", s);
        let observed = p.finish(ui);
        nodes = observed.semantic_nodes;
        layouts = observed.text_layouts;
        assert_eq!(observed.coverage.attempted_components, 3);
    });
    output.textures_delta.clear();
    assert_eq!(nodes.len(), 1);
    assert_eq!(layouts.len(), 2);
    for text in &layouts {
        assert_eq!(text.overflow, TextOverflow::Ellipsis);
        assert_eq!(text.line_count, 1);
        assert_eq!(text.clip_rect.max_y - text.clip_rect.min_y, 10.0);
        assert!(text.painted_rect.min_x >= text.allocated_rect.min_x - 1.0);
        assert!(text.painted_rect.max_x <= text.allocated_rect.max_x + 1.0);
        assert!(text.painted_rect.min_y >= text.allocated_rect.min_y - 1.0);
        assert!(text.painted_rect.max_y <= text.allocated_rect.max_y + 1.0);
        assert!(text.parent_id.is_some());
    }
    assert_eq!(nodes[0].rect.max_y - nodes[0].rect.min_y, 10.0);
    let update = output.platform_output.accesskit_update.take().unwrap();
    assert!(
        audit_accesskit(
            &UiSnapshot {
                nodes,
                ..Default::default()
            },
            &update
        )
        .is_empty()
    );
    assert!(
        !update
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Below the clip")
                || node.label() == Some("Unavailable in this layout"))
    );
}

#[test]
fn decorative_icon_and_badge_regions_activate_the_single_row() {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
    let identity = crate::PresentationScope::new("regions").instance("tasks");
    let mut clicks = 0;
    let mut row = Rect::NOTHING;
    for at_badge in [false, true] {
        let mut initial = context.run_ui(Default::default(), |ui| {
            ui.set_width(240.0);
            row = navigation_item(
                ui,
                &spec(
                    "Tasks",
                    false,
                    Some(NavigationBadge::Count {
                        value: 12,
                        meaning: "tasks",
                    }),
                ),
                identity,
                &tokens,
                1.0,
                &mut Vec::new(),
            )
            .rect;
        });
        initial.textures_delta.clear();
        let point = egui::pos2(
            if at_badge {
                row.right() - 16.0
            } else {
                row.left() + 16.0
            },
            row.center().y,
        );
        for pressed in [true, false] {
            let mut output = context.run_ui(
                egui::RawInput {
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    ..Default::default()
                },
                |ui| {
                    ui.set_width(240.0);
                    let response = navigation_item(
                        ui,
                        &spec(
                            "Tasks",
                            false,
                            Some(NavigationBadge::Count {
                                value: 12,
                                meaning: "tasks",
                            }),
                        ),
                        identity,
                        &tokens,
                        1.0,
                        &mut Vec::new(),
                    );
                    if response.clicked() {
                        clicks += 1;
                    }
                },
            );
            output.textures_delta.clear();
        }
    }
    assert_eq!(clicks, 2);
}

#[test]
fn current_marker_hover_press_and_focus_use_distinct_geometry() {
    for theme in [
        ThemeVariant::Light,
        ThemeVariant::Dark,
        ThemeVariant::LightHighContrast,
        ThemeVariant::DarkHighContrast,
    ] {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
        let identity = crate::PresentationScope::new("states").instance("home");
        let mut rect = Rect::NOTHING;
        let mut initial = context.run_ui(Default::default(), |ui| {
            ui.set_width(240.0);
            let response = navigation_item(
                ui,
                &spec("Home", true, None),
                identity,
                &tokens,
                1.0,
                &mut Vec::new(),
            );
            rect = response.rect;
            response.request_focus();
        });
        initial.textures_delta.clear();
        for pressed in [false, true] {
            let mut events = vec![egui::Event::PointerMoved(rect.center())];
            if pressed {
                events.push(egui::Event::PointerButton {
                    pos: rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let mut output = context.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    ui.set_width(240.0);
                    let response = navigation_item(
                        ui,
                        &spec("Home", true, None),
                        identity,
                        &tokens,
                        1.0,
                        &mut Vec::new(),
                    );
                    assert!(response.has_focus());
                    assert!(response.hovered());
                },
            );
            output.textures_delta.clear();
            let rectangles: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Rect(rect) = &shape.shape {
                        Some(rect)
                    } else {
                        None
                    }
                })
                .collect();
            assert!(
                rectangles
                    .iter()
                    .any(|shape| shape.rect.width() == tokens.spacing.unit.0 * 0.5
                        && shape.fill == egui::Color32::from(tokens.colours.selection_indicator)),
                "persistent current marker"
            );
            assert!(
                rectangles.iter().any(|shape| shape.rect == rect
                    && shape.stroke.color == egui::Color32::from(tokens.colours.focus_ring)),
                "outer focus"
            );
            assert!(
                rectangles.iter().any(|shape| shape.rect == rect
                    && shape.stroke.color == egui::Color32::from(tokens.colours.border_control)),
                "current hover"
            );
            assert_eq!(
                rectangles.iter().any(|shape| shape.rect
                    == rect.shrink(tokens.spacing.unit.0 * 0.5)
                    && shape.stroke.color
                        == egui::Color32::from(tokens.colours.selection_indicator)),
                pressed,
                "inset press"
            );
        }
    }
}

#[test]
fn navigation_parity_rejects_wrong_current_and_disabled_click() {
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::install_typography_fonts(&context);
    let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
    let identity = crate::PresentationScope::new("parity").instance("tasks");
    let mut nodes = Vec::new();
    let mut output = context.run_ui(Default::default(), |ui| {
        let mut s = spec("Tasks", true, None);
        s.availability = Availability::Disabled {
            reason: "Read-only".into(),
        };
        let response = navigation_item(ui, &s, identity, &tokens, 1.0, &mut Vec::new());
        nodes.push(navigation_item_semantic_node(
            &response,
            &s,
            identity,
            SemanticUiId::root(),
        ));
    });
    output.textures_delta.clear();
    let mut update = output.platform_output.accesskit_update.take().unwrap();
    let snapshot = UiSnapshot {
        nodes,
        ..Default::default()
    };
    assert!(audit_accesskit(&snapshot, &update).is_empty());
    let (_, node) = update
        .nodes
        .iter_mut()
        .find(|(_, node)| node.author_id() == Some(identity.semantic_id().0.as_str()))
        .unwrap();
    node.set_aria_current(egui::accesskit::AriaCurrent::False);
    node.add_action(egui::accesskit::Action::Click);
    let findings = audit_accesskit(&snapshot, &update);
    assert!(
        findings
            .iter()
            .any(|f| matches!(f, crate::AccessKitMismatch::Selected { .. }))
    );
    assert!(
        findings
            .iter()
            .any(|f| matches!(f, crate::AccessKitMismatch::ClickAction { .. }))
    );
}

#[test]
fn failed_measurements_remain_observable_when_fully_clipped() {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
    let mut output = context.run_ui(Default::default(), |ui| {
        ui.set_clip_rect(Rect::from_min_size(
            egui::pos2(0.0, 1000.0),
            egui::vec2(300.0, 50.0),
        ));
        let mut p = crate::PresentationContext::new(
            ui,
            tokens,
            2.0,
            crate::PresentationScope::new("failed-clip"),
            SemanticUiId::root(),
        );
        p.navigation_item(
            ui,
            "tasks",
            spec(
                "Tasks",
                false,
                Some(NavigationBadge::Count {
                    value: 3,
                    meaning: "tasks",
                }),
            ),
        );
        p.retain_visible_text(ui, |_| false);
        let publication = p.finish(ui);
        assert!(publication.semantic_nodes.is_empty());
        assert_eq!(publication.text_layouts.len(), 2);
        assert!(
            publication
                .text_layouts
                .iter()
                .all(|text| text.layout_error == Some(crate::TextLayoutError::InvalidFontScale))
        );
        assert_eq!(publication.coverage.attempted_components, 2);
        assert_eq!(publication.coverage.failed_components, 2);
        assert_eq!(publication.coverage.successful_components, 0);
        assert!(!audit_text_layouts(&publication.text_layouts).is_empty());
    });
    output.textures_delta.clear();
}

#[test]
fn valid_theme_with_aliased_hover_indicator_retains_unselected_press_feedback() {
    use crate::{ApplicationTheme, Rgba8, TypographyProfile};
    let black = Rgba8 {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 255,
    };
    let white = Rgba8 {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };
    let grey = Rgba8 {
        red: 102,
        green: 102,
        blue: 102,
        alpha: 255,
    };
    let mut colours = ApplicationTheme::analytical().colours();
    for colour in [
        &mut colours.light,
        &mut colours.dark,
        &mut colours.light_high_contrast,
        &mut colours.dark_high_contrast,
    ] {
        colour.surface_canvas = black;
        colour.surface_panel = black;
        colour.surface_raised = black;
        colour.surface_hover = black;
        colour.selection_background = black;
        colour.action_quiet_hover = black;
        colour.text_primary = white;
        colour.text_muted = white;
        colour.action_primary_background = white;
        colour.action_primary_foreground = black;
        colour.focus_ring = white;
        colour.selection_indicator = white;
    }
    colours.light.action_quiet_hover = grey;
    colours.light.selection_indicator = grey;
    let theme = ApplicationTheme::new(colours)
        .expect("aliased standard palette passes full theme validation");
    let tokens = theme.resolve(
        ThemeVariant::Light,
        DensityVariant::Comfortable,
        TypographyProfile::Dense,
    );
    let context = egui::Context::default();
    context.enable_accesskit();
    crate::install_typography_fonts(&context);
    let identity = crate::PresentationScope::new("alias").instance("tasks");
    let mut rect = Rect::NOTHING;
    let mut output = context.run_ui(Default::default(), |ui| {
        ui.set_width(240.0);
        let response = navigation_item(
            ui,
            &spec("Tasks", false, None),
            identity,
            &tokens,
            1.0,
            &mut Vec::new(),
        );
        rect = response.rect;
        response.request_focus();
    });
    output.textures_delta.clear();
    for pressed in [false, true] {
        let mut events = vec![egui::Event::PointerMoved(rect.center())];
        if pressed {
            events.push(egui::Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
        }
        let mut nodes = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                ui.set_width(240.0);
                let s = spec("Tasks", false, None);
                let response = navigation_item(ui, &s, identity, &tokens, 1.0, &mut Vec::new());
                assert!(response.has_focus());
                nodes.push(navigation_item_semantic_node(
                    &response,
                    &s,
                    identity,
                    SemanticUiId::root(),
                ));
            },
        );
        output.textures_delta.clear();
        let rectangles: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Rect(rect) = &shape.shape {
                    Some(rect)
                } else {
                    None
                }
            })
            .collect();
        assert!(rectangles.iter().any(|shape| shape.rect == rect
            && shape.fill == egui::Color32::from(if pressed { black } else { grey })));
        assert!(
            !rectangles
                .iter()
                .any(|shape| shape.rect.width() == tokens.spacing.unit.0 * 0.5),
            "no current marker on pressed alternative"
        );
        assert_eq!(
            rectangles.iter().any(
                |shape| shape.rect == rect.shrink(tokens.spacing.unit.0 * 0.5)
                    && shape.stroke.color == egui::Color32::from(grey)
            ),
            pressed
        );
        assert!(rectangles.iter().any(|shape| shape.rect == rect && shape.stroke.color == egui::Color32::from(white)), "independent focus");
        assert!(!nodes[0].selected);
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(
            audit_accesskit(
                &UiSnapshot {
                    nodes,
                    ..Default::default()
                },
                &update
            )
            .is_empty()
        );
    }
}
