use eframe::egui;
use polyorama_core::{
    ImageIntent, PaneId, RESULT_COUNT, ResultId, VirtualisationMetrics, result_at, virtual_rows,
};

use super::FrameOutput;

pub fn show(
    ui: &mut egui::Ui,
    selected_result: Option<ResultId>,
    virtualisation: &mut VirtualisationMetrics,
    outputs: &mut FrameOutput,
) {
    ui.horizontal(|ui| {
        ui.label(format!("{} logical detections", RESULT_COUNT));
        if let Some(selected) = selected_result
            && ui.button("Recenter Primary").clicked()
        {
            outputs.intents.push(ImageIntent::RecenterOnResult {
                result: selected,
                pane: PaneId(1),
            });
        }
    });
    const ROW_HEIGHT: f32 = 23.0;
    const OVERSCAN_ROWS: usize = 8;
    egui::ScrollArea::vertical()
        .id_salt("million-row-results")
        .show_viewport(ui, |ui, viewport| {
            let rows = virtual_rows(
                viewport.top(),
                viewport.height(),
                ROW_HEIGHT,
                RESULT_COUNT as usize,
                OVERSCAN_ROWS,
            );
            virtualisation.visible_rows = (rows.visible.start, rows.visible.end);
            virtualisation.materialised_rows = rows.materialised.len();
            virtualisation.row_overscan = rows.overscan;
            let origin = ui.min_rect().min;
            ui.set_min_height(RESULT_COUNT as f32 * ROW_HEIGHT);
            for index in rows.materialised {
                let result = result_at(index as u64);
                let selected = selected_result == Some(result.id);
                let row_rect = egui::Rect::from_min_size(
                    origin + egui::vec2(0.0, index as f32 * ROW_HEIGHT),
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(selected, format!("#{:07}", result.id.0))
                            .clicked()
                        {
                            outputs
                                .intents
                                .push(ImageIntent::SelectResult { result: result.id });
                        }
                        ui.monospace(format!(
                            "{:>8.0}  {:>8.0}",
                            result.position.x, result.position.y
                        ));
                        ui.label(format!("{:>5.1}%", result.confidence * 100.0));
                        ui.label(["Target", "Edge", "Cluster", "Review"][result.category as usize]);
                    });
                });
            }
        });
}
