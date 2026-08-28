use serde::{Deserialize, Serialize};

use crate::{ImagePoint, LinkGroupId, PaneId, ViewportPoint};

pub const RASTER_SIZE: f64 = 131_072.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub centre: ImagePoint,
    pub pixels_per_screen_point: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            centre: ImagePoint::new(RASTER_SIZE / 2.0, RASTER_SIZE / 2.0),
            pixels_per_screen_point: 256.0,
        }
    }
}

impl Camera {
    pub fn fit(viewport_width: f64, viewport_height: f64) -> Self {
        let extent = viewport_width.max(viewport_height).max(1.0);
        Self {
            pixels_per_screen_point: RASTER_SIZE / extent,
            ..Self::default()
        }
    }

    pub fn image_at(self, point: ViewportPoint, viewport: ViewportPoint) -> ImagePoint {
        ImagePoint::new(
            self.centre.x + (point.x - viewport.x * 0.5) * self.pixels_per_screen_point,
            self.centre.y + (point.y - viewport.y * 0.5) * self.pixels_per_screen_point,
        )
    }

    pub fn pan(&mut self, delta: ViewportPoint) {
        self.centre.x -= delta.x * self.pixels_per_screen_point;
        self.centre.y -= delta.y * self.pixels_per_screen_point;
    }

    pub fn zoom_around(&mut self, factor: f64, pointer: ViewportPoint, viewport: ViewportPoint) {
        let before = self.image_at(pointer, viewport);
        self.pixels_per_screen_point = (self.pixels_per_screen_point * factor).clamp(0.25, 1024.0);
        let after = self.image_at(pointer, viewport);
        self.centre.x += before.x - after.x;
        self.centre.y += before.y - after.y;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraState {
    pub pane: PaneId,
    pub camera: Camera,
    pub link: Option<LinkGroupId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraChange {
    pub pane: PaneId,
    pub before: Camera,
    pub after: Camera,
}

/// A coalesced camera drag built from the total physical pointer displacement.
///
/// Presentation integrations report displacement from the press origin. This
/// session always derives the preview from the exact starting cameras so linked
/// previews and undo retain their original values without floating-point drift
/// from repeated camera mutation or loss at the drag-recognition threshold.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraDragSession {
    source: PaneId,
    before: Vec<CameraState>,
    preview: Vec<CameraState>,
    pointer_origin: Option<ViewportPoint>,
    total_delta: ViewportPoint,
}

impl CameraDragSession {
    pub fn new(source: PaneId, before: Vec<CameraState>) -> Option<Self> {
        before
            .iter()
            .any(|state| state.pane == source)
            .then(|| Self {
                source,
                preview: before.clone(),
                before,
                pointer_origin: None,
                total_delta: ViewportPoint::default(),
            })
    }

    pub fn new_at(
        source: PaneId,
        before: Vec<CameraState>,
        pointer_origin: ViewportPoint,
    ) -> Option<Self> {
        Self::new(source, before).map(|mut session| {
            session.pointer_origin = Some(pointer_origin);
            session
        })
    }

    pub fn update_pointer(&mut self, pointer: ViewportPoint) -> &[CameraState] {
        let origin = *self.pointer_origin.get_or_insert(pointer);
        self.update_total(ViewportPoint::new(
            pointer.x - origin.x,
            pointer.y - origin.y,
        ))
    }

    pub fn update_total(&mut self, total_delta: ViewportPoint) -> &[CameraState] {
        self.total_delta = total_delta;
        if total_delta == ViewportPoint::default() {
            self.preview.clone_from(&self.before);
            return &self.preview;
        }
        let mut after = self
            .before
            .iter()
            .find(|state| state.pane == self.source)
            .expect("camera drag source was validated at construction")
            .camera;
        after.pan(self.total_delta);
        let changes = linked_camera_changes(&self.before, self.source, after);
        self.preview.clone_from(&self.before);
        apply_camera_changes(&mut self.preview, &changes, true);
        &self.preview
    }

    pub fn preview(&self) -> &[CameraState] {
        &self.preview
    }

    pub fn total_delta(&self) -> ViewportPoint {
        self.total_delta
    }

    pub fn finish(self) -> Vec<CameraChange> {
        if self.total_delta == ViewportPoint::default() {
            return Vec::new();
        }
        let after = self
            .preview
            .iter()
            .find(|state| state.pane == self.source)
            .expect("camera drag source was validated at construction")
            .camera;
        linked_camera_changes(&self.before, self.source, after)
            .into_iter()
            .filter(|change| change.before != change.after)
            .collect()
    }
}

pub fn linked_camera_changes(
    cameras: &[CameraState],
    source: PaneId,
    updated: Camera,
) -> Vec<CameraChange> {
    let group = cameras
        .iter()
        .find(|entry| entry.pane == source)
        .and_then(|entry| entry.link);
    cameras
        .iter()
        .filter(|entry| entry.pane == source || (group.is_some() && entry.link == group))
        .map(|entry| CameraChange {
            pane: entry.pane,
            before: entry.camera,
            after: updated,
        })
        .collect()
}

pub fn apply_camera_changes(cameras: &mut [CameraState], changes: &[CameraChange], after: bool) {
    for change in changes {
        if let Some(entry) = cameras.iter_mut().find(|entry| entry.pane == change.pane) {
            entry.camera = if after { change.after } else { change.before };
        }
    }
}

pub fn propagate_linked_camera(cameras: &mut [CameraState], source: PaneId, updated: Camera) {
    let group = cameras
        .iter()
        .find(|entry| entry.pane == source)
        .and_then(|entry| entry.link);
    for entry in cameras {
        if entry.pane == source || (group.is_some() && entry.link == group) {
            entry.camera = updated;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_camera_propagation_is_explicit() {
        let group = Some(LinkGroupId(1));
        let mut cameras = [
            CameraState {
                pane: PaneId(1),
                camera: Camera::default(),
                link: group,
            },
            CameraState {
                pane: PaneId(2),
                camera: Camera::default(),
                link: group,
            },
            CameraState {
                pane: PaneId(3),
                camera: Camera::default(),
                link: None,
            },
        ];
        let mut changed = Camera::default();
        changed.centre.x = 42.0;
        propagate_linked_camera(&mut cameras, PaneId(1), changed);
        assert_eq!(cameras[1].camera, changed);
        assert_ne!(cameras[2].camera, changed);
    }

    #[test]
    fn linked_camera_changes_capture_exact_original_values() {
        let group = Some(LinkGroupId(1));
        let mut second = Camera::default();
        second.centre.x = 12.0;
        let cameras = [
            CameraState {
                pane: PaneId(1),
                camera: Camera::default(),
                link: group,
            },
            CameraState {
                pane: PaneId(2),
                camera: second,
                link: group,
            },
        ];
        let mut updated = Camera::default();
        updated.centre.x = 42.0;

        let changes = linked_camera_changes(&cameras, PaneId(1), updated);

        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].before, Camera::default());
        assert_eq!(changes[1].before, second);
        assert!(changes.iter().all(|change| change.after == updated));
    }

    #[test]
    fn camera_drag_accumulates_frame_deltas_and_preserves_exact_linked_undo() {
        let group = Some(LinkGroupId(1));
        let first = Camera::default();
        let mut second = first;
        second.centre = ImagePoint::new(12.0, 34.0);
        let before = vec![
            CameraState {
                pane: PaneId(1),
                camera: first,
                link: group,
            },
            CameraState {
                pane: PaneId(2),
                camera: second,
                link: group,
            },
        ];
        let mut drag =
            CameraDragSession::new_at(PaneId(1), before.clone(), ViewportPoint::new(10.0, 20.0))
                .unwrap();

        drag.update_pointer(ViewportPoint::new(40.0, 30.0));
        drag.update_pointer(ViewportPoint::new(65.0, 45.0));
        let preview = drag
            .update_pointer(ViewportPoint::new(100.0, 70.0))
            .to_vec();

        let expected = Camera {
            centre: ImagePoint::new(
                first.centre.x - 90.0 * first.pixels_per_screen_point,
                first.centre.y - 50.0 * first.pixels_per_screen_point,
            ),
            ..first
        };
        assert_eq!(drag.total_delta(), ViewportPoint::new(90.0, 50.0));
        assert!(preview.iter().all(|state| state.camera == expected));

        let changes = drag.finish();
        assert_eq!(changes[0].before, first);
        assert_eq!(changes[1].before, second);
        assert!(changes.iter().all(|change| change.after == expected));

        let mut restored = before.clone();
        apply_camera_changes(&mut restored, &changes, true);
        assert!(restored.iter().all(|state| state.camera == expected));
        apply_camera_changes(&mut restored, &changes, false);
        assert_eq!(restored, before);
    }

    #[test]
    fn camera_drag_returning_to_its_origin_is_an_exact_no_op() {
        let group = Some(LinkGroupId(1));
        let linked = Camera {
            centre: ImagePoint::new(12.0, 34.0),
            ..Camera::default()
        };
        let before = vec![
            CameraState {
                pane: PaneId(1),
                camera: Camera::default(),
                link: group,
            },
            CameraState {
                pane: PaneId(2),
                camera: linked,
                link: group,
            },
        ];
        let origin = ViewportPoint::new(10.0, 20.0);
        let mut drag = CameraDragSession::new_at(PaneId(1), before.clone(), origin).unwrap();

        drag.update_pointer(ViewportPoint::new(100.0, 70.0));
        let preview = drag.update_pointer(origin).to_vec();

        assert_eq!(preview, before);
        assert_eq!(drag.total_delta(), ViewportPoint::default());
        assert!(drag.finish().is_empty());
    }
}
