use egui::Rect;
use serde::{Deserialize, Serialize};

/// Horizontal pane behaviour from the design-language breakpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneWidthClass {
    Narrow,
    Regular,
    Wide,
}

impl PaneWidthClass {
    pub fn from_points(width: f32) -> Self {
        if width < 360.0 {
            Self::Narrow
        } else if width < 720.0 {
            Self::Regular
        } else {
            Self::Wide
        }
    }
}

/// Vertical pane behaviour from the design-language breakpoints.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneHeightClass {
    Shallow,
    Regular,
    Tall,
}

impl PaneHeightClass {
    pub fn from_points(height: f32) -> Self {
        if height < 280.0 {
            Self::Shallow
        } else if height < 600.0 {
            Self::Regular
        } else {
            Self::Tall
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PaneSizeClass {
    pub width: PaneWidthClass,
    pub height: PaneHeightClass,
}

impl PaneSizeClass {
    pub fn for_rect(rect: Rect) -> Self {
        Self {
            width: PaneWidthClass::from_points(rect.width()),
            height: PaneHeightClass::from_points(rect.height()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_classes_use_the_documented_inclusive_breakpoints() {
        assert_eq!(PaneWidthClass::from_points(359.99), PaneWidthClass::Narrow);
        assert_eq!(PaneWidthClass::from_points(360.0), PaneWidthClass::Regular);
        assert_eq!(PaneWidthClass::from_points(719.99), PaneWidthClass::Regular);
        assert_eq!(PaneWidthClass::from_points(720.0), PaneWidthClass::Wide);
        assert_eq!(
            PaneHeightClass::from_points(279.99),
            PaneHeightClass::Shallow
        );
        assert_eq!(
            PaneHeightClass::from_points(280.0),
            PaneHeightClass::Regular
        );
        assert_eq!(
            PaneHeightClass::from_points(599.99),
            PaneHeightClass::Regular
        );
        assert_eq!(PaneHeightClass::from_points(600.0), PaneHeightClass::Tall);
    }
}
