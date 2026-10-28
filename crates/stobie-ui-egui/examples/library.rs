//! A hosted document library: Markdown documents that link to one another by
//! relative URL, followed within the application.
//!
//! Run it with `cargo run -p stobie-ui-egui --example library`. Set
//! `STOBIE_AUTOMATION_SOCKET=/absolute/path.sock` to inspect it with the
//! `stobie` command: an agent follows a relative link with
//! `stobie.open-link`, as a click, Enter or Space does.
//!
//! The application classifies every link the Markdown recipe finds. A
//! destination naming one of its documents is followed within the
//! application, scrolling to the heading its fragment names; one with a
//! scheme stays external, opening in the browser only when it is safe; any
//! other is unavailable, with a reason. The recipe knows none of these rules.

use serde::Serialize;
use stobie_ui_egui::{
    Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability, Frame,
    MarkdownLinkKind, MarkdownSpec, Observation, PresentationScope, SemanticUiId,
    application_identity, markdown_source_map,
};

/// Synthetic documents, by name.
const DOCUMENTS: [(&str, &str); 2] = [
    (
        "index.md",
        "# Field guide\n\n\
         Start with [Setting up](guide.md#setting-up), then read about \
         [the survey](guide.md#the-survey). The \
         [project site](https://example.org/field-guide) is outside the library, \
         and [the archive](archive.md) has not been written yet.\n",
    ),
    (
        "guide.md",
        "# Guide\n\nBack to [the field guide](index.md).\n\n\
         ## Setting up\n\nUnpack the kit and check each part against the list.\n\n\
         1. Lay out the quadrat frame, the tape and the clipboard.\n\
         2. Charge the camera and the spare batteries overnight.\n\
         3. Fill the water bottles and pack the first-aid kit.\n\
         4. Note the weather, the start time and the observers' names.\n\
         5. Mark the transect's start with a flag you can see from its end.\n\n\
         ## The survey\n\nWalk the transect at an even pace and record every sighting.\n",
    ),
];

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "library")]
enum LibraryAction {
    #[action(
        label = "Back",
        description = "Return to the previous document",
        shortcut = "Alt+Left"
    )]
    Back,
}

/// The UI emits a followed link; the model changes only in `apply`.
enum LibraryIntent {
    Follow(String),
}

/// The application's ID for the heading `text` in `document`: its own rule,
/// which the recipe never sees.
fn heading_id(document: &str, text: &str) -> SemanticUiId {
    SemanticUiId::new(format!(
        "library.{}.{}",
        document.trim_end_matches(".md"),
        text.to_lowercase().replace(' ', "-")
    ))
}

/// The document a relative destination names, and its fragment.
fn resolve(destination: &str) -> Option<(usize, &str)> {
    let (document, fragment) = destination.split_once('#').unwrap_or((destination, ""));
    let index = DOCUMENTS.iter().position(|(name, _)| *name == document)?;
    Some((index, fragment))
}

/// How the library treats a destination.
fn classify<'a>(destination: &str, reason: &'a str) -> MarkdownLinkKind<'a> {
    if destination.contains(':') {
        // The recipe still refuses anything but http, https and mailto.
        MarkdownLinkKind::External
    } else if resolve(destination).is_some() {
        MarkdownLinkKind::InApplication
    } else {
        MarkdownLinkKind::Unavailable(reason)
    }
}

#[derive(Default)]
struct Library {
    /// The index into [`DOCUMENTS`] of the document shown.
    current: usize,
    /// The documents shown before it, for Back.
    history: Vec<usize>,
}

/// UI-only state: the heading to reveal on the next pass.
#[derive(Default)]
struct Pending {
    reveal: Option<SemanticUiId>,
}

impl Application for Library {
    type Action = LibraryAction;
    type Intent = LibraryIntent;
    type UiState = Pending;

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("Library")
    }

    fn availability(&self, _: ActionTarget<LibraryAction>) -> Availability {
        if self.history.is_empty() {
            Availability::Disabled {
                reason: "This is the first document".into(),
            }
        } else {
            Availability::Enabled
        }
    }

    fn ui(
        &self,
        ui: &mut egui::Ui,
        pending: &mut Pending,
        frame: &mut Frame<'_, LibraryAction, LibraryIntent>,
    ) {
        // A heading revealed after its document changed, once it is presented.
        if let Some(id) = pending.reveal.take() {
            frame.reveal(id);
        }
        let (name, source) = DOCUMENTS[self.current];
        // The recipe reports each heading and link without presenting them;
        // the application names the headings and classifies the links.
        let map = markdown_source_map(source, MarkdownSpec::default().options, 0);
        let ids: Vec<_> = map
            .headings
            .iter()
            .map(|heading| heading_id(name, &heading.text))
            .collect();
        let reasons: Vec<_> = map
            .links
            .iter()
            .map(|link| format!("The library has no {}", link.destination))
            .collect();
        let kinds: Vec<_> = map
            .links
            .iter()
            .zip(&reasons)
            .map(|(link, reason)| classify(&link.destination, reason))
            .collect();
        egui::CentralPanel::default().show(ui, |ui| {
            let mut p =
                frame.presentation(ui, PresentationScope::new("library"), SemanticUiId::root());
            frame.action(ui, &mut p, LibraryAction::Back, ActionEmphasis::Normal);
            egui::ScrollArea::vertical().show(ui, |ui| {
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
            });
            // A pointer, keyboard, AccessKit or agent activation of an
            // in-application link arrives here, never in the browser.
            if let Some(link) = p.followed_link() {
                frame.emit(LibraryIntent::Follow(link.destination.clone()));
            }
            p.finish(ui);
        });
    }

    fn apply(&mut self, intent: LibraryIntent, pending: &mut Pending) -> Result<bool, String> {
        let LibraryIntent::Follow(destination) = intent;
        let (index, fragment) =
            resolve(&destination).ok_or_else(|| format!("The library has no {destination}"))?;
        if index != self.current {
            self.history.push(self.current);
            self.current = index;
        }
        if !fragment.is_empty() {
            pending.reveal = Some(SemanticUiId::new(format!(
                "library.{}.{fragment}",
                DOCUMENTS[index].0.trim_end_matches(".md")
            )));
        }
        Ok(true)
    }

    fn perform(
        &mut self,
        target: ActionTarget<LibraryAction>,
        _: &mut Pending,
    ) -> Result<bool, String> {
        match target.action {
            LibraryAction::Back => match self.history.pop() {
                Some(previous) => self.current = previous,
                None => return Ok(false),
            },
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        eprintln!("{message}");
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("document", DOCUMENTS[self.current].0);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    stobie_ui_egui::run_native("Library", eframe::NativeOptions::default(), |_| {
        Library::default()
    })
}

// A browser build exports `stobie_ui_egui::web_handle!` from a library instead.
#[cfg(target_arch = "wasm32")]
fn main() {
    let _ = (Library::default, stobie_ui_egui::AppHost::<Library>::new);
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable};
    use stobie_ui_egui::AppHost;

    use super::*;

    type Hosted = Option<AppHost<Library>>;

    /// A phone's width, and a height that leaves the guide's last heading
    /// below the fold.
    const VIEW: f32 = 200.0;

    fn harness() -> Harness<'static, Hosted> {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(390.0, VIEW))
            // A reveal scrolls over several passes.
            .with_max_steps(32)
            .build_ui_state(
                |ui, host: &mut Hosted| {
                    if let Some(host) = host {
                        host.present(ui);
                    }
                },
                None,
            );
        stobie_ui_egui::install_typography_fonts(&harness.ctx);
        let host = AppHost::new(&harness.ctx.clone(), Library::default());
        *harness.state_mut() = Some(host);
        harness.run();
        harness
    }

    fn library<'a>(harness: &'a Harness<'static, Hosted>) -> &'a Library {
        harness.state().as_ref().unwrap().app()
    }

    fn click_link(harness: &mut Harness<'static, Hosted>, label: &str) {
        harness
            .get_by_role_and_label(egui::accesskit::Role::Link, label)
            .click();
        harness.run();
    }

    #[test]
    fn a_relative_link_opens_its_document_at_the_heading_it_names() {
        let mut harness = harness();
        click_link(&mut harness, "the survey");
        assert_eq!(library(&harness).current, 1);
        // The heading the fragment names is revealed below the fold.
        let survey = heading_id("guide.md", "The survey");
        let snapshot = harness.state().as_ref().unwrap().ui_snapshot();
        let node = snapshot.node(&survey).unwrap();
        assert!(
            node.rect.min_y >= 0.0 && node.rect.max_y <= VIEW,
            "{:?}",
            node.rect
        );
        // The document scrolled to it: its title has left the view.
        let title = snapshot.node(&heading_id("guide.md", "Guide"));
        assert!(
            title.is_none_or(|title| title.rect.max_y <= 0.0),
            "{title:?}"
        );
        // Back returns to the field guide, whose first link is followed
        // the same way.
        harness
            .get_by_role_and_label(egui::accesskit::Role::Button, "Back")
            .click();
        harness.run();
        assert_eq!(library(&harness).current, 0);
        assert!(library(&harness).history.is_empty());
        click_link(&mut harness, "Setting up");
        assert_eq!(library(&harness).current, 1);
        assert_eq!(library(&harness).history, [0]);
    }

    #[test]
    fn the_application_classifies_every_link() {
        let classified: Vec<_> =
            markdown_source_map(DOCUMENTS[0].1, MarkdownSpec::default().options, 0)
                .links
                .iter()
                .map(|link| classify(&link.destination, "missing"))
                .collect();
        assert_eq!(
            classified,
            [
                MarkdownLinkKind::InApplication,
                MarkdownLinkKind::InApplication,
                MarkdownLinkKind::External,
                MarkdownLinkKind::Unavailable("missing"),
            ]
        );
        // A follow the library cannot resolve fails without moving.
        let mut library = Library::default();
        assert!(
            library
                .apply(
                    LibraryIntent::Follow("archive.md".into()),
                    &mut Pending::default()
                )
                .is_err()
        );
        assert_eq!(library.current, 0);
    }
}
