//! The feed's events and what the application keeps of them. Nothing here
//! knows about sockets: a transport turns its messages into [`FeedEvent`]s.

use serde::Deserialize;

/// One event as the feed server sends it: `{"seq": 4, "text": "..."}`.
/// Sequence numbers start at 1 and increase by one per event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct FeedEvent {
    pub seq: u64,
    pub text: String,
}

impl FeedEvent {
    /// Parse one text message, or `None` for anything that is not an event,
    /// which a transport ignores.
    pub fn parse(message: &str) -> Option<Self> {
        serde_json::from_str(message).ok()
    }
}

/// The transport's connection, as the application shows it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Connection {
    #[default]
    Connecting,
    Open,
    /// Closed; the transport retries after `delay_ms`.
    Waiting {
        attempt: u32,
        delay_ms: u32,
    },
    /// The transport has stopped and will not retry.
    Closed,
}

impl Connection {
    /// A short stable name, published as the `connection` fact.
    pub fn name(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Open => "open",
            Self::Waiting { .. } => "waiting",
            Self::Closed => "closed",
        }
    }

    pub fn status(self) -> String {
        match self {
            Self::Connecting => "Connecting…".into(),
            Self::Open => "Connected".into(),
            Self::Waiting { attempt, delay_ms } => {
                format!("Reconnecting in {delay_ms} ms (attempt {attempt})")
            }
            Self::Closed => "Disconnected".into(),
        }
    }
}

/// How many events the feed keeps to present.
pub const KEPT: usize = 200;

/// The events received so far, in sequence order, without duplicates.
#[derive(Debug, Default)]
pub struct Feed {
    /// The newest [`KEPT`] events.
    pub events: std::collections::VecDeque<FeedEvent>,
    /// The highest sequence number received.
    pub last_sequence: u64,
    /// Events received in total.
    pub received: u64,
    /// Sequence numbers skipped over, which a correct catch-up never leaves.
    pub missed: u64,
}

impl Feed {
    /// Add an event. Returns false for one already received, which changes
    /// nothing.
    pub fn receive(&mut self, event: FeedEvent) -> bool {
        if event.seq <= self.last_sequence {
            return false;
        }
        self.missed += event.seq - self.last_sequence - 1;
        self.last_sequence = event.seq;
        self.received += 1;
        if self.events.len() == KEPT {
            self.events.pop_front();
        }
        self.events.push_back(event);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: u64) -> FeedEvent {
        FeedEvent {
            seq,
            text: format!("Event {seq}"),
        }
    }

    #[test]
    fn events_parse_from_the_server_format() {
        assert_eq!(
            FeedEvent::parse(r#"{"seq": 3, "text": "Hello"}"#),
            Some(FeedEvent {
                seq: 3,
                text: "Hello".into()
            })
        );
        assert_eq!(FeedEvent::parse(r#"{"type": "ping"}"#), None);
        assert_eq!(FeedEvent::parse("not json"), None);
    }

    #[test]
    fn duplicates_change_nothing_and_gaps_are_counted() {
        let mut feed = Feed::default();
        assert!(feed.receive(event(1)));
        assert!(feed.receive(event(2)));
        assert!(!feed.receive(event(2)), "a replayed event is a duplicate");
        assert!(!feed.receive(event(1)));
        assert_eq!((feed.received, feed.missed), (2, 0));
        assert!(feed.receive(event(5)));
        assert_eq!((feed.last_sequence, feed.missed), (5, 2));
    }

    #[test]
    fn the_feed_keeps_a_bounded_tail() {
        let mut feed = Feed::default();
        for seq in 1..=(KEPT as u64 + 10) {
            feed.receive(event(seq));
        }
        assert_eq!(feed.events.len(), KEPT);
        assert_eq!(feed.events.front().map(|event| event.seq), Some(11));
        assert_eq!(feed.received, KEPT as u64 + 10);
    }
}
