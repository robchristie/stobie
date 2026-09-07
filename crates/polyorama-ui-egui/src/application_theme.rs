//! Bounded application-owned colours, independent of presentation preferences.
use crate::{ColourTokens, DensityVariant, DesignTokens, Rgba8, ThemeVariant, TypographyProfile};

/// Complete authored light/dark and high-contrast counterparts. JSON is an
/// optional build/startup interchange format, never a per-frame stylesheet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeColours {
    pub light: ColourTokens,
    pub dark: ColourTokens,
    pub light_high_contrast: ColourTokens,
    pub dark_high_contrast: ColourTokens,
}

/// The small geometry choices that give application chrome its own treatment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ApplicationGeometry {
    pub application_bar_height: f32,
    pub control_height: f32,
    pub control_radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApplicationTheme {
    colours: ThemeColours,
    geometry: Option<ApplicationGeometry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeValidationError(pub String);

impl std::fmt::Display for ThemeValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for ThemeValidationError {}

impl ApplicationTheme {
    /// Compatible analytical reference appearance, with unchanged authored colours.
    pub fn analytical() -> Self {
        let colours = |variant| DesignTokens::resolve(variant, DensityVariant::Comfortable).colours;
        Self {
            colours: ThemeColours {
                light: colours(ThemeVariant::Light),
                dark: colours(ThemeVariant::Dark),
                light_high_contrast: colours(ThemeVariant::LightHighContrast),
                dark_high_contrast: colours(ThemeVariant::DarkHighContrast),
            },
            geometry: None,
        }
    }

    /// Import only the exact authored analytical reference palette.
    ///
    /// This explicit compatibility route preserves historical colours that do
    /// not meet the strict muted-text state checks in [`Self::new`]. It accepts
    /// no edits, including changes to otherwise unchecked roles. Edited themes
    /// must pass [`Self::new`]; this is not a general validation bypass.
    pub fn from_analytical_colours(colours: ThemeColours) -> Result<Self, ThemeValidationError> {
        let reference = Self::analytical();
        if colours != reference.colours {
            return Err(ThemeValidationError(
                "analytical compatibility requires the exact authored reference colours".into(),
            ));
        }
        Ok(reference)
    }

    /// Validate opaque primary and muted text on canvas, panel, raised, hover,
    /// selection and quiet-hover backgrounds in every authored mode.
    /// This establishes token contrast, not whole-application accessibility.
    /// Control borders must be opaque; their contrast against actual adjacent
    /// surfaces remains application-qualified. The analytical reference retains
    /// historical borders below 3:1, so decorative and control roles can split
    /// without silently changing its palette.
    pub fn new(colours: ThemeColours) -> Result<Self, ThemeValidationError> {
        for (name, colour, minimum) in [
            ("light", colours.light, 4.5),
            ("dark", colours.dark, 4.5),
            ("light-high-contrast", colours.light_high_contrast, 7.0),
            ("dark-high-contrast", colours.dark_high_contrast, 7.0),
        ] {
            if colour.border_control.alpha != 255 {
                return Err(ThemeValidationError(format!(
                    "{name} control border requires an opaque colour"
                )));
            }
            for (role, background) in [
                ("canvas", colour.surface_canvas),
                ("panel", colour.surface_panel),
                ("raised", colour.surface_raised),
                ("hover", colour.surface_hover),
                ("selection", colour.selection_background),
                ("quiet hover", colour.action_quiet_hover),
            ] {
                for (text_role, foreground) in [
                    ("primary text", colour.text_primary),
                    ("muted text", colour.text_muted),
                ] {
                    check_pair(
                        name,
                        &format!("{text_role} on {role}"),
                        foreground,
                        background,
                        minimum,
                    )?;
                }
            }
            check_pair(
                name,
                "primary action",
                colour.action_primary_foreground,
                colour.action_primary_background,
                minimum,
            )?;
            for background in [
                colour.surface_canvas,
                colour.surface_panel,
                colour.surface_raised,
                colour.selection_background,
            ] {
                check_pair(name, "focus ring", colour.focus_ring, background, 3.0)?;
            }
            check_pair(
                name,
                "selection indicator",
                colour.selection_indicator,
                colour.selection_background,
                3.0,
            )?;
        }
        Ok(Self {
            colours,
            geometry: None,
        })
    }

    pub fn colours(&self) -> ThemeColours {
        self.colours
    }

    pub fn with_geometry(
        mut self,
        geometry: ApplicationGeometry,
    ) -> Result<Self, ThemeValidationError> {
        for (role, value, minimum, maximum) in [
            (
                "application bar height",
                geometry.application_bar_height,
                32.0,
                64.0,
            ),
            ("control height", geometry.control_height, 24.0, 48.0),
            ("control radius", geometry.control_radius, 0.0, 12.0),
        ] {
            if !value.is_finite() || !(minimum..=maximum).contains(&value) {
                return Err(ThemeValidationError(format!(
                    "{role} must be within {minimum}–{maximum} points"
                )));
            }
        }
        self.geometry = Some(geometry);
        Ok(self)
    }

    /// Use this same resolution for custom recipes and native egui styling.
    pub fn resolve(
        &self,
        variant: ThemeVariant,
        density: DensityVariant,
        profile: TypographyProfile,
    ) -> DesignTokens {
        let mut tokens = DesignTokens::resolve(variant, density).with_typography_profile(profile);
        tokens.colours = match variant {
            ThemeVariant::Light => self.colours.light,
            ThemeVariant::Dark => self.colours.dark,
            ThemeVariant::LightHighContrast => self.colours.light_high_contrast,
            ThemeVariant::DarkHighContrast => self.colours.dark_high_contrast,
        };
        if let Some(geometry) = self.geometry {
            tokens.geometry.application_bar_height.0 = geometry.application_bar_height;
            tokens.geometry.control_height.0 = geometry.control_height;
            tokens.geometry.control_radius.0 = geometry.control_radius;
        }
        tokens
    }
}

fn check_pair(
    mode: &str,
    role: &str,
    foreground: Rgba8,
    background: Rgba8,
    minimum: f64,
) -> Result<(), ThemeValidationError> {
    if foreground.alpha != 255 || background.alpha != 255 {
        return Err(ThemeValidationError(format!(
            "{mode} {role} requires opaque colours"
        )));
    }
    let luminance = |colour: Rgba8| {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(colour.red) + 0.7152 * linear(colour.green) + 0.0722 * linear(colour.blue)
    };
    let a = luminance(foreground);
    let b = luminance(background);
    let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
    if contrast < minimum {
        return Err(ThemeValidationError(format!(
            "{mode} {role} contrast {contrast:.2}:1 is below {minimum}:1"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytical_resolution_is_compatible_and_application_pairs_are_validated() {
        let theme = ApplicationTheme::analytical();
        assert!(ApplicationTheme::new(theme.colours()).is_err());
        let json = serde_json::to_string(&theme.colours()).unwrap();
        assert_eq!(
            ApplicationTheme::from_analytical_colours(serde_json::from_str(&json).unwrap())
                .unwrap(),
            theme
        );
        let mut edited = theme.colours();
        edited.dark.border_decorative.red ^= 1;
        assert!(ApplicationTheme::from_analytical_colours(edited).is_err());
        for variant in [
            ThemeVariant::Light,
            ThemeVariant::Dark,
            ThemeVariant::LightHighContrast,
            ThemeVariant::DarkHighContrast,
        ] {
            for density in [DensityVariant::Compact, DensityVariant::Comfortable] {
                assert_eq!(
                    theme.resolve(variant, density, TypographyProfile::Dense),
                    DesignTokens::resolve(variant, density)
                );
            }
        }
        let mut colours = strict_colours();
        colours.dark.action_primary_foreground = colours.dark.action_primary_background;
        assert!(
            ApplicationTheme::new(colours)
                .unwrap_err()
                .to_string()
                .contains("primary action")
        );
        let mut transparent_border = theme.colours();
        transparent_border.light.border_control.alpha = 0;
        assert!(
            ApplicationTheme::new(transparent_border)
                .unwrap_err()
                .to_string()
                .contains("control border")
        );
        assert!(
            theme
                .with_geometry(ApplicationGeometry {
                    application_bar_height: f32::NAN,
                    control_height: 34.0,
                    control_radius: 6.0
                })
                .is_err()
        );
    }

    fn strict_colours() -> ThemeColours {
        let mut colours = ApplicationTheme::analytical().colours();
        // A deliberately high-contrast fixture isolates resolver wiring from
        // the analytical reference palette.
        for colour in [
            &mut colours.light,
            &mut colours.dark,
            &mut colours.light_high_contrast,
            &mut colours.dark_high_contrast,
        ] {
            let black = Rgba8 {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            };
            let white = Rgba8 {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            };
            colour.surface_canvas = black;
            colour.surface_panel = black;
            colour.surface_raised = black;
            colour.surface_hover = black;
            colour.selection_background = black;
            colour.action_quiet_hover = black;
            colour.text_primary = white;
            colour.text_muted = white;
            colour.action_primary_background = white;
            colour.action_primary_foreground = black;
            colour.focus_ring = white;
            colour.selection_indicator = white;
        }
        colours
    }

    #[test]
    fn primary_and_muted_text_are_checked_on_every_state_in_all_modes() {
        for mode in 0..4 {
            for state in 0..6 {
                for muted in [false, true] {
                    let mut colours = strict_colours();
                    let colour = match mode {
                        0 => &mut colours.light,
                        1 => &mut colours.dark,
                        2 => &mut colours.light_high_contrast,
                        _ => &mut colours.dark_high_contrast,
                    };
                    // The foreground remains valid on all other black surfaces.
                    // It loses contrast only on the state under test.
                    let foreground = Rgba8 {
                        red: 170,
                        green: 171,
                        blue: 180,
                        alpha: 255,
                    };
                    if muted {
                        colour.text_muted = foreground;
                    } else {
                        colour.text_primary = foreground;
                    }
                    let background = Rgba8 {
                        red: 74,
                        green: 74,
                        blue: 74,
                        alpha: 255,
                    };
                    match state {
                        0 => colour.surface_canvas = background,
                        1 => colour.surface_panel = background,
                        2 => colour.surface_raised = background,
                        3 => colour.surface_hover = background,
                        4 => colour.selection_background = background,
                        _ => colour.action_quiet_hover = background,
                    }
                    let error = ApplicationTheme::new(colours).unwrap_err().to_string();
                    let mode_name =
                        ["light", "dark", "light-high-contrast", "dark-high-contrast"][mode];
                    let state_name = [
                        "canvas",
                        "panel",
                        "raised",
                        "hover",
                        "selection",
                        "quiet hover",
                    ][state];
                    let text_role = if muted { "muted text" } else { "primary text" };
                    assert!(
                        error.contains(&format!("{mode_name} {text_role} on {state_name}")),
                        "{error}"
                    );
                }
            }
        }
    }

    #[test]
    fn high_contrast_modes_require_seven_to_one_for_muted_states() {
        for mode in 0..4 {
            for state in 0..3 {
                let mut colours = strict_colours();
                let colour = match mode {
                    0 => &mut colours.light,
                    1 => &mut colours.dark,
                    2 => &mut colours.light_high_contrast,
                    _ => &mut colours.dark_high_contrast,
                };
                colour.text_muted = Rgba8 {
                    red: 170,
                    green: 170,
                    blue: 170,
                    alpha: 255,
                };
                let background = Rgba8 {
                    red: 51,
                    green: 51,
                    blue: 51,
                    alpha: 255,
                };
                match state {
                    0 => colour.surface_hover = background,
                    1 => colour.selection_background = background,
                    _ => colour.action_quiet_hover = background,
                }
                let result = ApplicationTheme::new(colours);
                if mode < 2 {
                    assert!(result.is_ok());
                } else {
                    let error = result.unwrap_err().to_string();
                    assert!(
                        error.contains("muted text") && error.contains("below 7:1"),
                        "{error}"
                    );
                }
            }
        }
    }

    #[test]
    fn review_hover_example_rejects_muted_text_despite_valid_primary_text() {
        let mut colours = strict_colours();
        colours.dark.surface_hover = Rgba8 {
            red: 0x65,
            green: 0x65,
            blue: 0x6d,
            alpha: 255,
        };
        colours.dark.text_primary = Rgba8 {
            red: 0xf0,
            green: 0xf0,
            blue: 0xf2,
            alpha: 255,
        };
        colours.dark.text_muted = Rgba8 {
            red: 0xaa,
            green: 0xab,
            blue: 0xb4,
            alpha: 255,
        };
        let error = ApplicationTheme::new(colours).unwrap_err().to_string();
        assert!(error.contains("dark muted text on hover"), "{error}");
    }

    #[test]
    fn native_and_custom_resolve_identical_application_values_in_each_mode() {
        let context = egui::Context::default();
        let colours = strict_colours();
        let theme = ApplicationTheme::new(colours)
            .unwrap()
            .with_geometry(ApplicationGeometry {
                application_bar_height: 46.0,
                control_height: 34.0,
                control_radius: 6.0,
            })
            .unwrap();
        let json = serde_json::to_string(&theme.colours()).unwrap();
        assert_eq!(
            ApplicationTheme::new(serde_json::from_str(&json).unwrap())
                .unwrap()
                .colours(),
            theme.colours()
        );
        for contrast in [
            crate::ContrastPreference::Standard,
            crate::ContrastPreference::High,
        ] {
            let preferences = crate::UiPreferences {
                appearance: crate::AppearancePreference::System,
                contrast,
                font_scale: 1.5,
                ..Default::default()
            };
            crate::apply_design_system_with_theme(
                &context,
                preferences,
                TypographyProfile::Reading,
                &theme,
            );
            for (native, dark) in [(egui::Theme::Light, false), (egui::Theme::Dark, true)] {
                let tokens = theme.resolve(
                    preferences.theme_variant(dark),
                    preferences.density_variant(),
                    TypographyProfile::Reading,
                );
                let style = context.style_of(native);
                assert_eq!(
                    style.visuals.panel_fill,
                    tokens.colours.surface_panel.into()
                );
                assert_eq!(
                    style.visuals.widgets.inactive.weak_bg_fill,
                    tokens.colours.surface_raised.into()
                );
                assert_eq!(
                    style.visuals.widgets.active.weak_bg_fill,
                    tokens.colours.surface_hover.into()
                );
                assert_eq!(
                    style.visuals.widgets.hovered.bg_fill,
                    tokens.colours.surface_hover.into()
                );
                assert_eq!(
                    style.visuals.selection.bg_fill,
                    tokens.colours.selection_background.into()
                );
                assert_eq!(
                    style.visuals.widgets.inactive.bg_stroke.color,
                    tokens.colours.border_control.into()
                );
                assert_eq!(
                    style.spacing.interact_size.y,
                    tokens.geometry.control_height.0 * 1.5
                );
                assert_eq!(
                    style.text_styles[&egui::TextStyle::Body].size,
                    tokens.typography.body_size.0 * 1.5
                );
                context.set_theme(native);
                let mut button_rect = egui::Rect::NOTHING;
                let mut output = context.run_ui(egui::RawInput::default(), |ui| {
                    button_rect = ui.button("Native action").rect;
                });
                output.textures_delta.clear();
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                    egui::Shape::Rect(rect) if rect.rect == button_rect && rect.fill == egui::Color32::from(tokens.colours.surface_raised)
                )), "native button must paint the resolved raised surface");
            }
        }
    }
}
