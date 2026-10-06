use crate::test_actions::TestAction;
use crate::*;
use egui::{Color32, Rect};

fn spec(
    availability: Availability,
    state: ActionButtonState,
    emphasis: ActionEmphasis,
) -> ActionButtonSpec<TestAction> {
    ActionButtonSpec {
        target: ActionTarget::pane(TestAction::CommitPolygon, polyorama_core::PaneId(7)),
        availability,
        state,
        emphasis,
        compact: false,
    }
}

#[test]
fn icon_actions_activate_through_pointer_keyboard_and_accesskit_without_mutating_toggle() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use std::{cell::Cell, rc::Rc};
    for content in [
        ActionButtonContent::IconOnly(IconId::Check),
        ActionButtonContent::IconLabel(IconId::Check),
    ] {
        let count = Rc::new(Cell::new(0));
        let observed = count.clone();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(400.0, 150.0))
            .build_ui(move |ui| {
                install_typography_fonts(ui.ctx());
                let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
                if action_button_with_content(
                    ui,
                    spec(
                        Availability::Enabled,
                        ActionButtonState::Toggle { pressed: true },
                        ActionEmphasis::QuietBorderless,
                    ),
                    content,
                    &tokens,
                    1.0,
                    &mut Vec::new(),
                )
                .clicked()
                {
                    observed.set(observed.get() + 1);
                }
            });
        harness.get_by_label("Commit polygon").click();
        harness.run();
        assert_eq!(count.get(), 1);
        harness.get_by_label("Commit polygon").focus();
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.key_press(egui::Key::Space);
        harness.run();
        assert_eq!(count.get(), 3);
        harness.get_by_label("Commit polygon").click_accesskit();
        harness.run();
        assert_eq!(count.get(), 4);
        assert_eq!(
            harness
                .get_by_label("Commit polygon")
                .accesskit_node()
                .toggled(),
            Some(egui::accesskit::Toggled::True)
        );
    }
}

#[test]
fn disabled_icon_actions_do_not_activate_and_retain_reason() {
    use egui_kittest::{
        Harness,
        kittest::{NodeT, Queryable},
    };
    use std::{cell::Cell, rc::Rc};
    let count = Rc::new(Cell::new(0));
    let observed = count.clone();
    let mut harness = Harness::builder().build_ui(move |ui| {
        let tokens = DesignTokens::resolve(ThemeVariant::Light, DensityVariant::Compact);
        if action_button_with_content(
            ui,
            spec(
                Availability::Disabled {
                    reason: "No preview vertices".into(),
                },
                ActionButtonState::Toggle { pressed: false },
                ActionEmphasis::Primary,
            ),
            ActionButtonContent::IconOnly(IconId::Check),
            &tokens,
            1.0,
            &mut Vec::new(),
        )
        .clicked()
        {
            observed.set(observed.get() + 1);
        }
    });
    let node = harness.get_by_label("Commit polygon");
    assert!(node.accesskit_node().is_disabled());
    assert!(
        node.accesskit_node()
            .description()
            .unwrap()
            .contains("No preview vertices")
    );
    node.click();
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(count.get(), 0);
}

#[test]
fn icon_label_reserves_artwork_and_measured_ellipsis_at_narrow_widths() {
    for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
        for scale in [1.0, 1.25, 1.5] {
            for width in [32.0, 80.0, 240.0] {
                let context = egui::Context::default();
                install_typography_fonts(&context);
                let tokens = DesignTokens::resolve(ThemeVariant::Dark, density);
                let mut layouts = Vec::new();
                let mut response_rect = Rect::NOTHING;
                let mut output = context.run_ui(Default::default(), |ui| {
                    ui.set_max_width(width);
                    response_rect = action_button_with_content(
                        ui,
                        spec(
                            Availability::Enabled,
                            ActionButtonState::Momentary,
                            ActionEmphasis::Normal,
                        ),
                        ActionButtonContent::IconLabel(IconId::Polygon),
                        &tokens,
                        scale,
                        &mut layouts,
                    )
                    .rect;
                });
                output.textures_delta.clear();
                assert_eq!(layouts.len(), 1);
                assert!(
                    audit_text_layouts(&layouts).is_empty(),
                    "{density:?}, {scale}, {width}: {:?}",
                    audit_text_layouts(&layouts)
                );
                let label = &layouts[0];
                assert_eq!(label.horizontal_alignment, HorizontalTextAlignment::Start);
                assert_eq!(
                    label.allocated_rect.min_x,
                    response_rect.left()
                        + tokens.geometry.control_padding_x.0
                        + icon_size(&tokens, scale)
                        + tokens.spacing.inline.0
                );
                assert!(
                    label.allocated_rect.max_x
                        <= response_rect.right() - tokens.geometry.control_padding_x.0 + 0.01
                );
                if width == 32.0 {
                    assert!(response_rect.width() > width);
                    assert!(label.truncated);
                }
            }
        }
    }
}

#[test]
fn icon_presentations_preserve_clipping_instances_metadata_and_semantic_parity() {
    for content in [
        ActionButtonContent::IconOnly(IconId::Polygon),
        ActionButtonContent::IconLabel(IconId::Polygon),
    ] {
        for availability in [
            Availability::Enabled,
            Availability::Disabled {
                reason: "No vertices".into(),
            },
        ] {
            let context = egui::Context::default();
            install_typography_fonts(&context);
            context.enable_accesskit();
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
            let root_rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 100.0));
            let clip = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 16.0));
            let scope = PresentationScope::new("icons").child("object-1");
            let domain = DomainReference::External {
                namespace: "test".into(),
                id: "object-1".into(),
            };
            let mut ids = Vec::new();
            for reversed in [false, true] {
                let mut publication = None;
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(root_rect),
                        ..Default::default()
                    },
                    |ui| {
                        ui.set_clip_rect(clip);
                        let mut presentation =
                            PresentationContext::new(ui, tokens, 1.0, scope, SemanticUiId::root())
                                .with_domain_reference(domain.clone());
                        ui.horizontal(|ui| {
                            for key in if reversed {
                                ["footer", "header"]
                            } else {
                                ["header", "footer"]
                            } {
                                let response = presentation.action_with_content(
                                    ui,
                                    key,
                                    spec(
                                        availability.clone(),
                                        ActionButtonState::Toggle { pressed: true },
                                        ActionEmphasis::Normal,
                                    ),
                                    content,
                                );
                                assert_eq!(response.id, scope.instance(key).egui_id());
                                assert_eq!(response.interact_rect.height(), 16.0);
                            }
                        });
                        ui.add_space(50.0);
                        presentation.action_with_content(
                            ui,
                            "hidden",
                            spec(
                                availability.clone(),
                                ActionButtonState::Momentary,
                                ActionEmphasis::Normal,
                            ),
                            content,
                        );
                        publication = Some(presentation.finish(ui));
                    },
                );
                output.textures_delta.clear();
                let published = publication.unwrap();
                assert_eq!(published.semantic_nodes.len(), 2);
                for node in &published.semantic_nodes {
                    assert_eq!(node.pane, Some(polyorama_core::PaneId(7)));
                    assert_eq!(node.domain_reference, Some(domain.clone()));
                    assert_eq!(node.name, "Commit polygon");
                    assert_eq!(node.checked, Some(true));
                    assert_eq!(node.rect.max_y, 16.0);
                }
                if matches!(content, ActionButtonContent::IconOnly(_)) {
                    assert!(published.text_layouts.is_empty());
                    assert_eq!(published.coverage.attempted_components, 0);
                }
                if matches!(content, ActionButtonContent::IconLabel(_)) {
                    assert_eq!(published.text_layouts.len(), 2);
                    assert_eq!(published.coverage.attempted_components, 3);
                }
                let update = output.platform_output.accesskit_update.unwrap();
                assert!(!update.nodes.iter().any(|(_, node)| node.author_id()
                    == Some(scope.instance("hidden").semantic_id().0.as_str())));
                let current_ids: Vec<_> = published
                    .semantic_nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect();
                if reversed {
                    assert_eq!(ids, current_ids.into_iter().rev().collect::<Vec<_>>());
                } else {
                    ids = current_ids;
                }
                let mut nodes = vec![UiNode::container(
                    SemanticUiId::root(),
                    None,
                    UiRole::Application,
                    root_rect.into(),
                )];
                nodes.extend(published.semantic_nodes);
                let snapshot = UiSnapshot {
                    root: SemanticUiId::root(),
                    nodes,
                    ..Default::default()
                };
                assert!(snapshot.audit().is_empty());
                assert!(
                    audit_accesskit(&snapshot, &update).is_empty(),
                    "{:?}",
                    audit_accesskit(&snapshot, &update)
                );
            }
        }
    }
}

#[test]
fn icon_and_label_foregrounds_match_and_keyboard_focus_is_visible() {
    for theme in [
        ThemeVariant::Light,
        ThemeVariant::Dark,
        ThemeVariant::LightHighContrast,
        ThemeVariant::DarkHighContrast,
    ] {
        for emphasis in [
            ActionEmphasis::Quiet,
            ActionEmphasis::QuietBorderless,
            ActionEmphasis::Normal,
            ActionEmphasis::Primary,
        ] {
            for disabled in [false, true] {
                let context = egui::Context::default();
                install_typography_fonts(&context);
                let tokens = DesignTokens::resolve(theme, DensityVariant::Comfortable);
                let action = spec(
                    if disabled {
                        Availability::Disabled {
                            reason: "No preview".into(),
                        }
                    } else {
                        Availability::Enabled
                    },
                    ActionButtonState::Toggle { pressed: true },
                    emphasis,
                );
                if !disabled {
                    context.memory_mut(|memory| {
                        memory.request_focus(
                            ActionButtonIdentity::for_target(action.target).widget_id,
                        )
                    });
                }
                let mut output = context.run_ui(Default::default(), |ui| {
                    let response = action_button_with_content(
                        ui,
                        spec(
                            if disabled {
                                Availability::Disabled {
                                    reason: "No preview".into(),
                                }
                            } else {
                                Availability::Enabled
                            },
                            ActionButtonState::Toggle { pressed: true },
                            emphasis,
                        ),
                        ActionButtonContent::IconLabel(IconId::Check),
                        &tokens,
                        1.0,
                        &mut Vec::new(),
                    );
                    assert_eq!(response.has_focus(), !disabled);
                });
                output.textures_delta.clear();
                let expected: Color32 = if disabled {
                    tokens.colours.text_muted
                } else if emphasis == ActionEmphasis::Primary {
                    tokens.colours.action_primary_foreground
                } else {
                    tokens.colours.text_primary
                }
                .into();
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment {stroke, ..} if stroke.color == expected)));
                for text in output.shapes.iter().filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        Some(text)
                    } else {
                        None
                    }
                }) {
                    assert!(
                        text.galley
                            .rows
                            .iter()
                            .flat_map(|row| &row.visuals.mesh.vertices)
                            .all(|vertex| vertex.color == expected)
                    );
                }
                if !disabled {
                    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == Color32::from(tokens.colours.focus_ring))));
                }
            }
        }
    }
}

#[test]
fn active_outlines_survive_focus_and_aliased_state_colours_for_all_presentations() {
    for content in [
        ActionButtonContent::Text,
        ActionButtonContent::IconOnly(IconId::Check),
        ActionButtonContent::IconLabel(IconId::Check),
    ] {
        for emphasis in [
            ActionEmphasis::Quiet,
            ActionEmphasis::QuietBorderless,
            ActionEmphasis::Normal,
            ActionEmphasis::Primary,
        ] {
            for (pressed, toggled, focused, disabled) in [
                (false, false, false, false),
                (true, false, false, false),
                (true, false, true, false),
                (false, true, true, false),
                (true, true, false, true),
            ] {
                let context = egui::Context::default();
                install_typography_fonts(&context);
                let mut tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
                tokens.colours.focus_ring = tokens.colours.selection_indicator;
                let mut centre = egui::Pos2::ZERO;
                let paint = |ui: &mut egui::Ui| {
                    action_button_with_content(
                        ui,
                        spec(
                            if disabled {
                                Availability::Disabled {
                                    reason: "Unavailable".into(),
                                }
                            } else {
                                Availability::Enabled
                            },
                            ActionButtonState::Toggle { pressed: toggled },
                            emphasis,
                        ),
                        content,
                        &tokens,
                        1.0,
                        &mut Vec::new(),
                    )
                };
                let mut initial = context.run_ui(Default::default(), |ui| {
                    centre = paint(ui).rect.center();
                });
                initial.textures_delta.clear();
                if focused {
                    context.memory_mut(|memory| {
                        memory.request_focus(
                            ActionButtonIdentity::for_target(
                                spec(
                                    Availability::Enabled,
                                    ActionButtonState::Momentary,
                                    emphasis,
                                )
                                .target,
                            )
                            .widget_id,
                        )
                    });
                }
                let mut events = vec![egui::Event::PointerMoved(centre)];
                if pressed {
                    events.push(egui::Event::PointerButton {
                        pos: centre,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let mut visual = Rect::NOTHING;
                let mut output = context.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        let response = paint(ui);
                        assert_eq!(response.has_focus(), focused);
                        visual = Rect::from_center_size(
                            response.rect.center(),
                            egui::vec2(
                                if matches!(content, ActionButtonContent::IconOnly(_)) {
                                    tokens.geometry.control_height.0
                                } else {
                                    response.rect.width()
                                },
                                tokens.geometry.control_height.0,
                            ),
                        );
                    },
                );
                output.textures_delta.clear();
                let outlines: Vec<_> = output
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
                let active = visual.shrink(tokens.spacing.unit.0 * 0.5);
                let state_colour: Color32 = if emphasis == ActionEmphasis::Primary {
                    tokens.colours.action_primary_foreground
                } else {
                    tokens.colours.selection_indicator
                }
                .into();
                assert_eq!(
                    outlines
                        .iter()
                        .any(|rect| rect.rect == active && rect.stroke.color == state_colour),
                    !disabled && (pressed || toggled),
                    "{content:?}, {emphasis:?}, {pressed}/{toggled}/{focused}/{disabled}"
                );
                if focused {
                    assert!(outlines.iter().any(|rect| rect.rect == visual
                        && rect.stroke.color == Color32::from(tokens.colours.focus_ring)));
                }
                if emphasis == ActionEmphasis::Primary && !disabled {
                    assert!(outlines.iter().any(|rect| rect.rect == visual
                        && rect.stroke.color
                            == Color32::from(if focused {
                                tokens.colours.focus_ring
                            } else {
                                tokens.colours.action_primary_foreground
                            })));
                }
            }
        }
    }
}
