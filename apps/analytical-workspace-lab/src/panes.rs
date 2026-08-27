#![allow(clippy::collapsible_if)] // Nested event guards mirror the gesture state machine.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use workspace_core::*;
use workspace_render_wgpu::{DisplayMap, DisplaySettings, ImageRenderRequest, PhysicalViewport};
use workspace_ui_egui::{PanePresenter, allocate_viewport, submit_scalar_callback};

#[derive(Default)]
pub struct PaneOutputs {
    pub intents: Vec<ImageIntent>,
    pub commands: Vec<Command>,
    pub demands: Vec<TileDemand>,
    pub interaction_active: bool,
}

#[derive(Default)]
pub struct UiBehaviour {
    camera_drag_start: BTreeMap<PaneId, Camera>,
    pointer_image: BTreeMap<PaneId, ImagePoint>,
}

pub struct PaneSurface<'a> {
    pub document: &'a Document,
    pub session: &'a mut Session,
    pub ui_behaviour: &'a mut UiBehaviour,
    pub display: &'a mut BTreeMap<PaneId, DisplaySettings>,
    pub diagnostics: &'a mut DiagnosticsSnapshot,
    pub resident_tiles: &'a BTreeSet<TileKey>,
    pub generation: u64,
    pub frame_number: u64,
    pub active_pane: PaneId,
    pub outputs: &'a mut PaneOutputs,
}

impl PanePresenter for PaneSurface<'_> {
    fn title(&self, pane: PaneId) -> &'static str {
        match pane.0 {
            1 => "Primary View",
            2 => "Linked Detail",
            3 => "Overview",
            4 => "Derived View",
            5 => "Results",
            6 => "Thumbnails",
            7 => "Inspector",
            8 => "Diagnostics",
            _ => "Unknown pane",
        }
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        ui.push_id(("window", 1_u32, "pane", pane.0), |ui| match pane.0 {
            1..=4 => self.image_pane(ui, pane),
            5 => self.results_pane(ui),
            6 => self.thumbnails_pane(ui),
            7 => self.inspector_pane(ui),
            8 => self.diagnostics_pane(ui),
            _ => {
                ui.label("Unknown pane");
            }
        });
    }
}

impl PaneSurface<'_> {
    fn image_pane(&mut self, ui: &mut egui::Ui, pane: PaneId) {
        let camera_index = self
            .session
            .cameras
            .iter()
            .position(|state| state.pane == pane)
            .unwrap_or(0);
        let mut fit = false;
        let mut commit_polygon = false;
        let mut delete_annotation = false;
        let display = self.display.entry(pane).or_default();
        ui.horizontal(|ui| {
            if pane.0 <= 2 {
                for (tool, label) in [(ActiveTool::Navigate, "Navigate"), (ActiveTool::Polygon, "Polygon"), (ActiveTool::EditVertex, "Edit")] {
                    if ui.selectable_label(self.session.active_tools.get(&pane) == Some(&tool), label).clicked() { self.session.active_tools.insert(pane, tool); }
                }
            }
            fit = ui.small_button("Fit").clicked();
            let mut linked = self.session.cameras[camera_index].link.is_some();
            if ui.checkbox(&mut linked, "Link A").changed() { self.outputs.intents.push(ImageIntent::SetCameraLink { pane, link: linked.then_some(LinkGroupId(1)) }); }
            egui::ComboBox::from_id_salt((pane.0, "map")).selected_text(match display.map { DisplayMap::Viridis => "Viridis", DisplayMap::Greyscale => "Greyscale", DisplayMap::Threshold => "Threshold" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut display.map, DisplayMap::Viridis, "Viridis");
                ui.selectable_value(&mut display.map, DisplayMap::Greyscale, "Greyscale");
                ui.selectable_value(&mut display.map, DisplayMap::Threshold, "Threshold");
            });
            ui.add(egui::Slider::new(&mut display.window_low, 0.0..=0.8).show_value(false).text("low"));
            ui.add(egui::Slider::new(&mut display.window_high, 0.2..=1.0).show_value(false).text("high"));
            if matches!(self.session.gesture, Some(GesturePreview::Polygon { ref vertices, .. }) if vertices.len() >= 3) {
                commit_polygon = ui.small_button("Commit polygon").clicked();
            }
            if self.session.selected_annotation.is_some() { delete_annotation = ui.small_button("Delete").clicked(); }
        });
        if fit {
            let size = ui.available_size();
            self.outputs.intents.push(ImageIntent::SetCamera {
                pane,
                camera: Camera::fit(size.x as f64, size.y as f64),
            });
        }
        if commit_polygon {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.session.gesture.take() {
                self.outputs
                    .intents
                    .push(ImageIntent::CommitPolygon { layer, vertices });
            }
        }
        if self.active_pane == pane && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.session.gesture.take() {
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
            if let Some(annotation) = self.session.selected_annotation {
                self.outputs
                    .intents
                    .push(ImageIntent::DeleteAnnotation { annotation });
            }
        }

        let status_height = 22.0;
        let desired = egui::vec2(
            ui.available_width(),
            (ui.available_height() - status_height).max(64.0),
        );
        let (allocation, response) = allocate_viewport(ui, pane, desired);
        let rect = allocation.logical_rect;
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_rgb(8, 12, 15));
        paint_placeholder(ui, rect, pane);

        let camera = self.session.cameras[camera_index].camera;
        let mut demands = visible_tile_demands(
            camera,
            (rect.width() as f64, rect.height() as f64),
            SourceId(1),
            self.generation,
            true,
        );
        demands.push(TileDemand {
            key: TileKey {
                source: SourceId(1),
                level: PYRAMID_LEVELS - 1,
                x: 0,
                y: 0,
            },
            priority: DemandPriority::Visible,
            generation: self.generation,
        });
        self.outputs.demands.extend(demands.iter().copied());
        let visible_tiles = demands
            .iter()
            .map(|demand| demand.key)
            .filter(|key| self.resident_tiles.contains(key))
            .collect();
        submit_scalar_callback(
            ui,
            rect,
            self.frame_number,
            ImageRenderRequest {
                pane,
                source: SourceId(1),
                viewport: PhysicalViewport {
                    origin: allocation.physical_origin,
                    size: allocation.physical_size,
                    scale_factor: allocation.scale_factor,
                },
                camera,
                display: *display,
                visible_tiles,
            },
        );

        self.handle_camera(ui, pane, camera_index, rect, &response);
        self.handle_annotations(ui, pane, camera_index, rect, &response);
        if response.clicked() && self.session.active_tools.get(&pane) == Some(&ActiveTool::Navigate)
        {
            if let Some(pointer) = response.interact_pointer_pos() {
                let image =
                    screen_to_image(pointer, rect, self.session.cameras[camera_index].camera);
                if pane == PaneId(3) {
                    let source = self
                        .session
                        .cameras
                        .iter()
                        .find(|state| state.pane == PaneId(1))
                        .map(|state| state.camera)
                        .unwrap_or_default();
                    let mut after = source;
                    after.centre = image;
                    self.outputs.commands.push(Command::SetCamera {
                        pane: PaneId(1),
                        before: source,
                        after,
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
        if pane == PaneId(3) {
            self.paint_overview_footprint(ui, rect);
        }
        if let Some(pointer) = allocation.pointer_local {
            let image = self.session.cameras[camera_index].camera.image_at(
                pointer,
                ViewportPoint::new(rect.width() as f64, rect.height() as f64),
            );
            self.ui_behaviour.pointer_image.insert(pane, image);
        }
        let pointer = self
            .ui_behaviour
            .pointer_image
            .get(&pane)
            .copied()
            .unwrap_or_default();
        let world = ImageToWorld::default().image_to_world(pointer);
        ui.horizontal(|ui| {
            ui.monospace(format!(
                "image {:>8.1}, {:>8.1}  ·  world {:>10.1}, {:>10.1}",
                pointer.x, pointer.y, world.x, world.y
            ));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!(
                    "L{} · {} tiles",
                    camera.pixels_per_screen_point.log2().round().max(0.0) as u8,
                    demands.len()
                ));
            });
        });
    }

    fn handle_camera(
        &mut self,
        ui: &egui::Ui,
        pane: PaneId,
        camera_index: usize,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let tool = self
            .session
            .active_tools
            .get(&pane)
            .copied()
            .unwrap_or(ActiveTool::Navigate);
        if tool != ActiveTool::Navigate {
            return;
        }
        if response.drag_started() {
            self.ui_behaviour
                .camera_drag_start
                .insert(pane, self.session.cameras[camera_index].camera);
        }
        if response.dragged() {
            if let Some(start) = self.ui_behaviour.camera_drag_start.get(&pane).copied() {
                let mut preview = start;
                preview.pan(ViewportPoint::new(
                    response.drag_delta().x as f64,
                    response.drag_delta().y as f64,
                ));
                propagate_linked_camera(&mut self.session.cameras, pane, preview);
                self.outputs.interaction_active = true;
            }
        }
        if response.drag_stopped() {
            if let Some(before) = self.ui_behaviour.camera_drag_start.remove(&pane) {
                let after = self.session.cameras[camera_index].camera;
                self.outputs.commands.push(Command::SetCamera {
                    pane,
                    before,
                    after,
                });
            }
        }
        if response.hovered() {
            let zoom_delta = ui.input(|input| input.smooth_scroll_delta.y);
            if zoom_delta.abs() > 0.01 {
                let before = self.session.cameras[camera_index].camera;
                let pointer = response.hover_pos().unwrap_or(rect.center());
                let local = ViewportPoint::new(
                    (pointer.x - rect.left()) as f64,
                    (pointer.y - rect.top()) as f64,
                );
                let mut after = before;
                after.zoom_around(
                    (-zoom_delta as f64 * 0.0025).exp(),
                    local,
                    ViewportPoint::new(rect.width() as f64, rect.height() as f64),
                );
                propagate_linked_camera(&mut self.session.cameras, pane, after);
                self.outputs.commands.push(Command::SetCamera {
                    pane,
                    before,
                    after,
                });
                self.outputs.interaction_active = true;
            }
        }
    }

    fn handle_annotations(
        &mut self,
        ui: &egui::Ui,
        pane: PaneId,
        camera_index: usize,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let camera = self.session.cameras[camera_index].camera;
        let to_screen = |world: WorldPoint| {
            let image = ImageToWorld::default().world_to_image(world);
            egui::pos2(
                rect.center().x
                    + ((image.x - camera.centre.x) / camera.pixels_per_screen_point) as f32,
                rect.center().y
                    + ((image.y - camera.centre.y) / camera.pixels_per_screen_point) as f32,
            )
        };
        for polygon in &self.document.annotations {
            let mut points: Vec<_> = polygon.vertices.iter().copied().map(to_screen).collect();
            if let Some(GesturePreview::Vertex {
                annotation,
                vertex,
                preview,
                ..
            }) = &self.session.gesture
            {
                if *annotation == polygon.id && *vertex < points.len() {
                    points[*vertex] = to_screen(*preview);
                }
            }
            if points.len() > 1 {
                points.push(points[0]);
                ui.painter().add(egui::Shape::line(
                    points.clone(),
                    egui::Stroke::new(
                        2.0,
                        if self.session.selected_annotation == Some(polygon.id) {
                            egui::Color32::from_rgb(255, 196, 74)
                        } else {
                            egui::Color32::from_rgb(105, 227, 210)
                        },
                    ),
                ));
                for point in points.iter().take(points.len() - 1) {
                    ui.painter()
                        .circle_filled(*point, 3.5, egui::Color32::from_rgb(240, 244, 238));
                }
            }
        }
        if let Some(GesturePreview::Polygon { vertices, .. }) = &self.session.gesture {
            let mut points: Vec<_> = vertices.iter().copied().map(to_screen).collect();
            if let Some(pointer) = response.hover_pos() {
                points.push(pointer);
            }
            if points.len() > 1 {
                ui.painter().add(egui::Shape::line(
                    points,
                    egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 181, 64)),
                ));
            }
        }
        let tool = self
            .session
            .active_tools
            .get(&pane)
            .copied()
            .unwrap_or(ActiveTool::Navigate);
        if tool == ActiveTool::Polygon && response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let image = screen_to_image(pointer, rect, camera);
                let world = ImageToWorld::default().image_to_world(image);
                match &mut self.session.gesture {
                    Some(GesturePreview::Polygon { vertices, .. }) => vertices.push(world),
                    _ => {
                        self.session.gesture = Some(GesturePreview::Polygon {
                            layer: LayerId(1),
                            vertices: vec![world],
                        })
                    }
                }
            }
            if response.double_clicked() {
                if matches!(&self.session.gesture, Some(GesturePreview::Polygon { vertices, .. }) if vertices.len() >= 3)
                {
                    if let Some(GesturePreview::Polygon { layer, vertices }) =
                        self.session.gesture.take()
                    {
                        self.outputs
                            .intents
                            .push(ImageIntent::CommitPolygon { layer, vertices });
                    }
                }
            }
        }
        if tool == ActiveTool::Polygon && response.secondary_clicked() {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.session.gesture.take() {
                if vertices.len() >= 3 {
                    self.outputs
                        .intents
                        .push(ImageIntent::CommitPolygon { layer, vertices });
                }
            }
        }
        if tool == ActiveTool::EditVertex {
            if response.drag_started() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let nearest = self
                        .document
                        .annotations
                        .iter()
                        .flat_map(|polygon| {
                            polygon
                                .vertices
                                .iter()
                                .enumerate()
                                .map(move |(index, vertex)| {
                                    (
                                        polygon.id,
                                        index,
                                        *vertex,
                                        to_screen(*vertex).distance(pointer),
                                    )
                                })
                        })
                        .min_by(|a, b| a.3.total_cmp(&b.3));
                    if let Some((annotation, vertex, original, _distance)) =
                        nearest.filter(|item| item.3 < 16.0)
                    {
                        self.session.selected_annotation = Some(annotation);
                        self.session.gesture = Some(GesturePreview::Vertex {
                            annotation,
                            vertex,
                            original,
                            preview: original,
                        });
                    }
                }
            }
            if response.dragged() {
                if let (Some(pointer), Some(GesturePreview::Vertex { preview, .. })) =
                    (response.interact_pointer_pos(), &mut self.session.gesture)
                {
                    *preview = ImageToWorld::default()
                        .image_to_world(screen_to_image(pointer, rect, camera));
                    self.outputs.interaction_active = true;
                }
            }
            if response.drag_stopped() {
                if let Some(GesturePreview::Vertex {
                    annotation,
                    vertex,
                    original,
                    preview,
                }) = self.session.gesture.take()
                {
                    self.outputs.commands.push(Command::MoveVertex {
                        annotation,
                        vertex,
                        before: original,
                        after: preview,
                    });
                }
            }
        }
    }

    fn paint_overview_footprint(&self, ui: &egui::Ui, rect: egui::Rect) {
        let primary = self
            .session
            .cameras
            .iter()
            .find(|camera| camera.pane == PaneId(1))
            .map(|camera| camera.camera)
            .unwrap_or_default();
        let overview = self
            .session
            .cameras
            .iter()
            .find(|camera| camera.pane == PaneId(3))
            .map(|camera| camera.camera)
            .unwrap_or_default();
        let centre = egui::pos2(
            rect.center().x
                + ((primary.centre.x - overview.centre.x) / overview.pixels_per_screen_point)
                    as f32,
            rect.center().y
                + ((primary.centre.y - overview.centre.y) / overview.pixels_per_screen_point)
                    as f32,
        );
        let size = egui::vec2(
            160.0 / overview.pixels_per_screen_point as f32
                * primary.pixels_per_screen_point as f32,
            100.0 / overview.pixels_per_screen_point as f32
                * primary.pixels_per_screen_point as f32,
        )
        .max(egui::vec2(20.0, 14.0));
        ui.painter().rect_stroke(
            egui::Rect::from_center_size(centre, size),
            1.0,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }

    fn results_pane(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(format!("{} logical detections", RESULT_COUNT));
            if let Some(selected) = self.session.selected_result {
                if ui.button("Recenter Primary").clicked() {
                    self.outputs.intents.push(ImageIntent::RecenterOnResult {
                        result: selected,
                        pane: PaneId(1),
                    });
                }
            }
        });
        let row_height = 23.0;
        egui::ScrollArea::vertical()
            .id_salt("million-row-results")
            .show_rows(ui, row_height, RESULT_COUNT as usize, |ui, range| {
                self.diagnostics.virtualisation.visible_rows = (range.start, range.end);
                self.diagnostics.virtualisation.materialised_rows = range.len();
                for index in range {
                    let result = result_at(index as u64);
                    let selected = self.session.selected_result == Some(result.id);
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(selected, format!("#{:07}", result.id.0))
                            .clicked()
                        {
                            self.outputs
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
                }
            });
    }

    fn thumbnails_pane(&mut self, ui: &mut egui::Ui) {
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
                self.diagnostics.virtualisation.visible_thumbnails = (
                    grid.visible_rows.start * grid.columns,
                    (grid.visible_rows.end * grid.columns).min(THUMBNAIL_COUNT as usize),
                );
                self.diagnostics.virtualisation.materialised_thumbnails =
                    grid.materialised_items.len();
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
                    self.outputs.demands.push(TileDemand {
                        key,
                        priority: DemandPriority::Visible,
                        generation: self.generation,
                    });
                    let ready = self.resident_tiles.contains(&key);
                    let response = ui.interact(
                        rect,
                        ui.id().with(("thumbnail", index)),
                        egui::Sense::click(),
                    );
                    if response.clicked() {
                        self.outputs.intents.push(ImageIntent::SelectResult {
                            result: ResultId(index as u64),
                        });
                    }
                    let selected = self.session.selected_result == Some(ResultId(index as u64));
                    ui.painter().rect_filled(
                        rect,
                        3.0,
                        if ready {
                            thumbnail_colour(index as u64)
                        } else {
                            egui::Color32::from_rgb(34, 39, 42)
                        },
                    );
                    if !ready {
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

    fn inspector_pane(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selection");
        if let Some(selected) = self.session.selected_result {
            let result = result_at(selected.0);
            egui::Grid::new("result-inspector")
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("Result");
                    ui.monospace(format!("#{}", result.id.0));
                    ui.end_row();
                    ui.label("Position");
                    ui.monospace(format!(
                        "{:.1}, {:.1}",
                        result.position.x, result.position.y
                    ));
                    ui.end_row();
                    ui.label("Confidence");
                    ui.label(format!("{:.2}%", result.confidence * 100.0));
                    ui.end_row();
                    ui.label("Category");
                    ui.label(["Target", "Edge", "Cluster", "Review"][result.category as usize]);
                    ui.end_row();
                });
            if ui.button("Recenter Primary View").clicked() {
                self.outputs.intents.push(ImageIntent::RecenterOnResult {
                    result: selected,
                    pane: PaneId(1),
                });
            }
        } else {
            ui.label("No result selected");
        }
        ui.separator();
        ui.heading("Annotation");
        if let Some(annotation) = self.session.selected_annotation {
            ui.monospace(format!("Polygon {}", annotation.0));
        } else {
            ui.label("No polygon selected");
        }
    }

    fn diagnostics_pane(&mut self, ui: &mut egui::Ui) {
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
                        "CPU frame",
                        format!("{:.2} ms", self.diagnostics.frame.cpu_frame_ms),
                    ),
                    (
                        "Recent CPU samples",
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
                        "Passes / draws / command buffers",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.render.render_passes,
                            self.diagnostics.render.draw_calls,
                            self.diagnostics.render.command_buffers
                        ),
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
                        "Cache hits / misses / evictions",
                        format!(
                            "{} / {} / {}",
                            self.diagnostics.runtime.cache_hits,
                            self.diagnostics.runtime.cache_misses,
                            self.diagnostics.runtime.evictions
                        ),
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
                        "Decode latency summary",
                        format!(
                            "~{:.2} ms",
                            self.diagnostics.runtime.decode_latency_ms_median
                        ),
                    ),
                    (
                        "Worker queue depth",
                        self.diagnostics.runtime.worker_queue_depth.to_string(),
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
                ],
            );
        });
    }
}

fn screen_to_image(point: egui::Pos2, rect: egui::Rect, camera: Camera) -> ImagePoint {
    camera.image_at(
        ViewportPoint::new(
            (point.x - rect.left()) as f64,
            (point.y - rect.top()) as f64,
        ),
        ViewportPoint::new(rect.width() as f64, rect.height() as f64),
    )
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

fn thumbnail_colour(index: u64) -> egui::Color32 {
    let result = result_at(index);
    let value = (result.confidence * 150.0) as u8;
    match result.category {
        0 => egui::Color32::from_rgb(24, 76 + value, 108),
        1 => egui::Color32::from_rgb(96 + value, 58, 38),
        2 => egui::Color32::from_rgb(62, 72, 96 + value),
        _ => egui::Color32::from_rgb(76 + value, 68, 42),
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
