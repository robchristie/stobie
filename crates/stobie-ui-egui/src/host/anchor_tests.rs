#![cfg(test)]
//! Heading anchors in a hosted document reader: a long Markdown document
//! scrolls inside a visible-only region, its headings carry the IDs the
//! application gave them, and an agent or the application reveals a heading
//! below the fold by its ID, at a phone's width and a desktop's.

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, INSPECTION_VERSION, InspectionOperation, InspectionResult, InspectionSelector,
    InvocationReceipt, MarkdownSpec, PresentationScope, REVEAL_CAPABILITY, ReceiptState,
    markdown_source_map,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "reader")]
enum Act {
    #[action(label = "Close", description = "Close the document")]
    Close,
}

enum Intent {
    /// Follow an in-document link to the heading with this ID.
    Follow(SemanticUiId),
}

/// The scrolled view's height.
const VIEW: f32 = 300.0;

/// A document whose later sections lie far below the fold.
fn document() -> String {
    let mut source = String::from("# Field notes\n\nNotes from the season.\n\n");
    for section in 1..=6 {
        source.push_str(&format!("## Section {section}\n\n"));
        for paragraph in 1..=4 {
            source.push_str(&format!(
                "Paragraph {paragraph} of section {section} describes the survey in enough \
                 words to wrap across several lines at a phone's width.\n\n"
            ));
        }
    }
    source.push_str(
        "> ### Quoted method\n>\n> Held in a quote.\n\n- ### Listed finding\n  In a list item.\n",
    );
    source
}

/// The application's ID for each heading: the document's name and the
/// heading's text, as an application's own rule might give them.
fn heading_ids(source: &str) -> Vec<SemanticUiId> {
    markdown_source_map(source, MarkdownSpec::default().options, 0)
        .headings
        .iter()
        .map(|heading| {
            SemanticUiId::new(format!(
                "notes.{}",
                heading.text.to_lowercase().replace(' ', "-")
            ))
        })
        .collect()
}

struct Reader {
    source: String,
    ids: Vec<SemanticUiId>,
}

impl Default for Reader {
    fn default() -> Self {
        let source = document();
        let ids = heading_ids(&source);
        Self { source, ids }
    }
}

#[derive(Default)]
struct Pending {
    reveal: Option<SemanticUiId>,
}

impl Application for Reader {
    type Action = Act;
    type Intent = Intent;
    type UiState = Pending;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Reader")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }

    fn ui(&self, ui: &mut Ui, pending: &mut Pending, frame: &mut Frame<'_, Act, Intent>) {
        if let Some(id) = pending.reveal.take() {
            frame.reveal(id);
        }
        crate::observe_visible_only(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(VIEW)
                .auto_shrink(false)
                .show(ui, |ui| {
                    let mut p = frame.presentation(
                        ui,
                        PresentationScope::new("reader"),
                        SemanticUiId::root(),
                    );
                    p.markdown(
                        ui,
                        "notes",
                        &self.source,
                        MarkdownSpec {
                            heading_ids: &self.ids,
                            ..MarkdownSpec::default()
                        },
                    );
                    p.finish(ui);
                });
        });
    }

    fn apply(&mut self, intent: Intent, pending: &mut Pending) -> Result<bool, String> {
        let Intent::Follow(id) = intent;
        pending.reveal = Some(id);
        Ok(true)
    }

    fn perform(&mut self, _: ActionTarget<Act>, _: &mut Pending) -> Result<bool, String> {
        Ok(false)
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Reader>>,
}

/// A phone's width and a desktop's.
const WIDTHS: [f32; 2] = [390.0, 1280.0];

fn harness(width: f32) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 400.0))
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
    let mut host = AppHost::new(&context, Reader::default());
    host.enable_inspection(Inspection::new("reader-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Reader> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Reader> {
    harness.state_mut().host.as_mut().unwrap()
}

/// The scrolled view on screen: the harness frames its page in 8 points.
fn view(width: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(8.0, 8.0), egui::vec2(width - 16.0, VIEW))
}

/// The heading node with this ID in the latest observation, if presented.
fn heading(harness: &Harness<'_, Probe>, id: &SemanticUiId) -> Option<UiNode> {
    host(harness)
        .ui_snapshot()
        .node(id)
        .filter(|node| node.role == UiRole::Heading)
        .cloned()
}

fn request(host: &AppHost<Reader>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: INSPECTION_VERSION,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

/// Reveal `target` through the inspection interface, as `stobie reveal`
/// does, run the host and read the receipt.
fn agent_reveal(harness: &mut Harness<'_, Probe>, target: &SemanticUiId) -> InvocationReceipt {
    let selector = InspectionSelector {
        capability: Some(REVEAL_CAPABILITY.into()),
        ..Default::default()
    };
    let reader = host(harness);
    let reply = reader.inspection().unwrap().handle(request(
        reader,
        "discover",
        InspectionOperation::Discover {
            selector: selector.clone(),
            limit: 8,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    let [capability] = capabilities.as_slice() else {
        panic!("one reveal capability: {capabilities:?}");
    };
    let request_id = format!("reveal-{}", target.0);
    let reply = reader.inspection().unwrap().handle(request(
        reader,
        &request_id,
        InspectionOperation::Invoke {
            selector,
            expected: capability.target.clone(),
            arguments: serde_json::json!({ "value": target.0 }),
        },
    ));
    assert!(reply.error.is_none(), "{reply:?}");
    harness.run();
    let host = host(harness);
    let reply = host.inspection().unwrap().handle(request(
        host,
        &format!("{request_id}-receipt"),
        InspectionOperation::Receipt { request_id },
    ));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("not a receipt: {reply:?}");
    };
    receipt
}

#[test]
fn headings_in_view_are_observed_with_the_applications_ids() {
    for width in WIDTHS {
        let harness = harness(width);
        let ids = &host(&harness).app().ids;
        let title = heading(&harness, &ids[0]).expect("the title is in view");
        assert_eq!(title.name, "Field notes");
        assert_eq!(title.id.0, "notes.field-notes");
        assert!(view(width).intersects(title.rect.into()));
        // Headings far below the fold publish no node.
        for id in &ids[4..] {
            assert!(heading(&harness, id).is_none(), "{width}: {id:?}");
        }
        let snapshot = host(&harness).ui_snapshot();
        assert!(
            snapshot.semantic_audit.is_empty(),
            "{width}: {:#?}",
            snapshot.semantic_audit
        );
        assert!(
            snapshot.text_audit.is_empty(),
            "{width}: {:#?}",
            snapshot.text_audit
        );
    }
}

#[test]
fn an_agent_reveals_a_heading_below_the_fold_by_its_id() {
    for width in WIDTHS {
        let mut harness = harness(width);
        let ids = host(&harness).app().ids.clone();
        for target in [&ids[5], ids.last().unwrap(), &ids[0]] {
            let receipt = agent_reveal(&mut harness, target);
            assert_eq!(
                receipt.state,
                ReceiptState::Completed,
                "{width}: {receipt:?}"
            );
            let node = heading(&harness, target).expect("the heading is observed");
            assert!(
                view(width).contains_rect(node.rect.into()),
                "{width}: {target:?} at {:?}",
                node.rect
            );
        }
        // The first heading's node is the one revealed, scrolled back to.
        assert!(heading(&harness, &ids[0]).is_some());
        assert!(heading(&harness, ids.last().unwrap()).is_none());
    }
}

#[test]
fn the_application_reveals_a_heading_an_in_document_link_leads_to() {
    for width in WIDTHS {
        let mut harness = harness(width);
        let ids = host(&harness).app().ids.clone();
        let quoted = ids
            .iter()
            .find(|id| id.0 == "notes.quoted-method")
            .unwrap()
            .clone();
        assert!(heading(&harness, &quoted).is_none());
        host_mut(&mut harness).submit(Intent::Follow(quoted.clone()));
        harness.run();
        assert_eq!(
            host(&harness).reveal_outcome(),
            Some((&quoted, Ok(()))),
            "{width}"
        );
        let node = heading(&harness, &quoted).expect("the heading is observed");
        assert_eq!(node.name, "Quoted method");
        assert!(view(width).contains_rect(node.rect.into()), "{width}");
        // An ID no heading has is reported, not ignored.
        let missing = SemanticUiId::new("notes.missing");
        host_mut(&mut harness).submit(Intent::Follow(missing.clone()));
        harness.run();
        let (id, outcome) = host(&harness).reveal_outcome().unwrap();
        assert_eq!(id, &missing);
        assert_eq!(outcome, Err("notes.missing is not presented"), "{width}");
    }
}
