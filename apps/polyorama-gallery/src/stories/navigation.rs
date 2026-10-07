use eframe::egui;
use polyorama_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionScope, ActionSpec,
    ActionTarget, Availability, ContentTextSpec, DesignTokens, IconId, NavigationBadge,
    NavigationItemSpec, PresentationContext, PresentationScope, SemanticUiId, TextInteraction,
    TextLayoutObservation, TextOverflow, TextRole, UiNode,
};
use serde::Serialize;

use crate::catalogue::StoryId;

/// The Gallery consumer's destinations; the component has no destination registry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Destination {
    Home,
    Tasks,
    NeedsAttention,
    Activity,
    Settings,
}

impl Destination {
    const ALL: [Self; 5] = [
        Self::Home,
        Self::Tasks,
        Self::NeedsAttention,
        Self::Activity,
        Self::Settings,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Tasks => "Tasks",
            Self::NeedsAttention => "Needs attention",
            Self::Activity => "Activity",
            Self::Settings => "Settings",
        }
    }
    fn icon(self) -> IconId {
        match self {
            Self::Home => IconId::Home,
            Self::Tasks => IconId::Tasks,
            Self::NeedsAttention => IconId::Notifications,
            Self::Activity => IconId::Clock,
            Self::Settings => IconId::Settings,
        }
    }
}
impl ActionKey for Destination {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Home => "gallery.navigation.home",
            Self::Tasks => "gallery.navigation.tasks",
            Self::NeedsAttention => "gallery.navigation.needs_attention",
            Self::Activity => "gallery.navigation.activity",
            Self::Settings => "gallery.navigation.settings",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: self.label(),
            description: "Open this destination",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum FixtureAction {
    CompleteTask,
}

impl ActionKey for FixtureAction {
    fn stable_id(self) -> &'static str {
        "gallery.navigation.complete_task"
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: "Complete a demo task",
            description: "Reduce the fixture's outstanding task count",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NavigationTarget {
    pub destination: Destination,
    pub id: SemanticUiId,
}

/// Consumer-owned selection and counts. Updated only when the response activates.
#[derive(Clone, Debug, Serialize)]
pub struct NavigationFixtureState {
    pub selected: Destination,
    pub task_count: u64,
    pub attention_count: u64,
    pub activations: u32,
    pub hovered: Option<Destination>,
    pub pointer_down: Option<Destination>,
    pub targets: Vec<NavigationTarget>,
    pub count_target: Option<SemanticUiId>,
}
impl Default for NavigationFixtureState {
    fn default() -> Self {
        Self {
            selected: Destination::Home,
            task_count: 12,
            attention_count: 3,
            activations: 0,
            hovered: None,
            pointer_down: None,
            targets: Vec::new(),
            count_target: None,
        }
    }
}

pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    nodes: &mut Vec<UiNode>,
    state: &mut NavigationFixtureState,
) {
    state.hovered = None;
    state.pointer_down = None;
    state.targets.clear();
    state.count_target = None;
    let scope = PresentationScope::new("gallery.navigation");
    let mut presentation = PresentationContext::new(
        ui,
        *tokens,
        scale,
        scope,
        SemanticUiId::new("gallery.story"),
    );
    let rendered_selection = state.selected;
    let mut navigation_intent = None;
    let mut complete_task = false;
    let adversarial = story == StoryId::NavigationLongNarrow;
    let unavailable = story != StoryId::NavigationSidebar;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(if adversarial {
                190.0 * scale
            } else {
                240.0 * scale
            });
            egui::ScrollArea::vertical()
                .id_salt("navigation-fixture-scroll")
                .max_height(if adversarial {
                    125.0 * scale
                } else {
                    260.0 * scale
                })
                .show(ui, |ui| {
                    for destination in Destination::ALL {
                        let badge = match destination {
                            Destination::Tasks => Some(NavigationBadge::Count {
                                value: if adversarial { 12500 } else { state.task_count },
                                meaning: "outstanding tasks",
                            }),
                            Destination::NeedsAttention => Some(NavigationBadge::Count {
                                value: state.attention_count,
                                meaning: "items needing attention",
                            }),
                            Destination::Activity if unavailable => Some(NavigationBadge::Count {
                                value: 0,
                                meaning: "new events",
                            }),
                            Destination::Settings if adversarial => Some(NavigationBadge::Label {
                                text: "Administrator permissions required",
                                description: "Administrator permissions required",
                            }),
                            _ => None,
                        };
                        let response = presentation.navigation_item(
                            ui,
                            destination,
                            NavigationItemSpec {
                                target: ActionTarget::application(destination),
                                icon: destination.icon(),
                                label: if adversarial && destination == Destination::Tasks {
                                    "Tasks awaiting review by the entire regional operations team"
                                } else {
                                    destination.label()
                                },
                                description: destination.specification().description,
                                selected: destination == rendered_selection,
                                availability: if unavailable && destination == Destination::Settings
                                {
                                    Availability::Disabled {
                                        reason: "Administrator access required".into(),
                                    }
                                } else {
                                    Availability::Enabled
                                },
                                badge,
                            },
                        );
                        state.targets.push(NavigationTarget {
                            destination,
                            id: scope.instance(destination).semantic_id(),
                        });
                        if response.hovered() {
                            state.hovered = Some(destination);
                        }
                        if response.is_pointer_button_down_on() {
                            state.pointer_down = Some(destination);
                        }
                        if response.clicked() {
                            navigation_intent = Some(destination);
                        }
                    }
                });
        });
        if !adversarial && ui.available_width() > 160.0 {
            ui.vertical(|ui| {
                presentation.heading(ui, "destination-heading", rendered_selection.label());
                presentation.content(
                    ui,
                    "destination-content",
                    "Your application supplies this destination's content.",
                    ContentTextSpec {
                        role: TextRole::Body,
                        overflow: TextOverflow::Wrap,
                        max_lines: 3,
                        interaction: TextInteraction::Inert,
                    },
                );
                let response = presentation.action_with_icon(
                    ui,
                    "complete-demo-task",
                    ActionButtonSpec {
                        target: ActionTarget::application(FixtureAction::CompleteTask),
                        availability: if state.task_count == 0 {
                            Availability::Disabled {
                                reason: "No outstanding demo tasks".into(),
                            }
                        } else {
                            Availability::Enabled
                        },
                        state: ActionButtonState::Momentary,
                        emphasis: ActionEmphasis::QuietBorderless,
                        compact: false,
                    },
                    IconId::Check,
                );
                state.count_target = Some(scope.instance("complete-demo-task").semantic_id());
                complete_task = response.clicked();
            });
        }
    });
    if adversarial {
        presentation.heading(ui, "destination-heading", rendered_selection.label());
    }
    presentation.content(
        ui,
        "feedback",
        &format!(
            "Destination: {}. {} outstanding tasks; {} items need attention.",
            rendered_selection.label(),
            state.task_count,
            state.attention_count
        ),
        ContentTextSpec {
            role: TextRole::Secondary,
            overflow: TextOverflow::Wrap,
            max_lines: 3,
            interaction: TextInteraction::Inert,
        },
    );
    // Audit the fully visible text subset. The pass inventory still counts all
    // submitted labels/badges, and failed observations are always retained.
    presentation.retain_visible_text(ui, |text| {
        text.clip_rect.min_x <= text.allocated_rect.min_x + 1.0
            && text.clip_rect.min_y <= text.allocated_rect.min_y + 1.0
            && text.clip_rect.max_x >= text.allocated_rect.max_x - 1.0
            && text.clip_rect.max_y >= text.allocated_rect.max_y - 1.0
    });
    let publication = presentation.finish(ui);
    observations.extend(publication.text_layouts);
    nodes.extend(publication.semantic_nodes);
    if let Some(destination) = navigation_intent {
        // Application intent handling after coherent presentation of this pass.
        state.selected = destination;
        state.activations += 1;
        ui.ctx().request_repaint();
    }
    if complete_task {
        state.task_count = state.task_count.saturating_sub(1);
        ui.ctx().request_repaint();
    }
}
