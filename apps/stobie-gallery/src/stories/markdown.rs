use eframe::egui;
use stobie_ui_egui::{
    ContentTextSpec, DesignTokens, MarkdownSpec, PresentationContext, PresentationScope,
    ReadingTextSpec, SemanticUiId, TextInteraction, TextOverflow, TextRole, observe_visible_only,
};

use super::StoryReport;
use crate::catalogue::StoryId;

const REQUEST: &str = "What came out overnight? Links and one line each, please.";

/// A morning digest as a model writes it: a heading, a numbered list of
/// linked titles with a note each, a short list, a quote and a command.
pub(crate) const DIGEST: &str = "## Morning digest\n\n\
Six items today, **top pick first**.\n\n\
1. [Flat-memory attention for long documents](https://example.org/papers/flat-memory-attention) \
— keeps memory use flat as the context grows, and the gains hold on small models.\n\
2. [Evaluating agents that call tools](https://example.org/papers/evaluating-tool-agents) \
— separates task success from its cost and proposes a short checklist for *honest* comparisons.\n\
3. [Speculative decoding on a phone](https://example.org/papers/speculative-decoding-phone) \
— twice the generation speed on a mid-range handset with `int4` weights.\n\
4. [Probing attention heads for retrieval](https://example.org/papers/retrieval-heads) \
— a handful of heads do most of the copying; ablating them breaks long-context recall.\n\n\
### Also noted\n\n\
- A reproducibility note on reward models ([discussion](https://example.org/discussions/reward-models)).\n\
- ~~Yesterday's benchmark~~ was withdrawn by its authors.\n\n\
> Two sources were unreachable this morning; they will be tried again tomorrow.\n\n\
Try the first one locally:\n\n\
```sh\npip install flat-memory-attention\npython -m flat_memory_attention.demo --context 128k\n```";

/// Adversarial Markdown: deep and mixed nesting, unsafe links, raw HTML,
/// a table and an image as text, a long code line and an unbroken
/// identifier, ending in a reply still arriving.
pub(crate) const ELEMENTS: &str = "# Vollständiger Bericht\n\n\
Größere Schrift, *schmale* Spalte: \
evaluation-run-2026-10-19-reasoning-coding-retrieval-final-candidate.\n\n\
```\nlet run = \"evaluation-run-2026-10-19-reasoning-coding-retrieval\";\n```\n\n\
Unsafe: [run script](javascript:alert(1)), [relative](/settings) and \
<b>raw HTML</b> stay text; ![a chart of results](https://example.org/chart.png) \
shows its description.\n\n\
1. First level\n   - Second level with a [long link label that wraps across lines](https://example.org/reports/nightly)\n     \
1. Third level\n        - Fourth level\n2. Back to the first\n\n\
| source | items |\n| --- | --- |\n| arXiv | 4 |\n\n\
---\n\n\
The reply is still arriving: [the survey](https://example.org/papers/survey) \
and **partial text with an open [link](https://exam";

/// Mail and a link inside a disabled region.
const DISABLED: &str = "Archived: [the original thread](https://example.org/archive) \
and <reports@example.org>.";

fn byline(ui: &mut egui::Ui, p: &mut PresentationContext, text: &str) {
    p.content(
        ui,
        "byline",
        text,
        ContentTextSpec {
            role: TextRole::Caption,
            overflow: TextOverflow::Ellipsis,
            max_lines: 1,
            interaction: TextInteraction::Selectable,
        },
    );
}

/// The conversation scrolls, so it publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| conversation(ui, story, tokens, scale, report));
}

fn conversation(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    report: &mut StoryReport,
) {
    let root = SemanticUiId::new("gallery.story");
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.markdown"),
        root,
    );
    let gap = tokens.spacing.section.0;
    let spec = MarkdownSpec {
        link_description: Some("Opens in a new tab"),
        ..MarkdownSpec::default()
    };
    egui::ScrollArea::vertical()
        .id_salt("markdown-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if story == StoryId::MarkdownDigest {
                p.scoped(ui, "request", |ui, p| {
                    byline(ui, p, "You");
                    p.reading_text(ui, "body", REQUEST, ReadingTextSpec::default());
                });
                ui.add_space(gap);
                p.scoped(ui, "reply", |ui, p| {
                    byline(ui, p, "Assistant");
                    p.markdown(ui, "body", DIGEST, spec);
                });
            } else {
                p.scoped(ui, "reply", |ui, p| {
                    byline(ui, p, "Assistant · Markdown in a narrow column");
                    p.markdown(ui, "body", ELEMENTS, spec);
                });
                ui.add_space(gap);
                p.scoped(ui, "archived", |ui, p| {
                    byline(ui, p, "Assistant · unavailable");
                    ui.add_enabled_ui(false, |ui| {
                        p.markdown(ui, "body", DISABLED, spec);
                    });
                });
            }
        });
    report.presented(&p.finish(ui));
}

#[cfg(test)]
mod tests {
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, TextComponentKind, ThemeVariant, UiRole, audit_accesskit,
        audit_text_layouts,
    };

    use super::*;
    use crate::stories::probe::{StoryProbe, render};

    fn probe(
        story: StoryId,
        variant: ThemeVariant,
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        let tokens = DesignTokens::resolve(variant, DensityVariant::Comfortable);
        render(
            &context,
            StoryProbe {
                story,
                tokens,
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
    fn digest_presents_numbered_linked_titles_with_clean_audits() {
        let (snapshot, mut output) = probe(
            StoryId::MarkdownDigest,
            ThemeVariant::Dark,
            1.0,
            egui::vec2(616.0, 536.0),
        );
        assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
        let markers = snapshot
            .text
            .iter()
            .filter(|text| text.component_id.kind == TextComponentKind::ListMarker)
            .count();
        assert_eq!(markers, 4);
        let links: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Link)
            .collect();
        assert_eq!(links.len(), 5);
        assert_eq!(links[0].name, "Flat-memory attention for long documents");
        assert!(
            links
                .iter()
                .all(|link| link.enabled
                    && link.description.as_deref() == Some("Opens in a new tab"))
        );
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn narrow_markdown_keeps_unsafe_links_unavailable_and_text_whole() {
        let (snapshot, mut output) = probe(
            StoryId::MarkdownLongNarrow,
            ThemeVariant::LightHighContrast,
            1.5,
            egui::vec2(296.0, 576.0),
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        assert!(
            snapshot
                .text
                .iter()
                .filter(|text| text.component_id.kind == TextComponentKind::ReadingText)
                .all(|text| !text.truncated)
        );
        let links: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Link)
            .collect();
        let unavailable: Vec<_> = links
            .iter()
            .filter(|link| link.disabled_reason.is_some())
            .map(|link| link.name.as_str())
            .collect();
        assert_eq!(unavailable, ["run script", "relative"]);
        // An unfinished link is still text; the finished one before it links.
        assert!(links.iter().all(|link| link.name != "link"));
        let coverage = snapshot.text_audit_coverage.clone().unwrap();
        assert!(coverage.attempted_components > coverage.measured_components);
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
    }
}
