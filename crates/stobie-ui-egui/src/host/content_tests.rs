#![cfg(test)]
//! Content text in observation: the bounded words of presented content text,
//! withheld where the application asks, and never a password.

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, CONTENT_EXCERPT_CHARS, CONTENT_TRUNCATION_MARKER, ContentTextSpec, InspectionOperation,
    InspectionResult, LinkSpec, MarkdownSpec, NativeTextControlKind, PresentationScope,
    ReadingTextSpec, TextComponentKind, TextContent, TextInteraction, TextOverflow, TextRole,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "notes")]
enum Act {
    #[action(label = "Archive everything", description = "Archive every note")]
    Archive,
}

const SECRET_CODE: &str = "Invitation code QX7-SECRET-4410";
const PASSWORD: &str = "hunter2-correct-horse";
const LABEL: &str = "Morning digest: five papers on reasoning, two on efficient inference \
                     and one on evaluation, chosen from today's arXiv listings";
const MARKDOWN: &str =
    "## Today's pick\n\nA *short* note with a [source](https://example.org/paper).";

/// A page of notes: a heading, a label, a long reply, Markdown, a link, a
/// secret shown once, a password field and text far down a scroll area.
struct Notes {
    reply: String,
    password: String,
}

impl Default for Notes {
    fn default() -> Self {
        Self {
            reply: "A long reply. ".repeat(40),
            password: PASSWORD.to_owned(),
        }
    }
}

impl Application for Notes {
    type Action = Act;
    type Intent = String;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Notes")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, _: &mut (), frame: &mut Frame<'_, Act, String>) {
        let mut p = frame.presentation(ui, PresentationScope::new("notes"), SemanticUiId::root());
        p.heading(ui, "heading", "Inbox");
        // One line, so the label elides at this width; its excerpt does not.
        p.content(
            ui,
            "label",
            LABEL,
            ContentTextSpec {
                role: TextRole::Body,
                overflow: TextOverflow::Ellipsis,
                max_lines: 1,
                interaction: TextInteraction::Selectable,
            },
        );
        p.reading_text(ui, "reply", &self.reply, ReadingTextSpec::default());
        p.markdown(ui, "digest", MARKDOWN, MarkdownSpec::default());
        p.link(
            ui,
            "source",
            LinkSpec::new("Read the paper", "https://example.org/paper"),
        );
        crate::withhold_content_text(ui, |ui| {
            p.content(
                ui,
                "code",
                SECRET_CODE,
                ContentTextSpec {
                    role: TextRole::MonospaceTechnical,
                    overflow: TextOverflow::Wrap,
                    max_lines: 2,
                    interaction: TextInteraction::Selectable,
                },
            );
        });
        p.reading_text(
            ui,
            "after",
            "Published again after the region.",
            ReadingTextSpec::default(),
        );
        frame.action(ui, &mut p, Act::Archive, ActionEmphasis::Normal);
        let mut password = self.password.clone();
        let (response, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut password)
                    .password(true)
                    .id(egui::Id::new("notes.password")),
            );
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_label("Password");
                node.set_author_id("notes.password".to_owned());
            });
            (response, ())
        });
        let mut node = UiNode::container(
            SemanticUiId::new("notes.password"),
            Some(SemanticUiId::root()),
            UiRole::TextInput,
            response.rect.into(),
        );
        node.name = "Password".into();
        p.observe_node(ui, node);
        if password != self.password {
            frame.emit(password);
        }
        egui::ScrollArea::vertical()
            .max_height(30.0)
            .show(ui, |ui| {
                p.reading_text(ui, "near", "Near the top.", ReadingTextSpec::default());
                ui.add_space(400.0);
                p.heading(ui, "far-heading", "Heading below the fold");
                p.content(
                    ui,
                    "far-label",
                    "Label below the fold",
                    ContentTextSpec {
                        role: TextRole::Body,
                        overflow: TextOverflow::Wrap,
                        max_lines: 2,
                        interaction: TextInteraction::Selectable,
                    },
                );
                p.link(
                    ui,
                    "far-link",
                    LinkSpec::new("Link below the fold", "https://example.org/far"),
                );
                p.reading_text(ui, "far", "Far below the fold.", ReadingTextSpec::default());
            });
    }

    fn apply(&mut self, intent: String, _: &mut ()) -> Result<bool, String> {
        self.password = intent;
        Ok(true)
    }

    fn perform(&mut self, _target: ActionTarget<Act>, _: &mut ()) -> Result<bool, String> {
        Ok(false)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Notes>>,
}

fn harness(notes: Notes) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(360.0, 900.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, notes);
    host.enable_inspection(Inspection::new("notes-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Notes> {
    harness.state().host.as_ref().unwrap()
}

/// The observation as an agent receives it, serialised as on the wire.
fn published_json(harness: &Harness<'_, Probe>) -> String {
    let host = host(harness);
    let reply = host.inspection().unwrap().handle(InspectionRequest {
        version: 1,
        request_id: "observe".into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation: InspectionOperation::Observe,
    });
    let Some(InspectionResult::Observe { observation }) = &reply.result else {
        panic!("observe failed: {:?}", reply.error);
    };
    // The published observation is the host's snapshot.
    assert_eq!(observation.snapshot.text, host.ui_snapshot().text);
    serde_json::to_string(&reply).unwrap()
}

fn content_of(harness: &Harness<'_, Probe>, kind: TextComponentKind, key: &str) -> TextContent {
    let instance = PresentationScope::new("notes")
        .instance(key)
        .text_instance();
    host(harness)
        .ui_snapshot()
        .text
        .iter()
        .find(|text| text.component_id.kind == kind && text.component_id.instance == instance)
        .unwrap_or_else(|| panic!("no {kind:?} text for {key}"))
        .content
        .clone()
        .unwrap_or_else(|| panic!("{kind:?} text for {key} has no content"))
}

#[test]
fn presented_content_text_is_observed_in_presentation_order() {
    let harness = harness(Notes::default());
    let snapshot = host(&harness).ui_snapshot();
    let content: Vec<&str> = snapshot.content_text().collect();
    assert_eq!(content[0], "Inbox");
    assert_eq!(content[1], LABEL);
    assert!(content.contains(&"Today's pick"), "{content:?}");
    // Markdown publishes the presented words, not its source.
    assert!(
        content.contains(&"A short note with a source."),
        "{content:?}"
    );
    assert!(content.contains(&"Read the paper"), "{content:?}");
    assert!(content.contains(&"Near the top."), "{content:?}");
    assert!(
        content.contains(&"Published again after the region."),
        "{content:?}"
    );
    // The label is elided on screen, but its excerpt is the whole text.
    let label = snapshot
        .text
        .iter()
        .find(|text| text.content_text() == Some(LABEL))
        .unwrap();
    assert!(label.truncated);
    assert_eq!(
        content_of(&harness, TextComponentKind::ContentLabel, "label"),
        TextContent::Excerpt {
            text: LABEL.into(),
            truncated: false
        }
    );
    // The text of controls stays with their semantic nodes.
    assert!(!content.iter().any(|text| text.contains("Archive")));
    assert!(
        snapshot
            .text
            .iter()
            .filter(|text| !text.component_id.kind.presents_content())
            .all(|text| text.content.is_none())
    );
    let json = published_json(&harness);
    assert!(
        json.contains(r#""content":{"kind":"excerpt","text":"Inbox","truncated":false}"#),
        "{json}"
    );
}

#[test]
fn long_content_is_published_as_a_bounded_excerpt_with_a_marker() {
    let short = harness(Notes::default());
    let reply = "A long reply. ".repeat(40);
    let TextContent::Excerpt { text, truncated } =
        content_of(&short, TextComponentKind::ReadingText, "reply")
    else {
        panic!("an excerpt");
    };
    assert!(truncated);
    // The bound itself is the protocol's: the first characters, then a marker.
    assert_eq!(
        TextContent::excerpt(&reply),
        TextContent::Excerpt {
            text: text.clone(),
            truncated
        }
    );
    assert_eq!(
        text.chars().nth(CONTENT_EXCERPT_CHARS),
        Some(CONTENT_TRUNCATION_MARKER)
    );
    assert!(reply.starts_with(text.trim_end_matches(CONTENT_TRUNCATION_MARKER)));
    // Every excerpt stays within the bound, however long the text.
    let long = harness(Notes {
        reply: "word ".repeat(20_000),
        ..Notes::default()
    });
    assert!(
        long.state()
            .host
            .as_ref()
            .unwrap()
            .ui_snapshot()
            .content_text()
            .all(|text| text.chars().nth(CONTENT_EXCERPT_CHARS + 1).is_none())
    );
    assert!(published_json(&long).len() < 64 * 1024);
}

#[test]
fn withheld_content_publishes_neither_its_text_nor_its_length() {
    let harness = harness(Notes::default());
    assert_eq!(
        content_of(&harness, TextComponentKind::ContentLabel, "code"),
        TextContent::Withheld
    );
    let json = published_json(&harness);
    assert!(!json.contains("SECRET"), "{json}");
    assert!(json.contains(r#""content":{"kind":"withheld"}"#), "{json}");
    // Its layout is still measured and audited, and passes.
    let code = PresentationScope::new("notes")
        .instance("code")
        .text_instance();
    let snapshot = host(&harness).ui_snapshot();
    let withheld = snapshot
        .text
        .iter()
        .find(|text| text.component_id.instance == code)
        .unwrap();
    assert!(withheld.line_count > 0 && withheld.layout_error.is_none());
    assert!(snapshot.text_audit_coverage.is_some(), "the text audit ran");
    assert!(
        !format!("{:?}", snapshot.text_audit).contains(&code.to_string()),
        "{:?}",
        snapshot.text_audit
    );
}

#[test]
fn a_password_never_reaches_observation() {
    let harness = harness(Notes::default());
    let snapshot = host(&harness).ui_snapshot();
    let field = snapshot.node(&SemanticUiId::new("notes.password")).unwrap();
    assert_eq!(field.value, None);
    let json = published_json(&harness);
    assert!(!json.contains(PASSWORD), "{json}");
    assert!(!json.contains("hunter2"), "{json}");
}

#[test]
fn text_scrolled_out_of_view_is_not_observed() {
    let harness = harness(Notes::default());
    let content: Vec<&str> = host(&harness).ui_snapshot().content_text().collect();
    assert!(content.contains(&"Near the top."));
    // No kind publishes text out of view, in a host with no pane to limit
    // observation to visible text.
    assert!(
        !content.iter().any(|text| text.contains("below the fold")),
        "{content:?}"
    );
    let json = published_json(&harness);
    assert!(!json.contains("below the fold"), "{json}");
}
