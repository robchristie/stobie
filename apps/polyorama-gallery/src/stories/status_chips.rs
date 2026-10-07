use eframe::egui;
use polyorama_ui_egui::{
    ContentTextSpec, DesignTokens, DomainReference, IconId, PresentationContext, PresentationScope,
    SemanticUiId, StatusChipSpec, StatusChipWidth, StatusTone, TextComponentId, TextComponentKind,
    TextInteraction, TextLayoutObservation, TextOverflow, TextRole, TextSpec, UiNode, UiRole,
    measure_component_text, paint_measured_text,
};
use serde::Serialize;

use crate::catalogue::StoryId;

#[derive(Clone, Debug, Default, Serialize)]
pub struct StatusChipFixtureState {
    pub selected_task: u64,
    pub activations: u32,
    pub completed: bool,
    pub hovered_task: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum FixtureAction {
    CompleteTask,
    OpenTask,
}
impl polyorama_ui_egui::ActionKey for FixtureAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::CompleteTask => "gallery.status-chip.complete",
            Self::OpenTask => "gallery.task.open",
        }
    }
    fn specification(self) -> polyorama_ui_egui::ActionSpec<Self> {
        polyorama_ui_egui::ActionSpec {
            id: self,
            label: match self {
                Self::CompleteTask => "Complete demo task",
                Self::OpenTask => "Open task",
            },
            description: match self {
                Self::CompleteTask => "Change the consumer-owned task status",
                Self::OpenTask => "Show the selected task detail",
            },
            compact_label: None,
            shortcut: None,
            scope: polyorama_ui_egui::ActionScope::Application,
        }
    }
}

const TASKS: [(u64, &str, &str, StatusTone, IconId); 5] = [
    (
        1,
        "Prepare field notes",
        "Active",
        StatusTone::Neutral,
        IconId::Clock,
    ),
    (
        2,
        "Survey northern site",
        "Scheduled",
        StatusTone::Neutral,
        IconId::Calendar,
    ),
    (
        3,
        "Check collection metadata",
        "Needs review",
        StatusTone::Warning,
        IconId::Warning,
    ),
    (
        4,
        "Archive approved records",
        "Completed",
        StatusTone::Success,
        IconId::Check,
    ),
    (
        5,
        "Upload regional report",
        "Upload failed",
        StatusTone::Error,
        IconId::Error,
    ),
];

fn fully_visible_text(layout: &TextLayoutObservation) -> bool {
    let a = layout.allocated_rect;
    let c = layout.clip_rect;
    a.min_x >= c.min_x && a.min_y >= c.min_y && a.max_x <= c.max_x && a.max_y <= c.max_y
}

fn publish(
    mut p: PresentationContext,
    ui: &mut egui::Ui,
    text: &mut Vec<TextLayoutObservation>,
    nodes: &mut Vec<UiNode>,
) {
    p.retain_visible_text(ui, fully_visible_text);
    let observed = p.finish(ui);
    text.extend(observed.text_layouts);
    nodes.extend(observed.semantic_nodes);
}

pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    text: &mut Vec<TextLayoutObservation>,
    nodes: &mut Vec<UiNode>,
    state: &mut StatusChipFixtureState,
) {
    let root = SemanticUiId::new("gallery.story");
    let scope = PresentationScope::new("gallery.status-chip");
    state.hovered_task = None;
    if story == StoryId::StatusChipTasks {
        let (detail_id, detail_name, initial_label, initial_tone, initial_icon) = TASKS
            .into_iter()
            .find(|(id, ..)| *id == state.selected_task)
            .unwrap_or(TASKS[0]);
        let completed = detail_id == 1 && state.completed;
        let mut p = PresentationContext::new(
            ui,
            *tokens,
            scale,
            scope.child(("detail", detail_id)),
            root.clone(),
        )
        .with_domain_reference(DomainReference::External {
            namespace: "gallery.task".into(),
            id: detail_id.to_string(),
        });
        ui.horizontal_wrapped(|ui| {
            ui.heading(detail_name);
            p.status_chip(
                ui,
                "status",
                StatusChipSpec {
                    icon: Some(if completed {
                        IconId::Check
                    } else {
                        initial_icon
                    }),
                    ..StatusChipSpec::new(
                        if completed {
                            "Completed"
                        } else {
                            initial_label
                        },
                        if completed {
                            StatusTone::Success
                        } else {
                            initial_tone
                        },
                    )
                },
            );
        });
        p.content(
            ui,
            "summary",
            "Capture observations and attach the approved collection record.",
            ContentTextSpec {
                role: TextRole::Body,
                overflow: TextOverflow::Wrap,
                max_lines: 2,
                interaction: TextInteraction::Selectable,
            },
        );
        ui.add_space(tokens.spacing.section.0);
        publish(p, ui, text, nodes);
        let mut chosen = None;
        for (id, name, label, tone, icon) in TASKS {
            let icon = if id == 1 && state.completed {
                IconId::Check
            } else {
                icon
            };
            let label = if id == 1 && state.completed {
                "Completed"
            } else {
                label
            };
            let tone = if id == 1 && state.completed {
                StatusTone::Success
            } else {
                tone
            };
            let height = tokens.geometry.minimum_hit_size.0.max(
                TextRole::Status.style(tokens, scale).line_height + tokens.spacing.block.0 * 4.0,
            );
            let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), height));
            let identity = scope.child(id).instance("row");
            let row = ui.interact(rect, identity.egui_id(), egui::Sense::click());
            let selected = state.selected_task == id;
            let fill = if selected {
                tokens.colours.selection_background
            } else if row.hovered() {
                tokens.colours.surface_hover
            } else {
                tokens.colours.surface_panel
            };
            ui.painter()
                .rect_filled(rect, tokens.geometry.control_radius.0, fill);
            if row.has_focus() {
                ui.painter().rect_stroke(
                    rect,
                    tokens.geometry.control_radius.0,
                    egui::Stroke::new(tokens.spacing.unit.0 * 0.5, tokens.colours.focus_ring),
                    egui::StrokeKind::Inside,
                );
            }
            if row.interact_rect.is_positive() {
                row.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        name,
                    )
                });
                ui.ctx().accesskit_node_builder(row.id, |node| {
                    node.set_role(egui::accesskit::Role::ListBoxOption);
                    node.set_label(name);
                    node.clear_toggled();
                    node.set_selected(selected);
                    node.set_author_id(identity.semantic_id().0);
                    let r = row.interact_rect;
                    node.set_bounds(egui::accesskit::Rect {
                        x0: r.min.x.into(),
                        y0: r.min.y.into(),
                        x1: r.max.x.into(),
                        y1: r.max.y.into(),
                    });
                });
            }
            let domain = DomainReference::External {
                namespace: "gallery.task".into(),
                id: id.to_string(),
            };
            if row.interact_rect.is_positive() {
                let mut node = UiNode::container(
                    identity.semantic_id(),
                    Some(root.clone()),
                    UiRole::ResultRow,
                    row.interact_rect.into(),
                );
                node.name = name.into();
                node.selected = selected;
                node.focused = row.has_focus();
                node.actions = vec![polyorama_ui_egui::SemanticActionId::from_action(
                    FixtureAction::OpenTask,
                )];
                node.domain_reference = Some(domain.clone());
                nodes.push(node);
            }
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(
                        rect.shrink2(egui::vec2(tokens.spacing.inline.0, tokens.spacing.block.0)),
                    )
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            let mut p = PresentationContext::new(
                &mut child,
                *tokens,
                scale,
                scope.child(id),
                identity.semantic_id(),
            )
            .with_domain_reference(domain);
            let name_width = (child.available_width() - 145.0 * scale).max(32.0);
            let measured = measure_component_text(
                child.painter(),
                name,
                TextSpec::single_line(TextRole::Body, TextOverflow::Ellipsis),
                tokens,
                scale,
                name_width,
            );
            let (name_rect, _) = child.allocate_exact_size(
                egui::vec2(measured.size().x, measured.size().y),
                egui::Sense::hover(),
            );
            let observation = paint_measured_text(
                child.painter(),
                &measured,
                name_rect,
                TextComponentId::new(TextComponentKind::ContentLabel, identity.text_instance()),
                None,
            );
            if observation.layout_error.is_some() || fully_visible_text(&observation) {
                text.push(observation);
            }
            p.status_chip(
                &mut child,
                "status",
                StatusChipSpec {
                    interaction: TextInteraction::Inert,
                    icon: Some(icon),
                    ..StatusChipSpec::new(label, tone)
                },
            );
            publish(p, &mut child, text, nodes);
            if row.hovered() {
                state.hovered_task = Some(id);
            }
            if row.clicked() {
                row.request_focus();
                chosen = Some(id);
            }
        }
        if let Some(id) = chosen {
            state.selected_task = id;
            state.activations += 1;
            ui.ctx().request_repaint();
        }
        let mut p = PresentationContext::new(ui, *tokens, scale, scope, root.clone());
        let complete = p.action(
            ui,
            "complete",
            polyorama_ui_egui::ActionButtonSpec {
                target: polyorama_ui_egui::ActionTarget::application(FixtureAction::CompleteTask),
                availability: polyorama_ui_egui::Availability::Enabled,
                state: polyorama_ui_egui::ActionButtonState::Momentary,
                emphasis: polyorama_ui_egui::ActionEmphasis::Quiet,
                compact: false,
            },
        );
        publish(p, ui, text, nodes);
        if complete.clicked() {
            state.completed = !state.completed;
            ui.ctx().request_repaint();
        }
    } else {
        let mut p = PresentationContext::new(ui, *tokens, scale, scope, root);
        if story == StoryId::StatusChipTreatments {
            for (index, (name, background)) in [
                ("Ordinary parent", tokens.colours.surface_panel),
                ("Hovered parent", tokens.colours.surface_hover),
                ("Selected parent", tokens.colours.selection_background),
            ]
            .into_iter()
            .enumerate()
            {
                ui.label(name);
                egui::Frame::new()
                    .fill(background.into())
                    .inner_margin(tokens.spacing.inline.0)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for (id, _, label, tone, icon) in TASKS {
                                p.status_chip(
                                    ui,
                                    (index, id),
                                    StatusChipSpec {
                                        icon: (index != 0).then_some(icon),
                                        ..StatusChipSpec::new(label, tone)
                                    },
                                );
                            }
                            p.status_chip(
                                ui,
                                (index, "paused"),
                                StatusChipSpec::new("Paused", StatusTone::Neutral),
                            );
                        });
                    });
            }
        } else {
            ui.label("Full labels and context remain available on truncation.");
            for (key, label, explanation, width) in [
                (
                    "localised",
                    "Überprüfung durch das regionale Forschungsteam ausstehend",
                    "Awaiting review by the regional research team",
                    150.0,
                ),
                (
                    "long",
                    "Scheduled after collection validation and approval",
                    "The application owns the scheduling rules",
                    100.0,
                ),
                (
                    "minimum",
                    "Needs review",
                    "The parent must provide the minimum label reservation",
                    8.0,
                ),
            ] {
                p.status_chip(
                    ui,
                    key,
                    StatusChipSpec {
                        icon: Some(IconId::Calendar),
                        explanation: Some(explanation),
                        width: StatusChipWidth::AtMost(width),
                        ..StatusChipSpec::new(label, StatusTone::Neutral)
                    },
                );
            }
            ui.add_space(tokens.spacing.section.0);
            ui.label("Scroll to inspect partial and hidden status observations.");
            egui::ScrollArea::vertical()
                .id_salt("status-chip-scroll")
                .max_height(90.0 * scale)
                .show(ui, |ui| {
                    for id in 0..12_u64 {
                        p.status_chip(
                            ui,
                            ("scroll", id),
                            StatusChipSpec {
                                icon: Some(IconId::Clock),
                                ..StatusChipSpec::new("Scheduled", StatusTone::Neutral)
                            },
                        );
                    }
                });
        }
        publish(p, ui, text, nodes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_enlarged_task_rows_publish_only_current_clipped_owners() {
        let context = egui::Context::default();
        context.enable_accesskit();
        polyorama_ui_egui::apply_design_system(
            &context,
            polyorama_ui_egui::UiPreferences::default(),
        );
        let tokens = DesignTokens::resolve(
            polyorama_ui_egui::ThemeVariant::DarkHighContrast,
            polyorama_ui_egui::DensityVariant::Comfortable,
        );
        let mut text = Vec::new();
        let mut nodes = Vec::new();
        let mut coverage = None;
        let clip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 240.0));
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(clip),
                ..Default::default()
            },
            |ui| {
                ui.set_clip_rect(clip);
                story(
                    ui,
                    StoryId::StatusChipTasks,
                    &tokens,
                    1.5,
                    &mut text,
                    &mut nodes,
                    &mut StatusChipFixtureState::default(),
                );
                coverage = Some(polyorama_ui_egui::text_audit_coverage(ui.ctx(), &text));
            },
        );
        output.textures_delta.clear();
        assert!(
            polyorama_ui_egui::audit_text_layouts(&text).is_empty(),
            "{:?}",
            polyorama_ui_egui::audit_text_layouts(&text)
        );
        assert!(nodes.iter().any(|n| n.role == UiRole::ResultRow));
        assert!(nodes.iter().all(|n| n.rect.max_y <= clip.bottom()));
        let coverage = coverage.unwrap();
        assert!(coverage.attempted_components > coverage.measured_components);
        let update = output.platform_output.accesskit_update.take().unwrap();
        let snapshot = polyorama_ui_egui::UiSnapshot {
            nodes,
            ..Default::default()
        };
        let findings = polyorama_ui_egui::audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
        for (_, native) in &update.nodes {
            let owned = native.role() == egui::accesskit::Role::ListBoxOption
                || (native.role() == egui::accesskit::Role::Label
                    && TASKS.iter().any(|task| native.value() == Some(task.2)))
                || native.label() == Some("Complete demo task");
            if owned && let Some(author) = native.author_id() {
                assert!(
                    snapshot.nodes.iter().any(|n| n.id.0 == author),
                    "hidden owner {author}"
                );
            }
        }
    }
}
