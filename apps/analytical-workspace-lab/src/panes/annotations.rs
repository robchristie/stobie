use super::*;

impl PaneSurface<'_> {
    pub(super) fn handle_annotations(
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
                match self.annotation_ui.get_mut() {
                    Some(GesturePreview::Polygon { vertices, .. }) => vertices.push(world),
                    _ => self.annotation_ui.set(GesturePreview::Polygon {
                        layer: LayerId(1),
                        vertices: vec![world],
                    }),
                }
            }
            if response.double_clicked() {
                if matches!(self.annotation_ui.get(), Some(GesturePreview::Polygon { vertices, .. }) if vertices.len() >= 3)
                {
                    if let Some(GesturePreview::Polygon { layer, vertices }) =
                        self.annotation_ui.take()
                    {
                        self.outputs
                            .intents
                            .push(ImageIntent::CommitPolygon { layer, vertices });
                    }
                }
            }
        }
        if tool == ActiveTool::Polygon && response.secondary_clicked() {
            if let Some(GesturePreview::Polygon { layer, vertices }) = self.annotation_ui.take() {
                if vertices.len() >= 3 {
                    self.outputs
                        .intents
                        .push(ImageIntent::CommitPolygon { layer, vertices });
                }
            }
        }
        if tool == ActiveTool::EditVertex {
            if let Some(pointer) = drag_start_pointer_sample(response) {
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
                    self.selected_annotation = Some(annotation);
                    self.outputs
                        .pane_intents
                        .push(PaneIntent::SelectAnnotation(Some(annotation)));
                    self.annotation_ui.set(GesturePreview::Vertex {
                        annotation,
                        vertex,
                        original,
                        preview: original,
                    });
                }
            }
            if let Some(pointer) = camera_gestures::drag_pointer_sample(response) {
                if let Some(GesturePreview::Vertex { preview, .. }) = self.annotation_ui.get_mut() {
                    let pointer = egui::pos2(pointer.x as f32, pointer.y as f32);
                    *preview = ImageToWorld::default()
                        .image_to_world(screen_to_image(pointer, rect, camera));
                    self.outputs.interaction_active = true;
                }
            }
            if response.drag_stopped() {
                if let Some(gesture @ GesturePreview::Vertex { .. }) = self.annotation_ui.take() {
                    self.outputs.finish_vertex_drag(gesture);
                }
            }
        }
    }
}

pub(super) fn drag_start_pointer_sample(response: &egui::Response) -> Option<egui::Pos2> {
    response
        .drag_started()
        .then(|| {
            response
                .interact_pointer_pos()
                .map(|pointer| pointer - response.drag_delta())
        })
        .flatten()
}

pub(super) fn paint_image_overlay(
    painter: &egui::Painter,
    overlay: &ImageOverlayRequest,
    gesture: Option<&GesturePreview>,
    camera: Camera,
    primary_camera: Camera,
    tokens: &DesignTokens,
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
        }) = gesture
            && *annotation == polygon.id
            && *vertex < points.len()
        {
            points[*vertex] = to_screen(*preview);
        }
        if points.len() > 1 {
            points.push(points[0]);
            let colour = if overlay.selected_annotation == Some(polygon.id) {
                tokens.colours.overlay_selected
            } else {
                tokens.colours.overlay_annotation
            };
            painter.add(egui::Shape::line(
                points.clone(),
                egui::Stroke::new(tokens.spacing.unit.0 * 0.5, colour),
            ));
            for point in points.iter().take(points.len() - 1) {
                painter.circle_filled(
                    *point,
                    tokens.spacing.unit.0 * 0.875,
                    tokens.colours.overlay_vertex,
                );
            }
        }
    }
    if let Some(GesturePreview::Polygon { vertices, .. }) = gesture {
        let mut points: Vec<_> = vertices.iter().copied().map(to_screen).collect();
        if let Some(pointer) = overlay.hover {
            points.push(pointer);
        }
        if points.len() > 1 {
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(tokens.spacing.unit.0 * 0.5, tokens.colours.overlay_selected),
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
            egui::Stroke::new(
                tokens.spacing.unit.0 * 0.5,
                tokens.colours.overlay_footprint,
            ),
            egui::StrokeKind::Inside,
        );
    }
}

pub(super) fn screen_to_image(point: egui::Pos2, rect: egui::Rect, camera: Camera) -> ImagePoint {
    camera.image_at(
        ViewportPoint::new(
            (point.x - rect.left()) as f64,
            (point.y - rect.top()) as f64,
        ),
        ViewportPoint::new(rect.width() as f64, rect.height() as f64),
    )
}
