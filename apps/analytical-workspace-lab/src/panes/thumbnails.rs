use eframe::egui;
use polyorama_core::{
    DemandPriority, ImageIntent, ResultId, SourceId, THUMBNAIL_COUNT, TileDemand, TileKey,
    VirtualisationMetrics, virtual_grid,
};

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
    let cell = (106.0, 96.0);
    egui::ScrollArea::vertical()
        .id_salt("thumbnail-grid")
        .show_viewport(ui, |ui, viewport| {
            let grid = virtual_grid(
                viewport.top(),
                viewport.width(),
                viewport.height(),
                cell,
                THUMBNAIL_COUNT as usize,
                2,
            );
            let rows = (THUMBNAIL_COUNT as usize).div_ceil(grid.columns);
            ui.set_min_height(rows as f32 * cell.1);
            virtualisation.visible_thumbnails = (
                grid.visible_rows.start * grid.columns,
                (grid.visible_rows.end * grid.columns).min(THUMBNAIL_COUNT as usize),
            );
            virtualisation.materialised_thumbnails = grid.materialised_items.len();
            let origin = ui.min_rect().min;
            for index in grid.materialised_items {
                let row = index / grid.columns;
                let column = index % grid.columns;
                let rect = egui::Rect::from_min_size(
                    origin + egui::vec2(column as f32 * cell.0, row as f32 * cell.1),
                    egui::vec2(cell.0 - 7.0, cell.1 - 7.0),
                );
                let key = TileKey {
                    source: SourceId(2),
                    level: 0,
                    x: index as u32,
                    y: 0,
                };
                outputs.demands.push(TileDemand {
                    key,
                    priority: DemandPriority::Visible,
                    generation,
                });
                let texture = cache.texture(key);
                let response = ui.interact(
                    rect,
                    ui.id().with(("thumbnail", index)),
                    egui::Sense::click(),
                );
                if response.clicked() {
                    outputs.intents.push(ImageIntent::SelectResult {
                        result: ResultId(index as u64),
                    });
                }
                let selected = selected_result == Some(ResultId(index as u64));
                ui.painter()
                    .rect_filled(rect, 3.0, egui::Color32::from_rgb(34, 39, 42));
                if let Some(texture) = texture {
                    ui.painter().image(
                        texture,
                        rect.shrink(2.0),
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "pending",
                        egui::FontId::proportional(11.0),
                        egui::Color32::GRAY,
                    );
                }
                ui.painter().text(
                    rect.left_bottom() + egui::vec2(5.0, -5.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("#{index}"),
                    egui::FontId::monospace(10.0),
                    egui::Color32::WHITE,
                );
                if selected {
                    ui.painter().rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 190, 72)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        });
}
