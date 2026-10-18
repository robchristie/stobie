//! A native event source: each line on standard input is one event. A line
//! that is a feed event in the server's JSON format keeps its sequence
//! number; any other line becomes the text of the next event.
//!
//! The thread blocks on its input and holds only an [`IntentSender`], the
//! shape of a native socket client: the application wakes only when a line
//! arrives.

use std::io::BufRead;

use stobie_ui_egui::IntentSender;

use crate::{Connection, FeedEvent, FeedIntent};

/// Read events from standard input on a new thread until it closes or the
/// host stops.
pub fn spawn(intents: IntentSender<FeedIntent>) {
    std::thread::Builder::new()
        .name("live-feed stdin".into())
        .spawn(move || read(std::io::stdin().lock(), &intents))
        .expect("spawn the standard input thread");
}

/// Send each line of `input` as an event, then report the source closed.
pub fn read(input: impl BufRead, intents: &IntentSender<FeedIntent>) {
    if intents
        .send(FeedIntent::Connection(Connection::Open))
        .is_err()
    {
        return;
    }
    let mut seq = 0;
    for line in input.lines() {
        let Ok(line) = line else { break };
        let event = FeedEvent::parse(&line).unwrap_or(FeedEvent {
            seq: seq + 1,
            text: line,
        });
        seq = seq.max(event.seq);
        if intents.send(FeedIntent::Event(event)).is_err() {
            return;
        }
    }
    let _ = intents.send(FeedIntent::Connection(Connection::Closed));
}
