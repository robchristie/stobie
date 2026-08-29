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
        let toolbar = ui.horizontal_wrapped(|ui| {
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
            if !compact_toolbar {
                present_display_controls(
                    ui,
                    pane,
                    &toolbar_id,
                    &mut display,
                    &self.tokens,
                    self.outputs,
                );
            } else {
                let target = ActionTarget::pane(ActionId::DisplaySettings, pane);
                let control_availability = availability(ActionId::DisplaySettings, action_context);
                let response = action_button(
                    ui,
                    ActionButtonSpec {
                        target,
                        availability: control_availability.clone(),
                        selected: false,
                        emphasis: ActionEmphasis::Quiet,
                        compact: true,
                    },
                    &self.tokens,
                    self.font_scale,
                    &mut self.outputs.ui_geometry.text_layouts,
                );
                let inside_root = self
                    .outputs
                    .ui_geometry
                    .root
                    .is_some_and(|root| root.contains(response.rect.into(), 1.0));
                if inside_root && response.rect.intersects(ui.clip_rect()) {
                    self.outputs
                        .ui_geometry
                        .control(Some(pane), "display_settings", response.rect);
                    self.outputs.ui_geometry.action(
                        toolbar_id.clone(),
                        target,
                        &control_availability,
                        false,
                        &response,
                    );
                }
                let _ = egui::Popup::menu(&response).show(|ui| {
                    ui.set_width(self.tokens.geometry.minimum_hit_size.0 * 7.0);
                    ui.vertical(|ui| {
                        present_display_controls(
                            ui,
                            pane,
                            &toolbar_id,
                            &mut display,
                            &self.tokens,
                            self.outputs,
                        );
                    });
                });
            }
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

        let status_height = image_status_height(&self.tokens, self.font_scale);
        let available = ui.available_rect_before_wrap().intersect(pane_rect);
        let desired = egui::vec2(
            available.width(),
            (available.height() - status_height).max(self.tokens.geometry.minimum_hit_size.0 * 2.0),
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
            .rect_filled(rect, 0.0, self.tokens.colours.surface_canvas);
        paint_placeholder(ui, rect, pane, &self.tokens);

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
            tokens: self.tokens,
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
            tokens: self.tokens,
            font_scale: self.font_scale,
        });
        let mut status_node = UiNode::container(
            SemanticUiId::new(format!("pane.{}.image_status", pane.0)),
            Some(SemanticUiId::pane(pane)),
            UiRole::Status,
            status_rect.into(),
        );
        status_node.name = "Image coordinates and tile level".into();
        status_node.pane = Some(pane);
        status_node.domain_reference = Some(DomainReference::Pane(pane));
        self.outputs.ui_geometry.record_node(status_node);
    }
}

fn present_display_controls(
    ui: &mut egui::Ui,
    pane: PaneId,
    parent: &SemanticUiId,
    display: &mut DisplaySettings,
    tokens: &DesignTokens,
    outputs: &mut FrameOutput,
) {
    normalise_display_window(display);
    let map = choice_control(
        ui,
        SemanticUiId::new(format!("pane.{}.display_map", pane.0)),
        parent.clone(),
        "Display map",
        &mut display.map,
        &[
            (DisplayMap::Viridis, "Viridis"),
            (DisplayMap::Greyscale, "Greyscale"),
            (DisplayMap::Threshold, "Threshold"),
        ],
        ActionId::DisplaySettings,
        tokens,
    );
    record_display_control(ui, pane, map, outputs);
    let low = range_control(
        ui,
        SemanticUiId::new(format!("pane.{}.display_low", pane.0)),
        parent.clone(),
        "Low",
        &mut display.window_low,
        low_window_range(display.window_high),
        ActionId::DisplaySettings,
        tokens,
    );
    record_display_control(ui, pane, low, outputs);
    let high = range_control(
        ui,
        SemanticUiId::new(format!("pane.{}.display_high", pane.0)),
        parent.clone(),
        "High",
        &mut display.window_high,
        high_window_range(display.window_low),
        ActionId::DisplaySettings,
        tokens,
    );
    record_display_control(ui, pane, high, outputs);
}

const DISPLAY_WINDOW_GAP: f32 = 0.01;

fn normalise_display_window(display: &mut DisplaySettings) {
    display.window_high = if display.window_high.is_finite() {
        display.window_high.clamp(DISPLAY_WINDOW_GAP, 1.0)
    } else {
        DisplaySettings::default().window_high
    };
    display.window_low = if display.window_low.is_finite() {
        display
            .window_low
            .clamp(0.0, display.window_high - DISPLAY_WINDOW_GAP)
    } else {
        DisplaySettings::default().window_low
    };
}

fn low_window_range(high: f32) -> std::ops::RangeInclusive<f32> {
    0.0..=(high - DISPLAY_WINDOW_GAP).max(0.0)
}

fn high_window_range(low: f32) -> std::ops::RangeInclusive<f32> {
    (low + DISPLAY_WINDOW_GAP).min(1.0)..=1.0
}

fn record_display_control(
    ui: &egui::Ui,
    pane: PaneId,
    mut control: polyorama_ui_egui::SemanticControlOutput,
    outputs: &mut FrameOutput,
) {
    let inside_root = outputs
        .ui_geometry
        .root
        .is_some_and(|root| root.contains(control.response.rect.into(), 1.0));
    if inside_root && control.response.rect.intersects(ui.clip_rect()) {
        control.node.pane = Some(pane);
        control.node.domain_reference = Some(DomainReference::Pane(pane));
        outputs.ui_geometry.record_node(control.node);
    }
}

fn paint_placeholder(ui: &egui::Ui, rect: egui::Rect, pane: PaneId, tokens: &DesignTokens) {
    let cell = tokens.spacing.unit.0 * 5.0;
    for row in 0..(rect.height() / cell).ceil() as usize {
        for column in 0..(rect.width() / cell).ceil() as usize {
            if (row + column + pane.0 as usize).is_multiple_of(2) {
                let min = rect.min + egui::vec2(column as f32 * cell, row as f32 * cell);
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(min, egui::vec2(cell, cell)).intersect(rect),
                    0.0,
                    tokens.colours.surface_panel,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_window_controls_always_retain_a_strict_ordering() {
        for mut display in [
            DisplaySettings {
                window_low: 0.9,
                window_high: 0.1,
                ..DisplaySettings::default()
            },
            DisplaySettings {
                window_low: f32::INFINITY,
                window_high: f32::NAN,
                ..DisplaySettings::default()
            },
        ] {
            normalise_display_window(&mut display);
            assert!((0.0..display.window_high).contains(&display.window_low));
            assert!(display.window_high <= 1.0);
            assert!(low_window_range(display.window_high).contains(&display.window_low));
            assert!(high_window_range(display.window_low).contains(&display.window_high));
        }
    }
}
