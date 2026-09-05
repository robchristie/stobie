use polyorama_ui_egui::{NativeTextControlKind, record_native_text_control};
use std::str::FromStr;

use eframe::egui;
use polyorama_ui_egui::{
    ActionKey, ActionScope, ActionShortcut, ActionSpec, AppearancePreference, ContrastPreference,
    DensityPreference, DesignTokens, SemanticUiId, ShortcutKey, TextAuditFinding, TextInteraction,
    TextLayoutObservation, UiNode, UiPreferences, UiRole, UiSnapshot, application_bar_frame,
    application_bar_height, apply_design_system, audit_text_layouts,
};
use serde::{Deserialize, Serialize};

use crate::{
    catalogue::{STORIES, StoryId, story_definition},
    stories::{DockSceneState, render_story},
};

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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn request_test_repaint(&self) {
        self.context.request_repaint();
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
        story_node.text_selectable = observations
            .iter()
            .any(|text| text.interaction == TextInteraction::Selectable);
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
}
