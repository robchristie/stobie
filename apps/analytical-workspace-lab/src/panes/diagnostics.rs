use super::*;

impl PaneSurface<'_> {
    pub(super) fn diagnostics_pane(&mut self, ui: &mut egui::Ui) {
        let pane = PaneId(8);
        let pane_id = SemanticUiId::pane(pane);
        section_heading(
            ui,
            8_000,
            "Live diagnostics",
            &self.tokens,
            self.font_scale,
            &mut self.outputs.ui_geometry.text_layouts,
        );
        let toolbar_id = SemanticUiId::new("pane.8.toolbar");
        let toolbar = ui.horizontal(|ui| {
            let context = crate::actions::ActionContext {
                active_pane: self.active_pane,
                target_pane: Some(pane),
                ..Default::default()
            };
            if present_action(
                ui,
                self.outputs,
                &self.tokens,
                self.font_scale,
                &toolbar_id,
                ActionTarget::pane(LabAction::CopyDiagnostics, pane),
                crate::actions::availability(LabAction::CopyDiagnostics, context),
                false,
                false,
                self.active_pane == pane,
                "copy_diagnostics",
            ) && let Ok(json) = self.diagnostics.json_pretty()
            {
                ui.ctx().copy_text(json);
            }
        });
        let mut toolbar_node = UiNode::container(
            toolbar_id,
            Some(pane_id.clone()),
            UiRole::Toolbar,
            toolbar.response.rect.into(),
        );
        toolbar_node.name = "Diagnostic actions".into();
        toolbar_node.pane = Some(pane);
        self.outputs.ui_geometry.record_node(toolbar_node);

        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let observations = &mut self.outputs.ui_geometry.text_layouts;
        let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
            metric_section(
                ui,
                1,
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
                &tokens,
                font_scale,
                observations,
            );
            metric_section(
                ui,
                2,
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
                &tokens,
                font_scale,
                observations,
            );
            metric_section(
                ui,
                3,
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
                &tokens,
                font_scale,
                observations,
            );
            metric_section(
                ui,
                4,
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
                &tokens,
                font_scale,
                observations,
            );
            metric_section(
                ui,
                5,
                "Accessibility",
                &[
                    ("Platform integration", platform_accessibility_status().into()),
                    (
                        "End-user qualification",
                        "No platform is qualified by this build alone; see the accessibility evidence report"
                            .into(),
                    ),
                ],
                &tokens,
                font_scale,
                observations,
            );
            metric_section(
                ui,
                6,
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
                            "{:?} / {:?} ({})",
                            self.diagnostics.virtualisation.visible_thumbnails,
                            self.diagnostics.virtualisation.materialised_thumbnail_range,
                            self.diagnostics.virtualisation.materialised_thumbnails
                        ),
                    ),
                    (
                        "Thumbnail grid",
                        format!(
                            "{} columns · {} rows",
                            self.diagnostics.virtualisation.thumbnail_columns,
                            self.diagnostics.virtualisation.thumbnail_total_rows
                        ),
                    ),
                    (
                        "Thumbnail scroll / extent",
                        format!(
                            "{:.1} / {:.1} · viewport {:.1}",
                            self.diagnostics.virtualisation.thumbnail_scroll_offset_y,
                            self.diagnostics.virtualisation.thumbnail_content_height,
                            self.diagnostics.virtualisation.thumbnail_viewport_height
                        ),
                    ),
                    (
                        "Thumbnail wheel input",
                        format!(
                            "{} frames · {:+.1} signed points",
                            self.diagnostics.virtualisation.thumbnail_wheel_input_frames,
                            self.diagnostics.virtualisation.thumbnail_wheel_delta_y
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
                &tokens,
                font_scale,
                observations,
            );
        });
        let mut scroll_node = UiNode::container(
            SemanticUiId::new("pane.8.diagnostics.scroll"),
            Some(pane_id),
            UiRole::ScrollArea,
            scroll.inner_rect.into(),
        );
        scroll_node.name = "Diagnostic metrics".into();
        scroll_node.text_selectable = true;
        scroll_node.pane = Some(pane);
        self.outputs.ui_geometry.record_node(scroll_node);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn platform_accessibility_status() -> &'static str {
    "Native AccessKit adapter compiled; activates when platform assistive technology connects"
}

#[cfg(target_arch = "wasm32")]
fn platform_accessibility_status() -> &'static str {
    "Unavailable: eframe 0.36.1 WebRunner does not expose AccessKit tree updates"
}

fn metric_section(
    ui: &mut egui::Ui,
    section: u64,
    title: &str,
    rows: &[(&str, String)],
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    section_heading(ui, 8_000 + section, title, tokens, font_scale, observations);
    for (index, (label, value)) in rows.iter().enumerate() {
        diagnostic_row(
            ui,
            section * 100 + index as u64,
            label,
            value,
            tokens,
            font_scale,
            observations,
        );
    }
    ui.add_space(tokens.spacing.section.0);
}
