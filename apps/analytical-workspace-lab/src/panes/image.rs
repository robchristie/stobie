use super::annotations::screen_to_image;
use super::*;
use crate::actions::{ActionContext, availability};

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
        let toolbar_id = SemanticUiId::new(format!("pane.{}.toolbar", pane.0));
        let polygon_vertices = match self.annotation_ui.get() {
            Some(GesturePreview::Polygon { vertices, .. }) => vertices.len(),
            _ => 0,
        };
        let action_context = ActionContext {
            active_pane: self.active_pane,
            target_pane: Some(pane),
            selected_annotation: self.selected_annotation,
            selected_result: self.selected_result,
            polygon_vertices,
            ..Default::default()
        };
        let compact_toolbar = ui.available_width() < 720.0;
        let toolbar = ui.horizontal(|ui| {
            if pane.0 <= 2 {
                for (tool, action, control) in [
                    (ActiveTool::Navigate, ActionId::NavigateTool, "navigate"),
                    (ActiveTool::Polygon, ActionId::PolygonTool, "polygon"),
                    (
                        ActiveTool::EditVertex,
                        ActionId::EditVerticesTool,
                        "edit_vertex",
                    ),
                ] {
                    if present_action(
                        ui,
                        self.outputs,
                        &self.tokens,
                        self.font_scale,
                        &toolbar_id,
                        ActionTarget::pane(action, pane),
                        availability(action, action_context),
                        active_tool == tool,
                        compact_toolbar,
                        self.active_pane == pane,
                        control,
                    ) {
                        active_tool = tool;
                    }
                }
            }
            fit = present_action(
                ui,
                self.outputs,
                &self.tokens,
                self.font_scale,
                &toolbar_id,
                ActionTarget::pane(ActionId::FitView, pane),
                availability(ActionId::FitView, action_context),
                false,
                true,
                self.active_pane == pane,
                "fit",
            );
            let linked = self.cameras[camera_index].link.is_some();
            if present_action(
                ui,
                self.outputs,
                &self.tokens,
                self.font_scale,
                &toolbar_id,
                ActionTarget::pane(ActionId::LinkViews, pane),
                availability(ActionId::LinkViews, action_context),
                linked,
                true,
                self.active_pane == pane,
                "link_a",
            ) {
                self.outputs.intents.push(ImageIntent::SetCameraLink {
                    pane,
                    link: (!linked).then_some(LinkGroupId(1)),
                });
            }
            egui::ComboBox::from_id_salt((pane.0, "map"))
                .selected_text(match display.map {
                    DisplayMap::Viridis => "Viridis",
                    DisplayMap::Greyscale => "Greyscale",
                    DisplayMap::Threshold => "Threshold",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut display.map, DisplayMap::Viridis, "Viridis");
                    ui.selectable_value(&mut display.map, DisplayMap::Greyscale, "Greyscale");
                    ui.selectable_value(&mut display.map, DisplayMap::Threshold, "Threshold");
                });
            ui.add(
                egui::Slider::new(&mut display.window_low, 0.0..=0.8)
                    .show_value(false)
                    .text("low"),
            );
            ui.add(
                egui::Slider::new(&mut display.window_high, 0.2..=1.0)
                    .show_value(false)
                    .text("high"),
            );
            if self.annotation_ui.get().is_some() {
                commit_polygon = present_action(
                    ui,
                    self.outputs,
                    &self.tokens,
                    self.font_scale,
                    &toolbar_id,
                    ActionTarget::pane(ActionId::CommitPolygon, pane),
                    availability(ActionId::CommitPolygon, action_context),
                    false,
                    true,
                    self.active_pane == pane,
                    "commit_polygon",
                );
            }
            if self.selected_annotation.is_some() {
                delete_annotation = present_action(
                    ui,
                    self.outputs,
                    &self.tokens,
                    self.font_scale,
                    &toolbar_id,
                    ActionTarget::pane(ActionId::DeleteAnnotation, pane),
                    availability(ActionId::DeleteAnnotation, action_context),
                    false,
                    true,
                    self.active_pane == pane,
                    "delete_annotation",
                );
            }
        });
        let toolbar_rect = toolbar.response.rect.intersect(pane_rect);
        self.outputs
            .ui_geometry
            .image_toolbars
            .push(crate::ui_geometry::PaneUiRect {
                pane,
                rect: toolbar_rect.into(),
            });
        let mut toolbar_node = UiNode::container(
            toolbar_id,
            Some(SemanticUiId::pane(pane)),
            UiRole::Toolbar,
            toolbar_rect.into(),
        );
        toolbar_node.name = "Image actions".into();
        toolbar_node.pane = Some(pane);
        self.outputs.ui_geometry.record_node(toolbar_node);
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
        if delete_annotation {
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
        self.outputs.ui_geometry.record_node(UiNode {
            id: SemanticUiId::new(format!("pane.{}.viewport", pane.0)),
            parent: Some(SemanticUiId::pane(pane)),
            role: UiRole::Viewport,
            name: format!("{} viewport", self.title(pane)),
            description: Some("Scientific scalar image viewport".into()),
            rect: rect.into(),
            enabled: true,
            focused: response.has_focus(),
            selected: self.active_pane == pane,
            checked: None,
            expanded: None,
            pane: Some(pane),
            domain_reference: Some(DomainReference::Pane(pane)),
            actions: Vec::new(),
            disabled_reason: None,
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
