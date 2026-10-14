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
