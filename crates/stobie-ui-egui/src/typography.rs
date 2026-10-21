use crate::DesignTokens;

pub const REGULAR_FONT_FAMILY: &str = "Stobie Source Sans 3 Regular";
pub const SEMIBOLD_FONT_FAMILY: &str = "Stobie Source Sans 3 Semibold";

/// The regular face's bytes, which the browser host also gives native text
/// fields.
pub(crate) const REGULAR_FONT_BYTES: &[u8] =
    include_bytes!("../assets/fonts/SourceSans3-Regular.ttf");

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

/// The bundled monochrome emoji face's font-data name. It follows the text
/// face in both named families and in egui's proportional and monospace
/// families.
const EMOJI_FONT: &str = "Stobie Noto Emoji";

const EMOJI_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoEmoji-Regular.ttf");

/// Install the bundled SIL OFL faces before the first egui pass: Source Sans 3
/// regular and semibold, and Noto Emoji for emoji. Existing font definitions
/// are preserved, and egui's default fonts, when the application enables them
/// (egui's `default_fonts` feature), stay as further fallbacks. Additional
/// script fallback fonts can be installed by the application using egui's
/// font APIs.
pub fn install_typography_fonts(context: &egui::Context) {
    install_typography_fonts_over(context, &egui::FontDefinitions::default());
}

/// Install Stobie's faces with `fallbacks`' proportional faces after them in
/// the named families: egui's default fonts when they are compiled in.
fn install_typography_fonts_over(context: &egui::Context, fallbacks: &egui::FontDefinitions) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};
    let id = egui::Id::new("stobie.typography-fonts-installed");
    if context.data_mut(|data| {
        let installed = data.get_temp_mut_or_default::<bool>(id);
        let previous = *installed;
        *installed = true;
        previous
    }) {
        return;
    }
    let named = |family: &str| egui::FontFamily::Name(family.into());
    let insert = |family, priority| InsertFontFamily { family, priority };
    // Fonts are inserted in order: each `Lowest` insertion follows the faces
    // already in that family.
    context.add_font(FontInsert {
        name: REGULAR_FONT_FAMILY.into(),
        data: egui::FontData::from_static(REGULAR_FONT_BYTES),
        families: vec![
            insert(named(REGULAR_FONT_FAMILY), FontPriority::Highest),
            insert(egui::FontFamily::Proportional, FontPriority::Highest),
            // Monospace roles keep egui's monospace face when it is compiled
            // in; without it they are set in the regular face.
            insert(egui::FontFamily::Monospace, FontPriority::Lowest),
        ],
    });
    context.add_font(FontInsert {
        name: SEMIBOLD_FONT_FAMILY.into(),
        data: egui::FontData::from_static(include_bytes!(
            "../assets/fonts/SourceSans3-Semibold.ttf"
        )),
        families: vec![insert(named(SEMIBOLD_FONT_FAMILY), FontPriority::Highest)],
    });
    // Emoji follow the text face, so the digits, `#` and `*` that the emoji
    // face also maps keep the text face's glyphs.
    context.add_font(FontInsert {
        name: EMOJI_FONT.into(),
        data: egui::FontData::from_static(EMOJI_FONT_BYTES).tweak(egui::FontTweak {
            // As egui scales its own bundled emoji face, so that emoji sit
            // within the text's line.
            scale: 0.81,
            ..Default::default()
        }),
        families: [
            named(REGULAR_FONT_FAMILY),
            named(SEMIBOLD_FONT_FAMILY),
            egui::FontFamily::Proportional,
            egui::FontFamily::Monospace,
        ]
        .into_iter()
        .map(|family| insert(family, FontPriority::Lowest))
        .collect(),
    });
    // Keep egui's bundled script coverage in both named families when the
    // application compiles it in. Alias the fallback data so add_font can
    // extend families while preserving the application's existing font
    // definitions.
    for name in fallbacks
        .families
        .get(&egui::FontFamily::Proportional)
        .into_iter()
        .flatten()
    {
        context.add_font(FontInsert {
            name: format!("Stobie fallback {name}"),
            data: (*fallbacks.font_data[name]).clone(),
            families: [REGULAR_FONT_FAMILY, SEMIBOLD_FONT_FAMILY]
                .into_iter()
                .map(|family| insert(named(family), FontPriority::Lowest))
                .collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextRoleStyles;
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
                    assert_eq!(members[1], EMOJI_FONT, "emoji follow the text face");
                    for fallback in
                        &egui::FontDefinitions::default().families[&egui::FontFamily::Proportional]
                    {
                        assert!(members.contains(&format!("Stobie fallback {fallback}")));
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

    /// The emoji that had no glyph in any bundled font on an iPhone (the iOS
    /// PWA spike report, "Emoji").
    const SPIKE_EMOJI: [char; 7] = ['🧠', '🤗', '🫣', '🛑', '🤣', '🪿', '🌧'];

    fn families() -> [egui::FontFamily; 4] {
        [
            egui::FontFamily::Name(REGULAR_FONT_FAMILY.into()),
            egui::FontFamily::Name(SEMIBOLD_FONT_FAMILY.into()),
            egui::FontFamily::Proportional,
            egui::FontFamily::Monospace,
        ]
    }

    /// The drawn glyphs of `text`: those with pixels in the atlas.
    fn drawn(
        ui: &egui::Ui,
        text: &str,
        family: &egui::FontFamily,
    ) -> Vec<egui::epaint::text::Glyph> {
        let galley = ui.painter().layout_no_wrap(
            text.into(),
            egui::FontId::new(20.0, family.clone()),
            egui::Color32::WHITE,
        );
        galley
            .rows
            .iter()
            .flat_map(|row| row.row.glyphs.iter())
            .filter(|glyph| !glyph.uv_rect.is_nothing())
            .cloned()
            .collect()
    }

    fn assert_spike_emoji_drawn(context: &egui::Context) {
        let mut output = context.run_ui(Default::default(), |ui| {
            for family in families() {
                // egui's `has_glyph` is false for every character of the face
                // that holds its replacement glyph, U+25FB, which the emoji
                // face maps; ask which faces map the character instead.
                let characters =
                    ui.fonts_mut(|fonts| fonts.fonts.font(&family).characters().clone());
                for emoji in SPIKE_EMOJI {
                    assert_eq!(
                        characters
                            .get(&emoji)
                            .and_then(|faces| faces.first())
                            .map(String::as_str),
                        Some(EMOJI_FONT),
                        "{emoji} in {family:?}"
                    );
                    let drawn = drawn(ui, &emoji.to_string(), &family);
                    assert_eq!(drawn.len(), 1, "{emoji} in {family:?}");
                    assert_eq!(drawn[0].chr, emoji);
                }
            }
        });
        output.textures_delta.clear();
    }

    #[test]
    fn every_family_draws_the_spike_report_emoji() {
        let context = egui::Context::default();
        install_typography_fonts(&context);
        assert_spike_emoji_drawn(&context);
    }

    /// An application that leaves out egui's default fonts keeps text,
    /// monospace roles and emoji, from Stobie's faces alone.
    #[test]
    fn stobie_faces_alone_cover_every_family_without_eguis_default_fonts() {
        let context = egui::Context::default();
        context.set_fonts(egui::FontDefinitions::empty());
        install_typography_fonts_over(&context, &egui::FontDefinitions::empty());
        assert_spike_emoji_drawn(&context);
        let mut output = context.run_ui(Default::default(), |ui| {
            let definitions = ui.fonts(|fonts| fonts.definitions().clone());
            assert_eq!(
                definitions.font_data.keys().collect::<Vec<_>>(),
                [EMOJI_FONT, REGULAR_FONT_FAMILY, SEMIBOLD_FONT_FAMILY]
            );
            assert_eq!(
                definitions.families[&egui::FontFamily::Monospace],
                [REGULAR_FONT_FAMILY, EMOJI_FONT]
            );
            for family in families() {
                let characters =
                    ui.fonts_mut(|fonts| fonts.fonts.font(&family).characters().clone());
                for character in "Status: 12 µm → ok".chars() {
                    assert_eq!(
                        characters.get(&character).and_then(|faces| faces.first()),
                        Some(&definitions.families[&family][0]),
                        "{character:?} in {family:?} comes from the text face"
                    );
                }
            }
        });
        output.textures_delta.clear();
    }

    /// egui shapes with the emoji face's ligatures, so a sequence draws as one
    /// glyph, but the face is monochrome: a skin tone draws the untoned glyph,
    /// and a keycap whose digit the text face maps draws the bare digit.
    #[test]
    fn emoji_sequences_draw_one_monochrome_glyph() {
        let context = egui::Context::default();
        install_typography_fonts(&context);
        let mut output = context.run_ui(Default::default(), |ui| {
            let family = egui::FontFamily::Name(REGULAR_FONT_FAMILY.into());
            let atlas = |text: &str| {
                let drawn = drawn(ui, text, &family);
                assert_eq!(drawn.len(), 1, "{text:?} draws one glyph");
                (drawn[0].uv_rect.min, drawn[0].uv_rect.max)
            };
            for (sequence, alone) in [
                ("👨\u{200d}👩\u{200d}👧", "👨"),
                ("🇦🇺", "🇦"),
                ("🏳\u{fe0f}\u{200d}🌈", "🏳"),
            ] {
                assert_ne!(
                    atlas(sequence),
                    atlas(alone),
                    "{sequence:?} has its own glyph"
                );
            }
            assert_eq!(
                atlas("👍🏽"),
                atlas("👍"),
                "a skin tone draws the untoned glyph"
            );
            assert_eq!(
                atlas("1\u{fe0f}\u{20e3}"),
                atlas("1"),
                "a keycap draws the bare digit"
            );
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
