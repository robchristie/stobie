use std::collections::BTreeSet;

use eframe::egui;
use polyorama_core::{
    CommandHistory, DockNode, DockNodeId, Document, PaneId, Session, SplitAxis, Workspace,
};
use polyorama_ui_egui::{
    DesignTokens, DockBehaviour, DockTextContext, DomainReference, PanePresenter, SemanticUiId,
    SplitterVisualState, StatusTone, TextLayoutObservation, UiNode, UiRole, dock_workspace,
    paint_splitter, status_badge,
};

use crate::catalogue::StoryId;

pub(crate) struct DockSceneState {
    workspace: Workspace,
    behaviour: DockBehaviour,
    history: CommandHistory,
    document: Document,
    session: Session,
}

impl DockSceneState {
    pub(crate) fn new(story: StoryId) -> Self {
        let root = if story == StoryId::ReferenceApplicationShell {
            DockNode::Split {
                id: DockNodeId(700),
                axis: SplitAxis::Horizontal,
                fraction: 0.62,
                first: Box::new(DockNode::Tabs {
                    id: DockNodeId(701),
                    tabs: vec![PaneId(1), PaneId(2), PaneId(3), PaneId(4)],
                    active: 0,
                }),
                second: Box::new(DockNode::Tabs {
                    id: DockNodeId(702),
                    tabs: vec![PaneId(5), PaneId(6)],
                    active: 0,
                }),
            }
        } else if story == StoryId::TabsNarrow {
            DockNode::Tabs {
                id: DockNodeId(703),
                tabs: (1..=6).map(PaneId).collect(),
                active: 3,
            }
        } else {
            DockNode::Tabs {
                id: DockNodeId(703),
                tabs: (1..=10).map(PaneId).collect(),
                active: 3,
            }
        };
        let active_pane = if matches!(root, DockNode::Split { .. }) {
            PaneId(1)
        } else {
            PaneId(4)
        };
        Self {
            workspace: Workspace {
                schema_version: polyorama_core::LAYOUT_SCHEMA_VERSION,
                root,
                active_pane,
                closed_optional_panes: BTreeSet::new(),
                next_node_id: 704,
            },
            behaviour: DockBehaviour::default(),
            history: CommandHistory::default(),
            document: Document::default(),
            session: Session::default(),
        }
    }

    fn show(
        &mut self,
        ui: &mut egui::Ui,
        tokens: &DesignTokens,
        font_scale: f32,
        observations: &mut Vec<TextLayoutObservation>,
        semantic_nodes: &mut Vec<UiNode>,
    ) {
        let mut presenter = GalleryDockPresenter {
            observations,
            semantic_nodes,
        };
        if let Some(command) = dock_workspace(
            ui,
            &mut self.workspace,
            &mut self.behaviour,
            &mut presenter,
            DockTextContext {
                tokens: *tokens,
                font_scale,
            },
        ) {
            self.history.execute(
                command,
                &mut self.document,
                &mut self.session,
                &mut self.workspace,
            );
        }
    }
}

pub(super) fn dock_story(
    ui: &mut egui::Ui,
    narrow: bool,
    dock: &mut DockSceneState,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
) {
    if narrow {
        ui.allocate_ui_with_layout(
            egui::vec2(296.0_f32.min(ui.available_width()), ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| dock.show(ui, tokens, font_scale, observations, semantic_nodes),
        );
    } else {
        dock.show(ui, tokens, font_scale, observations, semantic_nodes);
    }
}

fn splitter_story_states() -> [(&'static str, SplitterVisualState); 4] {
    [
        (
            "Hover",
            SplitterVisualState {
                hovered: true,
                ..Default::default()
            },
        ),
        (
            "Pressed",
            SplitterVisualState {
                hovered: true,
                active: true,
                focused: false,
            },
        ),
        (
            "Keyboard focus",
            SplitterVisualState {
                focused: true,
                ..Default::default()
            },
        ),
        (
            "Active drag",
            SplitterVisualState {
                hovered: true,
                active: true,
                focused: true,
            },
        ),
    ]
}

pub(super) fn splitter_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    let spacing = ui.spacing().item_spacing.x;
    let sample_width = ((ui.available_width() - spacing * 3.0) / 4.0).max(48.0);
    ui.horizontal(|ui| {
        for (label, state) in splitter_story_states() {
            ui.allocate_ui_with_layout(
                egui::vec2(sample_width, 156.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.label(label);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(tokens.geometry.minimum_hit_size.0, 128.0),
                        egui::Sense::hover(),
                    );
                    let visual = egui::Rect::from_center_size(
                        rect.center(),
                        egui::vec2(polyorama_ui_egui::SPLITTER_VISUAL_WIDTH, rect.height()),
                    );
                    paint_splitter(ui.painter(), visual, state, tokens);
                },
            );
        }
    });
    status_badge(
        ui,
        46,
        "The live dock derives these same four treatments from pointer, drag and focus state.",
        StatusTone::Neutral,
        tokens,
        font_scale,
        observations,
    );
}

struct GalleryDockPresenter<'a> {
    observations: &'a mut Vec<TextLayoutObservation>,
    semantic_nodes: &'a mut Vec<UiNode>,
}

impl PanePresenter for GalleryDockPresenter<'_> {
    fn title(&self, pane: PaneId) -> &'static str {
        match pane.0 {
            1 => "Primary image — Antarctic reconstruction sector 04",
            2 => "Secondary linked scalar image with expanded label",
            3 => "Overview",
            4 => "Results: one million deterministic rows",
            5 => "Inspector",
            6 => "Diagnostics and worker request provenance",
            7 => "Thumbnails",
            8 => "Annotations",
            9 => {
                "ExtremelyLongUnbrokenScientificPaneIdentifier_0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ"
            }
            _ => "Loading and error states",
        }
    }
    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: PaneId, pane_rect: egui::Rect) {
        ui.painter()
            .rect_filled(pane_rect, 0.0, ui.visuals().extreme_bg_color);
        ui.put(
            pane_rect.shrink(16.0),
            egui::Label::new(format!(
                "Deterministic reference content for pane {}",
                pane.0
            ))
            .wrap(),
        );
        let mut node = UiNode::container(
            SemanticUiId::pane(pane),
            Some(SemanticUiId::new("gallery.story")),
            UiRole::Pane,
            pane_rect.into(),
        );
        node.name = self.title(pane).to_owned();
        node.pane = Some(pane);
        node.domain_reference = Some(DomainReference::Pane(pane));
        self.semantic_nodes.push(node);
    }
    fn record_text_layout(&mut self, observation: TextLayoutObservation) {
        self.observations.push(observation);
    }
    fn record_tab_rect(&mut self, pane: PaneId, rect: egui::Rect, selected: bool, focused: bool) {
        self.semantic_nodes.push(UiNode {
            id: SemanticUiId::tab(pane),
            parent: Some(SemanticUiId::new("gallery.story")),
            role: UiRole::Tab,
            name: self.title(pane).to_owned(),
            description: None,
            rect: rect.into(),
            enabled: true,
            focused,
            selected,
            checked: None,
            expanded: None,
            pane: Some(pane),
            domain_reference: Some(DomainReference::Pane(pane)),
            actions: Vec::new(),
            text_selectable: false,
            disabled_reason: None,
        });
    }
    fn record_splitter_rect(
        &mut self,
        node: DockNodeId,
        rect: egui::Rect,
        horizontal: bool,
        focused: bool,
    ) {
        self.semantic_nodes.push(UiNode {
            id: SemanticUiId::splitter(node),
            parent: Some(SemanticUiId::new("gallery.story")),
            role: UiRole::Splitter,
            name: if horizontal {
                "Vertical splitter".into()
            } else {
                "Horizontal splitter".into()
            },
            description: Some("Resize adjacent dock panes".into()),
            rect: rect.into(),
            enabled: true,
            focused,
            selected: false,
            checked: None,
            expanded: None,
            pane: None,
            domain_reference: Some(DomainReference::DockNode(node)),
            actions: Vec::new(),
            text_selectable: false,
            disabled_reason: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GalleryConfiguration, GalleryWidth};
    use polyorama_ui_egui::apply_design_system;

    #[test]
    fn splitter_story_deterministically_renders_each_declared_interaction_treatment() {
        assert_eq!(
            splitter_story_states(),
            [
                (
                    "Hover",
                    SplitterVisualState {
                        hovered: true,
                        active: false,
                        focused: false
                    }
                ),
                (
                    "Pressed",
                    SplitterVisualState {
                        hovered: true,
                        active: true,
                        focused: false
                    }
                ),
                (
                    "Keyboard focus",
                    SplitterVisualState {
                        hovered: false,
                        active: false,
                        focused: true
                    }
                ),
                (
                    "Active drag",
                    SplitterVisualState {
                        hovered: true,
                        active: true,
                        focused: true
                    }
                )
            ]
        );
    }

    #[test]
    fn narrow_tabs_keep_narrow_geometry_under_a_wide_gallery_configuration() {
        let context = egui::Context::default();
        let configuration = GalleryConfiguration {
            width: GalleryWidth::Wide,
            ..GalleryConfiguration::default()
        };
        apply_design_system(&context, configuration.preferences());
        let tokens = configuration.preferences().tokens(true);
        let mut dock = DockSceneState::new(StoryId::TabsNarrow);
        let mut observations = Vec::new();
        let mut semantic_nodes = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 300.0),
                )),
                ..Default::default()
            },
            |ui| {
                dock_story(
                    ui,
                    true,
                    &mut dock,
                    &tokens,
                    1.0,
                    &mut observations,
                    &mut semantic_nodes,
                )
            },
        );
        output.textures_delta.clear();
        assert!(!observations.is_empty());
        let min_x = observations
            .iter()
            .map(|observation| observation.allocated_rect.min_x)
            .fold(f32::INFINITY, f32::min);
        let max_x = observations
            .iter()
            .map(|observation| observation.allocated_rect.max_x)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_x - min_x <= 296.0);
    }
}
