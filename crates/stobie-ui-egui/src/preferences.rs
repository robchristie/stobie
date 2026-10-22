use serde::{Deserialize, Serialize};

use crate::{DensityVariant, DesignTokens, ThemeVariant};

#[cfg(target_arch = "wasm32")]
#[path = "preferences/web.rs"]
mod web;

pub const UI_PREFERENCES_SCHEMA_VERSION: u32 = 1;
pub const MIN_FONT_SCALE: f32 = 1.0;
pub const MAX_FONT_SCALE: f32 = 1.5;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearancePreference {
    Light,
    #[default]
    Dark,
    System,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContrastPreference {
    #[default]
    Standard,
    High,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DensityPreference {
    Compact,
    #[default]
    Comfortable,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionPreference {
    #[default]
    Full,
    Reduced,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPreferences {
    pub schema_version: u32,
    pub appearance: AppearancePreference,
    pub contrast: ContrastPreference,
    pub density: DensityPreference,
    pub font_scale: f32,
    pub motion: MotionPreference,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            schema_version: UI_PREFERENCES_SCHEMA_VERSION,
            appearance: AppearancePreference::Dark,
            contrast: ContrastPreference::Standard,
            density: DensityPreference::Comfortable,
            font_scale: MIN_FONT_SCALE,
            motion: MotionPreference::Full,
        }
    }
}

impl UiPreferences {
    /// What an application shows until it chooses otherwise: the system's
    /// light or dark appearance and its text size ([`system_font_scale`]),
    /// with the other fields at their defaults. `Application::preferences`
    /// returns it by default. [`UiPreferences::default`] is unchanged: dark,
    /// at 100%, whatever the system's settings.
    pub fn system() -> Self {
        Self {
            appearance: AppearancePreference::System,
            font_scale: system_font_scale(),
            ..Self::default()
        }
    }

    /// Returns a safe current-schema value. Obsolete schema versions reset as
    /// one unit; malformed individual current fields fall back independently.
    pub fn validated(self) -> Self {
        if self.schema_version != UI_PREFERENCES_SCHEMA_VERSION {
            return Self::default();
        }
        let defaults = Self::default();
        Self {
            schema_version: UI_PREFERENCES_SCHEMA_VERSION,
            appearance: match self.appearance {
                AppearancePreference::Unknown => defaults.appearance,
                value => value,
            },
            contrast: match self.contrast {
                ContrastPreference::Unknown => defaults.contrast,
                value => value,
            },
            density: match self.density {
                DensityPreference::Unknown => defaults.density,
                value => value,
            },
            font_scale: if self.font_scale.is_finite() {
                self.font_scale.clamp(MIN_FONT_SCALE, MAX_FONT_SCALE)
            } else {
                defaults.font_scale
            },
            motion: match self.motion {
                MotionPreference::Unknown => defaults.motion,
                value => value,
            },
        }
    }

    pub const fn theme_variant(self, system_dark: bool) -> ThemeVariant {
        let dark = match self.appearance {
            AppearancePreference::Light => false,
            AppearancePreference::Dark | AppearancePreference::Unknown => true,
            AppearancePreference::System => system_dark,
        };
        match (dark, self.contrast) {
            (false, ContrastPreference::High) => ThemeVariant::LightHighContrast,
            (true, ContrastPreference::High) => ThemeVariant::DarkHighContrast,
            (false, _) => ThemeVariant::Light,
            (true, _) => ThemeVariant::Dark,
        }
    }

    pub const fn density_variant(self) -> DensityVariant {
        match self.density {
            DensityPreference::Compact => DensityVariant::Compact,
            DensityPreference::Comfortable | DensityPreference::Unknown => {
                DensityVariant::Comfortable
            }
        }
    }

    pub const fn tokens(self, system_dark: bool) -> DesignTokens {
        DesignTokens::resolve(self.theme_variant(system_dark), self.density_variant())
    }
}

/// The text size the platform asks for, as a font scale from
/// [`MIN_FONT_SCALE`] to [`MAX_FONT_SCALE`].
///
/// In the browser it is the root element's computed font size over 16 CSS
/// pixels, which follows the browser's text size setting (and any stylesheet
/// that sets the root's size), or in a WebKit browser the larger of that and
/// the system body text size (`-apple-system-body`, which follows Dynamic
/// Type on iOS and iPadOS) over its default of 17 pixels. Smaller sizes give
/// 100%. It is read when first asked for and again after the page regains
/// focus, is shown again or is resized, so a setting changed in another tab
/// or in the system's settings applies when the person returns. Natively it
/// is 100%: Stobie does not yet read a desktop text size.
pub fn system_font_scale() -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        web::font_scale()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        MIN_FONT_SCALE
    }
}

/// The browser's root font size at 100%, in CSS pixels.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const ROOT_TEXT_PIXELS: f32 = 16.0;
/// WebKit's system body text size at the default Dynamic Type size.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const SYSTEM_BODY_TEXT_PIXELS: f32 = 17.0;

/// The font scale for a computed root font size and, in WebKit, the system
/// body font size, both as CSS values such as `"20px"`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn font_scale_for(root: Option<&str>, system_body: Option<&str>) -> f32 {
    let root = root
        .and_then(css_pixels)
        .map(|pixels| pixels / ROOT_TEXT_PIXELS);
    let body = system_body
        .and_then(css_pixels)
        .map(|pixels| pixels / SYSTEM_BODY_TEXT_PIXELS);
    root.into_iter()
        .chain(body)
        .fold(MIN_FONT_SCALE, f32::max)
        .min(MAX_FONT_SCALE)
}

/// A computed CSS length in pixels, such as `"16px"` or `"17.5px"`.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn css_pixels(value: &str) -> Option<f32> {
    let pixels: f32 = value.trim().strip_suffix("px")?.parse().ok()?;
    (pixels.is_finite() && pixels > 0.0).then_some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_preferences_follow_the_system_and_default_stays_dark() {
        let system = UiPreferences::system();
        assert_eq!(system.appearance, AppearancePreference::System);
        assert_eq!(system.font_scale, MIN_FONT_SCALE);
        assert_eq!(
            system,
            UiPreferences {
                appearance: AppearancePreference::System,
                ..UiPreferences::default()
            }
        );
        assert_eq!(system.validated(), system);
        // Applications outside the host build on the default and rely on it.
        assert_eq!(
            UiPreferences::default().appearance,
            AppearancePreference::Dark
        );
        assert_eq!(system_font_scale(), MIN_FONT_SCALE);
    }

    #[test]
    fn browser_text_sizes_give_a_bounded_font_scale() {
        for (root, body, scale) in [
            (Some("16px"), None, 1.0),
            (Some("20px"), None, 1.25),
            (Some("24px"), None, 1.5),
            (Some("32px"), None, 1.5),
            (Some("12px"), None, 1.0),
            (Some(" 18.4px "), None, 1.15),
            // iOS at its default Dynamic Type size, and larger.
            (Some("16px"), Some("17px"), 1.0),
            (Some("16px"), Some("21px"), 21.0 / 17.0),
            (Some("16px"), Some("53px"), 1.5),
            // macOS Safari's smaller body text leaves the root's size.
            (Some("20px"), Some("13px"), 1.25),
            (None, None, 1.0),
            (Some(""), None, 1.0),
            (Some("1.25em"), None, 1.0),
            (Some("120%"), None, 1.0),
            (Some("px"), None, 1.0),
            (Some("-20px"), None, 1.0),
            (Some("NaNpx"), None, 1.0),
            (Some("infpx"), None, 1.0),
            (Some("0px"), Some("bogus"), 1.0),
        ] {
            let actual = font_scale_for(root, body);
            assert!((actual - scale).abs() < 1e-6, "{root:?} {body:?}: {actual}");
            assert!((MIN_FONT_SCALE..=MAX_FONT_SCALE).contains(&actual));
        }
    }

    #[test]
    fn preferences_migrate_missing_fields_and_unknown_values() {
        let missing: UiPreferences = serde_json::from_str(r#"{ "schema_version": 1 }"#).unwrap();
        assert_eq!(missing.validated(), UiPreferences::default());

        let unknown: UiPreferences = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "appearance": "future_auto",
                "contrast": "future_contrast",
                "density": "future_density",
                "font_scale": 1.25,
                "motion": "future_motion"
            }"#,
        )
        .unwrap();
        assert_eq!(unknown.validated().font_scale, 1.25);
        assert_eq!(unknown.validated().appearance, AppearancePreference::Dark);
        assert_eq!(unknown.validated().contrast, ContrastPreference::Standard);
        assert_eq!(unknown.validated().density, DensityPreference::Comfortable);
        assert_eq!(unknown.validated().motion, MotionPreference::Full);
    }

    #[test]
    fn obsolete_preferences_reset_and_font_scale_is_bounded() {
        let obsolete = UiPreferences {
            schema_version: 0,
            appearance: AppearancePreference::Light,
            font_scale: 1.4,
            ..UiPreferences::default()
        };
        assert_eq!(obsolete.validated(), UiPreferences::default());

        let too_large = UiPreferences {
            font_scale: 9.0,
            ..UiPreferences::default()
        };
        assert_eq!(too_large.validated().font_scale, MAX_FONT_SCALE);
        let non_finite = UiPreferences {
            font_scale: f32::NAN,
            ..UiPreferences::default()
        };
        assert_eq!(non_finite.validated().font_scale, MIN_FONT_SCALE);
    }

    #[test]
    fn theme_and_density_selection_are_orthogonal() {
        let preferences = UiPreferences {
            appearance: AppearancePreference::System,
            contrast: ContrastPreference::High,
            density: DensityPreference::Compact,
            ..UiPreferences::default()
        };
        assert_eq!(
            preferences.theme_variant(false),
            ThemeVariant::LightHighContrast
        );
        assert_eq!(
            preferences.theme_variant(true),
            ThemeVariant::DarkHighContrast
        );
        assert_eq!(preferences.density_variant(), DensityVariant::Compact);
        assert_eq!(
            preferences.tokens(false).geometry.application_bar_height.0,
            32.0
        );
    }
}
