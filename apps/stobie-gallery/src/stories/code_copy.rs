use eframe::egui;
use stobie_ui_egui::{
    CodeCopySpec, DesignTokens, MarkdownSpec, MessageBubbleSpec, MessageSide, PresentationContext,
    PresentationScope, ReadingTextSpec, SemanticUiId, observe_visible_only,
};

use super::StoryReport;

const REQUEST: &str = "How do I run the checks before I push?";

/// A reply with two code blocks: a short command, and a longer one whose
/// lines wrap in a narrow column.
pub(crate) const REPLY: &str = "Install the tools once:\n\n\
```sh\ncargo install --locked cargo-nextest\n```\n\n\
Then run the whole suite, which stops at the first failure:\n\n\
```sh\ncargo nextest run --workspace --all-features\n\
cargo clippy --workspace --all-targets -- -D warnings\n```\n\n\
Both print a summary at the end.";

/// A conversation whose reply has a copy control on each code block. The
/// page scrolls, so it publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| conversation(ui, tokens, scale, report));
}

fn conversation(ui: &mut egui::Ui, tokens: &DesignTokens, scale: f32, report: &mut StoryReport) {
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.code-copy"),
        SemanticUiId::new("gallery.story"),
    );
    let gap = tokens.spacing.block.0;
    egui::ScrollArea::vertical()
        .id_salt("code-copy-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let own = MessageBubbleSpec {
                side: MessageSide::Own,
                sender: "You",
            };
            p.message_bubble(ui, ("message", 1), own, |ui, p| {
                p.reading_text(ui, "body", REQUEST, ReadingTextSpec::default());
            });
            ui.add_space(gap);
            let reply = MessageBubbleSpec {
                side: MessageSide::Other,
                sender: "Assistant",
            };
            p.message_bubble(ui, ("message", 2), reply, |ui, p| {
                p.markdown_with_code_copy(
                    ui,
                    "body",
                    REPLY,
                    MarkdownSpec::default(),
                    &CodeCopySpec::new("Copy code", "Copied", "Couldn't copy"),
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
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        render(
            &context,
            StoryProbe {
                story: StoryId::MarkdownCodeCopy,
                tokens: DesignTokens::resolve(variant, DensityVariant::Comfortable),
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
    fn each_code_block_has_its_own_named_copy_control_in_the_reply() {
        let (snapshot, mut output) = probe(ThemeVariant::Dark, 1.0, egui::vec2(616.0, 900.0));
        assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
        let reply = snapshot
            .nodes
            .iter()
            .find(|node| node.role == UiRole::Section && node.name == "Assistant")
            .unwrap();
        let buttons: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Button)
            .map(|node| (node.name.as_str(), node.parent.as_ref() == Some(&reply.id)))
            .collect();
        assert_eq!(buttons, [("Copy code", true), ("Copy code 2", true)]);
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
        output.textures_delta.clear();
    }

    #[test]
    fn a_narrow_column_at_larger_text_wraps_the_code_beneath_its_control() {
        let (snapshot, mut output) = probe(
            ThemeVariant::LightHighContrast,
            1.5,
            egui::vec2(296.0, 1600.0),
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        let buttons: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Button)
            .map(|node| egui::Rect::from(node.rect))
            .collect();
        assert_eq!(buttons.len(), 2);
        for button in &buttons {
            assert!(button.right() <= 296.0 + 0.5, "{button:?}");
        }
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        output.textures_delta.clear();
    }
}
