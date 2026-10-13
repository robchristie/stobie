use serde::{Deserialize, Serialize};

use crate::{DensityVariant, DesignTokens, ThemeVariant};

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

#[cfg(test)]
mod tests {
    use super::*;

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
