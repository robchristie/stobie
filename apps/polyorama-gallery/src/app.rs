use polyorama_ui_egui::{NativeTextControlKind, record_native_text_control};
use std::{collections::BTreeSet, str::FromStr};

use eframe::egui;
use polyorama_core::{
    CommandHistory, DockNode, DockNodeId, Document, PaneId, Session, SplitAxis, Workspace,
};
use polyorama_ui_egui::{
    ActionButtonSpec, ActionEmphasis, ActionKey, ActionScope, ActionShortcut, ActionSpec,
    ActionTarget, AppearancePreference, Availability, ContrastPreference, DensityPreference,
    DesignTokens, DockBehaviour, DockTextContext, DomainReference, PanePresenter, SemanticUiId,
    ShortcutKey, SplitterVisualState, StatusTone, TextAuditFinding, TextLayoutObservation,
    ThumbnailCellSpec, ThumbnailState, UiNode, UiPreferences, UiRole, UiSnapshot, action_button,
    action_semantic_node, application_bar_frame, application_bar_height, apply_design_system,
    audit_text_layouts, choice_control, dock_workspace, paint_splitter, property_row,
    range_control, result_row, status_badge, thumbnail_cell,
};
use serde::{Deserialize, Serialize};

use crate::catalogue::{STORIES, StoryId, story_definition};

/// Gallery-owned fixture actions used to demonstrate the generic controls.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryAction {
    Undo,
    SaveLayout,
    ResetWorkspace,
    FitView,
    LinkViews,
    NavigateTool,
    PolygonTool,
    EditVerticesTool,
    DisplaySettings,
}

impl ActionKey for GalleryAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Undo => "undo",
            Self::SaveLayout => "save_layout",
            Self::ResetWorkspace => "reset_workspace",
            Self::FitView => "fit_view",
            Self::LinkViews => "link_views",
            Self::NavigateTool => "navigate_tool",
            Self::PolygonTool => "polygon_tool",
            Self::EditVerticesTool => "edit_vertices_tool",
            Self::DisplaySettings => "display_settings",
        }
    }

    fn specification(self) -> ActionSpec<Self> {
        let (label, description, compact_label, shortcut, scope) = match self {
            Self::Undo => (
                "Undo",
                "Undo the most recent change",
                None,
                Some(ActionShortcut::command(ShortcutKey::Z)),
                ActionScope::Application,
            ),
            Self::SaveLayout => (
                "Save layout",
                "Persist the current workspace layout",
                Some("Save"),
                Some(ActionShortcut::command(ShortcutKey::S)),
                ActionScope::Application,
            ),
            Self::ResetWorkspace => (
                "Reset workspace",
                "Restore the default workspace, cameras and tools",
                Some("Reset"),
                None,
                ActionScope::Application,
            ),
            Self::FitView => (
                "Fit view",
                "Fit the complete image into the active viewport",
                Some("Fit"),
                Some(ActionShortcut::new(ShortcutKey::F)),
                ActionScope::Pane,
            ),
            Self::LinkViews => (
                "Link views",
                "Link or unlink this camera with camera group A",
                Some("Link A"),
                Some(ActionShortcut::new(ShortcutKey::L)),
                ActionScope::Pane,
            ),
            Self::NavigateTool => (
                "Navigate",
                "Pan and zoom the active viewport",
                None,
                Some(ActionShortcut::new(ShortcutKey::One)),
                ActionScope::Pane,
            ),
            Self::PolygonTool => (
                "Polygon",
                "Create a polygon annotation",
                None,
                Some(ActionShortcut::new(ShortcutKey::Two)),
                ActionScope::Pane,
            ),
            Self::EditVerticesTool => (
                "Edit vertices",
                "Move vertices of the selected polygon",
                Some("Edit"),
                Some(ActionShortcut::new(ShortcutKey::Three)),
                ActionScope::Pane,
            ),
            Self::DisplaySettings => (
                "Display",
                "Adjust the image colour map and scalar window",
                None,
                None,
                ActionScope::Pane,
            ),
        };
        ActionSpec {
            id: self,
            label,
            description,
            compact_label,
            shortcut,
            scope,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GalleryWidth {
    Narrow,
    #[default]
    Regular,
    Wide,
}

impl GalleryWidth {
    pub const fn points(self) -> f32 {
        match self {
            Self::Narrow => 320.0,
            Self::Regular => 640.0,
            Self::Wide => 960.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GalleryConfiguration {
    pub appearance: AppearancePreference,
    pub contrast: ContrastPreference,
    pub density: DensityPreference,
    pub font_scale: f32,
    pub width: GalleryWidth,
}

impl Default for GalleryConfiguration {
    fn default() -> Self {
        Self {
            appearance: AppearancePreference::Dark,
            contrast: ContrastPreference::Standard,
            density: DensityPreference::Comfortable,
            font_scale: 1.0,
            width: GalleryWidth::Regular,
        }
    }
}

impl GalleryConfiguration {
    pub fn validated(self) -> Self {
        let preferences = self.preferences();
        Self {
            appearance: preferences.appearance,
            contrast: preferences.contrast,
            density: preferences.density,
            font_scale: preferences.font_scale,
            width: self.width,
        }
    }

    pub fn preferences(self) -> UiPreferences {
        UiPreferences {
            appearance: self.appearance,
            contrast: self.contrast,
            density: self.density,
            font_scale: self.font_scale,
            ..UiPreferences::default()
        }
        .validated()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct GalleryRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl From<egui::Rect> for GalleryRect {
    fn from(value: egui::Rect) -> Self {
        Self {
            min_x: value.min.x,
            min_y: value.min.y,
            max_x: value.max.x,
            max_y: value.max.y,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GallerySnapshot {
    pub frame: u64,
    pub story: StoryId,
    pub configuration: GalleryConfiguration,
    pub story_count: usize,
    pub story_rect: GalleryRect,
    pub text: Vec<TextLayoutObservation>,
    pub text_audit: Vec<TextAuditFinding>,
    pub text_audit_coverage: Option<polyorama_ui_egui::TextAuditCoverage>,
    pub ui_snapshot: UiSnapshot,
}

pub struct GalleryApp {
    context: egui::Context,
    selected: StoryId,
    configuration: GalleryConfiguration,
    applied_configuration: Option<GalleryConfiguration>,
    dock: DockSceneState,
    frame: u64,
    snapshot: GallerySnapshot,
    focus_story: Option<StoryId>,
}

impl GalleryApp {
    pub fn new(creation: &eframe::CreationContext<'_>) -> Self {
        let selected = std::env::var("POLYORAMA_GALLERY_STORY")
            .ok()
            .and_then(|value| StoryId::from_str(&value).ok())
            .unwrap_or(StoryId::ReferenceApplicationShell);
        let configuration = GalleryConfiguration::default();
        apply_design_system(&creation.egui_ctx, configuration.preferences());
        Self {
            context: creation.egui_ctx.clone(),
            selected,
            configuration,
            applied_configuration: Some(configuration),
            dock: DockSceneState::new(selected),
            frame: 0,
            snapshot: GallerySnapshot {
                frame: 0,
                story: selected,
                configuration,
                story_count: STORIES.len(),
                story_rect: GalleryRect::default(),
                text: Vec::new(),
                text_audit: Vec::new(),
                text_audit_coverage: None,
                ui_snapshot: UiSnapshot::default(),
            },
            focus_story: None,
        }
    }

    pub fn select_story(&mut self, story: StoryId) {
        if self.selected != story {
            self.selected = story;
            self.dock = DockSceneState::new(story);
            self.focus_story = None;
            self.context.request_repaint();
        }
    }

    pub fn set_configuration(&mut self, configuration: GalleryConfiguration) {
        let configuration = configuration.validated();
        if self.configuration != configuration {
            self.configuration = configuration;
            self.context.request_repaint();
        }
    }

    pub fn snapshot(&self) -> GallerySnapshot {
        self.snapshot.clone()
    }

    fn update_style(&mut self) {
        if self.applied_configuration != Some(self.configuration) {
            apply_design_system(&self.context, self.configuration.preferences());
            self.applied_configuration = Some(self.configuration);
        }
    }
}

impl eframe::App for GalleryApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = root_ui.ctx().clone();
        self.update_style();
        self.frame += 1;
        let root_rect = root_ui.max_rect();
        let tokens = self
            .configuration
            .preferences()
            .tokens(context.theme() == egui::Theme::Dark);
        let mut observations = Vec::new();
        let mut semantic_nodes = vec![UiNode::container(
            SemanticUiId::root(),
            None,
            UiRole::Application,
            root_rect.into(),
        )];
        gallery_bar(root_ui, self, &tokens);
        story_navigation(root_ui, self);
        let story_rect = egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(tokens.colours.surface_canvas.into())
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(root_ui, |ui| {
                let available = ui.available_rect_before_wrap();
                let width = self.configuration.width.points().min(available.width());
                let definition = story_definition(self.selected);
                let height = f32::from(definition.recommended_viewport.height)
                    .min(available.height())
                    .max(120.0);
                let rect = egui::Rect::from_min_size(
                    egui::pos2(available.center().x - width * 0.5, available.min.y),
                    egui::vec2(width, height),
                );
                ui.painter().rect(
                    rect,
                    tokens.geometry.control_radius.0,
                    tokens.colours.surface_panel,
                    egui::Stroke::new(1.0, tokens.colours.border_subtle),
                    egui::StrokeKind::Inside,
                );
                let content_rect = rect.shrink(tokens.spacing.section.0);
                let mut story_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(content_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                story_ui.set_clip_rect(content_rect);
                render_story(
                    &mut story_ui,
                    self.selected,
                    &mut self.dock,
                    &tokens,
                    self.configuration.font_scale,
                    &mut observations,
                    &mut semantic_nodes,
                    &mut self.focus_story,
                );
                rect
            })
            .inner;
        let text_audit = audit_text_layouts(&observations);
        let text_audit_coverage = Some(polyorama_ui_egui::text_audit_coverage(
            &context,
            &observations,
        ));
        let story_id = SemanticUiId::new("gallery.story");
        let mut story_node = UiNode::container(
            story_id,
            Some(SemanticUiId::root()),
            UiRole::Pane,
            story_rect.into(),
        );
        story_node.name = self.selected.as_str().to_owned();
        semantic_nodes.push(story_node);
        let mut ui_snapshot = UiSnapshot {
            frame: self.frame,
            pixels_per_point: context.pixels_per_point(),
            root: SemanticUiId::root(),
            nodes: semantic_nodes,
            text: observations.clone(),
            text_audit: text_audit.clone(),
            text_audit_coverage: text_audit_coverage.clone(),
            semantic_audit: Vec::new(),
        };
        ui_snapshot.semantic_audit = ui_snapshot.audit();
        self.snapshot = GallerySnapshot {
            frame: self.frame,
            story: self.selected,
            configuration: self.configuration,
            story_count: STORIES.len(),
            story_rect: story_rect.into(),
            text: observations,
            text_audit,
            text_audit_coverage,
            ui_snapshot,
        };

        #[cfg(not(target_arch = "wasm32"))]
        if root_ui.input(|input| input.key_pressed(egui::Key::F12))
            && let Ok(path) = std::env::var("POLYORAMA_GALLERY_SNAPSHOT_PATH")
            && let Ok(json) = serde_json::to_vec_pretty(&self.snapshot)
        {
            let _ = std::fs::write(path, json);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

fn gallery_bar(root_ui: &mut egui::Ui, app: &mut GalleryApp, tokens: &DesignTokens) {
    egui::Panel::top("polyorama.gallery.application-bar")
        .exact_size(application_bar_height(tokens, app.configuration.font_scale))
        .frame(application_bar_frame(tokens))
        .show(root_ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.strong("Polyorama component gallery");
                ui.separator();
                let combo = egui::ComboBox::from_id_salt("gallery.appearance")
                    .selected_text(format!("{:?}", app.configuration.appearance))
                    .show_ui(ui, |ui| {
                        let option = ui.selectable_value(
                            &mut app.configuration.appearance,
                            AppearancePreference::Light,
                            "Light",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                        let option = ui.selectable_value(
                            &mut app.configuration.appearance,
                            AppearancePreference::Dark,
                            "Dark",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                    });
                record_native_text_control(&combo.response, NativeTextControlKind::ComboBox);
                let combo = egui::ComboBox::from_id_salt("gallery.contrast")
                    .selected_text(format!("{:?}", app.configuration.contrast))
                    .show_ui(ui, |ui| {
                        let option = ui.selectable_value(
                            &mut app.configuration.contrast,
                            ContrastPreference::Standard,
                            "Standard contrast",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                        let option = ui.selectable_value(
                            &mut app.configuration.contrast,
                            ContrastPreference::High,
                            "High contrast",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                    });
                record_native_text_control(&combo.response, NativeTextControlKind::ComboBox);
                let combo = egui::ComboBox::from_id_salt("gallery.density")
                    .selected_text(format!("{:?}", app.configuration.density))
                    .show_ui(ui, |ui| {
                        let option = ui.selectable_value(
                            &mut app.configuration.density,
                            DensityPreference::Compact,
                            "Compact",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                        let option = ui.selectable_value(
                            &mut app.configuration.density,
                            DensityPreference::Comfortable,
                            "Comfortable",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                    });
                record_native_text_control(&combo.response, NativeTextControlKind::ComboBox);
                let combo = egui::ComboBox::from_id_salt("gallery.scale")
                    .selected_text(format!(
                        "{}%",
                        (app.configuration.font_scale * 100.0) as u16
                    ))
                    .show_ui(ui, |ui| {
                        for scale in [1.0, 1.25, 1.5] {
                            let option = ui.selectable_value(
                                &mut app.configuration.font_scale,
                                scale,
                                format!("{}%", (scale * 100.0) as u16),
                            );
                            record_native_text_control(&option, NativeTextControlKind::Selectable);
                        }
                    });
                record_native_text_control(&combo.response, NativeTextControlKind::ComboBox);
                let combo = egui::ComboBox::from_id_salt("gallery.width")
                    .selected_text(format!("{:?}", app.configuration.width))
                    .show_ui(ui, |ui| {
                        let option = ui.selectable_value(
                            &mut app.configuration.width,
                            GalleryWidth::Narrow,
                            "Narrow",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                        let option = ui.selectable_value(
                            &mut app.configuration.width,
                            GalleryWidth::Regular,
                            "Regular",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                        let option = ui.selectable_value(
                            &mut app.configuration.width,
                            GalleryWidth::Wide,
                            "Wide",
                        );
                        record_native_text_control(&option, NativeTextControlKind::Selectable);
                    });
                record_native_text_control(&combo.response, NativeTextControlKind::ComboBox);
            });
        });
}

fn story_navigation(root_ui: &mut egui::Ui, app: &mut GalleryApp) {
    egui::Panel::left("polyorama.gallery.catalogue")
        .exact_size(244.0)
        .resizable(false)
        .show(root_ui, |ui| {
            ui.add_space(8.0);
            ui.strong("Stories");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut group = None;
                for definition in &STORIES {
                    if group != Some(definition.group) {
                        group = Some(definition.group);
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!("{:?}", definition.group))
                                .small()
                                .strong(),
                        );
                    }
                    let option =
                        ui.selectable_label(app.selected == definition.id, definition.id.as_str());
                    record_native_text_control(&option, NativeTextControlKind::Selectable);
                    if option.clicked() {
                        app.select_story(definition.id);
                    }
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn gallery_action_button(
    ui: &mut egui::Ui,
    target: ActionTarget<GalleryAction>,
    availability: Availability,
    selected: bool,
    emphasis: ActionEmphasis,
    compact: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
) -> egui::Response {
    let response = action_button(
        ui,
        ActionButtonSpec {
            target,
            availability: availability.clone(),
            selected,
            emphasis,
            compact,
        },
        tokens,
        font_scale,
        observations,
    );
    semantic_nodes.push(action_semantic_node(
        &response,
        target,
        &availability,
        selected,
        SemanticUiId::new("gallery.story"),
    ));
    response
}

#[allow(clippy::too_many_arguments)]
fn render_story(
    ui: &mut egui::Ui,
    story: StoryId,
    dock: &mut DockSceneState,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
    focus_story: &mut Option<StoryId>,
) {
    let definition = story_definition(story);
    ui.strong(definition.id.as_str());
    ui.label(definition.description);
    ui.separator();
    match story {
        StoryId::ButtonDefault => {
            ui.horizontal(|ui| {
                gallery_action_button(
                    ui,
                    ActionTarget::application(GalleryAction::SaveLayout),
                    Availability::Enabled,
                    false,
                    ActionEmphasis::Normal,
                    false,
                    tokens,
                    font_scale,
                    observations,
                    semantic_nodes,
                );
                gallery_action_button(
                    ui,
                    ActionTarget::application(GalleryAction::ResetWorkspace),
                    Availability::Enabled,
                    false,
                    ActionEmphasis::Primary,
                    false,
                    tokens,
                    font_scale,
                    observations,
                    semantic_nodes,
                );
                gallery_action_button(
                    ui,
                    ActionTarget::pane(GalleryAction::LinkViews, PaneId(1)),
                    Availability::Enabled,
                    true,
                    ActionEmphasis::Quiet,
                    false,
                    tokens,
                    font_scale,
                    observations,
                    semantic_nodes,
                );
            });
        }
        StoryId::ButtonDisabled => {
            gallery_action_button(
                ui,
                ActionTarget::application(GalleryAction::Undo),
                Availability::Disabled {
                    reason: "History is empty".into(),
                },
                false,
                ActionEmphasis::Normal,
                false,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
        }
        StoryId::ButtonKeyboardFocus => {
            if *focus_story != Some(story) {
                ui.memory_mut(|memory| {
                    memory.request_focus(egui::Id::new((
                        "polyorama.action-button",
                        GalleryAction::FitView.stable_id(),
                        Some(PaneId(1)),
                    )));
                });
                *focus_story = Some(story);
            }
            let response = gallery_action_button(
                ui,
                ActionTarget::pane(GalleryAction::FitView, PaneId(1)),
                Availability::Enabled,
                false,
                ActionEmphasis::Normal,
                false,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
            debug_assert!(response.has_focus());
        }
        StoryId::TabsManyLongLabels | StoryId::ReferenceApplicationShell => {
            dock.show(ui, tokens, font_scale, observations, semantic_nodes);
        }
        StoryId::TabsNarrow => {
            ui.allocate_ui_with_layout(
                egui::vec2(296.0_f32.min(ui.available_width()), ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| dock.show(ui, tokens, font_scale, observations, semantic_nodes),
            );
        }
        StoryId::SplitterHoverActive => {
            splitter_story(ui, tokens, font_scale, observations);
        }
        StoryId::ToolbarNarrow | StoryId::ReferenceImageToolbarNarrow => {
            toolbar_story(ui, true, tokens, font_scale, observations, semantic_nodes)
        }
        StoryId::ReferenceImageToolbarWide => {
            toolbar_story(ui, false, tokens, font_scale, observations, semantic_nodes)
        }
        StoryId::PropertyRowLongValue => {
            property_row(
                ui,
                20,
                "Dataset identifier",
                "urn:polyorama:observations:antarctic-sector-04:reconstruction-with-an-intentionally-long-unbroken-suffix",
                tokens,
                font_scale,
                observations,
            );
        }
        StoryId::StatusErrorLongMessage => {
            status_badge(
                ui,
                30,
                "Worker decode failed after three attempts. The original scientific tile remains unavailable; inspect Diagnostics for request token 18446744073709551615.",
                StatusTone::Error,
                tokens,
                font_scale,
                observations,
            );
        }
        StoryId::VirtualGridLoading => thumbnail_grid(ui, false, tokens, font_scale, observations),
        StoryId::VirtualGridPartial | StoryId::ReferenceThumbnails => {
            thumbnail_grid(ui, true, tokens, font_scale, observations)
        }
        StoryId::ReferenceInspector => inspector_story(ui, tokens, font_scale, observations),
        StoryId::ReferenceResults => results_story(ui, tokens, font_scale, observations),
        StoryId::ReferenceDiagnostics => diagnostics_story(ui, tokens, font_scale, observations),
    }
}

fn toolbar_story(
    ui: &mut egui::Ui,
    narrow: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
) {
    ui.set_max_width((if narrow { 296.0_f32 } else { 720.0_f32 }).min(ui.available_width()));
    ui.horizontal_wrapped(|ui| {
        for (action, selected) in [
            (GalleryAction::NavigateTool, true),
            (GalleryAction::PolygonTool, false),
            (GalleryAction::EditVerticesTool, false),
            (GalleryAction::FitView, false),
            (GalleryAction::LinkViews, true),
        ] {
            gallery_action_button(
                ui,
                ActionTarget::pane(action, PaneId(1)),
                Availability::Enabled,
                selected,
                ActionEmphasis::Quiet,
                narrow,
                tokens,
                font_scale,
                observations,
                semantic_nodes,
            );
        }
        let parent = SemanticUiId::new("gallery.story");
        let mut map = 0_u8;
        let map = choice_control(
            ui,
            SemanticUiId::new("gallery.image-toolbar.display-map"),
            parent.clone(),
            "Display map",
            &mut map,
            &[(0, "Viridis"), (1, "Greyscale"), (2, "Threshold")],
            GalleryAction::DisplaySettings,
            tokens,
        );
        semantic_nodes.push(map.node);
        let mut low = 0.1;
        let low = range_control(
            ui,
            SemanticUiId::new("gallery.image-toolbar.low"),
            parent.clone(),
            "Low",
            &mut low,
            0.0..=0.8,
            GalleryAction::DisplaySettings,
            tokens,
        );
        semantic_nodes.push(low.node);
        let mut high = 0.9;
        let high = range_control(
            ui,
            SemanticUiId::new("gallery.image-toolbar.high"),
            parent,
            "High",
            &mut high,
            0.2..=1.0,
            GalleryAction::DisplaySettings,
            tokens,
        );
        semantic_nodes.push(high.node);
    });
    ui.add_space(tokens.spacing.section.0);
    status_badge(
        ui,
        45,
        if narrow {
            "Linked • 256 px/pt"
        } else {
            "Camera link A • 256 image pixels per screen point"
        },
        StatusTone::Success,
        tokens,
        font_scale,
        observations,
    );
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

fn splitter_story(
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

fn inspector_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, label, value) in [
        (50, "Result", "#000,842,771"),
        (51, "Position", "−12,345.625, 98,765.125 px"),
        (52, "Confidence", "99.875 %"),
        (
            53,
            "Category",
            "Review — exceptionally long deterministic classification name",
        ),
        (54, "Annotation", "Polygon 17 · 128 vertices · selected"),
    ] {
        property_row(ui, instance, label, value, tokens, font_scale, observations);
    }
}

fn results_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, id, position, confidence, category, selected) in [
        (
            60,
            "#842769",
            "−12345.6, 98765.1",
            "−0.125 %",
            "Edge",
            false,
        ),
        (
            61,
            "#842770",
            "65536.0, 65536.0",
            "99.875 %",
            "Selected review target with long label",
            true,
        ),
        (
            62,
            "#842771",
            "131071.9, −0.125",
            "100.000 %",
            "Cluster",
            false,
        ),
        (63, "#842772", "0.0, 0.0", "7.500 %", "Target", false),
    ] {
        result_row(
            ui,
            polyorama_ui_egui::ResultRowSpec {
                instance,
                identifier: id,
                position,
                confidence,
                category,
                selected,
            },
            tokens,
            font_scale,
            observations,
        );
    }
}

fn thumbnail_grid(
    ui: &mut egui::Ui,
    partial: bool,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    let count = if ui.available_width() < 400.0 { 4 } else { 8 };
    ui.horizontal_wrapped(|ui| {
        for index in 0..count {
            let state = if !partial {
                ThumbnailState::Loading
            } else {
                match index % 4 {
                    0 => ThumbnailState::Resident,
                    1 => ThumbnailState::Loading,
                    2 => ThumbnailState::Empty,
                    _ => ThumbnailState::Error,
                }
            };
            thumbnail_cell(
                ui,
                ThumbnailCellSpec {
                    instance: 70 + index,
                    label: &format!("Tile {:06}", 120_000 + index),
                    state,
                    selected: index == 0,
                    texture: None,
                },
                tokens,
                font_scale,
                observations,
            );
        }
    });
}

fn diagnostics_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, label, value) in [
        (
            90,
            "Application update CPU p95 across the retained deterministic observation window",
            "126.300 ms",
        ),
        (
            91,
            "Resident texture bytes / configured cache budget",
            "67,108,864 / 67,108,864 bytes",
        ),
        (
            92,
            "Outstanding request token",
            "generation=4 epoch=18 sequence=18,446,744,073,709,551,615",
        ),
        (
            93,
            "GPU timestamp",
            "unavailable — adapter does not expose timestamp queries",
        ),
    ] {
        property_row(ui, instance, label, value, tokens, font_scale, observations);
    }
    status_badge(
        ui,
        99,
        "Worker running · zero queued · zero in flight · event-driven repaint idle",
        StatusTone::Success,
        tokens,
        font_scale,
        observations,
    );
}

struct DockSceneState {
    workspace: Workspace,
    behaviour: DockBehaviour,
    history: CommandHistory,
    document: Document,
    session: Session,
}

impl DockSceneState {
    fn new(story: StoryId) -> Self {
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
            disabled_reason: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_matrix_is_finite_bounded_and_token_resolvable() {
        for appearance in [AppearancePreference::Light, AppearancePreference::Dark] {
            for contrast in [ContrastPreference::Standard, ContrastPreference::High] {
                for density in [DensityPreference::Compact, DensityPreference::Comfortable] {
                    for font_scale in [1.0, 1.25, 1.5] {
                        for width in [
                            GalleryWidth::Narrow,
                            GalleryWidth::Regular,
                            GalleryWidth::Wide,
                        ] {
                            let configuration = GalleryConfiguration {
                                appearance,
                                contrast,
                                density,
                                font_scale,
                                width,
                            }
                            .validated();
                            let tokens = configuration
                                .preferences()
                                .tokens(appearance == AppearancePreference::Dark);
                            assert!(configuration.width.points().is_finite());
                            assert!(tokens.geometry.control_height.0.is_finite());
                        }
                    }
                }
            }
        }
    }

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
                        focused: false,
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
                        hovered: false,
                        active: false,
                        focused: true,
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
        let mut focus_story = None;
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 300.0),
                )),
                ..Default::default()
            },
            |ui| {
                render_story(
                    ui,
                    StoryId::TabsNarrow,
                    &mut dock,
                    &tokens,
                    1.0,
                    &mut observations,
                    &mut semantic_nodes,
                    &mut focus_story,
                );
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

    #[test]
    fn every_story_renders_through_production_components_without_text_audit_findings() {
        let representative = [
            GalleryConfiguration::default(),
            GalleryConfiguration {
                appearance: AppearancePreference::Light,
                density: DensityPreference::Compact,
                width: GalleryWidth::Narrow,
                ..GalleryConfiguration::default()
            },
            GalleryConfiguration {
                contrast: ContrastPreference::High,
                font_scale: 1.25,
                ..GalleryConfiguration::default()
            },
            GalleryConfiguration {
                appearance: AppearancePreference::Light,
                contrast: ContrastPreference::High,
                font_scale: 1.5,
                width: GalleryWidth::Wide,
                ..GalleryConfiguration::default()
            },
        ];
        for configuration in representative {
            let context = egui::Context::default();
            apply_design_system(&context, configuration.preferences());
            let tokens = configuration
                .preferences()
                .tokens(configuration.appearance == AppearancePreference::Dark);
            for story in StoryId::ALL {
                let mut dock = DockSceneState::new(story);
                let mut observations = Vec::new();
                let mut semantic_nodes = Vec::new();
                let mut focus_story = None;
                let size = egui::vec2(
                    configuration.width.points(),
                    f32::from(story_definition(story).recommended_viewport.height),
                );
                let mut output = context.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        render_story(
                            ui,
                            story,
                            &mut dock,
                            &tokens,
                            configuration.font_scale,
                            &mut observations,
                            &mut semantic_nodes,
                            &mut focus_story,
                        );
                    },
                );
                output.textures_delta.clear();
                assert!(
                    !observations.is_empty(),
                    "story {story} emitted no measured text"
                );
                let findings = audit_text_layouts(&observations);
                assert!(
                    findings.is_empty(),
                    "story {story} at {configuration:?} failed text audit: {findings:#?}"
                );
            }
        }
    }
}
