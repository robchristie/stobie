use eframe::egui;
use stobie_ui_egui::{
    DesignTokens, TextInteraction, TextLayoutObservation, TextOverflow, TextRole, TextRoleStyles,
    TypographyProfile, measured_content_label, measured_fixed_slot_label,
};

pub(super) fn story(
    ui: &mut egui::Ui,
    profile: TypographyProfile,
    tokens: &DesignTokens,
    scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    let tokens = tokens.with_typography_profile(profile);
    for (instance, text, role) in [
        (700, "Linked observations", TextRole::ApplicationTitle),
        (701, "Current selection", TextRole::PaneTitle),
        (702, "Evidence and interpretation", TextRole::SectionHeading),
        (
            703,
            "Inspect the selected observation and its supporting evidence.",
            TextRole::Body,
        ),
        (
            704,
            "Last observation received · 12 minutes ago",
            TextRole::Secondary,
        ),
    ] {
        measured_content_label(
            ui,
            instance,
            text,
            role,
            TextOverflow::Wrap,
            2,
            TextInteraction::Selectable,
            &tokens,
            scale,
            observations,
        );
    }
    ui.separator();
    ui.label(
        TextRole::SectionHeading
            .style(&tokens, scale)
            .rich_text("Native semantic heading"),
    );
    measured_content_label(
        ui,
        705,
        "Content-sized: one line with a two-line limit",
        TextRole::Caption,
        TextOverflow::Wrap,
        2,
        TextInteraction::Inert,
        &tokens,
        scale,
        observations,
    );
    measured_fixed_slot_label(
        ui,
        706,
        "Fixed slot: two deliberate lines",
        TextRole::Caption,
        TextOverflow::Wrap,
        2,
        TextInteraction::Inert,
        &tokens,
        scale,
        observations,
    );
}

/// The seven emoji a chat application's users typed that egui's own faces
/// lack, and the sequences that a monochrome face draws as one glyph or
/// simplifies.
const EMOJI_LINE: &str = "Brain 🧠 hug 🤗 peek 🫣 stop 🛑 laugh 🤣 goose 🪿 rain 🌧";
const SEQUENCE_LINE: &str = "Family 👨‍👩‍👧 flag 🇦🇺 pride 🏳️‍🌈 toned 👍🏽 keycap 1️⃣ heart ❤️";
const EMOJI_NOTE: &str = "Emoji draw in the text colour: egui fills glyph outlines and has no \
colour glyphs. A skin tone draws as its untoned emoji and a keycap as its digit.";

/// Emoji set in each kind of text role, then the sequences and their limits.
pub(super) fn emoji_story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) {
    for (instance, text, role) in [
        (710, "Emoji in text roles", TextRole::SectionHeading),
        (711, EMOJI_LINE, TextRole::Body),
        (712, EMOJI_LINE, TextRole::ButtonLabel),
        (713, EMOJI_LINE, TextRole::Secondary),
        (714, EMOJI_LINE, TextRole::MonospaceTechnical),
        (715, "Sequences", TextRole::SectionHeading),
        (716, SEQUENCE_LINE, TextRole::Body),
        (717, EMOJI_NOTE, TextRole::Caption),
    ] {
        measured_content_label(
            ui,
            instance,
            text,
            role,
            TextOverflow::Wrap,
            stobie_ui_egui::MAX_TEXT_LINES,
            TextInteraction::Selectable,
            tokens,
            scale,
            observations,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_story_contains_every_required_role_and_semantic_string() {
        for profile in [TypographyProfile::Dense, TypographyProfile::Reading] {
            let context = egui::Context::default();
            stobie_ui_egui::install_typography_fonts(&context);
            context.enable_accesskit();
            let tokens = DesignTokens::resolve(
                stobie_ui_egui::ThemeVariant::Dark,
                stobie_ui_egui::DensityVariant::Comfortable,
            );
            let mut observations = Vec::new();
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 560.0),
                    )),
                    ..Default::default()
                },
                |ui| story(ui, profile, &tokens, 1.5, &mut observations),
            );
            assert_eq!(
                observations
                    .iter()
                    .map(|item| item.component_id.instance)
                    .collect::<Vec<_>>(),
                vec![700, 701, 702, 703, 704, 705, 706]
            );
            for role in [
                TextRole::ApplicationTitle,
                TextRole::PaneTitle,
                TextRole::SectionHeading,
                TextRole::Body,
                TextRole::Secondary,
            ] {
                assert!(observations.iter().any(|item| item.role == role));
            }
            let update = output.platform_output.accesskit_update.take().unwrap();
            for required in [
                "Linked observations",
                "Current selection",
                "Evidence and interpretation",
                "Inspect the selected observation and its supporting evidence.",
                "Last observation received · 12 minutes ago",
                "Native semantic heading",
            ] {
                assert!(
                    update
                        .nodes
                        .iter()
                        .any(|(_, node)| node.value() == Some(required)),
                    "missing required semantic content: {required}"
                );
            }
            assert!(stobie_ui_egui::audit_text_layouts(&observations).is_empty());
            output.textures_delta.clear();
        }
    }

    #[test]
    fn emoji_story_sets_every_emoji_in_each_role_kind() {
        let context = egui::Context::default();
        stobie_ui_egui::install_typography_fonts(&context);
        context.enable_accesskit();
        let tokens = DesignTokens::resolve(
            stobie_ui_egui::ThemeVariant::Light,
            stobie_ui_egui::DensityVariant::Comfortable,
        );
        let mut observations = Vec::new();
        let mut output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320.0, 560.0),
                )),
                ..Default::default()
            },
            |ui| emoji_story(ui, &tokens, 1.5, &mut observations),
        );
        assert_eq!(
            observations
                .iter()
                .map(|item| item.component_id.instance)
                .collect::<Vec<_>>(),
            (710..=717).collect::<Vec<_>>()
        );
        for role in [
            TextRole::Body,
            TextRole::ButtonLabel,
            TextRole::Secondary,
            TextRole::MonospaceTechnical,
        ] {
            assert!(observations.iter().any(|item| item.role == role));
        }
        for emoji in ["🧠", "🤗", "🫣", "🛑", "🤣", "🪿", "🌧"] {
            assert!(EMOJI_LINE.contains(emoji));
        }
        let update = output.platform_output.accesskit_update.take().unwrap();
        for required in [EMOJI_LINE, SEQUENCE_LINE, EMOJI_NOTE] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some(required)),
                "missing required semantic content: {required}"
            );
        }
        assert!(stobie_ui_egui::audit_text_layouts(&observations).is_empty());
        output.textures_delta.clear();
    }
}
