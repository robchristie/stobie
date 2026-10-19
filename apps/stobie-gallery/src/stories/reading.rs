use eframe::egui;
use stobie_ui_egui::{
    ContentTextSpec, DesignTokens, LinkSpec, PresentationContext, PresentationScope,
    ReadingTextSpec, SemanticUiId, TextInteraction, TextOverflow, TextRole, observe_visible_only,
};

use super::StoryReport;
use crate::catalogue::StoryId;

const REQUEST: &str = "Summarise this morning's papers and link the sources.";

const REPLY: &str = "Two papers stand out this morning.\n\n\
The first describes an attention layout for long documents that keeps memory \
use flat as the context grows. Its evaluation covers question answering over \
books and retrieval across a hundred thousand tokens, and the gains hold on \
smaller models as well as larger ones, which makes it practical on a laptop.\n\n\
The second surveys how agents that call tools are evaluated. It separates \
task success from the cost of reaching it, argues that most published \
results report only the first, and proposes a short checklist for honest \
comparisons: fixed budgets, repeated runs and published transcripts. Both \
links open in a new tab, so this conversation stays where it is.";

const FOLLOW_UP: &str = "Thanks. I'll read the survey after lunch.";

const NOTICE: &str = "Révision complète : trois sources ont changé d'adresse. \
The longest identifier was \
evaluation-run-2026-10-19-reasoning-coding-retrieval-final-candidate.";

const LONG_TEXT: &str = "Größere Schrift, schmale Spalte und hoher Kontrast bleiben lesbar.\n\n\
Every line here is measured, wrapped at the column's width and kept: \
nothing is cut off with an ellipsis, however long the reply grows. A scroll \
area clips what lies below the fold, and the observation records that clip \
rather than reporting the text as overflowing.";

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

/// The conversation scrolls, so it publishes only what is visible: bounded
/// text is audited once fully visible, while reading text and links keep
/// their clipped observations as they cross the fold.
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
        PresentationScope::new("gallery.reading"),
        root,
    );
    let gap = tokens.spacing.section.0;
    egui::ScrollArea::vertical()
        .id_salt("reading-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if story == StoryId::ReadingConversation {
                p.scoped(ui, "request", |ui, p| {
                    byline(ui, p, "You");
                    p.reading_text(ui, "body", REQUEST, ReadingTextSpec::default());
                });
                ui.add_space(gap);
                p.scoped(ui, "reply", |ui, p| {
                    byline(ui, p, "Assistant");
                    p.reading_text(ui, "body", REPLY, ReadingTextSpec::default());
                    ui.add_space(tokens.spacing.block.0);
                    p.link(
                        ui,
                        "attention",
                        LinkSpec::new(
                            "Flat-memory attention for long documents",
                            "https://example.org/papers/flat-memory-attention",
                        ),
                    );
                    p.link(
                        ui,
                        "survey",
                        LinkSpec::new(
                            "Evaluating agents that call tools: a survey",
                            "https://example.org/papers/evaluating-tool-agents",
                        ),
                    );
                });
                ui.add_space(gap);
                p.scoped(ui, "follow-up", |ui, p| {
                    byline(ui, p, "You");
                    p.reading_text(ui, "body", FOLLOW_UP, ReadingTextSpec::default());
                });
            } else {
                p.scoped(ui, "notice", |ui, p| {
                    byline(ui, p, "Assistant · a long reply in a narrow column");
                    p.reading_text(ui, "body", NOTICE, ReadingTextSpec::default());
                    ui.add_space(tokens.spacing.block.0);
                    p.link(
                        ui,
                        "long",
                        LinkSpec::new(
                            "Vollständiger Bericht über die nächtliche Auswertung aller Quellen",
                            "https://example.org/reports/nightly-evaluation",
                        ),
                    );
                    // A blank label shows the URL itself.
                    p.link(ui, "bare", LinkSpec::new("", "https://example.org/changes"));
                    ui.add_enabled_ui(false, |ui| {
                        p.link(
                            ui,
                            "unavailable",
                            LinkSpec::new(
                                "Archived copy (unavailable offline)",
                                "https://example.org/archive",
                            ),
                        );
                    });
                    ui.add_space(gap);
                    // This text crosses the fold: it is observed with the
                    // scroll area's clip. The copy after it starts below the
                    // fold and adds no observation until scrolled into view.
                    p.reading_text(ui, "detail", LONG_TEXT, ReadingTextSpec::default());
                    ui.add_space(gap);
                    p.reading_text(ui, "after", LONG_TEXT, ReadingTextSpec::default());
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
    fn conversation_reply_runs_past_eight_lines_with_new_tab_links_and_clean_audits() {
        let (snapshot, mut output) = probe(
            StoryId::ReadingConversation,
            ThemeVariant::Dark,
            1.0,
            egui::vec2(616.0, 536.0),
        );
        assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
        let reading: Vec<_> = snapshot
            .text
            .iter()
            .filter(|text| text.component_id.kind == TextComponentKind::ReadingText)
            .collect();
        assert_eq!(reading.len(), 3);
        assert!(reading.iter().any(|text| text.line_count > 8));
        let links: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Link)
            .collect();
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|link| {
            link.value
                .as_deref()
                .is_some_and(|url| url.starts_with("https://example.org/"))
        }));
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn narrow_enlarged_reading_keeps_partial_text_and_omits_text_below_the_fold() {
        let size = egui::vec2(296.0, 576.0);
        let (snapshot, mut output) = probe(
            StoryId::ReadingLongNarrow,
            ThemeVariant::LightHighContrast,
            1.5,
            size,
        );
        assert!(
            audit_text_layouts(&snapshot.text).is_empty(),
            "{:#?}",
            audit_text_layouts(&snapshot.text)
        );
        let reading: Vec<_> = snapshot
            .text
            .iter()
            .filter(|text| text.component_id.kind == TextComponentKind::ReadingText)
            .collect();
        // The notice is whole, the detail crosses the fold and the copy after
        // it starts below the fold, adding no observation.
        assert_eq!(reading.len(), 2, "{reading:#?}");
        assert!(reading.iter().all(|text| text.line_count > 4));
        let crossing = reading[1];
        assert!(crossing.allocated_rect.max_y > crossing.clip_rect.max_y);
        let coverage = snapshot.text_audit_coverage.clone().unwrap();
        assert!(coverage.attempted_components > coverage.measured_components);
        let links: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Link)
            .collect();
        assert_eq!(links.len(), 3);
        assert!(
            links
                .iter()
                .any(|link| link.name == "https://example.org/changes")
        );
        assert!(links.iter().any(|link| !link.enabled));
        let long = snapshot
            .text
            .iter()
            .find(|text| text.component_id.kind == TextComponentKind::Link)
            .unwrap();
        assert!(long.line_count > 1 && !long.truncated);
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
    }
}
