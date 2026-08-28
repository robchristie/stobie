use super::annotations::screen_to_image;
use super::*;

impl PaneSurface<'_> {
    pub(super) fn image_pane(&mut self, ui: &mut egui::Ui, pane: PaneId, pane_rect: egui::Rect) {
        let camera_index = self
            .cameras
            .iter()
            .position(|state| state.pane == pane)
            .expect("validated session has one camera per image pane");
        let mut fit = false;
        let mut commit_polygon = false;
        let mut delete_annotation = false;
        let before_display = self.display.get(&pane).copied().unwrap_or_default();
        let mut display = before_display;
        let before_tool = self
            .active_tools
            .get(&pane)
            .copied()
            .unwrap_or(ActiveTool::Navigate);
        let mut active_tool = before_tool;
        let toolbar = ui.horizontal(|ui| {
            if pane.0 <= 2 {
                for (tool, label, control) in [
                    (ActiveTool::Navigate, "Navigate", "navigate"),
                    (ActiveTool::Polygon, "Polygon", "polygon"),
                    (ActiveTool::EditVertex, "Edit", "edit_vertex"),
                ] {
                    let response = ui.selectable_label(active_tool == tool, label);
                    self.outputs
                        .ui_geometry
                        .control(Some(pane), control, response.rect);
                    if response.clicked() {
                        active_tool = tool;
                    }
                }
            }
            let fit_button = ui.small_button("Fit");
            self.outputs
                .ui_geometry
                .control(Some(pane), "fit", fit_button.rect);
            fit = fit_button.clicked();
            let mut linked = self.cameras[camera_index].link.is_some();
            let link = ui.checkbox(&mut linked, "Link A");
            self.outputs
                .ui_geometry
                .control(Some(pane), "link_a", link.rect);
            if link.changed() {
                self.outputs.intents.push(ImageIntent::SetCameraLink {
                    pane,
                    link: linked.then_some(LinkGroupId(1)),
                });
            }
            egui::ComboBox::from_id_salt((pane.0, "map")).selected_text(match display.map { DisplayMap::Viridis => "Viridis", DisplayMap::Greyscale => "Greyscale", DisplayMap::Threshold => "Threshold" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut display.map, DisplayMap::Viridis, "Viridis");
                ui.selectable_value(&mut display.map, DisplayMap::Greyscale, "Greyscale");
                ui.selectable_value(&mut display.map, DisplayMap::Threshold, "Threshold");
            });
            ui.add(egui::Slider::new(&mut display.window_low, 0.0..=0.8).show_value(false).text("low"));
            ui.add(egui::Slider::new(&mut display.window_high, 0.2..=1.0).show_value(false).text("high"));
            if matches!(self.annotation_ui.get(), Some(GesturePreview::Polygon { vertices, .. }) if vertices.len() >= 3) {
                commit_polygon = ui.small_button("Commit polygon").clicked();
            }
            if self.selected_annotation.is_some() { delete_annotation = ui.small_button("Delete").clicked(); }
        });
        self.outputs
            .ui_geometry
            .image_toolbars
            .push(crate::ui_geometry::PaneUiRect {
                pane,
                rect: toolbar.response.rect.into(),
            });
        if active_tool != before_tool {
            self.active_tools.insert(pane, active_tool);
            self.outputs.pane_intents.push(PaneIntent::SetActiveTool {
                pane,
                tool: active_tool,
            });
        }
        if display != before_display {
            self.display.insert(pane, display);
            self.outputs.pane_intents.push(PaneIntent::SetDisplay {
                pane,
                settings: display,
            });
        }
        if fit {
            let size = ui.available_size();
            self.outputs.intents.push(ImageIntent::SetCamera {
                pane,
                camera: Camera::fit(size.x as f64, size.y as f64),
            });
        }
        if commit_polygon {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.annotation_ui.take() {
                self.outputs
                    .intents
                    .push(ImageIntent::CommitPolygon { layer, vertices });
            }
        }
        if self.active_pane == pane && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.annotation_ui.take() {
                if vertices.len() >= 3 {
                    self.outputs
                        .intents
                        .push(ImageIntent::CommitPolygon { layer, vertices });
                }
            }
        }
        if delete_annotation
            || (self.active_pane == pane && ui.input(|input| input.key_pressed(egui::Key::Delete)))
        {
            if let Some(annotation) = self.selected_annotation {
                self.outputs
                    .intents
                    .push(ImageIntent::DeleteAnnotation { annotation });
            }
        }

        let status_height = 22.0;
        let available = ui.available_rect_before_wrap().intersect(pane_rect);
        let desired = egui::vec2(
            available.width(),
            (available.height() - status_height).max(64.0),
        );
        let (allocation, response) = allocate_viewport(ui, pane, desired);
        let rect = allocation.logical_rect;
        self.outputs
            .ui_geometry
            .image_viewports
            .push(crate::ui_geometry::PaneUiRect {
                pane,
                rect: rect.into(),
            });
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(8, 12, 15));
        paint_placeholder(ui, rect, pane);

        self.handle_camera(ui, pane, rect, &response);
        let frame = derive_image_frame(
            self.ui_behaviour,
            pane,
            self.cameras,
            (rect.width() as f64, rect.height() as f64),
            self.generation,
        );
        let camera = frame.camera;
        let demands = frame.demands;
        self.outputs.demands.extend(demands.iter().copied());
        let request = ImageRenderRequest {
            pane,
            source: SourceId(1),
            source_generation: self.generation,
            viewport: PhysicalViewport {
                origin: allocation.physical_origin,
                size: allocation.physical_size,
                scale_factor: allocation.scale_factor,
            },
            camera,
            display,
            desired_tiles: demands.iter().map(|demand| demand.key).collect(),
        };
        self.outputs.render_plan.submit(request.clone());
        self.outputs.render_targets.push(stage_render_callback(
            ui,
            rect,
            self.frame_number,
            request,
        ));

        self.handle_annotations(pane, camera, rect, &response);
        self.outputs.overlays.push(ImageOverlayRequest {
            pane,
            rect,
            annotations: self.document.annotations.clone(),
            gesture: self.annotation_ui.cloned(),
            selected_annotation: self.selected_annotation,
            hover: response.hover_pos(),
        });
        if response.clicked() && self.active_tools.get(&pane) == Some(&ActiveTool::Navigate) {
            if let Some(pointer) = response.interact_pointer_pos() {
                let image = screen_to_image(pointer, rect, camera);
                if pane == PaneId(3) {
                    let source = self
                        .cameras
                        .iter()
                        .find(|state| state.pane == PaneId(1))
                        .map(|state| state.camera)
                        .unwrap_or_default();
                    let mut after = source;
                    after.centre = image;
                    self.outputs.commands.push(Command::SetCameras {
                        changes: linked_camera_changes(self.cameras, PaneId(1), after),
                    });
                } else {
                    let result = ResultId(
                        (image.y.max(0.0) as u64 * 131_071 + image.x.max(0.0) as u64)
                            % RESULT_COUNT,
                    );
                    self.outputs
                        .intents
                        .push(ImageIntent::SelectResult { result });
                }
            }
        }
        if let Some(pointer) = allocation.pointer_local {
            let image = camera.image_at(
                pointer,
                ViewportPoint::new(rect.width() as f64, rect.height() as f64),
            );
            self.ui_behaviour.pointer_image.insert(pane, image);
        }
        let fallback_pointer = self
            .ui_behaviour
            .pointer_image
            .get(&pane)
            .copied()
            .unwrap_or_default();
        let _ = ui.allocate_exact_size(
            egui::vec2(rect.width(), status_height),
            egui::Sense::hover(),
        );
        let status_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.bottom()),
            egui::vec2(rect.width(), status_height),
        )
        .intersect(ui.max_rect());
        self.outputs.statuses.push(ImageStatusRequest {
            pane,
            rect: status_rect,
            viewport: rect,
            pointer_local: allocation.pointer_local,
            fallback_pointer,
        });
    }
}

fn paint_placeholder(ui: &egui::Ui, rect: egui::Rect, pane: PaneId) {
    let cell = 20.0;
    for row in 0..(rect.height() / cell).ceil() as usize {
        for column in 0..(rect.width() / cell).ceil() as usize {
            if (row + column + pane.0 as usize).is_multiple_of(2) {
                let min = rect.min + egui::vec2(column as f32 * cell, row as f32 * cell);
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(rect),
                    0.0,
                    egui::Color32::from_rgb(14, 20, 23),
                );
            }
        }
    }
}
