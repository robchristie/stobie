use eframe::egui;
use polyorama_core::{
    DemandPriority, ImageIntent, ResultId, SourceId, THUMBNAIL_COUNT, TileDemand, TileKey,
    VirtualisationMetrics,
};
use polyorama_ui_egui::VirtualGridPresenter;

use crate::thumbnail_cache::ThumbnailCache;

use super::FrameOutput;

pub fn show(
    ui: &mut egui::Ui,
    selected_result: Option<ResultId>,
    generation: u64,
    cache: &mut ThumbnailCache,
    virtualisation: &mut VirtualisationMetrics,
    outputs: &mut FrameOutput,
) {
    ui.label(format!(
        "{} logical thumbnails · progressive worker decode",
        THUMBNAIL_COUNT
    ));
    let cell = egui::vec2(106.0, 96.0);
    let output = VirtualGridPresenter::new(cell, 2).show(
        ui,
        ui.id().with("thumbnail-grid"),
        THUMBNAIL_COUNT as usize,
        |grid_ui, layout, origin| {
            for index in layout.materialised_items.clone() {
                let row = index / layout.columns;
                let column = index % layout.columns;
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(column as f32 * cell.x, row as f32 * cell.y),
                    egui::vec2(cell.x - 7.0, cell.y - 7.0),
                );
                let key = TileKey {
                    source: SourceId(2),
                    level: 0,
                    x: index as u32,
                    y: 0,
                };
                outputs.demands.push(TileDemand {
                    key,
                    priority: if layout.visible_items.contains(&index) {
                        DemandPriority::Visible
                    } else {
                        DemandPriority::Prefetch
                    },
                    generation,
                });
                let texture = cache.texture(key);
                let response = grid_ui.interact(
                    rect,
                    grid_ui.id().with(("thumbnail", index)),
                    egui::Sense::click(),
                );
                if response.clicked() {
                    outputs.intents.push(ImageIntent::SelectResult {
                        result: ResultId(index as u64),
                    });
                }
                let selected = selected_result == Some(ResultId(index as u64));
                grid_ui
                    .painter()
                    .rect_filled(rect, 3.0, egui::Color32::from_rgb(34, 39, 42));
                if let Some(texture) = texture {
                    grid_ui.painter().image(
                        texture,
                        rect.shrink(2.0),
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    grid_ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "pending",
                        egui::FontId::proportional(11.0),
                        egui::Color32::GRAY,
                    );
                }
                grid_ui.painter().text(
                    rect.left_bottom() + egui::vec2(5.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("#{index}"),
                    egui::FontId::monospace(10.0),
                    egui::Color32::WHITE,
                );
                if selected {
                    grid_ui.painter().rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 190, 72)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        },
    );
    virtualisation.visible_thumbnails = (
        output.layout.visible_items.start,
        output.layout.visible_items.end,
    );
    virtualisation.materialised_thumbnails = output.layout.materialised_items.len();
    virtualisation.materialised_thumbnail_range = (
        output.layout.materialised_items.start,
        output.layout.materialised_items.end,
    );
    virtualisation.thumbnail_columns = output.layout.columns;
    virtualisation.thumbnail_total_rows = output.layout.total_rows;
    virtualisation.thumbnail_scroll_offset_y = output.scroll_offset_y;
    virtualisation.thumbnail_content_height = output.content_height;
    virtualisation.thumbnail_viewport_height = output.viewport_height;
    outputs.ui_geometry.thumbnail_scroll = Some(output.viewport_rect.into());
    if output.wheel_delta_y != 0.0 {
        virtualisation.thumbnail_wheel_input_frames += 1;
        virtualisation.thumbnail_wheel_delta_y += output.wheel_delta_y;
    }
}
