use eframe::egui;
use polyorama_ui_egui::{
    DesignTokens, TextInteraction, TextLayoutObservation, TextOverflow, TextRole,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hierarchy_story_contains_every_required_role_and_semantic_string() {
        for profile in [TypographyProfile::Dense, TypographyProfile::Reading] {
            let context = egui::Context::default();
            polyorama_ui_egui::install_typography_fonts(&context);
            context.enable_accesskit();
            let tokens = DesignTokens::resolve(
                polyorama_ui_egui::ThemeVariant::Dark,
                polyorama_ui_egui::DensityVariant::Comfortable,
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
            assert!(polyorama_ui_egui::audit_text_layouts(&observations).is_empty());
            output.textures_delta.clear();
        }
    }
}
