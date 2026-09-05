use crate::DesignTokens;

pub const REGULAR_FONT_FAMILY: &str = "Polyorama Source Sans 3 Regular";
pub const SEMIBOLD_FONT_FAMILY: &str = "Polyorama Source Sans 3 Semibold";

/// Dense instruments and application reading surfaces share semantic roles.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TypographyProfile {
    #[default]
    Dense,
    Reading,
}

impl DesignTokens {
    /// Resolve a consumer profile before passing tokens to component recipes.
    pub fn with_typography_profile(mut self, profile: TypographyProfile) -> Self {
        if profile == TypographyProfile::Reading {
            self.typography.application_title_size = self.typography.reading_application_title_size;
            self.typography.pane_title_size = self.typography.reading_pane_title_size;
            self.typography.section_heading_size = self.typography.reading_section_heading_size;
            self.typography.body_size = self.typography.reading_body_size;
        }
        self
    }
}

/// Install the bundled, unmodified SIL OFL faces before the first egui pass.
/// Existing font definitions are preserved. Additional script fallback fonts
/// can be installed by the application using egui's font APIs.
pub fn install_typography_fonts(context: &egui::Context) {
    let id = egui::Id::new("polyorama.typography-fonts-installed");
    if context.data_mut(|data| {
        let installed = data.get_temp_mut_or_default::<bool>(id);
        let previous = *installed;
        *installed = true;
        previous
    }) {
        return;
    }
    for (name, bytes, family, proportional) in [
        (
            REGULAR_FONT_FAMILY,
            include_bytes!("../assets/fonts/SourceSans3-Regular.ttf").as_slice(),
            REGULAR_FONT_FAMILY,
            true,
        ),
        (
            SEMIBOLD_FONT_FAMILY,
            include_bytes!("../assets/fonts/SourceSans3-Semibold.ttf").as_slice(),
            SEMIBOLD_FONT_FAMILY,
            false,
        ),
    ] {
        let mut families = vec![egui::epaint::text::InsertFontFamily {
            family: egui::FontFamily::Name(family.into()),
            priority: egui::epaint::text::FontPriority::Highest,
        }];
        if proportional {
            families.push(egui::epaint::text::InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: egui::epaint::text::FontPriority::Highest,
            });
        }
        context.add_font(egui::epaint::text::FontInsert {
            name: name.into(),
            data: egui::FontData::from_static(bytes),
            families,
        });
    }
    // Keep egui's bundled script and emoji coverage in both named families.
    // Alias the fallback data so add_font can extend families while preserving
    // the application's existing font definitions.
    let defaults = egui::FontDefinitions::default();
    for name in &defaults.families[&egui::FontFamily::Proportional] {
        context.add_font(egui::epaint::text::FontInsert {
            name: format!("Polyorama fallback {name}"),
            data: (*defaults.font_data[name]).clone(),
            families: [REGULAR_FONT_FAMILY, SEMIBOLD_FONT_FAMILY]
                .into_iter()
                .map(|family| egui::epaint::text::InsertFontFamily {
                    family: egui::FontFamily::Name(family.into()),
                    priority: egui::epaint::text::FontPriority::Lowest,
                })
                .collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DensityVariant, TextOverflow, TextRole, TextSpec, ThemeVariant, UiPreferences,
        apply_design_system_with_typography, measure_text,
    };

    #[test]
    fn named_faces_are_real_distinct_and_preserve_default_fallbacks() {
        let context = egui::Context::default();
        install_typography_fonts(&context);
        install_typography_fonts(&context);
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let mut output = context.run_ui(Default::default(), |ui| {
            ui.fonts(|fonts| {
                let definitions = fonts.definitions();
                let regular = &definitions.font_data[REGULAR_FONT_FAMILY];
                let semibold = &definitions.font_data[SEMIBOLD_FONT_FAMILY];
                assert_ne!(regular.font, semibold.font);
                for family in [REGULAR_FONT_FAMILY, SEMIBOLD_FONT_FAMILY] {
                    let members = &definitions.families[&egui::FontFamily::Name(family.into())];
                    assert_eq!(members[0], family);
                    for fallback in
                        &egui::FontDefinitions::default().families[&egui::FontFamily::Proportional]
                    {
                        assert!(members.contains(&format!("Polyorama fallback {fallback}")));
                    }
                }
            });
            let mut equal_size = tokens;
            equal_size.typography.label_size = equal_size.typography.body_size;
            let measure = |role| {
                measure_text(
                    ui.painter(),
                    "Minimum observed width",
                    TextSpec::single_line(role, TextOverflow::Expand),
                    &equal_size,
                    1.0,
                    500.0,
                )
                .unwrap()
            };
            assert_ne!(
                measure(TextRole::Body).size().x,
                measure(TextRole::ButtonLabel).size().x,
                "regular and semibold must select distinct real font metrics"
            );
            for role in [TextRole::Body, TextRole::ButtonLabel] {
                let glyphs = measure_text(
                    ui.painter(),
                    "µm → ⚠ 😀",
                    TextSpec::single_line(role, TextOverflow::Expand),
                    &tokens,
                    1.0,
                    500.0,
                )
                .unwrap();
                assert!(glyphs.size().x > 0.0);
            }
        });
        output.textures_delta.clear();
    }

    #[test]
    fn native_and_measured_roles_share_the_profile_and_scale() {
        for profile in [TypographyProfile::Dense, TypographyProfile::Reading] {
            for scale in [1.0, 1.25, 1.5] {
                let context = egui::Context::default();
                apply_design_system_with_typography(
                    &context,
                    UiPreferences {
                        font_scale: scale,
                        ..Default::default()
                    },
                    profile,
                );
                let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable)
                    .with_typography_profile(profile);
                let title = TextRole::ApplicationTitle.style(&tokens, scale);
                let heading = TextRole::SectionHeading.style(&tokens, scale);
                let body = TextRole::Body.style(&tokens, scale);
                let secondary = TextRole::Secondary.style(&tokens, scale);
                assert!(
                    title.font_id.size > heading.font_id.size
                        && heading.font_id.size > body.font_id.size
                );
                assert!(secondary.font_id.size < body.font_id.size);
                assert_ne!(secondary.colour, body.colour);
                assert_eq!(title.weight.0, 600);
                assert_eq!(body.weight.0, 400);
                assert_eq!(
                    context.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Heading],
                    heading.font_id
                );
                assert_eq!(
                    context.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Body],
                    body.font_id
                );
                let mut output = context.run_ui(Default::default(), |ui| {
                    let measured = measure_text(
                        ui.painter(),
                        "Current actions",
                        TextSpec::single_line(TextRole::SectionHeading, TextOverflow::Expand),
                        &tokens,
                        scale,
                        500.0,
                    )
                    .unwrap();
                    let native = ui.label(heading.rich_text("Current actions"));
                    assert!((measured.size().y - native.rect.height()).abs() < 1.0);
                });
                output.textures_delta.clear();
            }
        }
    }
}
