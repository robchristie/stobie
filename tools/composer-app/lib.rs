//! The composer check's application. `tools/outside-app-smoke.sh` creates
//! `outside-composer` with `stobie new`, replaces its `src/lib.rs` with this
//! file, and `tools/composer-smoke.mjs` operates it natively and in Chromium
//! through `stobie`, the keyboard and a touch screen.
//!
//! A conversation whose draft the application owns, edited in a composer:
//! Attach at the left, the microphone at the right, and Send, which carries
//! the draft and is hidden while the draft is empty. A daily limit, switched
//! by a toggle, refuses each send and keeps the draft.

use serde::Serialize;
use stobie_ui_egui::{
    Action, ActionArgument, ActionTarget, Application, ApplicationIdentity, Availability,
    ComposerControl, ComposerSpec, Frame, IconId, Observation, PresentationScope, SemanticUiId,
    application_identity,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "outside-composer")]
pub enum ComposerAction {
    #[action(
        label = "Send",
        description = "Send the message",
        argument = "text",
        max_length = 500
    )]
    Send,
    #[action(label = "Attach", description = "Attach a file to the message")]
    Attach,
    #[action(label = "Dictate", description = "Dictate a message")]
    Dictate,
    #[action(
        label = "Daily limit",
        description = "Refuse every message, as a service over its daily limit would"
    )]
    Limit,
}

pub enum ComposerIntent {
    Draft(String),
}

#[derive(Default)]
pub struct App {
    draft: String,
    sent: Vec<String>,
    attached: u32,
    dictated: u32,
    limited: bool,
    error: String,
}

impl Application for App {
    type Action = ComposerAction;
    type Intent = ComposerIntent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("Outside Composer")
    }

    fn availability(&self, target: ActionTarget<ComposerAction>) -> Availability {
        self.availability_with(target, &ActionArgument::None)
    }

    /// Send is available for a draft with something in it; an empty draft
    /// shows the microphone alone.
    fn availability_with(
        &self,
        target: ActionTarget<ComposerAction>,
        argument: &ActionArgument,
    ) -> Availability {
        match (target.action, argument) {
            (ComposerAction::Send, ActionArgument::Text(text)) if !text.trim().is_empty() => {
                Availability::Enabled
            }
            (ComposerAction::Send, _) => Availability::Hidden,
            _ => Availability::Enabled,
        }
    }

    fn ui(
        &self,
        ui: &mut egui::Ui,
        _: &mut (),
        frame: &mut Frame<'_, ComposerAction, ComposerIntent>,
    ) {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut p = frame.presentation(
                ui,
                PresentationScope::new("conversation"),
                SemanticUiId::root(),
            );
            p.heading(ui, "title", "Conversation");
            frame.toggle(ui, &mut p, "limit", ComposerAction::Limit, self.limited);
            for message in &self.sent {
                ui.label(message);
            }
            let spec = ComposerSpec::new(
                "Message",
                ComposerControl::icon(ComposerAction::Send, IconId::ArrowUp),
            )
            .hint("Message Ada")
            .max_rows(4)
            .leading(ComposerControl::icon(ComposerAction::Attach, IconId::Add))
            .trailing(ComposerControl::label(ComposerAction::Dictate));
            frame.composer(
                ui,
                &mut p,
                "composer",
                &spec,
                &self.draft,
                ComposerIntent::Draft,
            );
            let status = if self.error.is_empty() {
                format!("{} sent", self.sent.len())
            } else {
                self.error.clone()
            };
            p.status(ui, "status", &status, !self.error.is_empty());
        });
    }

    fn apply(&mut self, intent: ComposerIntent, _: &mut ()) -> Result<bool, String> {
        match intent {
            ComposerIntent::Draft(text) => self.draft = text,
        }
        Ok(true)
    }

    fn perform(
        &mut self,
        target: ActionTarget<ComposerAction>,
        _: &mut (),
    ) -> Result<bool, String> {
        match target.action {
            ComposerAction::Attach => self.attached += 1,
            ComposerAction::Dictate => self.dictated += 1,
            ComposerAction::Limit => self.limited = !self.limited,
            ComposerAction::Send => return Err("Send needs a message".into()),
        }
        self.error.clear();
        Ok(true)
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<ComposerAction>,
        argument: ActionArgument,
        state: &mut (),
    ) -> Result<bool, String> {
        let (ComposerAction::Send, ActionArgument::Text(text)) = (target.action, argument) else {
            return self.perform(target, state);
        };
        if self.limited {
            // The draft stays, to send once the limit lifts.
            return Err("Daily message limit reached".into());
        }
        self.sent.push(text);
        self.draft.clear();
        self.error.clear();
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.error = message;
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("draft", self.draft.as_str());
        observation.fact("sent", self.sent.len());
        observation.fact("last", self.sent.last().map_or("", String::as_str));
        observation.fact("attached", self.attached);
        observation.fact("dictated", self.dictated);
        observation.fact("limited", self.limited);
        observation.fact("error", self.error.as_str());
    }
}

#[cfg(target_arch = "wasm32")]
stobie_ui_egui::web_handle!(App, |_| App::default());
