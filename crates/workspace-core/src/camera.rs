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
}
