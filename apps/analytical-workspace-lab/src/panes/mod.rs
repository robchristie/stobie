#![allow(clippy::collapsible_if)] // Nested event guards mirror the gesture state machine.

use std::{collections::BTreeMap, time::Duration};

use eframe::egui;
use polyorama_core::*;
use polyorama_render_wgpu::{
    DisplayMap, DisplaySettings, ImageRenderRequest, PhysicalViewport, RenderPlan,
};
use polyorama_ui_egui::{ImagePlanTarget, PanePresenter, allocate_viewport, stage_render_callback};
use web_time::Instant;

use crate::thumbnail_cache::ThumbnailCache;

mod inspector;
mod results;
mod thumbnails;

#[derive(Default)]
pub struct FrameOutput {
    pub intents: Vec<ImageIntent>,
    pub commands: Vec<Command>,
    pub demands: Vec<TileDemand>,
    pub render_plan: RenderPlan,
    pub render_targets: Vec<ImagePlanTarget>,
    overlays: Vec<ImageOverlayRequest>,
    statuses: Vec<ImageStatusRequest>,
    pub interaction_active: bool,
    pub repaint_after: Option<Duration>,
}

#[derive(Clone)]
struct ImageOverlayRequest {
    pane: PaneId,
    rect: egui::Rect,
    annotations: Vec<Polygon>,
    gesture: Option<GesturePreview>,
    selected_annotation: Option<AnnotationId>,
    hover: Option<egui::Pos2>,
}

struct ImageStatusRequest {
    pane: PaneId,
    rect: egui::Rect,
    viewport: egui::Rect,
    pointer_local: Option<ViewportPoint>,
    fallback_pointer: ImagePoint,
}

impl FrameOutput {
    pub fn finalise_camera_previews(
        &mut self,
        context: &egui::Context,
        behaviour: &UiBehaviour,
        committed: &[CameraState],
        generation: u64,
    ) {
        self.demands
            .retain(|demand| demand.key.source != SourceId(1));
        for request in &mut self.render_plan.images {
            request.camera = behaviour.camera(request.pane, committed);
            request.source_generation = generation;
            let viewport = (
                request.viewport.size.x / f64::from(request.viewport.scale_factor),
                request.viewport.size.y / f64::from(request.viewport.scale_factor),
            );
            let demands = image_demands(request.camera, viewport, generation);
            request.desired_tiles = demands.iter().map(|demand| demand.key).collect();
            self.demands.extend(demands);
        }
        for overlay in &self.overlays {
            let camera = behaviour.camera(overlay.pane, committed);
            let painter = context
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new(("image-overlays", overlay.pane.0)),
                ))
                .with_clip_rect(overlay.rect);
            paint_image_overlay(
                &painter,
                overlay,
                camera,
                behaviour.camera(PaneId(1), committed),
            );
        }
        for status in &self.statuses {
            let camera = behaviour.camera(status.pane, committed);
            let pointer = status
                .pointer_local
                .map(|pointer| {
                    camera.image_at(
                        pointer,
                        ViewportPoint::new(
                            status.viewport.width() as f64,
                            status.viewport.height() as f64,
                        ),
                    )
                })
                .unwrap_or(status.fallback_pointer);
            let world = ImageToWorld::default().image_to_world(pointer);
            let style = context.style_of(egui::Theme::Dark);
            let painter = context
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new(("image-status", status.pane.0)),
                ))
                .with_clip_rect(status.rect);
            painter.rect_filled(status.rect, 0.0, egui::Color32::from_rgb(19, 23, 27));
            let right_width = 88.0_f32.min(status.rect.width() * 0.35);
            painter
                .with_clip_rect(egui::Rect::from_min_max(
                    status.rect.min,
                    egui::pos2(status.rect.right() - right_width, status.rect.bottom()),
                ))
                .text(
                    status.rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    format!(
                        "image {:>8.1}, {:>8.1}  ·  world {:>10.1}, {:>10.1}",
                        pointer.x, pointer.y, world.x, world.y
                    ),
                    egui::TextStyle::Monospace.resolve(&style),
                    style.visuals.text_color(),
                );
            let tile_count = self
                .render_plan
                .images
                .iter()
                .find(|request| request.pane == status.pane)
                .map_or(0, |request| request.desired_tiles.len());
            painter
                .with_clip_rect(egui::Rect::from_min_max(
                    egui::pos2(status.rect.right() - right_width, status.rect.top()),
                    status.rect.max,
                ))
                .text(
                    status.rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    format!(
                        "L{} · {} tiles",
                        camera.pixels_per_screen_point.log2().round().max(0.0) as u8,
                        tile_count
                    ),
                    egui::TextStyle::Body.resolve(&style),
                    style.visuals.text_color(),
                );
        }
    }
}

#[derive(Default)]
pub struct UiBehaviour {
    camera_drags: BTreeMap<PaneId, CameraGesture>,
    camera_wheels: BTreeMap<PaneId, CameraGesture>,
    frame_camera_overrides: BTreeMap<PaneId, Camera>,
    pointer_image: BTreeMap<PaneId, ImagePoint>,
}

struct CameraGesture {
    before: Vec<CameraState>,
    preview: Vec<CameraState>,
    last_input: Instant,
}

struct ImageFrame {
    camera: Camera,
    demands: Vec<TileDemand>,
}

const WHEEL_GESTURE_IDLE: Duration = Duration::from_millis(140);

impl UiBehaviour {
    pub fn begin_frame(&mut self) {
        self.frame_camera_overrides.clear();
    }

    fn camera(&self, pane: PaneId, committed: &[CameraState]) -> Camera {
        self.frame_camera_overrides
            .get(&pane)
            .copied()
            .or_else(|| {
                self.camera_drags
                    .values()
                    .chain(self.camera_wheels.values())
                    .find_map(|gesture| {
                        gesture
                            .preview
                            .iter()
                            .find(|state| state.pane == pane)
                            .map(|state| state.camera)
                    })
            })
            .or_else(|| {
                committed
                    .iter()
                    .find(|state| state.pane == pane)
                    .map(|state| state.camera)
            })
            .unwrap_or_default()
    }

    fn camera_states(&self, committed: &[CameraState]) -> Vec<CameraState> {
        let mut states = committed.to_vec();
        for state in &mut states {
            if let Some(camera) = self.frame_camera_overrides.get(&state.pane) {
                state.camera = *camera;
            }
        }
        states
    }

    fn expose_preview(&mut self, preview: &[CameraState]) {
        self.frame_camera_overrides
            .extend(preview.iter().map(|state| (state.pane, state.camera)));
    }
}

pub struct PaneSurface<'a> {
    pub document: &'a Document,
    pub cameras: &'a [CameraState],
    pub active_tools: &'a mut BTreeMap<PaneId, ActiveTool>,
    pub gesture: &'a mut Option<GesturePreview>,
    pub selected_result: &'a mut Option<ResultId>,
    pub selected_annotation: &'a mut Option<AnnotationId>,
    pub ui_behaviour: &'a mut UiBehaviour,
    pub display: &'a mut BTreeMap<PaneId, DisplaySettings>,
    pub diagnostics: &'a DiagnosticsSnapshot,
    pub virtualisation: &'a mut VirtualisationMetrics,
    pub thumbnail_cache: &'a mut ThumbnailCache,
    pub generation: u64,
    pub frame_number: u64,
    pub active_pane: PaneId,
    pub outputs: &'a mut FrameOutput,
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

    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: PaneId, pane_rect: egui::Rect) {
        ui.push_id(("window", 1_u32, "pane", pane.0), |ui| match pane.0 {
            1..=4 => self.image_pane(ui, pane, pane_rect),
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
    fn image_pane(&mut self, ui: &mut egui::Ui, pane: PaneId, pane_rect: egui::Rect) {
        let camera_index = self
            .cameras
            .iter()
            .position(|state| state.pane == pane)
            .expect("validated session has one camera per image pane");
        let mut fit = false;
        let mut commit_polygon = false;
        let mut delete_annotation = false;
        let display = self.display.entry(pane).or_default();
        ui.horizontal(|ui| {
            if pane.0 <= 2 {
                for (tool, label) in [(ActiveTool::Navigate, "Navigate"), (ActiveTool::Polygon, "Polygon"), (ActiveTool::EditVertex, "Edit")] {
                    if ui.selectable_label(self.active_tools.get(&pane) == Some(&tool), label).clicked() { self.active_tools.insert(pane, tool); }
                }
            }
            fit = ui.small_button("Fit").clicked();
            let mut linked = self.cameras[camera_index].link.is_some();
            if ui.checkbox(&mut linked, "Link A").changed() { self.outputs.intents.push(ImageIntent::SetCameraLink { pane, link: linked.then_some(LinkGroupId(1)) }); }
            egui::ComboBox::from_id_salt((pane.0, "map")).selected_text(match display.map { DisplayMap::Viridis => "Viridis", DisplayMap::Greyscale => "Greyscale", DisplayMap::Threshold => "Threshold" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut display.map, DisplayMap::Viridis, "Viridis");
                ui.selectable_value(&mut display.map, DisplayMap::Greyscale, "Greyscale");
                ui.selectable_value(&mut display.map, DisplayMap::Threshold, "Threshold");
            });
            ui.add(egui::Slider::new(&mut display.window_low, 0.0..=0.8).show_value(false).text("low"));
            ui.add(egui::Slider::new(&mut display.window_high, 0.2..=1.0).show_value(false).text("high"));
            if matches!(self.gesture.as_ref(), Some(GesturePreview::Polygon { vertices, .. }) if vertices.len() >= 3) {
                commit_polygon = ui.small_button("Commit polygon").clicked();
            }
            if self.selected_annotation.is_some() { delete_annotation = ui.small_button("Delete").clicked(); }
        });
        let display = *display;
        if fit {
            let size = ui.available_size();
            self.outputs.intents.push(ImageIntent::SetCamera {
                pane,
                camera: Camera::fit(size.x as f64, size.y as f64),
            });
        }
        if commit_polygon {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.gesture.take() {
                self.outputs
                    .intents
                    .push(ImageIntent::CommitPolygon { layer, vertices });
            }
        }
        if self.active_pane == pane && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.gesture.take() {
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
            if let Some(annotation) = *self.selected_annotation {
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
            gesture: self.gesture.clone(),
            selected_annotation: *self.selected_annotation,
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

    fn handle_camera(
        &mut self,
        ui: &egui::Ui,
        pane: PaneId,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let tool = self
            .active_tools
            .get(&pane)
            .copied()
            .unwrap_or(ActiveTool::Navigate);
        if tool != ActiveTool::Navigate {
            return;
        }
        let now = Instant::now();
        if let Some(gesture) = self.ui_behaviour.camera_wheels.get(&pane) {
            let elapsed = now.duration_since(gesture.last_input);
            if elapsed >= WHEEL_GESTURE_IDLE {
                let gesture = self.ui_behaviour.camera_wheels.remove(&pane).unwrap();
                let after = gesture
                    .preview
                    .iter()
                    .find(|state| state.pane == pane)
                    .map(|state| state.camera)
                    .unwrap_or_default();
                self.ui_behaviour.expose_preview(&gesture.preview);
                self.outputs.commands.push(Command::SetCameras {
                    changes: linked_camera_changes(&gesture.before, pane, after),
                });
            } else {
                self.outputs.repaint_after = Some(
                    self.outputs
                        .repaint_after
                        .map_or(WHEEL_GESTURE_IDLE - elapsed, |scheduled| {
                            scheduled.min(WHEEL_GESTURE_IDLE - elapsed)
                        }),
                );
            }
        }
        if response.drag_started() {
            let before = self.ui_behaviour.camera_wheels.remove(&pane).map_or_else(
                || self.ui_behaviour.camera_states(self.cameras),
                |gesture| gesture.preview,
            );
            self.ui_behaviour.camera_drags.insert(
                pane,
                CameraGesture {
                    preview: before.clone(),
                    before,
                    last_input: now,
                },
            );
        }
        if response.dragged() {
            if let Some(gesture) = self.ui_behaviour.camera_drags.get_mut(&pane) {
                let Some(source) = gesture.before.iter().find(|state| state.pane == pane) else {
                    return;
                };
                let mut preview = source.camera;
                preview.pan(ViewportPoint::new(
                    response.drag_delta().x as f64,
                    response.drag_delta().y as f64,
                ));
                let changes = linked_camera_changes(&gesture.before, pane, preview);
                gesture.preview = gesture.before.clone();
                apply_camera_changes(&mut gesture.preview, &changes, true);
                gesture.last_input = now;
                let preview = gesture.preview.clone();
                self.ui_behaviour.expose_preview(&preview);
                self.outputs.interaction_active = true;
            }
        }
        if response.drag_stopped() {
            if let Some(gesture) = self.ui_behaviour.camera_drags.remove(&pane) {
                let after = gesture
                    .preview
                    .iter()
                    .find(|state| state.pane == pane)
                    .map(|state| state.camera)
                    .unwrap_or_default();
                self.ui_behaviour.expose_preview(&gesture.preview);
                self.outputs.commands.push(Command::SetCameras {
                    changes: linked_camera_changes(&gesture.before, pane, after),
                });
            }
        }
        if response.hovered() {
            let zoom_delta = ui.input(|input| input.smooth_scroll_delta.y);
            if zoom_delta.abs() > 0.01 {
                let current = self.ui_behaviour.camera_wheels.get(&pane).map_or_else(
                    || self.ui_behaviour.camera_states(self.cameras),
                    |gesture| gesture.preview.clone(),
                );
                let before = current
                    .iter()
                    .find(|state| state.pane == pane)
                    .map(|state| state.camera)
                    .unwrap_or_default();
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
                let changes = linked_camera_changes(&current, pane, after);
                let gesture = self
                    .ui_behaviour
                    .camera_wheels
                    .entry(pane)
                    .or_insert_with(|| CameraGesture {
                        before: current.clone(),
                        preview: current.clone(),
                        last_input: now,
                    });
                gesture.preview = current;
                apply_camera_changes(&mut gesture.preview, &changes, true);
                gesture.last_input = now;
                let preview = gesture.preview.clone();
                self.ui_behaviour.expose_preview(&preview);
                self.outputs.interaction_active = true;
                self.outputs.repaint_after = Some(
                    self.outputs
                        .repaint_after
                        .map_or(WHEEL_GESTURE_IDLE, |scheduled| {
                            scheduled.min(WHEEL_GESTURE_IDLE)
                        }),
                );
            }
        }
    }

    fn handle_annotations(
        &mut self,
        pane: PaneId,
        camera: Camera,
        rect: egui::Rect,
        response: &egui::Response,
    ) {
        let to_screen = |world: WorldPoint| {
            let image = ImageToWorld::default().world_to_image(world);
            egui::pos2(
                rect.center().x
                    + ((image.x - camera.centre.x) / camera.pixels_per_screen_point) as f32,
                rect.center().y
                    + ((image.y - camera.centre.y) / camera.pixels_per_screen_point) as f32,
            )
        };
        let tool = self
            .active_tools
            .get(&pane)
            .copied()
            .unwrap_or(ActiveTool::Navigate);
        if tool == ActiveTool::Polygon && response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let image = screen_to_image(pointer, rect, camera);
                let world = ImageToWorld::default().image_to_world(image);
                match self.gesture.as_mut() {
                    Some(GesturePreview::Polygon { vertices, .. }) => vertices.push(world),
                    _ => {
                        *self.gesture = Some(GesturePreview::Polygon {
                            layer: LayerId(1),
                            vertices: vec![world],
                        })
                    }
                }
            }
            if response.double_clicked() {
                if matches!(self.gesture.as_ref(), Some(GesturePreview::Polygon { vertices, .. }) if vertices.len() >= 3)
                {
                    if let Some(GesturePreview::Polygon { layer, vertices }) = self.gesture.take() {
                        self.outputs
                            .intents
                            .push(ImageIntent::CommitPolygon { layer, vertices });
                    }
                }
            }
        }
        if tool == ActiveTool::Polygon && response.secondary_clicked() {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.gesture.take() {
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
                        *self.selected_annotation = Some(annotation);
                        *self.gesture = Some(GesturePreview::Vertex {
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
                    (response.interact_pointer_pos(), self.gesture.as_mut())
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
                }) = self.gesture.take()
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

    fn results_pane(&mut self, ui: &mut egui::Ui) {
        results::show(ui, *self.selected_result, self.virtualisation, self.outputs);
    }

    fn thumbnails_pane(&mut self, ui: &mut egui::Ui) {
        thumbnails::show(
            ui,
            *self.selected_result,
            self.generation,
            self.thumbnail_cache,
            self.virtualisation,
            self.outputs,
        );
    }

    fn inspector_pane(&mut self, ui: &mut egui::Ui) {
        inspector::show(
            ui,
            *self.selected_result,
            *self.selected_annotation,
            self.outputs,
        );
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
                        "Callback passes / draws / returned buffers",
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

fn derive_image_frame(
    behaviour: &UiBehaviour,
    pane: PaneId,
    cameras: &[CameraState],
    viewport: (f64, f64),
    generation: u64,
) -> ImageFrame {
    let camera = behaviour.camera(pane, cameras);
    let demands = image_demands(camera, viewport, generation);
    ImageFrame { camera, demands }
}

fn image_demands(camera: Camera, viewport: (f64, f64), generation: u64) -> Vec<TileDemand> {
    let mut demands = visible_tile_demands(camera, viewport, SourceId(1), generation, true);
    demands.push(TileDemand {
        key: TileKey {
            source: SourceId(1),
            level: PYRAMID_LEVELS - 1,
            x: 0,
            y: 0,
        },
        priority: DemandPriority::Visible,
        generation,
    });
    demands
}

fn paint_image_overlay(
    painter: &egui::Painter,
    overlay: &ImageOverlayRequest,
    camera: Camera,
    primary_camera: Camera,
) {
    let to_screen = |world: WorldPoint| {
        let image = ImageToWorld::default().world_to_image(world);
        egui::pos2(
            overlay.rect.center().x
                + ((image.x - camera.centre.x) / camera.pixels_per_screen_point) as f32,
            overlay.rect.center().y
                + ((image.y - camera.centre.y) / camera.pixels_per_screen_point) as f32,
        )
    };
    for polygon in &overlay.annotations {
        let mut points: Vec<_> = polygon.vertices.iter().copied().map(to_screen).collect();
        if let Some(GesturePreview::Vertex {
            annotation,
            vertex,
            preview,
            ..
        }) = &overlay.gesture
            && *annotation == polygon.id
            && *vertex < points.len()
        {
            points[*vertex] = to_screen(*preview);
        }
        if points.len() > 1 {
            points.push(points[0]);
            painter.add(egui::Shape::line(
                points.clone(),
                egui::Stroke::new(
                    2.0,
                    if overlay.selected_annotation == Some(polygon.id) {
                        egui::Color32::from_rgb(255, 196, 74)
                    } else {
                        egui::Color32::from_rgb(105, 227, 210)
                    },
                ),
            ));
            for point in points.iter().take(points.len() - 1) {
                painter.circle_filled(*point, 3.5, egui::Color32::from_rgb(240, 244, 238));
            }
        }
    }
    if let Some(GesturePreview::Polygon { vertices, .. }) = &overlay.gesture {
        let mut points: Vec<_> = vertices.iter().copied().map(to_screen).collect();
        if let Some(pointer) = overlay.hover {
            points.push(pointer);
        }
        if points.len() > 1 {
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 181, 64)),
            ));
        }
    }
    if overlay.pane == PaneId(3) {
        let centre = egui::pos2(
            overlay.rect.center().x
                + ((primary_camera.centre.x - camera.centre.x) / camera.pixels_per_screen_point)
                    as f32,
            overlay.rect.center().y
                + ((primary_camera.centre.y - camera.centre.y) / camera.pixels_per_screen_point)
                    as f32,
        );
        let size = egui::vec2(
            160.0 / camera.pixels_per_screen_point as f32
                * primary_camera.pixels_per_screen_point as f32,
            100.0 / camera.pixels_per_screen_point as f32
                * primary_camera.pixels_per_screen_point as f32,
        )
        .max(egui::vec2(20.0, 14.0));
        painter.rect_stroke(
            egui::Rect::from_center_size(centre, size),
            1.0,
            egui::Stroke::new(2.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gesture_preview_is_the_single_camera_for_frame_demand_and_presentation() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let mut preview = committed.clone();
        let expected = Camera {
            centre: ImagePoint::new(8_192.0, 4_096.0),
            pixels_per_screen_point: 4.0,
        };
        let changes = linked_camera_changes(&committed, PaneId(1), expected);
        apply_camera_changes(&mut preview, &changes, true);
        behaviour.expose_preview(&preview);

        let frame = derive_image_frame(&behaviour, PaneId(1), &committed, (640.0, 480.0), 7);
        assert_eq!(frame.camera, expected);
        assert!(frame.demands.iter().all(|demand| demand.generation == 7));
        assert!(frame.demands.iter().any(|demand| demand.key.level == 2));
    }

    #[test]
    fn final_frame_plan_reconciles_an_interaction_from_a_later_linked_pane() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let expected = Camera {
            centre: ImagePoint::new(12_000.0, 24_000.0),
            pixels_per_screen_point: 8.0,
        };
        let changes = linked_camera_changes(&committed, PaneId(2), expected);
        let mut preview = committed.clone();
        apply_camera_changes(&mut preview, &changes, true);
        behaviour.expose_preview(&preview);

        let mut output = FrameOutput::default();
        for pane in [PaneId(1), PaneId(2)] {
            output.render_plan.submit(ImageRenderRequest {
                pane,
                source: SourceId(1),
                source_generation: 9,
                viewport: PhysicalViewport {
                    origin: PhysicalPoint::new(0.0, 0.0),
                    size: PhysicalPoint::new(640.0, 480.0),
                    scale_factor: 1.0,
                },
                camera: committed
                    .iter()
                    .find(|state| state.pane == pane)
                    .unwrap()
                    .camera,
                display: DisplaySettings::default(),
                desired_tiles: Vec::new(),
            });
        }
        output.finalise_camera_previews(&egui::Context::default(), &behaviour, &committed, 9);

        assert!(
            output
                .render_plan
                .images
                .iter()
                .all(|request| request.camera == expected && !request.desired_tiles.is_empty())
        );
        assert!(
            output
                .demands
                .iter()
                .all(|demand| demand.key.source == SourceId(1) && demand.generation == 9)
        );
    }
}
