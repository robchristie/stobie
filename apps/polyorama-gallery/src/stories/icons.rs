use std::collections::BTreeMap;

use eframe::egui;
use polyorama_core::PaneId;
use polyorama_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionTarget, Availability,
    ContentTextSpec, DesignTokens, IconId, PresentationContext, PresentationScope, SemanticUiId,
    TextInteraction, TextLayoutObservation, TextOverflow, TextRole, UiNode, icon_size,
    measured_content_label, paint_icon,
};
use serde::{Serialize, Serializer, ser::SerializeStruct};

use crate::{app::GalleryAction, catalogue::StoryId};

/// Fixed logical controls used by the physical input qualification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IconControl {
    Text,
    Icon,
    Labelled,
    Toggle,
    Disabled,
    Navigate,
    Polygon,
    Display,
    Export,
}

impl IconControl {
    fn field_name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Icon => "icon",
            Self::Labelled => "labelled",
            Self::Toggle => "toggle",
            Self::Disabled => "disabled",
            Self::Navigate => "navigate",
            Self::Polygon => "polygon",
            Self::Display => "display",
            Self::Export => "export",
        }
    }
}

// Serialise this fixed typed fixture inventory as object fields on both native
// and Wasm; serde-wasm-bindgen otherwise turns a Rust map into an ES Map.
fn serialize_control_map<S: Serializer, T: Serialize>(
    values: &BTreeMap<IconControl, T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut fields = serializer.serialize_struct("IconControls", values.len())?;
    for (control, value) in values {
        fields.serialize_field(control.field_name(), value)?;
    }
    fields.end()
}

/// Bounded fixture feedback; counters change only after real control activation.
#[derive(Clone, Debug, Serialize)]
pub struct IconFixtureState {
    #[serde(serialize_with = "serialize_control_map")]
    pub activations: BTreeMap<IconControl, u32>,
    #[serde(serialize_with = "serialize_control_map")]
    pub targets: BTreeMap<IconControl, SemanticUiId>,
    pub last_activated: Option<IconControl>,
    pub hovered: Option<IconControl>,
    pub pointer_down: Option<IconControl>,
    pub linked: bool,
    pub selected_tool: IconControl,
}

impl Default for IconFixtureState {
    fn default() -> Self {
        Self {
            activations: [
                IconControl::Text,
                IconControl::Icon,
                IconControl::Labelled,
                IconControl::Toggle,
                IconControl::Disabled,
                IconControl::Navigate,
                IconControl::Polygon,
                IconControl::Display,
                IconControl::Export,
            ]
            .into_iter()
            .map(|control| (control, 0))
            .collect(),
            targets: BTreeMap::new(),
            last_activated: None,
            hovered: None,
            pointer_down: None,
            linked: true,
            selected_tool: IconControl::Navigate,
        }
    }
}

impl IconFixtureState {
    fn observe(&mut self, control: IconControl, response: egui::Response) {
        if response.hovered() {
            self.hovered = Some(control);
        }
        if response.is_pointer_button_down_on() {
            self.pointer_down = Some(control);
        }
        if response.clicked() {
            let count = self
                .activations
                .get_mut(&control)
                .expect("fixed fixture control");
            *count = count.saturating_add(1);
            self.last_activated = Some(control);
            match control {
                IconControl::Toggle => self.linked = !self.linked,
                IconControl::Navigate | IconControl::Polygon => self.selected_tool = control,
                _ => {}
            }
            response.ctx.request_repaint();
        }
    }
}

pub(super) fn vocabulary(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    // Captions are ordinary measured text, never semantic names for the artwork.
    let size = icon_size(tokens, scale);
    let minimum_cell = tokens.geometry.control_height.0 * scale * 3.0;
    let columns = (ui.available_width() / minimum_cell)
        .floor()
        .clamp(2.0, 6.0) as usize;
    let gap = tokens.spacing.inline.0;
    let width = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("gallery.icons.vocabulary")
            .num_columns(columns)
            .spacing(egui::vec2(gap, tokens.spacing.section.0))
            .show(ui, |ui| {
                for (index, icon) in IconId::ALL.iter().copied().enumerate() {
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                        paint_icon(ui.painter(), icon, rect, tokens.colours.text_primary.into());
                        measured_content_label(
                            ui,
                            1000 + index as u64,
                            &format!("{icon:?}"),
                            TextRole::Caption,
                            TextOverflow::Ellipsis,
                            1,
                            TextInteraction::Inert,
                            tokens,
                            scale,
                            observations,
                        );
                    });
                    if (index + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    });
}

fn spec(
    action: GalleryAction,
    emphasis: ActionEmphasis,
    state: ActionButtonState,
) -> ActionButtonSpec<GalleryAction> {
    let target = match action {
        GalleryAction::FitView
        | GalleryAction::LinkViews
        | GalleryAction::NavigateTool
        | GalleryAction::PolygonTool
        | GalleryAction::DisplaySettings => ActionTarget::pane(action, PaneId(1)),
        _ => ActionTarget::application(action),
    };
    ActionButtonSpec {
        target,
        availability: Availability::Enabled,
        state,
        emphasis,
        compact: false,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn actions(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    semantic_nodes: &mut Vec<UiNode>,
    state: &mut IconFixtureState,
) {
    state.hovered = None;
    state.pointer_down = None;
    let scope = PresentationScope::new("gallery.icons").child(story.as_str());
    state.targets = state
        .activations
        .keys()
        .copied()
        .map(|control| (control, scope.instance(control).semantic_id()))
        .collect();
    let mut presentation = PresentationContext::new(
        ui,
        *tokens,
        scale,
        scope,
        SemanticUiId::new("gallery.story"),
    );
    match story {
        StoryId::IconsActionPresentations => {
            state.observe(
                IconControl::Text,
                presentation.action(
                    ui,
                    IconControl::Text,
                    spec(
                        GalleryAction::SaveLayout,
                        ActionEmphasis::Normal,
                        ActionButtonState::Momentary,
                    ),
                ),
            );
            state.observe(
                IconControl::Icon,
                presentation.icon_action(
                    ui,
                    IconControl::Icon,
                    spec(
                        GalleryAction::FitView,
                        ActionEmphasis::QuietBorderless,
                        ActionButtonState::Momentary,
                    ),
                    IconId::FitView,
                ),
            );
            state.observe(
                IconControl::Labelled,
                presentation.action_with_icon(
                    ui,
                    IconControl::Labelled,
                    spec(
                        GalleryAction::SaveLayout,
                        ActionEmphasis::Primary,
                        ActionButtonState::Momentary,
                    ),
                    IconId::Save,
                ),
            );
            state.observe(
                IconControl::Toggle,
                presentation.icon_action(
                    ui,
                    IconControl::Toggle,
                    spec(
                        GalleryAction::LinkViews,
                        ActionEmphasis::Quiet,
                        ActionButtonState::Toggle {
                            pressed: state.linked,
                        },
                    ),
                    IconId::Link,
                ),
            );
            let mut unavailable = spec(
                GalleryAction::Undo,
                ActionEmphasis::Normal,
                ActionButtonState::Momentary,
            );
            unavailable.availability = Availability::Disabled {
                reason: "History is empty".into(),
            };
            state.observe(
                IconControl::Disabled,
                presentation.icon_action(ui, IconControl::Disabled, unavailable, IconId::Undo),
            );
        }
        StoryId::IconsToolbar => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = tokens.spacing.inline.0;
                for (control, key, action, icon) in [
                    (
                        IconControl::Navigate,
                        IconControl::Navigate,
                        GalleryAction::NavigateTool,
                        IconId::Navigate,
                    ),
                    (
                        IconControl::Polygon,
                        IconControl::Polygon,
                        GalleryAction::PolygonTool,
                        IconId::Polygon,
                    ),
                ] {
                    state.observe(
                        control,
                        presentation.icon_action(
                            ui,
                            key,
                            spec(
                                action,
                                ActionEmphasis::QuietBorderless,
                                ActionButtonState::Toggle {
                                    pressed: state.selected_tool == control,
                                },
                            ),
                            icon,
                        ),
                    );
                }
                ui.separator();
                state.observe(
                    IconControl::Icon,
                    presentation.icon_action(
                        ui,
                        IconControl::Icon,
                        spec(
                            GalleryAction::FitView,
                            ActionEmphasis::QuietBorderless,
                            ActionButtonState::Momentary,
                        ),
                        IconId::FitView,
                    ),
                );
                state.observe(
                    IconControl::Display,
                    presentation.icon_action(
                        ui,
                        IconControl::Display,
                        spec(
                            GalleryAction::DisplaySettings,
                            ActionEmphasis::QuietBorderless,
                            ActionButtonState::Momentary,
                        ),
                        IconId::Settings,
                    ),
                );
                ui.add_space(tokens.spacing.inline.0);
                state.observe(
                    IconControl::Labelled,
                    presentation.action_with_icon(
                        ui,
                        IconControl::Labelled,
                        spec(
                            GalleryAction::SaveLayout,
                            ActionEmphasis::Primary,
                            ActionButtonState::Momentary,
                        ),
                        IconId::Save,
                    ),
                );
            });
        }
        StoryId::IconsLongNarrow => {
            state.observe(
                IconControl::Export,
                presentation.action_with_icon(
                    ui,
                    IconControl::Export,
                    spec(
                        GalleryAction::ExportSelection,
                        ActionEmphasis::Primary,
                        ActionButtonState::Momentary,
                    ),
                    IconId::Save,
                ),
            );
            // An adversarial slot derives from the normal hit geometry, leaving a
            // deliberately small measured label while retaining the same recipe.
            ui.scope(|ui| {
                ui.set_max_width(tokens.geometry.control_height.0 * scale * 3.0);
                state.observe(
                    IconControl::Labelled,
                    presentation.action_with_icon(
                        ui,
                        IconControl::Labelled,
                        spec(
                            GalleryAction::ExportSelection,
                            ActionEmphasis::Normal,
                            ActionButtonState::Momentary,
                        ),
                        IconId::Save,
                    ),
                );
            });
        }
        _ => unreachable!("only typed icon action stories enter this renderer"),
    }
    ui.add_space(tokens.spacing.section.0);
    let total: u32 = state.activations.values().copied().sum();
    presentation.content(
        ui,
        "feedback",
        &format!(
            "Completed actions: {total}. Linked views: {}. Selected tool: {:?}.",
            if state.linked { "on" } else { "off" },
            state.selected_tool
        ),
        ContentTextSpec {
            role: TextRole::Secondary,
            overflow: TextOverflow::Wrap,
            max_lines: 3,
            interaction: TextInteraction::Inert,
        },
    );
    let rendered = presentation.finish(ui);
    observations.extend(rendered.text_layouts);
    semantic_nodes.extend(rendered.semantic_nodes);
}
