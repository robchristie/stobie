use serde::{Deserialize, Serialize};

macro_rules! point_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct $name {
            pub x: f64,
            pub y: f64,
        }

        impl $name {
            pub const fn new(x: f64, y: f64) -> Self {
                Self { x, y }
            }
        }
    };
}

point_type!(LogicalPoint);
point_type!(PhysicalPoint);
point_type!(ViewportPoint);
point_type!(ImagePoint);
point_type!(WorldPoint);

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageToWorld {
    pub scale: WorldPoint,
    pub offset: WorldPoint,
}

impl Default for ImageToWorld {
    fn default() -> Self {
        Self {
            scale: WorldPoint::new(2.0, -2.0),
            offset: WorldPoint::new(500_000.0, 7_000_000.0),
        }
    }
}

impl ImageToWorld {
    pub fn image_to_world(self, point: ImagePoint) -> WorldPoint {
        WorldPoint::new(
            point.x * self.scale.x + self.offset.x,
            point.y * self.scale.y + self.offset.y,
        )
    }

    pub fn world_to_image(self, point: WorldPoint) -> ImagePoint {
        ImagePoint::new(
            (point.x - self.offset.x) / self.scale.x,
            (point.y - self.offset.y) / self.scale.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_coordinate_transform_round_trips() {
        let transform = ImageToWorld::default();
        let point = ImagePoint::new(1234.5, 6789.25);
        let restored = transform.world_to_image(transform.image_to_world(point));
        assert!((restored.x - point.x).abs() < f64::EPSILON);
        assert!((restored.y - point.y).abs() < f64::EPSILON);
    }
}
