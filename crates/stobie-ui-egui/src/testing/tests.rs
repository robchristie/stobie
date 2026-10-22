//! The host harness lays an application out as eframe does, shares its
//! context with the host, and reports the page's visibility; the Web Push
//! double stands in for the browser's.

use egui::{Pos2, Rect, Vec2};
use egui_kittest::kittest::Queryable;
use serde::Serialize;

use super::*;
use crate::web_push::{
    NotificationPermission, NotificationRoute, PushCall, PushDouble, PushEvent, PushOperation,
    PushState, PushSubscription, PushSupport, WebPush,
};
use crate::{
    Action, ActionEmphasis, ActionTarget, ApplicationIdentity, Availability, Frame, HostRepaint,
    IntentSender, PaneWidthClass, PresentationScope, SemanticUiId,
};

/// A VAPID public key in base64url, as a server publishes it.
const KEY: &str =
    "BEl62iUYgUivxIkv69yViEuiBIa-Ib9-SkvMeAtA3LFgDzkrxZJjSgSnfckjBJuBkr3qBUYIHBQFLXYp5Nksh8U";

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "inbox")]
enum Act {
    #[action(label = "Turn on notifications", description = "Subscribe to Web Push")]
    TurnOn,
    #[action(
        label = "Turn off notifications",
        description = "Unsubscribe from Web Push"
    )]
    TurnOff,
}

enum Msg {
    Arrived,
    Push(PushEvent),
}

/// An inbox whose open conversation reads what arrives while it can be seen.
#[derive(Default)]
struct Inbox {
    hidden: bool,
    unread: u32,
    read: u32,
    visibility_changes: Vec<HostVisibility>,
    started: usize,
    sender: Option<IntentSender<Msg>>,
    push: Option<WebPush>,
    pushes: Vec<PushEvent>,
}

#[derive(Default)]
struct View {
    width: Option<PaneWidthClass>,
}

impl Application for Inbox {
    type Action = Act;
    type Intent = Msg;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Inbox")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut egui::Ui, view: &mut View, frame: &mut Frame<'_, Act, Msg>) {
        view.width = Some(PaneWidthClass::from_points(ui.max_rect().width()));
        let mut p = frame.presentation(ui, PresentationScope::new("inbox"), SemanticUiId::root());
        frame.action(ui, &mut p, Act::TurnOn, ActionEmphasis::Normal);
        frame.action(ui, &mut p, Act::TurnOff, ActionEmphasis::Normal);
    }

    fn apply(&mut self, intent: Msg, _: &mut View) -> Result<bool, String> {
        match intent {
            Msg::Arrived if self.hidden => self.unread += 1,
            Msg::Arrived => self.read += 1,
            Msg::Push(event) => self.pushes.push(event),
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _: &mut View) -> Result<bool, String> {
        let push = self.push.as_ref().ok_or("push has not started")?;
        match target.action {
            Act::TurnOn => push.subscribe(KEY),
            Act::TurnOff => push.unsubscribe(),
        }
        Ok(false)
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }

    fn started(&mut self, intents: IntentSender<Msg>) {
        self.started += 1;
        self.push = Some(WebPush::start(intents.clone(), Msg::Push));
        self.sender = Some(intents);
    }

    fn visibility_changed(&mut self, visibility: HostVisibility, _: &mut View) -> bool {
        self.visibility_changes.push(visibility);
        self.hidden = !visibility.is_visible();
        if self.hidden || self.unread == 0 {
            return false;
        }
        // Shown again: what arrived meanwhile is read now.
        self.read += std::mem::take(&mut self.unread);
        true
    }
}

fn root_rect(harness: &HostHarness<Inbox>) -> Rect {
    harness
        .state()
        .ui_snapshot()
        .node(&SemanticUiId::root())
        .expect("the root is observed")
        .rect
        .into()
}

#[test]
fn the_application_is_laid_out_at_exactly_the_harness_size() {
    for (width, class) in [
        (720.0, PaneWidthClass::Wide),
        (719.0, PaneWidthClass::Regular),
    ] {
        let size = Vec2::new(width, 480.0);
        let harness = host_harness(size, Inbox::default());
        assert_eq!(root_rect(&harness), Rect::from_min_size(Pos2::ZERO, size));
        // The 720-point breakpoint flips at its own width, as in a browser
        // canvas of that size.
        assert_eq!(harness.state().ui_state().width, Some(class), "{width}");
    }
}

#[test]
fn the_host_shares_the_harness_context_and_starts_once() {
    let mut harness = host_harness(Vec2::new(390.0, 844.0), Inbox::default());
    assert_eq!(harness.state().app().started, 1);
    assert!(!harness.ctx.has_requested_repaint(), "idle until an event");
    let presented = harness.state().ui_snapshot().frame;

    // The sender `started` received wakes the harness.
    let sender = harness.state().app().sender.clone().unwrap();
    sender.send(Msg::Arrived).unwrap();
    assert!(harness.ctx.has_requested_repaint());
    harness.step();
    assert_eq!(harness.state().ui_snapshot().frame, presented + 1);
    assert_eq!(harness.state().app().read, 1);
    harness.run();
    assert!(!harness.ctx.has_requested_repaint());
}

#[test]
fn a_hidden_page_is_reported_before_the_intents_sent_while_hidden() {
    let mut harness = host_harness(Vec2::new(390.0, 844.0), Inbox::default());
    let sender = harness.state().app().sender.clone().unwrap();
    assert_eq!(harness.state().visibility(), HostVisibility::Visible);
    assert!(harness.state().app().visibility_changes.is_empty());
    let presented = harness.state().ui_snapshot().frame;

    // The page is hidden and a message arrives before the host next wakes.
    set_visibility(&mut harness, HostVisibility::Hidden);
    sender.send(Msg::Arrived).unwrap();
    harness.step();
    let host = harness.state();
    assert_eq!(host.visibility(), HostVisibility::Hidden);
    assert_eq!(host.app().visibility_changes, [HostVisibility::Hidden]);
    assert_eq!((host.app().unread, host.app().read), (1, 0));
    // eframe runs no pass while hidden, and neither does the harness; the
    // host waits for a visible pass to present what it applied.
    assert_eq!(host.ui_snapshot().frame, presented);
    assert!(host.repaint_reasons().contains(&HostRepaint::Scheduled {
        reason: "sent intents await a visible pass",
        delay: crate::HIDDEN_RECHECK,
    }));

    // Once per change, however long it stays hidden.
    sender.send(Msg::Arrived).unwrap();
    harness.step();
    assert_eq!(harness.state().app().visibility_changes.len(), 1);
    assert_eq!(harness.state().app().unread, 2);

    // Shown again: the application reads what arrived and the same step's
    // pass presents it.
    set_visibility(&mut harness, HostVisibility::Visible);
    harness.step();
    let host = harness.state();
    assert_eq!(
        host.app().visibility_changes,
        [HostVisibility::Hidden, HostVisibility::Visible]
    );
    assert_eq!((host.app().unread, host.app().read), (0, 2));
    assert_eq!(host.ui_snapshot().frame, presented + 1);
    // The pass presented the change, so it asks for no other.
    assert!(
        host.repaint_reasons().is_empty(),
        "{:?}",
        host.repaint_reasons()
    );
}

#[cfg(feature = "eframe")]
#[test]
fn eframe_logic_reports_visibility_while_the_viewport_is_hidden() {
    // eframe's hidden path: no pass, only `App::logic` in egui's logic run.
    let context = egui::Context::default();
    let mut host = AppHost::new(&context, Inbox::default());
    let sender = host.app().sender.clone().unwrap();
    sender.send(Msg::Arrived).unwrap();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .occluded = Some(true);
    let mut frame = eframe::Frame::_new_kittest();
    let _ = context.run_logic(&input, |context| {
        eframe::App::logic(&mut host, context, &mut frame);
    });
    assert_eq!(host.visibility(), HostVisibility::Hidden);
    assert_eq!(host.app().visibility_changes, [HostVisibility::Hidden]);
    assert_eq!((host.app().unread, host.app().read), (1, 0));
}

#[test]
fn natively_without_a_double_push_is_unsupported() {
    let harness = host_harness(Vec2::new(390.0, 844.0), Inbox::default());
    let app = harness.state().app();
    assert!(!app.push.as_ref().unwrap().support().is_supported());
    assert!(matches!(
        &app.pushes[..],
        [PushEvent::State(PushState {
            support: PushSupport::Unsupported(_),
            ..
        })]
    ));
}

#[test]
fn a_push_double_records_calls_and_delivers_the_browsers_results() {
    let push = PushDouble::install();
    let mut harness = host_harness(Vec2::new(390.0, 844.0), Inbox::default());
    let state = PushState {
        support: PushSupport::Supported,
        permission: Some(NotificationPermission::Default),
        subscription: None,
    };
    assert_eq!(harness.state().app().pushes, [PushEvent::State(state)]);
    assert_eq!(push.take_calls(), [PushCall::Start]);

    // A tap subscribes with the server's key; the browser's answer comes later.
    harness.get_by_label("Turn on notifications").click();
    harness.run();
    assert_eq!(
        push.take_calls(),
        [PushCall::Subscribe {
            application_server_key: KEY.into()
        }]
    );
    assert_eq!(harness.state().app().pushes.len(), 1);
    let subscription = PushSubscription {
        endpoint: "https://push.test/1".into(),
        json: r#"{"endpoint":"https://push.test/1"}"#.into(),
    };
    assert!(push.deliver(PushEvent::Subscribed(subscription.clone())));
    harness.run();
    assert_eq!(
        harness.state().app().pushes.last(),
        Some(&PushEvent::Subscribed(subscription.clone()))
    );

    // A refresh reads the state the test set; badges and routes too.
    let granted = PushState {
        support: PushSupport::Supported,
        permission: Some(NotificationPermission::Granted),
        subscription: Some(subscription),
    };
    push.set_state(granted.clone());
    let app = harness.state().app();
    let web_push = app.push.as_ref().unwrap();
    assert_eq!(web_push.permission(), Some(NotificationPermission::Granted));
    web_push.refresh();
    assert!(web_push.set_app_badge(3));
    push.set_badges(false);
    assert!(!web_push.set_app_badge(0));
    assert_eq!(push.badge(), Some(0));
    let route = NotificationRoute {
        url: "https://app.test/?item=inbox".into(),
        path: "?item=inbox".into(),
    };
    assert!(push.deliver(PushEvent::Route(route.clone())));
    harness.get_by_label("Turn off notifications").click();
    harness.run();
    assert_eq!(
        push.take_calls(),
        [
            PushCall::Refresh,
            PushCall::SetAppBadge(3),
            PushCall::SetAppBadge(0),
            PushCall::Unsubscribe,
        ]
    );
    assert!(push.deliver(PushEvent::Unsubscribed));
    harness.run();
    assert_eq!(
        harness.state().app().pushes[2..],
        [
            PushEvent::State(granted),
            PushEvent::Route(route),
            PushEvent::Unsubscribed,
        ]
    );

    // A key that is not a VAPID key fails at once, as in the browser.
    harness
        .state()
        .app()
        .push
        .as_ref()
        .unwrap()
        .subscribe("BAAA");
    harness.run();
    assert!(matches!(
        harness.state().app().pushes.last(),
        Some(PushEvent::Failed {
            operation: PushOperation::Subscribe,
            ..
        })
    ));

    // Once the application drops its connection, nothing is delivered.
    harness.state_mut().app_mut().push = None;
    assert!(!push.deliver(PushEvent::Unsubscribed));
}

#[test]
fn dropping_the_double_uninstalls_it() {
    let push = PushDouble::install();
    drop(push);
    let harness = host_harness(Vec2::new(390.0, 844.0), Inbox::default());
    assert!(
        !harness
            .state()
            .app()
            .push
            .as_ref()
            .unwrap()
            .support()
            .is_supported()
    );
}
