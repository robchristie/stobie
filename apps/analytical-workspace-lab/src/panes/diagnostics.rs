use super::*;

impl PaneSurface<'_> {
    pub(super) fn diagnostics_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Live diagnostics");
            if ui.button("Copy JSON snapshot").clicked() {
                if let Ok(json) = self.diagnostics.json_pretty() {
                    ui.ctx().copy_text(json);
                }
            }
        });
        egui::ScrollArea::vertical().show(ui, |ui| {
            metric_section(
                ui,
                "Frame and UI",
                &[
                    ("Frame", self.diagnostics.frame.frame_number.to_string()),
                    (
                        "Application update CPU",
                        format!("{:.2} ms", self.diagnostics.frame.cpu_frame_ms),
                    ),
                    (
                        "Recent update CPU samples",
                        self.diagnostics
                            .frame
                            .cpu_frame_history_ms
                            .len()
                            .to_string(),
                    ),
                    (
                        "Runtime poll",
                        format!("{:.3} ms", self.diagnostics.frame.runtime_poll_ms),
                    ),
                    (
                        "UI construction",
                        format!("{:.2} ms", self.diagnostics.frame.ui_ms),
                    ),
                    (
                        "Demand reconciliation",
                        format!("{:.3} ms", self.diagnostics.frame.demand_ms),
                    ),
                    (
                        "Repaint reason",
                        format!("{:?}", self.diagnostics.frame.repaint_reason),
                    ),
                    (
                        "Application repaint requests",
                        self.diagnostics.frame.repaint_requests.to_string(),
                    ),
                    (
                        "Interaction active",
                        self.diagnostics.frame.interaction_active.to_string(),
                    ),
                ],
            );
            metric_section(
                ui,
                "Workspace",
                &[
                    (
                        "Registered / visible panes",
                        format!(
                            "{} / {}",
                            self.diagnostics.workspace.registered_panes,
                            self.diagnostics.workspace.visible_panes
                        ),
                    ),
                    (
                        "Active pane",
                        format!("{:?}", self.diagnostics.workspace.active_pane),
                    ),
                    (
                        "Dock nodes",
                        self.diagnostics.workspace.dock_nodes.to_string(),
                    ),
                    (
                        "Layout JSON",
                        format!("{} bytes", self.diagnostics.workspace.serialised_bytes),
                    ),
                ],
            );
            metric_section(
                ui,
                "Rendering",
                &[
                    (
                        "GPU viewports / jobs",
                        format!(
                            "{} / {}",
                            self.diagnostics.render.gpu_viewports,
                            self.diagnostics.render.render_jobs
                        ),
                    ),
                    (
                        "Paint callbacks / draws / returned buffers",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.render.paint_callbacks,
                            self.diagnostics.render.draw_calls,
                            self.diagnostics.render.returned_command_buffers
                        ),
                    ),
                    (
                        "Actual renderer passes",
                        self.diagnostics
                            .render
                            .actual_render_passes
                            .map_or_else(|| "unavailable".into(), |value| value.to_string()),
                    ),
                    (
                        "Uploaded / pending",
                        format!(
                            "{} / {} bytes",
                            self.diagnostics.render.uploaded_bytes,
                            self.diagnostics.render.pending_upload_bytes
                        ),
                    ),
                    (
                        "Resident texture bytes",
                        self.diagnostics.render.resident_texture_bytes.to_string(),
                    ),
                    (
                        "Render preparation",
                        format!("{:.3} ms", self.diagnostics.render.prepare_ms),
                    ),
                    (
                        "Capability",
                        self.diagnostics.render.capability_profile.clone(),
                    ),
                    (
                        "GPU timestamp",
                        self.diagnostics
                            .render
                            .gpu_timestamp_ms
                            .map_or_else(|| "unavailable".into(), |value| format!("{value:.3} ms")),
                    ),
                ],
            );
            metric_section(
                ui,
                "Tiles and workers",
                &[
                    (
                        "Demand total / visible / prefetch",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.total_demands,
                            self.diagnostics.runtime.visible_demands,
                            self.diagnostics.runtime.prefetch_demands
                        ),
                    ),
                    (
                        "Duplicates removed",
                        self.diagnostics
                            .runtime
                            .duplicate_demands_removed
                            .to_string(),
                    ),
                    (
                        "Resident re-demands / admissions / evictions",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.cache_hits,
                            self.diagnostics.runtime.cache_misses,
                            self.diagnostics.runtime.evictions
                        ),
                    ),
                    (
                        "Stale demands rejected",
                        self.diagnostics.runtime.stale_demands_rejected.to_string(),
                    ),
                    (
                        "Invalid demands rejected",
                        self.diagnostics
                            .runtime
                            .invalid_demands_rejected
                            .to_string(),
                    ),
                    (
                        "Queued / in-flight",
                        format!(
                            "{} / {}",
                            self.diagnostics.runtime.queued, self.diagnostics.runtime.in_flight
                        ),
                    ),
                    (
                        "Completed / failed / stale",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.completed,
                            self.diagnostics.runtime.failed,
                            self.diagnostics.runtime.stale_discarded
                        ),
                    ),
                    (
                        "Decode latency p50 / p95",
                        format!(
                            "{:.2} / {:.2} ms",
                            self.diagnostics.runtime.decode_latency_ms.p50,
                            self.diagnostics.runtime.decode_latency_ms.p95
                        ),
                    ),
                    (
                        "Worker health",
                        format!("{:?}", self.diagnostics.runtime.worker_health),
                    ),
                    (
                        "Queue / native / decoded depths",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.worker_queue_depth,
                            self.diagnostics.runtime.native_queue_depth,
                            self.diagnostics.runtime.decoded
                        ),
                    ),
                    (
                        "Scheduler / external / browser bounds",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.scheduler_capacity,
                            self.diagnostics.runtime.external_queue_capacity,
                            self.diagnostics.runtime.browser_credit_capacity
                        ),
                    ),
                    (
                        "Obsolete / superseded / duplicate",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.completion_obsolete,
                            self.diagnostics.runtime.completion_superseded,
                            self.diagnostics.runtime.completion_duplicate
                        ),
                    ),
                    (
                        "Worker failures",
                        format!(
                            "{} · {}",
                            self.diagnostics.runtime.worker_failures,
                            if self.diagnostics.runtime.last_worker_error.is_empty() {
                                "none"
                            } else {
                                &self.diagnostics.runtime.last_worker_error
                            }
                        ),
                    ),
                ],
            );
            metric_section(
                ui,
                "Virtualisation",
                &[
                    (
                        "Logical result rows",
                        self.diagnostics.virtualisation.result_count.to_string(),
                    ),
                    (
                        "Visible / materialised rows",
                        format!(
                            "{:?} / {}",
                            self.diagnostics.virtualisation.visible_rows,
                            self.diagnostics.virtualisation.materialised_rows
                        ),
                    ),
                    (
                        "Row overscan",
                        self.diagnostics.virtualisation.row_overscan.to_string(),
                    ),
                    (
                        "Logical thumbnails",
                        self.diagnostics.virtualisation.thumbnail_count.to_string(),
                    ),
                    (
                        "Visible / materialised thumbnails",
                        format!(
                            "{:?} / {}",
                            self.diagnostics.virtualisation.visible_thumbnails,
                            self.diagnostics.virtualisation.materialised_thumbnails
                        ),
                    ),
                    (
                        "Decoded thumbnail cache",
                        format!(
                            "{} items / {} bytes",
                            self.diagnostics.virtualisation.resident_thumbnails,
                            self.diagnostics.virtualisation.thumbnail_cache_bytes
                        ),
                    ),
                ],
            );
        });
    }
}

fn metric_section(ui: &mut egui::Ui, title: &str, rows: &[(&str, String)]) {
    ui.strong(title);
    egui::Grid::new(("metrics", title))
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            for (label, value) in rows {
                ui.label(*label);
                ui.monospace(value);
                ui.end_row();
            }
        });
    ui.add_space(8.0);
}
