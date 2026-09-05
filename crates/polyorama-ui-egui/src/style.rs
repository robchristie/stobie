use egui::{Color32, Stroke, TextStyle, Theme, Visuals};

use crate::{
    AppearancePreference, ContrastPreference, DesignTokens, MotionPreference, ThemeVariant,
    UiPreferences,
};

/// Apply one deterministic token-derived egui style for both light and dark
/// modes. Rebuilding from egui defaults makes repeated preference changes
/// idempotent instead of multiplying font and spacing values each time.
pub fn apply_design_system(context: &egui::Context, preferences: UiPreferences) {
    apply_design_system_with_typography(context, preferences, crate::TypographyProfile::Dense);
}

/// Apply the same semantic typography profile to measured and native controls.
pub fn apply_design_system_with_typography(
    context: &egui::Context,
    preferences: UiPreferences,
    profile: crate::TypographyProfile,
) {
    crate::install_typography_fonts(context);
    let preferences = preferences.validated();
    match preferences.appearance {
        AppearancePreference::Light => context.set_theme(Theme::Light),
        AppearancePreference::Dark | AppearancePreference::Unknown => {
            context.set_theme(Theme::Dark)
        }
        AppearancePreference::System => context.set_theme(egui::ThemePreference::System),
    }

    for theme in [Theme::Light, Theme::Dark] {
        let variant = match (theme, preferences.contrast) {
            (Theme::Light, ContrastPreference::High) => ThemeVariant::LightHighContrast,
            (Theme::Dark, ContrastPreference::High) => ThemeVariant::DarkHighContrast,
            (Theme::Light, _) => ThemeVariant::Light,
            (Theme::Dark, _) => ThemeVariant::Dark,
        };
        let tokens = DesignTokens::resolve(variant, preferences.density_variant())
            .with_typography_profile(profile);
        let mut style = egui::Style {
            visuals: visuals(theme, &tokens),
            ..egui::Style::default()
        };
        for (native, role) in [
            (TextStyle::Body, crate::TextRole::Body),
            (TextStyle::Button, crate::TextRole::ButtonLabel),
            (TextStyle::Small, crate::TextRole::Caption),
            (TextStyle::Monospace, crate::TextRole::MonospaceTechnical),
            (TextStyle::Heading, crate::TextRole::SectionHeading),
        ] {
            style
                .text_styles
                .insert(native, role.style(&tokens, preferences.font_scale).font_id);
        }
        style.spacing.item_spacing = egui::vec2(tokens.spacing.inline.0, tokens.spacing.block.0);
        style.spacing.button_padding = egui::vec2(
            tokens.geometry.control_padding_x.0,
            tokens.geometry.control_padding_y.0,
        );
        style.spacing.interact_size.y = tokens.geometry.control_height.0 * preferences.font_scale;
        style.animation_time = if matches!(preferences.motion, MotionPreference::Reduced) {
            0.0
        } else {
            tokens.motion.quick.0 as f32 / 1_000.0
        };
        context.set_style_of(theme, style);
    }
}

fn visuals(theme: Theme, tokens: &DesignTokens) -> Visuals {
    let mut visuals = match theme {
        Theme::Light => Visuals::light(),
        Theme::Dark => Visuals::dark(),
    };
    let canvas: Color32 = tokens.colours.surface_canvas.into();
    let panel: Color32 = tokens.colours.surface_panel.into();
    let raised: Color32 = tokens.colours.surface_raised.into();
    let border: Color32 = tokens.colours.border_subtle.into();
    let text: Color32 = tokens.colours.text_primary.into();
    let accent: Color32 = tokens.colours.accent_primary.into();
    visuals.override_text_color = Some(text);
    visuals.panel_fill = panel;
    visuals.window_fill = panel;
    visuals.extreme_bg_color = canvas;
    visuals.faint_bg_color = raised;
    visuals.code_bg_color = canvas;
    visuals.window_stroke = Stroke::new(1.0, border);
    visuals.widgets.noninteractive.bg_fill = panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.inactive.bg_fill = raised;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
    visuals.widgets.hovered.bg_fill = tokens.colours.selection_background.into();
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, accent);
    visuals.widgets.active.bg_fill = accent;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, accent);
    visuals.widgets.open.bg_fill = raised;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, accent);
    let radius = egui::CornerRadius::same(
        tokens
            .geometry
            .control_radius
            .0
            .round()
            .clamp(0.0, u8::MAX as f32) as u8,
    );
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;
    visuals.selection.bg_fill = tokens.colours.selection_background.into();
    visuals.selection.stroke = Stroke::new(1.0, tokens.colours.focus_ring);
    visuals.hyperlink_color = accent;
    visuals
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityPreference, MAX_FONT_SCALE};

    #[test]
    fn repeated_application_is_idempotent_and_uses_token_geometry() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let preferences = UiPreferences {
            appearance: AppearancePreference::Light,
            density: DensityPreference::Compact,
            font_scale: 1.25,
            ..UiPreferences::default()
        };
        apply_design_system(&context, preferences);
        let first = context.style_of(Theme::Light);
        apply_design_system(&context, preferences);
        let second = context.style_of(Theme::Light);
        assert_eq!(first.spacing, second.spacing);
        assert_eq!(first.text_styles, second.text_styles);
        assert_eq!(second.spacing.interact_size.y, 30.0);
        assert_eq!(context.theme(), Theme::Light);
    }

    #[test]
    fn high_contrast_and_font_scale_are_applied_to_both_theme_styles() {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let preferences = UiPreferences {
            contrast: ContrastPreference::High,
            font_scale: MAX_FONT_SCALE,
            ..UiPreferences::default()
        };
        apply_design_system(&context, preferences);
        for (theme, variant) in [
            (Theme::Light, ThemeVariant::LightHighContrast),
            (Theme::Dark, ThemeVariant::DarkHighContrast),
        ] {
            let style = context.style_of(theme);
            let tokens = DesignTokens::resolve(variant, preferences.density_variant());
            assert_eq!(
                style.visuals.panel_fill,
                tokens.colours.surface_panel.into()
            );
            assert_eq!(
                style.text_styles[&TextStyle::Body].size,
                tokens.typography.body_size.0 * MAX_FONT_SCALE
            );
        }
    }
}
