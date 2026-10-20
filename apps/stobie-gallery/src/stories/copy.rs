use eframe::egui;
use stobie_ui_egui::{
    ActionEmphasis, Availability, ContentTextSpec, CopyContent, CopySpec, DesignTokens,
    PresentationContext, PresentationScope, ReadingTextSpec, SemanticUiId, TextInteraction,
    TextOverflow, TextRole, observe_visible_only,
};

use super::StoryReport;

/// The invitation code the story copies; the browser smoke reads it back
/// from the clipboard.
pub(crate) const INVITATION_CODE: &str = "QX7-4410-KM2P";

const MESSAGE: &str = "The meeting moved to Thursday at 10:30. Bring the printed agenda \
and the two reports; the room is on the third floor.";

const HEADING: ContentTextSpec = ContentTextSpec {
    role: TextRole::SectionHeading,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

const CODE: ContentTextSpec = ContentTextSpec {
    role: TextRole::MonospaceTechnical,
    overflow: TextOverflow::Wrap,
    max_lines: 2,
    interaction: TextInteraction::Selectable,
};

/// Copy controls as applications use them: beside a code, under a message,
/// with long localised words and unavailable. The page scrolls, so it
/// publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| controls(ui, tokens, scale, report));
}

fn controls(ui: &mut egui::Ui, tokens: &DesignTokens, scale: f32, report: &mut StoryReport) {
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.copy"),
        SemanticUiId::new("gallery.story"),
    );
    let gap = tokens.spacing.section.0;
    egui::ScrollArea::vertical()
        .id_salt("copy-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            p.scoped(ui, "invitation", |ui, p| {
                p.content(ui, "heading", "Invitation code", HEADING);
                p.content(ui, "code", INVITATION_CODE, CODE);
                p.copy(
                    ui,
                    "copy",
                    &CopySpec::new(INVITATION_CODE, "Copy code", "Copied", "Couldn't copy"),
                );
            });
            ui.add_space(gap);
            p.scoped(ui, "message", |ui, p| {
                p.content(ui, "heading", "Message", HEADING);
                p.reading_text(ui, "body", MESSAGE, ReadingTextSpec::default());
                ui.horizontal(|ui| {
                    p.copy(
                        ui,
                        "copy",
                        &CopySpec {
                            content: CopyContent::Icon,
                            emphasis: ActionEmphasis::QuietBorderless,
                            ..CopySpec::new(MESSAGE, "Copy message", "Copied", "Couldn't copy")
                        },
                    );
                });
            });
            ui.add_space(gap);
            p.scoped(ui, "localised", |ui, p| {
                p.content(ui, "heading", "Einladungslink", HEADING);
                p.copy(
                    ui,
                    "copy",
                    &CopySpec {
                        content: CopyContent::Label,
                        emphasis: ActionEmphasis::Normal,
                        ..CopySpec::new(
                            "https://example.org/einladung/QX7-4410-KM2P",
                            "Link kopieren",
                            "In die Zwischenablage kopiert",
                            "Kopieren nicht möglich",
                        )
                    },
                );
            });
            ui.add_space(gap);
            p.scoped(ui, "expired", |ui, p| {
                p.content(ui, "heading", "Expired invitation", HEADING);
                p.copy(
                    ui,
                    "copy",
                    &CopySpec {
                        availability: Availability::Disabled {
                            reason: "The invitation has expired".into(),
                        },
                        ..CopySpec::new("ZK2-9031-AB7Q", "Copy code", "Copied", "Couldn't copy")
                    },
                );
            });
        });
    report.presented(&p.finish(ui));
}

#[cfg(test)]
mod tests {
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, ThemeVariant, UiRole, audit_accesskit, audit_text_layouts,
    };

    use super::*;
    use crate::catalogue::StoryId;
    use crate::stories::probe::{StoryProbe, render};

    fn probe(
        variant: ThemeVariant,
        density: DensityVariant,
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        render(
            &context,
            StoryProbe {
                story: StoryId::CopyActions,
                tokens: DesignTokens::resolve(variant, density),
                scale,
                clip: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                theme: ApplicationTheme::analytical(),
                preferences: Default::default(),
                heading: false,
            },
            size,
        )
    }

    #[test]
    fn copy_controls_are_named_buttons_and_never_publish_their_text() {
        for (variant, density) in [
            (ThemeVariant::Dark, DensityVariant::Comfortable),
            (ThemeVariant::Light, DensityVariant::Compact),
        ] {
            let (snapshot, mut output) = probe(variant, density, 1.0, egui::vec2(616.0, 900.0));
            assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
            let buttons: Vec<_> = snapshot
                .nodes
                .iter()
                .filter(|node| node.role == UiRole::Button)
                .map(|node| (node.name.as_str(), node.enabled))
                .collect();
            assert_eq!(
                buttons,
                [
                    ("Copy code", true),
                    ("Copy message", true),
                    ("Link kopieren", true),
                    ("Copy code", false),
                ]
            );
            // The code is shown as content, but no button carries it.
            let published = serde_json::to_string(&snapshot.nodes).unwrap();
            assert!(!published.contains("ZK2-9031-AB7Q"));
            let update = output.platform_output.accesskit_update.take().unwrap();
            let findings = audit_accesskit(&snapshot, &update);
            assert!(findings.is_empty(), "{findings:?}");
            output.textures_delta.clear();
        }
    }

    #[test]
    fn a_narrow_column_at_larger_text_keeps_every_control_whole() {
        let (snapshot, mut output) = probe(
            ThemeVariant::LightHighContrast,
            DensityVariant::Comfortable,
            1.5,
            egui::vec2(296.0, 900.0),
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        output.textures_delta.clear();
    }
}
