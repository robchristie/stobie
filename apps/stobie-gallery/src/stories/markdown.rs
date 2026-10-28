use eframe::egui;
use stobie_ui_egui::{
    ContentTextSpec, DEFAULT_MARKDOWN_OPTIONS, DesignTokens, MarkdownOptions, MarkdownSpec,
    PresentationContext, PresentationScope, ReadingTextSpec, SemanticUiId, TextInteraction,
    TextOverflow, TextRole, markdown_source_map, observe_visible_only,
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

/// The extensions a knowledge document uses, short of tables.
const DOCUMENT_OPTIONS: MarkdownOptions = DEFAULT_MARKDOWN_OPTIONS
    .union(MarkdownOptions::ENABLE_TASKLISTS)
    .union(MarkdownOptions::ENABLE_FOOTNOTES)
    .union(MarkdownOptions::ENABLE_MATH);

/// Field notes as a document repository holds them, with CRLF line endings:
/// tasks (one whose marker stands alone, its text a paragraph after the list,
/// as pulldown-cmark reads it), footnotes (one opening with a list), and
/// inline and display maths.
pub(crate) const DOCUMENT: &str = "# Rock-pool survey\r\n\r\n\
Notes from three low tides on the northern platform, with the open tasks and \
the arithmetic behind each count.[^tides]\r\n\r\n\
## Open tasks\r\n\r\n\
- [x] Photograph the northern pools\r\n\
- [ ] Count the anemones in pool C[^count]\r\n\
- [ ]\r\n\r\n  \
Compare the counts with last season's survey, which used a coarser grid.\r\n\r\n\
## Density\r\n\r\n\
Density is $d = n / A$ for $n$ animals in an area $A$, so pool C holds:\r\n\r\n\
$$\r\nd_C = \\frac{42}{3.5\\,\\mathrm{m}^2} \\approx 12\\ \\mathrm{m}^{-2}\r\n$$\r\n\r\n\
Pool A was too deep to grid on the second tide, so its estimate rests on two \
counts. Pool B dried out at the edges, and the count there includes only the \
wet area. Pool D is new this season.\r\n\r\n\
## Method\r\n\r\n\
Each count follows the grid laid at the first tide, read from the seaward \
corner. The tally sheets are in the appendix, with the photographs filed by \
pool and tide.\r\n\r\n\
[^tides]: Each visit began an hour before low water.\r\n\r\n\
[^count]: - Anemones closed by the sun were counted once.\r\n    \
- Animals on the grid line were counted with the square to their north.\r\n";

/// A heading's ID as an application might derive it: the document's name and
/// the heading's text.
fn heading_id(text: &str) -> SemanticUiId {
    let slug: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    SemanticUiId::new(format!("gallery.document.{slug}"))
}

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
        .id_salt(("markdown-scroll", story.as_str()))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if story == StoryId::MarkdownDocument {
                let map = markdown_source_map(DOCUMENT, DOCUMENT_OPTIONS, 0);
                let headings: Vec<_> = map.headings.iter().map(|h| heading_id(&h.text)).collect();
                let footnotes: Vec<_> = map
                    .footnotes
                    .iter()
                    .map(|f| SemanticUiId::new(format!("gallery.document.footnote-{}", f.label)))
                    .collect();
                let spec = MarkdownSpec {
                    options: DOCUMENT_OPTIONS,
                    heading_ids: &headings,
                    footnote_ids: &footnotes,
                    ..MarkdownSpec::default()
                };
                p.markdown(ui, "document", DOCUMENT, spec);
            } else if story == StoryId::MarkdownDigest {
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
        ApplicationTheme, DensityVariant, TextComponentKind, TextContent, ThemeVariant, UiRole,
        audit_accesskit, audit_text_layouts,
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

    /// The document story at phone and desktop widths, at 100% and 150%
    /// text, in dark and light high contrast: its read-only task boxes, its
    /// headings with the application's IDs, a footnote reference and maths
    /// in its technical face, with clean text and AccessKit audits.
    #[test]
    fn document_presents_tasks_headings_footnotes_and_maths_at_each_width_and_contrast() {
        let headings = [
            ("gallery.document.rock-pool-survey", "Rock-pool survey"),
            ("gallery.document.open-tasks", "Open tasks"),
        ];
        for (variant, scale, size) in [
            (ThemeVariant::Dark, 1.0, egui::vec2(616.0, 1600.0)),
            (
                ThemeVariant::LightHighContrast,
                1.5,
                egui::vec2(296.0, 3200.0),
            ),
        ] {
            let (snapshot, mut output) = probe(StoryId::MarkdownDocument, variant, scale, size);
            let label = format!("{variant:?} at {scale}");
            let findings = audit_text_layouts(&snapshot.text);
            assert!(findings.is_empty(), "{label}: {findings:#?}");
            let tasks: Vec<_> = snapshot
                .nodes
                .iter()
                .filter(|node| node.role == UiRole::Checkbox)
                .map(|node| (node.name.as_str(), node.checked, node.enabled))
                .collect();
            assert_eq!(
                tasks,
                [
                    ("Photograph the northern pools", Some(true), false),
                    ("Count the anemones in pool C[2]", Some(false), false),
                    // pulldown-cmark ends this item at the blank line, so
                    // its text is a paragraph after the list.
                    ("Task", Some(false), false),
                ],
                "{label}"
            );
            for (id, name) in headings {
                let node = snapshot
                    .nodes
                    .iter()
                    .find(|node| node.id.0 == id)
                    .unwrap_or_else(|| panic!("{label}: heading {id}"));
                assert_eq!(
                    (node.role, node.name.as_str()),
                    (UiRole::Heading, name),
                    "{label}"
                );
            }
            let reference = snapshot
                .nodes
                .iter()
                .find(|node| node.role == UiRole::Link && node.name == "[1]")
                .unwrap_or_else(|| panic!("{label}: the first footnote reference"));
            assert!(
                reference.enabled && reference.disabled_reason.is_none(),
                "{label}"
            );
            for id in [
                "gallery.document.footnote-tides",
                "gallery.document.footnote-count",
            ] {
                let note = snapshot.nodes.iter().find(|node| node.id.0 == id);
                assert_eq!(
                    note.map(|node| node.role),
                    Some(UiRole::Section),
                    "{label}: {id}"
                );
            }
            assert!(
                snapshot.text.iter().any(|text| {
                    text.role == TextRole::MonospaceTechnical
                        && matches!(&text.content, Some(TextContent::Excerpt { text, .. })
                            if text.contains("\\frac{42}") && !text.contains('\r'))
                }),
                "{label}: display maths as source, without a carriage return"
            );
            let update = output.platform_output.accesskit_update.take().unwrap();
            let findings = audit_accesskit(&snapshot, &update);
            assert!(findings.is_empty(), "{label}: {findings:?}");
        }
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
