#![cfg(test)]
//! Application-resolved links in a hosted document library: the
//! application classifies each Markdown link, follows its own documents'
//! relative links within itself, and leaves external links to the browser.
//! Agents operate every kind through `stobie.open-link`.

use std::{cell::RefCell, rc::Rc};

use egui::Vec2;
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, INSPECTION_VERSION, InspectionErrorCode, InspectionOperation, InspectionResult,
    InspectionSelector, InvocationReceipt, MarkdownLinkKind, MarkdownSpec, OPEN_LINK_CAPABILITY,
    PresentationScope, ReceiptState, markdown_source_map,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "library")]
enum Act {
    #[action(label = "Close", description = "Close the library")]
    Close,
}

enum Intent {
    /// Follow an in-application link to this destination.
    Follow(String),
}

/// Synthetic documents, by name, that link to one another by relative URL.
const DOCUMENTS: [(&str, &str); 2] = [
    (
        "index.md",
        "# Index\n\nStart with [Setting up](guide.md#setting-up), read [the site](https://example.org/site), \
         skip [the lost page](lost.md) and [a script](javascript:alert(1)).\n",
    ),
    (
        "guide.md",
        "# Guide\n\nA short introduction.\n\n## Setting up\n\nBack to [the index](index.md).\n",
    ),
];

/// The application's ID for the heading `text` in `document`.
fn heading_id(document: &str, text: &str) -> SemanticUiId {
    SemanticUiId::new(format!(
        "{}.{}",
        document.trim_end_matches(".md"),
        text.to_lowercase().replace(' ', "-")
    ))
}

#[derive(Default)]
struct Library {
    /// The index into [`DOCUMENTS`] of the document shown.
    current: usize,
    /// Every in-application link followed, in order.
    followed: Vec<String>,
    /// Whether the application handles `javascript:` links itself.
    scripts_in_application: bool,
    /// Whether the document is hidden.
    hidden: bool,
}

impl Library {
    /// The application's own rule: a destination with a scheme is external
    /// (except scripts, when it handles them), one naming a document is
    /// in-application, and any other is unavailable.
    fn classify<'a>(&self, destination: &str, reason: &'a str) -> MarkdownLinkKind<'a> {
        if destination.starts_with("javascript:") && self.scripts_in_application {
            return MarkdownLinkKind::InApplication;
        }
        if destination.contains(':') {
            return MarkdownLinkKind::External;
        }
        let document = destination.split('#').next().unwrap_or_default();
        if DOCUMENTS.iter().any(|(name, _)| *name == document) {
            MarkdownLinkKind::InApplication
        } else {
            MarkdownLinkKind::Unavailable(reason)
        }
    }
}

#[derive(Default)]
struct Pending {
    reveal: Option<SemanticUiId>,
}

impl Application for Library {
    type Action = Act;
    type Intent = Intent;
    type UiState = Pending;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Library")
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
        if self.hidden {
            return;
        }
        let (name, source) = DOCUMENTS[self.current];
        let map = markdown_source_map(source, MarkdownSpec::default().options, 0);
        let ids: Vec<_> = map
            .headings
            .iter()
            .map(|heading| heading_id(name, &heading.text))
            .collect();
        let reasons: Vec<_> = map
            .links
            .iter()
            .map(|link| format!("No document is named {}", link.destination))
            .collect();
        let kinds: Vec<_> = map
            .links
            .iter()
            .zip(&reasons)
            .map(|(link, reason)| self.classify(&link.destination, reason))
            .collect();
        let mut p = frame.presentation(ui, PresentationScope::new("library"), SemanticUiId::root());
        p.markdown(
            ui,
            "document",
            source,
            MarkdownSpec {
                heading_ids: &ids,
                link_kinds: &kinds,
                ..MarkdownSpec::default()
            },
        );
        if let Some(link) = p.followed_link() {
            frame.emit(Intent::Follow(link.destination.clone()));
        }
        p.finish(ui);
    }

    fn apply(&mut self, intent: Intent, pending: &mut Pending) -> Result<bool, String> {
        let Intent::Follow(destination) = intent;
        self.followed.push(destination.clone());
        let (document, fragment) = destination
            .split_once('#')
            .unwrap_or((destination.as_str(), ""));
        if let Some(index) = DOCUMENTS.iter().position(|(name, _)| *name == document) {
            self.current = index;
            if !fragment.is_empty() {
                pending.reveal = Some(SemanticUiId::new(format!(
                    "{}.{fragment}",
                    document.trim_end_matches(".md")
                )));
            }
        }
        Ok(true)
    }

    fn perform(&mut self, _: ActionTarget<Act>, _: &mut Pending) -> Result<bool, String> {
        Ok(false)
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Library>>,
}

type Opened = Rc<RefCell<Vec<egui::OpenUrl>>>;

fn harness() -> (Harness<'static, Probe>, Opened) {
    let opened = Opened::default();
    let recorded = opened.clone();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            move |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
                ui.ctx().output(|output| {
                    recorded
                        .borrow_mut()
                        .extend(output.commands.iter().filter_map(|command| match command {
                            egui::OutputCommand::OpenUrl(open) => Some(open.clone()),
                            _ => None,
                        }));
                });
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, Library::default());
    host.enable_inspection(Inspection::new("library-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    (harness, opened)
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Library> {
    harness.state().host.as_ref().unwrap()
}

fn library<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut Library {
    harness.state_mut().host.as_mut().unwrap().app_mut()
}

fn taken(opened: &Opened) -> Vec<egui::OpenUrl> {
    std::mem::take(&mut *opened.borrow_mut())
}

fn request(host: &AppHost<Library>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: INSPECTION_VERSION,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

/// The selector for the `index`th link of the document shown.
fn link(index: usize) -> InspectionSelector {
    InspectionSelector {
        id: Some(link_id(index)),
        capability: Some(OPEN_LINK_CAPABILITY.into()),
        ..Default::default()
    }
}

fn link_id(index: usize) -> SemanticUiId {
    PresentationScope::new("library")
        .instance("document")
        .child(("link", index))
        .semantic_id()
}

/// The one open-link capability the `index`th link offers.
fn capability(harness: &Harness<'_, Probe>, index: usize) -> crate::CapabilityDescription {
    let library = host(harness);
    let reply = library.inspection().unwrap().handle(request(
        library,
        "discover",
        InspectionOperation::Discover {
            selector: link(index),
            limit: 4,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    let [capability] = capabilities.as_slice() else {
        panic!("one open-link capability: {capabilities:?}");
    };
    capability.clone()
}

/// Submit an open-link invocation of the `index`th link, as `stobie invoke`
/// does, without running the host.
fn submit(harness: &Harness<'_, Probe>, id: &str, index: usize) -> Option<crate::InspectionError> {
    let expected = capability(harness, index).target;
    let library = host(harness);
    let reply = library.inspection().unwrap().handle(request(
        library,
        id,
        InspectionOperation::Invoke {
            selector: link(index),
            expected,
            arguments: serde_json::Value::Null,
        },
    ));
    reply.error
}

fn receipt(harness: &Harness<'_, Probe>, id: &str) -> InvocationReceipt {
    let library = host(harness);
    let reply = library.inspection().unwrap().handle(request(
        library,
        &format!("{id}-receipt"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    ));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

/// Open the `index`th link as an agent: submit, run the host, read the
/// receipt.
fn agent_open(harness: &mut Harness<'_, Probe>, id: &str, index: usize) -> InvocationReceipt {
    if let Some(error) = submit(harness, id, index) {
        panic!("{id} was refused: {error:?}");
    }
    harness.run();
    receipt(harness, id)
}

/// Open the `index`th link as an agent and return why it was refused.
fn agent_refused(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    index: usize,
) -> crate::InspectionError {
    if let Some(error) = submit(harness, id, index) {
        return error;
    }
    harness.run();
    let receipt = receipt(harness, id);
    assert_eq!(receipt.state, ReceiptState::Rejected, "{receipt:?}");
    receipt.error.expect("a rejected receipt has an error")
}

fn node(harness: &Harness<'_, Probe>, id: &SemanticUiId) -> Option<UiNode> {
    host(harness).ui_snapshot().node(id).cloned()
}

#[test]
fn an_agent_follows_a_relative_link_within_the_application() {
    let (mut harness, opened) = harness();
    let snapshot = host(&harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    let setting_up = capability(&harness, 0);
    assert!(setting_up.availability.enabled());
    assert!(setting_up.semantic_invocable);
    assert_eq!(setting_up.target.meaning, "guide.md#setting-up");
    let link = node(&harness, &link_id(0)).unwrap();
    assert_eq!(link.role, UiRole::Link);
    assert_eq!(link.value.as_deref(), Some("guide.md#setting-up"));
    assert_eq!(link.description, None);

    let receipt = agent_open(&mut harness, "setting-up", 0);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    // The application followed it, to the heading the fragment names, and
    // nothing opened in the browser.
    harness.run();
    assert_eq!(library(&mut harness).followed, ["guide.md#setting-up"]);
    assert_eq!(library(&mut harness).current, 1);
    assert!(node(&harness, &heading_id("guide.md", "Setting up")).is_some());
    assert!(taken(&opened).is_empty());
    // Nothing is followed again without another activation.
    harness.run();
    harness.run();
    assert_eq!(library(&mut harness).followed.len(), 1);

    // The guide's link back to the index, also by agent.
    assert_eq!(capability(&harness, 0).target.meaning, "index.md");
    let receipt = agent_open(&mut harness, "index", 0);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    harness.run();
    assert_eq!(
        library(&mut harness).followed,
        ["guide.md#setting-up", "index.md"]
    );
    assert_eq!(library(&mut harness).current, 0);
    assert!(taken(&opened).is_empty());
}

#[test]
fn a_person_follows_a_relative_link_by_pointer_and_keyboard() {
    let (mut harness, opened) = harness();
    let rect = node(&harness, &link_id(0)).unwrap().rect;
    let centre = egui::pos2(
        (rect.min_x + rect.max_x) / 2.0,
        (rect.min_y + rect.max_y) / 2.0,
    );
    harness.hover_at(centre);
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: centre,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: centre,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert_eq!(library(&mut harness).followed, ["guide.md#setting-up"]);
    assert_eq!(library(&mut harness).current, 1);
    assert!(taken(&opened).is_empty());

    // Tab reaches the guide's link back; Enter follows it.
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(node(&harness, &link_id(0)).unwrap().focused);
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        library(&mut harness).followed,
        ["guide.md#setting-up", "index.md"]
    );
    assert_eq!(library(&mut harness).current, 0);
    assert!(taken(&opened).is_empty());
}

#[test]
fn external_unavailable_and_unsafe_links_keep_their_routes_for_agents() {
    let (mut harness, opened) = harness();
    // External: a new tab, and the application is told nothing.
    let receipt = agent_open(&mut harness, "site", 1);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(
        taken(&opened),
        [egui::OpenUrl::new_tab("https://example.org/site")]
    );
    harness.run();
    assert!(library(&mut harness).followed.is_empty());

    // Unavailable: the application's reason, and no effect.
    let lost = capability(&harness, 2);
    assert_eq!(
        lost.availability.disabled_reason(),
        Some("No document is named lost.md")
    );
    let error = agent_refused(&mut harness, "lost", 2);
    assert_eq!(error.code, InspectionErrorCode::Unavailable);

    // An unsafe URL the application calls external stays blocked.
    let script = capability(&harness, 3);
    assert_eq!(
        script.availability.disabled_reason(),
        Some(crate::INVALID_LINK_URL)
    );
    let error = agent_refused(&mut harness, "script", 3);
    assert_eq!(error.code, InspectionErrorCode::Unavailable);
    harness.run();
    assert!(taken(&opened).is_empty());
    assert!(library(&mut harness).followed.is_empty());

    // An application that handles the URL itself is told; nothing opens.
    library(&mut harness).scripts_in_application = true;
    harness.run();
    assert!(capability(&harness, 3).availability.enabled());
    let receipt = agent_open(&mut harness, "script-in-application", 3);
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    harness.run();
    assert_eq!(library(&mut harness).followed, ["javascript:alert(1)"]);
    assert!(taken(&opened).is_empty());
}

#[test]
fn a_follow_for_a_link_no_longer_presented_lapses() {
    let (mut harness, opened) = harness();
    assert!(submit(&harness, "setting-up", 0).is_none());
    // The pass that runs the invocation presents no document.
    library(&mut harness).hidden = true;
    harness.run();
    // The host took the invocation for the link it last presented.
    assert_eq!(
        receipt(&harness, "setting-up").state,
        ReceiptState::Completed
    );
    library(&mut harness).hidden = false;
    harness.run();
    harness.run();
    assert!(node(&harness, &link_id(0)).unwrap().enabled);
    assert!(library(&mut harness).followed.is_empty());
    assert_eq!(library(&mut harness).current, 0);
    assert!(taken(&opened).is_empty());
}
