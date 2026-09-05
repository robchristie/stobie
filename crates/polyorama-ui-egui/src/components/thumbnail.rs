use egui::{Color32, Rect, Response, Sense, Stroke};

use super::{ComponentTextSpec, paint_text_observation};
use crate::{
    DesignTokens, HorizontalTextAlignment, TextComponentId, TextOverflow, TextRole, TextSpec,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThumbnailState {
    Loading,
    Resident,
    Error,
    Empty,
}

pub struct ThumbnailCellSpec<'a> {
    pub instance: u64,
    pub label: &'a str,
    pub state: ThumbnailState,
    pub selected: bool,
    /// Optional progressively decoded content for the resident state.
    pub texture: Option<egui::TextureId>,
}

pub fn thumbnail_cell_side(tokens: &DesignTokens, font_scale: f32) -> f32 {
    tokens.geometry.control_height.0 * 3.0 * font_scale.clamp(1.0, 1.5)
}

pub fn thumbnail_cell(
    ui: &mut egui::Ui,
    spec: ThumbnailCellSpec<'_>,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<crate::TextLayoutObservation>,
) -> Response {
    let side = thumbnail_cell_side(tokens, font_scale)
        .min(ui.available_width().max(tokens.geometry.minimum_hit_size.0));
    let (_, rect) = ui.allocate_space(egui::vec2(side, side));
    let response = ui.interact(
        rect,
        egui::Id::new(("polyorama.thumbnail-cell", spec.instance)),
        Sense::click(),
    );
    if response.clicked() {
        response.request_focus();
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::SelectableLabel,
            true,
            format!("{}; {:?}", spec.label, spec.state),
        )
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role};
        node.set_role(Role::ListBoxOption);
        node.set_label(format!("{}; {:?}", spec.label, spec.state));
        node.set_author_id(format!("polyorama.thumbnail-cell.{}", spec.instance));
        node.clear_toggled();
        node.set_selected(spec.selected);
        node.add_action(Action::Click);
    });
    let border: Color32 = if spec.selected {
        tokens.colours.accent_primary.into()
    } else {
        tokens.colours.border_subtle.into()
    };
    ui.painter().rect(
        rect,
        tokens.geometry.control_radius.0,
        tokens.colours.surface_raised,
        Stroke::new(if spec.selected { 2.0 } else { 1.0 }, border),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            tokens.geometry.control_radius.0,
            Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let label_height =
        tokens.typography.label_size.0 * font_scale * tokens.typography.line_height.0;
    let image_rect = Rect::from_min_max(
        rect.min + egui::vec2(tokens.spacing.unit.0, tokens.spacing.unit.0),
        egui::pos2(
            rect.max.x - tokens.spacing.unit.0,
            rect.max.y - label_height - tokens.spacing.block.0 * 2.0,
        ),
    );
    match spec.state {
        ThumbnailState::Loading => {
            ui.painter()
                .rect_filled(image_rect, 1.0, tokens.colours.surface_canvas);
            ui.painter().line_segment(
                [image_rect.left_center(), image_rect.right_center()],
                Stroke::new(2.0, tokens.colours.text_muted),
            );
        }
        ThumbnailState::Resident => {
            if let Some(texture) = spec.texture {
                ui.painter().image(
                    texture,
                    image_rect,
                    Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.painter()
                    .rect_filled(image_rect, 1.0, tokens.colours.selection_background);
                let centre = image_rect.center();
                ui.painter().circle_filled(
                    centre,
                    image_rect.width().min(image_rect.height()) * 0.27,
                    tokens.colours.accent_primary,
                );
            }
        }
        ThumbnailState::Error => {
            ui.painter()
                .rect_filled(image_rect, 1.0, tokens.colours.surface_canvas);
            ui.painter().line_segment(
                [image_rect.left_top(), image_rect.right_bottom()],
                Stroke::new(2.0, tokens.colours.status_error),
            );
            ui.painter().line_segment(
                [image_rect.right_top(), image_rect.left_bottom()],
                Stroke::new(2.0, tokens.colours.status_error),
            );
        }
        ThumbnailState::Empty => {
            ui.painter().rect_stroke(
                image_rect,
                1.0,
                Stroke::new(1.0, tokens.colours.border_subtle),
                egui::StrokeKind::Inside,
            );
        }
    }
    let label_rect = Rect::from_min_max(
        egui::pos2(
            rect.min.x + tokens.spacing.unit.0,
            image_rect.max.y + tokens.spacing.block.0,
        ),
        egui::pos2(
            rect.max.x - tokens.spacing.unit.0,
            rect.max.y - tokens.spacing.unit.0,
        ),
    );
    paint_text_observation(
        ui,
        ComponentTextSpec {
            text: spec.label,
            rect: label_rect,
            spec: TextSpec {
                horizontal_alignment: HorizontalTextAlignment::Centre,
                ..TextSpec::single_line(TextRole::Caption, TextOverflow::Ellipsis)
            },
            component_id: TextComponentId::new(
                crate::TextComponentKind::ThumbnailCell,
                spec.instance,
            ),
            parent_id: None,
            accessible: false,
        },
        tokens,
        font_scale,
        observations,
    );
    response
}
