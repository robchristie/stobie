use egui::{Frame, Margin, Stroke};

use crate::DesignTokens;

/// The isolated application-bar recipe is the first production token
/// consumer. Further component migration belongs to later campaign increments.
pub fn application_bar_frame(tokens: &DesignTokens) -> Frame {
    Frame::new()
        .fill(tokens.colours.surface_panel.into())
        .stroke(Stroke::new(1.0, tokens.colours.border_subtle))
        .inner_margin(Margin::symmetric(
            bounded_margin(tokens.spacing.inline.0),
            0,
        ))
}

fn bounded_margin(points: f32) -> i8 {
    if points.is_finite() {
        points.round().clamp(0.0, f32::from(i8::MAX)) as i8
    } else {
        0
    }
}

pub fn application_bar_height(tokens: &DesignTokens, font_scale: f32) -> f32 {
    (tokens.geometry.application_bar_height.0 * font_scale).max(tokens.geometry.minimum_hit_size.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant};

    #[test]
    fn application_bar_recipe_uses_visual_geometry_without_shrinking_hit_geometry() {
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Compact);
        assert_eq!(tokens.geometry.application_bar_height.0, 32.0);
        assert_eq!(application_bar_height(&tokens, 1.0), 32.0);
        assert_eq!(application_bar_height(&tokens, 1.5), 48.0);
    }

    #[test]
    fn application_bar_margin_conversion_is_explicitly_bounded() {
        assert_eq!(bounded_margin(-2.0), 0);
        assert_eq!(bounded_margin(7.4), 7);
        assert_eq!(bounded_margin(200.0), i8::MAX);
        assert_eq!(bounded_margin(f32::NAN), 0);
    }
}
