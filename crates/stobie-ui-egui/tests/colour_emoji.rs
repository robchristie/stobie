//! Why Stobie's emoji are monochrome: egui 0.36 rasterises only a glyph's
//! outline, filled with white, into a coverage atlas that the text colour
//! tints (epaint 0.36.1 `src/text/font.rs`, `allocate_glyph_uncached`:
//! `outline_glyphs` and `set_paint(WHITE)`). A colour glyph draws nothing:
//! a COLRv1 glyph's own outline is empty (its colour layers are other glyphs
//! that a COLR paint graph composes) and a CBDT glyph is a bitmap with no
//! outline at all.

use eframe::egui;

const COLRV1: &[u8] = include_bytes!("../../../tests/fixtures/colour-emoji/Noto-COLRv1-1F600.ttf");
const CBDT: &[u8] = include_bytes!("../../../tests/fixtures/colour-emoji/NotoColorEmoji-1F600.ttf");

fn glyphs(ui: &egui::Ui, text: &str, family: egui::FontFamily) -> Vec<egui::epaint::text::Glyph> {
    let galley = ui.painter().layout_no_wrap(
        text.into(),
        egui::FontId::new(32.0, family),
        egui::Color32::WHITE,
    );
    galley
        .rows
        .iter()
        .flat_map(|row| row.row.glyphs.clone())
        .collect()
}

#[test]
fn colour_glyphs_are_mapped_but_draw_nothing() {
    for (format, bytes) in [("COLRv1", COLRV1), ("CBDT", CBDT)] {
        let context = egui::Context::default();
        let mut fonts = egui::FontDefinitions::empty();
        fonts
            .font_data
            .insert(format.into(), egui::FontData::from_static(bytes).into());
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.insert(family, vec![format.into()]);
        }
        context.set_fonts(fonts);
        let mut output = context.run_ui(Default::default(), |ui| {
            let mapped = ui.fonts_mut(|fonts| {
                fonts
                    .fonts
                    .font(&egui::FontFamily::Proportional)
                    .characters()
                    .contains_key(&'😀')
            });
            assert!(mapped, "the {format} face maps U+1F600");
            let glyphs = glyphs(ui, "😀", egui::FontFamily::Proportional);
            assert_eq!(glyphs.len(), 1);
            assert_eq!(glyphs[0].chr, '😀');
            assert!(
                glyphs[0].advance_width > 0.0,
                "the {format} glyph takes space"
            );
            assert!(
                glyphs[0].uv_rect.is_nothing(),
                "the {format} glyph has no pixels: {:?}",
                glyphs[0].uv_rect
            );
        });
        output.textures_delta.clear();
    }
}

#[test]
fn monochrome_emoji_reach_the_atlas_as_coverage_only() {
    let context = egui::Context::default();
    stobie_ui_egui::install_typography_fonts(&context);
    let mut output = context.run_ui(Default::default(), |ui| {
        let family = egui::FontFamily::Name(stobie_ui_egui::REGULAR_FONT_FAMILY.into());
        let glyphs = glyphs(ui, "😀🧠🇦🇺", family);
        let atlas = ui.fonts(|fonts| fonts.image());
        let mut covered = 0;
        for glyph in glyphs.iter().filter(|glyph| !glyph.uv_rect.is_nothing()) {
            let [x0, y0] = glyph.uv_rect.min.map(usize::from);
            let [x1, y1] = glyph.uv_rect.max.map(usize::from);
            for y in y0..y1 {
                for x in x0..x1 {
                    let texel = atlas[(x, y)];
                    assert!(
                        texel.r() == texel.g() && texel.g() == texel.b(),
                        "{} has a coloured texel {texel:?}",
                        glyph.chr
                    );
                    covered += usize::from(texel.a() > 0);
                }
            }
        }
        assert_eq!(
            glyphs
                .iter()
                .filter(|glyph| !glyph.uv_rect.is_nothing())
                .count(),
            3
        );
        assert!(covered > 0, "the emoji have coverage");
    });
    output.textures_delta.clear();
}
