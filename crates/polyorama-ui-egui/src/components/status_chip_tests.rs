use crate::{
    AccessKitMismatch, ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionTarget,
    ApplicationTheme, Availability, DensityVariant, DesignTokens, DomainReference,
    HorizontalTextAlignment, IconId, PresentationContext, PresentationScope, Rgba8, SemanticUiId,
    StatusChipSpec, StatusChipWidth, StatusTone, TextAuditFinding, TextComponentKind,
    TextInteraction, TextOverflow, TextRole, TextSpec, ThemeVariant, TypographyProfile, UiRole,
    UiSnapshot, VerticalTextAlignment, audit_accesskit, audit_text_layouts, icon_size,
    measure_component_text, status_chip, status_chip_colours, status_chip_semantic_node,
    test_actions::TestAction,
};
use egui::{Color32, Rect, Sense};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{cell::Cell, rc::Rc};

const LONG_LABEL: &str = "Waiting for a deliberately long independent review of this task";
const VARIANTS: [ThemeVariant; 4] = [
    ThemeVariant::Light,
    ThemeVariant::Dark,
    ThemeVariant::LightHighContrast,
    ThemeVariant::DarkHighContrast,
];

fn context() -> egui::Context {
    let context = egui::Context::default();
    crate::install_typography_fonts(&context);
    context.enable_accesskit();
    context
}

fn tokens() -> DesignTokens {
    DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
}

fn prepare_harness_fonts(ui: &egui::Ui) -> bool {
    crate::install_typography_fonts(ui.ctx());
    let ready = ui.fonts(|fonts| {
        fonts
            .definitions()
            .families
            .contains_key(&TextRole::Status.style(&tokens(), 1.0).font_id.family)
    });
    if !ready {
        ui.ctx()
            .request_discard("install status-chip probe fonts before presenting text");
    }
    ready
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.1, "{actual} != {expected}");
}

#[test]
fn measured_content_size_and_single_line_elision_across_appearances() {
    for variant in VARIANTS {
        for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
            for scale in [1.0, 1.5] {
                let context = context();
                let tokens = DesignTokens::resolve(variant, density);
                let mut layouts = Vec::new();
                let mut output = context.run_ui(Default::default(), |ui| {
                    ui.set_width(420.0);
                    let scope = PresentationScope::new("measured-chip");
                    let narrow = status_chip(
                        ui,
                        StatusChipSpec::new("iii", StatusTone::Neutral),
                        scope.instance("narrow-glyphs"),
                        &tokens,
                        scale,
                        &mut layouts,
                    );
                    let wide = status_chip(
                        ui,
                        StatusChipSpec::new("WWW", StatusTone::Neutral),
                        scope.instance("wide-glyphs"),
                        &tokens,
                        scale,
                        &mut layouts,
                    );
                    assert!(wide.visual_rect.width() > narrow.visual_rect.width());
                    for (chip, label) in [(&narrow, "iii"), (&wide, "WWW")] {
                        let measured = measure_component_text(
                            ui.painter(),
                            label,
                            TextSpec::single_line(TextRole::Status, TextOverflow::Expand),
                            &tokens,
                            scale,
                            420.0,
                        );
                        let ellipsis = measure_component_text(
                            ui.painter(),
                            "…",
                            TextSpec::single_line(TextRole::Status, TextOverflow::Expand),
                            &tokens,
                            scale,
                            420.0,
                        );
                        near(
                            chip.text_rect.width(),
                            measured.size().x.max(ellipsis.size().x),
                        );
                        near(
                            chip.visual_rect.height(),
                            measured.size().y + 2.0 * tokens.spacing.block.0,
                        );
                        assert!(!chip.truncated);
                        assert!(chip.visual_rect.contains_rect(chip.text_rect));
                        assert_ne!(chip.response.rect, chip.visual_rect);
                    }
                    let capped = status_chip(
                        ui,
                        StatusChipSpec::new(LONG_LABEL, StatusTone::Warning),
                        scope.instance("default-cap"),
                        &tokens,
                        scale,
                        &mut layouts,
                    );
                    near(
                        capped.visual_rect.width(),
                        tokens.status_chip_max_width().0 * scale,
                    );
                    assert!(capped.truncated);
                    let bounded = status_chip(
                        ui,
                        StatusChipSpec {
                            width: StatusChipWidth::AtMost(96.0),
                            ..StatusChipSpec::new(LONG_LABEL, StatusTone::Warning)
                        },
                        scope.instance("bounded"),
                        &tokens,
                        scale,
                        &mut layouts,
                    );
                    near(bounded.visual_rect.width(), 96.0);
                    assert!(bounded.truncated);
                });
                output.textures_delta.clear();
                assert_eq!(layouts.len(), 4);
                assert!(
                    audit_text_layouts(&layouts).is_empty(),
                    "{:#?}",
                    audit_text_layouts(&layouts)
                );
                for layout in layouts {
                    assert_eq!(layout.component_id.kind, TextComponentKind::StatusChip);
                    assert_eq!(layout.role, TextRole::Status);
                    assert_eq!(layout.horizontal_alignment, HorizontalTextAlignment::Start);
                    assert_eq!(layout.vertical_alignment, VerticalTextAlignment::Centre);
                    assert_eq!(layout.overflow, TextOverflow::Ellipsis);
                    assert_eq!(layout.interaction, TextInteraction::Selectable);
                    assert_eq!(layout.declared_max_lines, 1);
                    assert_eq!(layout.line_count, 1);
                }
            }
        }
    }
}

#[test]
fn tiny_parent_preserves_useful_label_and_scaled_icon_reservation() {
    for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
        for scale in [1.0, 1.5] {
            let context = context();
            let tokens = DesignTokens::resolve(ThemeVariant::Dark, density);
            let mut layouts = Vec::new();
            let mut output = context.run_ui(Default::default(), |ui| {
                ui.set_width(1.0);
                let scope = PresentationScope::new("tiny-chip");
                let spec = StatusChipSpec {
                    width: StatusChipWidth::AtMost(1.0),
                    ..StatusChipSpec::new(LONG_LABEL, StatusTone::Warning)
                };
                let plain = status_chip(
                    ui,
                    spec,
                    scope.instance("plain"),
                    &tokens,
                    scale,
                    &mut layouts,
                );
                let with_icon = status_chip(
                    ui,
                    StatusChipSpec {
                        icon: Some(IconId::Warning),
                        ..spec
                    },
                    scope.instance("icon"),
                    &tokens,
                    scale,
                    &mut layouts,
                );
                let icon = with_icon.icon_rect.unwrap();
                let ellipsis = measure_component_text(
                    ui.painter(),
                    "…",
                    TextSpec::single_line(TextRole::Status, TextOverflow::Expand),
                    &tokens,
                    scale,
                    100.0,
                );
                assert!(plain.visual_rect.width() > 1.0);
                assert!(with_icon.visual_rect.width() > plain.visual_rect.width());
                assert!(plain.text_rect.width() >= ellipsis.size().x);
                assert!(plain.text_rect.width() >= tokens.geometry.minimum_hit_size.0 * scale);
                near(with_icon.text_rect.width(), plain.text_rect.width());
                near(icon.width(), icon_size(&tokens, scale));
                near(icon.height(), icon.width());
                near(icon.center().y, with_icon.visual_rect.center().y);
                near(
                    with_icon.text_rect.left() - icon.right(),
                    tokens.spacing.inline.0,
                );
                near(
                    with_icon.visual_rect.width() - plain.visual_rect.width(),
                    icon.width() + tokens.spacing.inline.0,
                );
                assert!(with_icon.visual_rect.contains_rect(icon));
                assert!(with_icon.visual_rect.contains_rect(with_icon.text_rect));
                assert!(!icon.intersects(with_icon.text_rect));
                assert!(plain.truncated && with_icon.truncated);
            });
            output.textures_delta.clear();
            assert!(audit_text_layouts(&layouts).is_empty());
        }
    }
}

fn contrast(foreground: Color32, background: Color32) -> f64 {
    let luminance = |colour: Color32| {
        let linear = |channel: u8| {
            let s = f64::from(channel) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(colour.r()) + 0.7152 * linear(colour.g()) + 0.0722 * linear(colour.b())
    };
    let (a, b) = (luminance(foreground), luminance(background));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn strict_theme() -> ApplicationTheme {
    // Distinct application values prove resolver wiring, independently of the
    // analytical reference. The Gallery owns its authored preset regressions.
    let rgba = |value| Rgba8 {
        red: value,
        green: value,
        blue: value,
        alpha: 255,
    };
    let mut colours = ApplicationTheme::analytical().colours();
    for colour in [
        &mut colours.light,
        &mut colours.dark,
        &mut colours.light_high_contrast,
        &mut colours.dark_high_contrast,
    ] {
        colour.surface_canvas = rgba(8);
        colour.surface_panel = rgba(12);
        colour.surface_raised = rgba(20);
        colour.surface_hover = rgba(30);
        colour.selection_background = rgba(40);
        colour.action_quiet_hover = rgba(30);
        colour.text_primary = rgba(255);
        colour.text_muted = rgba(230);
        colour.action_primary_background = rgba(255);
        colour.action_primary_foreground = rgba(12);
        colour.focus_ring = rgba(255);
        colour.selection_indicator = rgba(255);
    }
    ApplicationTheme::new(colours).expect("application fixture passes full contrast validation")
}

#[test]
fn actual_painted_pair_is_opaque_and_contrasts_on_all_parent_surfaces() {
    for theme in [ApplicationTheme::analytical(), strict_theme()] {
        for variant in VARIANTS {
            let tokens = theme.resolve(
                variant,
                DensityVariant::Comfortable,
                TypographyProfile::Dense,
            );
            let minimum = if matches!(
                variant,
                ThemeVariant::LightHighContrast | ThemeVariant::DarkHighContrast
            ) {
                7.0
            } else {
                4.5
            };
            for tone in [
                StatusTone::Neutral,
                StatusTone::Success,
                StatusTone::Warning,
                StatusTone::Error,
            ] {
                for parent in [
                    tokens.colours.surface_panel,
                    tokens.colours.surface_hover,
                    tokens.colours.selection_background,
                ] {
                    let context = context();
                    let colours = status_chip_colours(&tokens, tone);
                    let mut visual = Rect::NOTHING;
                    let mut output = context.run_ui(Default::default(), |ui| {
                        ui.painter()
                            .rect_filled(ui.max_rect(), 0.0, Color32::from(parent));
                        visual = status_chip(
                            ui,
                            StatusChipSpec::new("Needs review", tone),
                            PresentationScope::new("contrast").instance("status"),
                            &tokens,
                            1.0,
                            &mut Vec::new(),
                        )
                        .visual_rect;
                    });
                    output.textures_delta.clear();
                    let background = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Rect(rect) if rect.rect == visual => Some(rect.fill),
                            _ => None,
                        })
                        .expect("production chip paints its own surface");
                    let text = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.text() == "Needs review" => {
                                Some(text)
                            }
                            _ => None,
                        })
                        .expect("production chip paints its measured text");
                    assert_eq!(background, colours.background);
                    assert_eq!(background.a(), 255);
                    assert_eq!(text.fallback_color, colours.foreground);
                    assert_eq!(text.fallback_color.a(), 255);
                    assert!(contrast(text.fallback_color, background) >= minimum);
                    assert_eq!(
                        text.galley.job.sections[0].format.font_id,
                        TextRole::Status.style(&tokens, 1.0).font_id
                    );
                }
            }
        }
    }
}

#[test]
fn scoped_identity_metadata_and_full_value_survive_status_and_layout_updates() {
    let context = context();
    let scope = PresentationScope::new("task").child(17_u64);
    let parent = SemanticUiId::new("task.detail");
    let domain = DomainReference::External {
        namespace: "fixture.task".into(),
        id: "17".into(),
    };
    let mut response_id = None;
    for (index, (label, tone, icon)) in [
        ("Active", StatusTone::Neutral, None),
        (LONG_LABEL, StatusTone::Warning, Some(IconId::Warning)),
        (
            "Révision terminée — prête",
            StatusTone::Success,
            Some(IconId::Check),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut publication = None;
        let mut output = context.run_ui(Default::default(), |ui| {
            ui.set_width(120.0);
            let mut p = PresentationContext::new(ui, tokens(), 1.0, scope, parent.clone())
                .with_domain_reference(domain.clone());
            ui.push_id(index, |ui| {
                let chip = p.status_chip(
                    ui,
                    "status",
                    StatusChipSpec {
                        icon,
                        explanation: Some("Application-owned status explanation"),
                        ..StatusChipSpec::new(label, tone)
                    },
                );
                if let Some(id) = response_id {
                    assert_eq!(chip.response.id, id);
                } else {
                    response_id = Some(chip.response.id);
                }
            });
            publication = Some(p.finish(ui));
        });
        output.textures_delta.clear();
        let publication = publication.unwrap();
        assert_eq!(publication.semantic_nodes.len(), 1);
        assert_eq!(publication.text_layouts.len(), 1);
        assert_eq!(publication.coverage.attempted_components, 1);
        assert_eq!(publication.coverage.measured_components, 1);
        let node = &publication.semantic_nodes[0];
        assert_eq!(node.id, scope.instance("status").semantic_id());
        assert_eq!(node.parent, Some(parent.clone()));
        assert_eq!(node.domain_reference, Some(domain.clone()));
        assert_eq!(node.name, label);
        assert_eq!(node.role, UiRole::StatusChip);
        assert!(node.text_selectable);
        assert!(node.actions.is_empty());
        assert!(!node.selected);
        assert_eq!(
            publication.text_layouts[0].component_id.instance,
            scope.instance("status").text_instance()
        );
        let update = output.platform_output.accesskit_update.take().unwrap();
        let owners: Vec<_> = update
            .nodes
            .iter()
            .filter(|(_, native)| native.author_id() == Some(node.id.0.as_str()))
            .collect();
        assert_eq!(owners.len(), 1);
        let native = &owners[0].1;
        assert_eq!(native.role(), egui::accesskit::Role::Label);
        assert_eq!(native.value(), Some(label));
        assert_eq!(native.label(), None);
        assert_eq!(
            native.description(),
            Some("Application-owned status explanation")
        );
        assert!(!native.supports_action(egui::accesskit::Action::Click));
        assert!(
            audit_accesskit(
                &UiSnapshot {
                    nodes: publication.semantic_nodes,
                    ..Default::default()
                },
                &update
            )
            .is_empty()
        );
    }
}

#[test]
fn parity_detects_missing_owner_name_and_unexpected_or_missing_click_action() {
    let context = context();
    let scope = PresentationScope::new("parity");
    let chip_id = scope.instance("chip").semantic_id();
    let button_id = scope.instance("button").semantic_id();
    let mut snapshot = UiSnapshot::default();
    let mut output = context.run_ui(Default::default(), |ui| {
        let mut p = PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root());
        p.status_chip(
            ui,
            "chip",
            StatusChipSpec {
                icon: Some(IconId::Warning),
                explanation: Some("Full explanation"),
                width: StatusChipWidth::AtMost(80.0),
                ..StatusChipSpec::new(LONG_LABEL, StatusTone::Warning)
            },
        );
        p.action(
            ui,
            "button",
            ActionButtonSpec {
                target: ActionTarget::application(TestAction::Undo),
                availability: Availability::Enabled,
                state: ActionButtonState::Momentary,
                emphasis: ActionEmphasis::Normal,
                compact: false,
            },
        );
        snapshot.nodes = p.finish(ui).semantic_nodes;
    });
    output.textures_delta.clear();
    let update = output.platform_output.accesskit_update.take().unwrap();
    assert!(audit_accesskit(&snapshot, &update).is_empty());
    assert_eq!(
        update
            .nodes
            .iter()
            .filter(|(_, node)| node.value() == Some(LONG_LABEL))
            .count(),
        1
    );

    let mut missing = update.clone();
    missing
        .nodes
        .retain(|(_, node)| node.author_id() != Some(chip_id.0.as_str()));
    assert!(
        audit_accesskit(&snapshot, &missing).contains(&AccessKitMismatch::MissingNode {
            id: chip_id.clone()
        })
    );

    let mut wrong_name = update.clone();
    let native = &mut wrong_name
        .nodes
        .iter_mut()
        .find(|(_, node)| node.author_id() == Some(chip_id.0.as_str()))
        .unwrap()
        .1;
    native.clear_value();
    native.set_label(LONG_LABEL);
    assert!(
        audit_accesskit(&snapshot, &wrong_name).contains(&AccessKitMismatch::Name {
            id: chip_id.clone()
        })
    );

    let mut extra_click = update.clone();
    extra_click
        .nodes
        .iter_mut()
        .find(|(_, node)| node.author_id() == Some(chip_id.0.as_str()))
        .unwrap()
        .1
        .add_action(egui::accesskit::Action::Click);
    assert!(
        audit_accesskit(&snapshot, &extra_click).contains(&AccessKitMismatch::ClickAction {
            id: chip_id.clone()
        })
    );

    for property in 0..6 {
        let mut interactive = update.clone();
        let native = &mut interactive
            .nodes
            .iter_mut()
            .find(|(_, node)| node.author_id() == Some(chip_id.0.as_str()))
            .unwrap()
            .1;
        match property {
            0 => native.add_action(egui::accesskit::Action::Focus),
            1 => native.set_selected(true),
            2 => native.set_toggled(egui::accesskit::Toggled::True),
            3 => native.set_aria_current(egui::accesskit::AriaCurrent::True),
            4 => native.set_live(egui::accesskit::Live::Polite),
            5 => native.set_label(LONG_LABEL),
            _ => unreachable!(),
        }
        assert!(
            audit_accesskit(&snapshot, &interactive).contains(
                &AccessKitMismatch::InformationalState {
                    id: chip_id.clone()
                }
            ),
            "informational property {property} must be rejected"
        );
    }

    let mut missing_click = update.clone();
    missing_click
        .nodes
        .iter_mut()
        .find(|(_, node)| node.author_id() == Some(button_id.0.as_str()))
        .unwrap()
        .1
        .remove_action(egui::accesskit::Action::Click);
    assert!(
        audit_accesskit(&snapshot, &missing_click)
            .contains(&AccessKitMismatch::ClickAction { id: button_id })
    );
}

#[test]
fn drag_and_copy_preserve_full_genuinely_elided_ascii_and_unicode_labels() {
    for label in [
        LONG_LABEL,
        "Révision différée — Prüfung vollständig erforderlich für cette tâche",
    ] {
        let text_rect = Rc::new(Cell::new(Rect::NOTHING));
        let observed_rect = text_rect.clone();
        let mut harness = Harness::builder()
            .with_size(egui::vec2(240.0, 100.0))
            .build_ui(move |ui| {
                if !prepare_harness_fonts(ui) {
                    return;
                }
                let mut layouts = Vec::new();
                let chip = status_chip(
                    ui,
                    StatusChipSpec {
                        width: StatusChipWidth::AtMost(100.0),
                        ..StatusChipSpec::new(label, StatusTone::Neutral)
                    },
                    PresentationScope::new("copy").instance("status"),
                    &tokens(),
                    1.0,
                    &mut layouts,
                );
                assert!(
                    chip.truncated && layouts[0].truncated,
                    "copy probe must exercise real elision"
                );
                assert_eq!(layouts[0].interaction, TextInteraction::Selectable);
                assert!(
                    audit_text_layouts(&layouts).is_empty(),
                    "{:#?}; layouts {layouts:#?}",
                    audit_text_layouts(&layouts)
                );
                observed_rect.set(chip.text_rect);
            });
        harness.run();
        let rect = text_rect.get();
        let start = egui::pos2(rect.left() + 0.5, rect.center().y);
        let end = egui::pos2(rect.right() - 0.5, rect.center().y);
        harness.hover_at(start);
        harness.drag_at(start);
        harness.hover_at(end);
        harness.drop_at(end);
        harness.step();
        let native = harness.get_by_value(label);
        assert_eq!(native.accesskit_node().role(), egui::accesskit::Role::Label);
        assert!(native.accesskit_node().text_selection().is_some());
        assert!(
            !native
                .accesskit_node()
                .data()
                .supports_action(egui::accesskit::Action::Click)
        );
        harness.event(egui::Event::Copy);
        harness.step();
        assert!(
            harness.output().platform_output.commands.iter().any(
                |command| matches!(command, egui::OutputCommand::CopyText(text) if text == label)
            ),
            "full original value must remain copyable after elision: {:?}",
            harness.output().platform_output.commands
        );
    }
}

#[test]
fn inert_chip_preserves_owning_rows_pointer_click_and_drag() {
    const LABEL: &str = "Needs review";
    let clicks = Rc::new(Cell::new(0));
    let drags = Rc::new(Cell::new(0));
    let text_rect = Rc::new(Cell::new(Rect::NOTHING));
    let (observed_clicks, observed_drags, observed_rect) =
        (clicks.clone(), drags.clone(), text_rect.clone());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(300.0, 120.0))
        .build_ui(move |ui| {
            if !prepare_harness_fonts(ui) {
                return;
            }
            let (row, response) =
                ui.allocate_exact_size(egui::vec2(250.0, 50.0), Sense::click_and_drag());
            if response.clicked() {
                observed_clicks.set(observed_clicks.get() + 1);
            }
            if response.drag_started() {
                observed_drags.set(observed_drags.get() + 1);
            }
            let mut content = ui.new_child(egui::UiBuilder::new().max_rect(row.shrink(5.0)));
            let mut layouts = Vec::new();
            let spec = StatusChipSpec {
                interaction: TextInteraction::Inert,
                ..StatusChipSpec::new(LABEL, StatusTone::Warning)
            };
            let identity = PresentationScope::new("row").instance("status");
            let chip = status_chip(&mut content, spec, identity, &tokens(), 1.0, &mut layouts);
            observed_rect.set(chip.text_rect);
            assert!(!chip.response.sense.senses_click() && !chip.response.sense.senses_drag());
            assert_eq!(layouts[0].interaction, TextInteraction::Inert);
            let node = status_chip_semantic_node(&chip, spec, identity, SemanticUiId::root(), None)
                .unwrap();
            assert!(!node.text_selectable && node.actions.is_empty());
        });
    harness.run();
    harness.get_by_value(LABEL).click();
    harness.run();
    assert_eq!(clicks.get(), 1);
    let start = text_rect.get().center();
    let end = start + egui::vec2(30.0, 0.0);
    harness.hover_at(start);
    harness.drag_at(start);
    harness.step();
    harness.hover_at(end);
    harness.step();
    harness.drop_at(end);
    harness.run();
    assert_eq!(drags.get(), 1);
    assert_eq!(clicks.get(), 1);
    assert!(
        harness
            .get_by_value(LABEL)
            .accesskit_node()
            .text_selection()
            .is_none()
    );
}

#[test]
fn partial_and_full_clips_keep_current_bounds_and_honest_attempt_coverage() {
    for interaction in [TextInteraction::Selectable, TextInteraction::Inert] {
        let context = context();
        let scope = PresentationScope::new("clipped-chip");
        let mut publication = None;
        let mut output = context.run_ui(Default::default(), |ui| {
            ui.set_width(220.0);
            let start = ui.cursor().min;
            let clip = Rect::from_min_size(
                start + egui::vec2(0.0, tokens().spacing.block.0 + 3.0),
                egui::vec2(220.0, 4.0),
            );
            ui.set_clip_rect(clip);
            let mut p = PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root());
            let partial = p.status_chip(
                ui,
                "partial",
                StatusChipSpec {
                    interaction,
                    ..StatusChipSpec::new("Partly visible", StatusTone::Neutral)
                },
            );
            assert_eq!(
                partial.response.interact_rect,
                partial.text_rect.intersect(clip)
            );
            near(partial.response.interact_rect.height(), 4.0);
            assert!(partial.response.interact_rect.height() < partial.text_rect.height());
            let hidden = p.status_chip(
                ui,
                "hidden",
                StatusChipSpec {
                    interaction,
                    ..StatusChipSpec::new("Below the current clip", StatusTone::Neutral)
                },
            );
            assert!(!hidden.response.interact_rect.is_positive());
            publication = Some(p.finish(ui));
        });
        output.textures_delta.clear();
        let publication = publication.unwrap();
        assert_eq!(publication.semantic_nodes.len(), 1);
        assert_eq!(publication.text_layouts.len(), 1);
        assert_eq!(publication.coverage.attempted_components, 2);
        assert_eq!(publication.coverage.successful_components, 2);
        assert_eq!(publication.coverage.failed_components, 0);
        assert_eq!(publication.coverage.measured_components, 1);
        assert_eq!(
            audit_text_layouts(&publication.text_layouts),
            vec![TextAuditFinding::TextOutsideClip {
                component_id: publication.text_layouts[0].component_id,
            }]
        );
        let node = &publication.semantic_nodes[0];
        near(node.rect.max_y - node.rect.min_y, 4.0);
        assert_eq!(node.id, scope.instance("partial").semantic_id());
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(
            !update
                .nodes
                .iter()
                .any(|(_, node)| node.value() == Some("Below the current clip"))
        );
        assert!(
            audit_accesskit(
                &UiSnapshot {
                    nodes: publication.semantic_nodes,
                    ..Default::default()
                },
                &update
            )
            .is_empty()
        );
    }
}

#[test]
fn repeated_layout_passes_replace_chip_publication_and_reset_coverage() {
    let context = context();
    let scope = PresentationScope::new("chip-retry");
    let mut publication = None;
    let mut passes = 0;
    let mut output = context.run_ui(Default::default(), |ui| {
        passes += 1;
        let mut p = PresentationContext::new(ui, tokens(), 1.0, scope, SemanticUiId::root());
        if passes == 1 {
            p.status_chip(
                ui,
                "provisional",
                StatusChipSpec::new("Provisional status", StatusTone::Neutral),
            );
            p.status_chip(
                ui,
                "second",
                StatusChipSpec::new("Other first-pass status", StatusTone::Neutral),
            );
            ui.ctx()
                .request_discard("exercise current status-chip publication");
        } else {
            p.scoped(ui, 17_u64, |ui, row| {
                row.status_chip(
                    ui,
                    "status",
                    StatusChipSpec::new("Current status", StatusTone::Success),
                );
            });
        }
        publication = Some(p.finish(ui));
    });
    output.textures_delta.clear();
    assert_eq!(passes, 2);
    let publication = publication.unwrap();
    assert_eq!(publication.pass, 1);
    assert_eq!(publication.viewport, egui::ViewportId::ROOT);
    assert_eq!(publication.coverage.attempted_components, 1);
    assert_eq!(publication.coverage.measured_components, 1);
    assert_eq!(publication.semantic_nodes.len(), 1);
    assert_eq!(publication.text_layouts.len(), 1);
    assert_eq!(publication.semantic_nodes[0].name, "Current status");
    assert_eq!(
        publication.semantic_nodes[0].id,
        scope.child(17_u64).instance("status").semantic_id()
    );
}
