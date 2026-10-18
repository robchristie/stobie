use egui_kittest::Harness;
use stobie_ui_egui::AppHost;

use super::*;

fn harness() -> Harness<'static, AppHost<LiveFeed>> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(640.0, 480.0))
        .build_ui_state(
            |ui, host: &mut AppHost<LiveFeed>| host.present(ui),
            AppHost::new(&egui::Context::default(), LiveFeed::default()),
        );
    // The host must share the harness's context to be woken through it.
    let context = harness.ctx.clone();
    *harness.state_mut() = AppHost::new(&context, LiveFeed::default());
    harness.run();
    harness
}

fn event(seq: u64, text: &str) -> FeedIntent {
    FeedIntent::Event(FeedEvent {
        seq,
        text: text.into(),
    })
}

#[test]
fn an_idle_feed_wakes_for_an_event_and_presents_it() {
    let mut harness = harness();
    assert!(!harness.ctx.has_requested_repaint(), "idle until an event");
    let intents = harness.state().intent_sender();
    let presented = harness.state().ui_snapshot().frame;

    intents.send(event(1, "First")).unwrap();
    assert!(harness.ctx.has_requested_repaint());
    harness.step();
    let host = harness.state();
    assert_eq!(host.ui_snapshot().frame, presented + 1);
    assert_eq!(host.app().feed().last_sequence, 1);
    assert!(host.repaint_reasons().is_empty());
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());

    // A replayed event changes nothing.
    intents.send(event(1, "First")).unwrap();
    harness.run();
    assert_eq!(harness.state().app().feed().received, 1);
}

#[test]
fn standard_input_lines_become_events_in_order() {
    let mut harness = harness();
    let intents = harness.state().intent_sender();
    let input = "Plain line\n{\"seq\": 5, \"text\": \"Numbered\"}\nAfter it\n";
    stdin::read(std::io::Cursor::new(input), &intents);
    harness.run();
    let host = harness.state();
    let feed = host.app().feed();
    let seen: Vec<_> = feed
        .events
        .iter()
        .map(|event| (event.seq, event.text.as_str()))
        .collect();
    assert_eq!(seen, [(1, "Plain line"), (5, "Numbered"), (6, "After it")]);
    assert_eq!(feed.missed, 3);
    assert_eq!(host.app().connection(), Connection::Closed);
}

#[test]
fn clear_empties_the_list_but_keeps_the_sequence() {
    let mut harness = harness();
    let intents = harness.state().intent_sender();
    intents.send(event(1, "One")).unwrap();
    intents.send(event(2, "Two")).unwrap();
    harness.run();
    let app = harness.state_mut().app_mut();
    let clear = ActionTarget::application(FeedAction::Clear);
    assert!(app.availability(clear).enabled());
    app.perform(clear, &mut ()).unwrap();
    assert!(!app.availability(clear).enabled());
    assert_eq!(app.feed().last_sequence, 2);
}
