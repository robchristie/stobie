//! A live feed: numbered events from a server, received while the
//! application is otherwise idle.
//!
//! It shows how an [`Application`] takes events from outside its passes. The
//! host hands [`Application::started`] an [`IntentSender`]; the application
//! gives it to a transport, and each event the transport sends applies as an
//! ordinary intent at the start of the next pass, which presents it. Nothing
//! runs between events.
//!
//! - In the browser, [`socket::FeedSocket`] is a WebSocket client that
//!   reconnects with a growing delay and catches up from the last sequence
//!   number it received, by connecting to `<feed URL>?after=<seq>`.
//! - Natively, [`stdin`] reads events from standard input on a thread, the
//!   route a native socket client's thread would take.
//!
//! Both are transport policy, so they belong to the application, not to
//! Stobie. Copy and adapt them.

pub mod feed;
#[cfg(target_arch = "wasm32")]
pub mod socket;
#[cfg(not(target_arch = "wasm32"))]
pub mod stdin;

use serde::Serialize;
use stobie_ui_egui::{
    Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability,
    ContentTextSpec, Frame, HostVisibility, IntentSender, Observation, PresentationScope,
    SemanticUiId, TextInteraction, TextOverflow, TextRole, application_identity,
    observe_visible_only,
};

pub use feed::{Connection, Feed, FeedEvent};

pub const APPLICATION_NAME: &str = "Live Feed";

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "live-feed")]
pub enum FeedAction {
    #[action(
        label = "Clear",
        description = "Clear the events shown; new events still arrive"
    )]
    Clear,
}

/// Changes that reach the application, all of them from its transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FeedIntent {
    Event(FeedEvent),
    Connection(Connection),
}

/// Where the application's events come from.
#[derive(Clone, Debug, Default)]
pub enum Source {
    /// No transport, as in tests, which send intents themselves.
    #[default]
    None,
    /// Lines on standard input.
    #[cfg(not(target_arch = "wasm32"))]
    Stdin,
    /// A WebSocket feed at this URL.
    #[cfg(target_arch = "wasm32")]
    WebSocket(String),
}

#[derive(Default)]
pub struct LiveFeed {
    feed: Feed,
    connection: Connection,
    error: Option<String>,
    /// Whether the page or window is hidden, as the host reports it.
    hidden: bool,
    /// Events that arrived while the page or window was last hidden.
    arrived_while_hidden: u64,
    source: Source,
    #[cfg(target_arch = "wasm32")]
    socket: Option<socket::FeedSocket>,
}

impl LiveFeed {
    pub fn new(source: Source) -> Self {
        Self {
            source,
            ..Self::default()
        }
    }

    pub fn feed(&self) -> &Feed {
        &self.feed
    }

    pub fn connection(&self) -> Connection {
        self.connection
    }

    /// Events that arrived while the page or window was last hidden.
    pub fn arrived_while_hidden(&self) -> u64 {
        self.arrived_while_hidden
    }
}

impl Application for LiveFeed {
    type Action = FeedAction;
    type Intent = FeedIntent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        application_identity!(APPLICATION_NAME)
    }

    fn availability(&self, target: ActionTarget<FeedAction>) -> Availability {
        match target.action {
            FeedAction::Clear if self.feed.events.is_empty() => Availability::Disabled {
                reason: "No events are shown".into(),
            },
            FeedAction::Clear => Availability::Enabled,
        }
    }

    fn started(&mut self, intents: IntentSender<FeedIntent>) {
        match &self.source {
            Source::None => {}
            #[cfg(not(target_arch = "wasm32"))]
            Source::Stdin => stdin::spawn(intents),
            #[cfg(target_arch = "wasm32")]
            Source::WebSocket(url) => self.socket = Some(socket::FeedSocket::open(url, intents)),
        }
    }

    fn visibility_changed(&mut self, visibility: HostVisibility, _: &mut ()) -> bool {
        self.hidden = !visibility.is_visible();
        if self.hidden {
            // A new absence starts; what the last one brought has been seen.
            return std::mem::take(&mut self.arrived_while_hidden) > 0;
        }
        false
    }

    fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, FeedAction, FeedIntent>) {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut p = frame.presentation(
                ui,
                PresentationScope::new("live-feed"),
                SemanticUiId::root(),
            );
            p.heading(ui, "title", APPLICATION_NAME);
            match &self.error {
                Some(error) => p.status(ui, "connection", error, true),
                None => p.status(ui, "connection", &self.connection.status(), false),
            };
            ui.horizontal(|ui| {
                frame.action(ui, &mut p, FeedAction::Clear, ActionEmphasis::Normal);
                let mut summary = format!(
                    "{} received, last {}",
                    self.feed.received, self.feed.last_sequence
                );
                if self.arrived_while_hidden > 0 {
                    summary += &format!(", {} while away", self.arrived_while_hidden);
                }
                p.content(ui, "summary", &summary, line(TextRole::Secondary));
            });
            ui.separator();
            // The list scrolls, so publish only what is visible.
            observe_visible_only(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        let mut p = frame.presentation(
                            ui,
                            PresentationScope::new("events"),
                            SemanticUiId::root(),
                        );
                        for event in &self.feed.events {
                            let text = format!("{}. {}", event.seq, event.text);
                            p.content(ui, ("event", event.seq), &text, line(TextRole::Body));
                        }
                    });
            });
        });
    }

    fn apply(&mut self, intent: FeedIntent, _: &mut ()) -> Result<bool, String> {
        Ok(match intent {
            FeedIntent::Event(event) => {
                let received = self.feed.receive(event);
                if received && self.hidden {
                    self.arrived_while_hidden += 1;
                }
                received
            }
            FeedIntent::Connection(connection) => {
                let changed = connection != self.connection || self.error.is_some();
                self.connection = connection;
                self.error = None;
                changed
            }
        })
    }

    fn perform(&mut self, target: ActionTarget<FeedAction>, _: &mut ()) -> Result<bool, String> {
        match target.action {
            FeedAction::Clear => self.feed.events.clear(),
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.error = Some(message);
    }

    fn meaning(&self, target: ActionTarget<FeedAction>) -> String {
        match target.action {
            FeedAction::Clear => format!("shown:{}", self.feed.events.len()),
        }
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("connection", self.connection.name());
        observation.fact("last_sequence", self.feed.last_sequence);
        observation.fact("received", self.feed.received);
        observation.fact("missed", self.feed.missed);
        observation.fact("shown", self.feed.events.len());
        observation.fact("arrived_while_hidden", self.arrived_while_hidden);
        let latest = self.feed.events.back().map_or("", |event| &event.text);
        observation.fact("latest", latest.chars().take(200).collect::<String>());
    }
}

fn line(role: TextRole) -> ContentTextSpec {
    ContentTextSpec {
        role,
        overflow: TextOverflow::Wrap,
        max_lines: 4,
        interaction: TextInteraction::Selectable,
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{LiveFeed, Source};

    // The page passes `?feed=<url>`; by default the feed is `feed` beside the page.
    stobie_ui_egui::web_handle!(LiveFeed, |_creation| LiveFeed::new(Source::WebSocket(
        super::socket::page_feed_url()
    )));
}

#[cfg(test)]
mod tests;
