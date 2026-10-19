//! The Web Push check's application. `tools/outside-app-smoke.sh` creates
//! `outside-push` with `stobie new`, replaces its `src/lib.rs` with this file
//! and opts its page into Web Push; `tools/web-push-smoke.mjs` then operates
//! it in Chromium. It subscribes through `stobie_ui_egui::web_push` when its
//! button is tapped, and publishes what it hears as facts.

use serde::Serialize;
use stobie_ui_egui::web_push::{NotificationPermission, PushEvent, PushSupport, WebPush};
use stobie_ui_egui::{
    Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability, Frame,
    IntentSender, Observation, PresentationScope, SemanticUiId, application_identity,
};

/// A VAPID public key for the check. The smoke's stand-in push service
/// records it and never contacts a real one.
pub const VAPID_KEY: &str =
    "BEl62iUYgUivxIkv69yViEuiBIa-Ib9-SkvMeAtA3LFgDzkrxZJjSgSnfckjBJuBkr3qBUYIHBQFLXYp5Nksh8U";

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "outside-push")]
pub enum PushAction {
    #[action(
        label = "Turn on notifications",
        description = "Subscribe this device to push notifications"
    )]
    Subscribe,
    #[action(
        label = "Turn off notifications",
        description = "Remove this device's push subscription"
    )]
    Unsubscribe,
    #[action(label = "Show badge", description = "Show 2 on the application's icon")]
    Badge,
}

pub enum PushIntent {
    Push(PushEvent),
}

#[derive(Default)]
pub struct App {
    push: Option<WebPush>,
    states: u64,
    support: String,
    permission: String,
    endpoint: String,
    subscription: String,
    route: String,
    path: String,
    routes: u64,
    badge: bool,
    error: String,
}

fn permission(permission: Option<NotificationPermission>) -> String {
    match permission {
        None => "unavailable",
        Some(NotificationPermission::Default) => "default",
        Some(NotificationPermission::Granted) => "granted",
        Some(NotificationPermission::Denied) => "denied",
    }
    .into()
}

impl App {
    fn status(&self) -> String {
        if !self.error.is_empty() {
            return self.error.clone();
        }
        match (self.endpoint.is_empty(), self.route.is_empty()) {
            (_, false) => format!("Opened from a notification: {}", self.path),
            (false, true) => "Notifications are on".into(),
            (true, true) => format!("Notifications: {}", self.support),
        }
    }
}

impl Application for App {
    type Action = PushAction;
    type Intent = PushIntent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("Outside Push")
    }

    fn availability(&self, target: ActionTarget<PushAction>) -> Availability {
        let reason = match target.action {
            PushAction::Subscribe if self.support != "supported" => Some("Push is unavailable"),
            PushAction::Unsubscribe if self.endpoint.is_empty() => Some("Notifications are off"),
            _ => None,
        };
        reason.map_or(Availability::Enabled, |reason| Availability::Disabled {
            reason: reason.into(),
        })
    }

    fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, PushAction, PushIntent>) {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut p =
                frame.presentation(ui, PresentationScope::new("push"), SemanticUiId::root());
            p.heading(ui, "title", "Notifications");
            for action in [
                PushAction::Subscribe,
                PushAction::Unsubscribe,
                PushAction::Badge,
            ] {
                frame.action(ui, &mut p, action, ActionEmphasis::Normal);
            }
            p.status(ui, "status", &self.status(), !self.error.is_empty());
        });
    }

    fn apply(&mut self, intent: PushIntent, _: &mut ()) -> Result<bool, String> {
        let PushIntent::Push(event) = intent;
        match event {
            PushEvent::State(state) => {
                self.states += 1;
                self.support = match state.support {
                    PushSupport::Supported => "supported".into(),
                    PushSupport::Unsupported(reason) => reason,
                };
                self.permission = permission(state.permission);
                let subscription = state.subscription.unwrap_or_default();
                self.endpoint = subscription.endpoint;
                self.subscription = subscription.json;
            }
            PushEvent::Subscribed(subscription) => {
                self.endpoint = subscription.endpoint;
                self.subscription = subscription.json;
                self.permission = permission(self.push.as_ref().and_then(WebPush::permission));
                self.error.clear();
            }
            PushEvent::Unsubscribed => {
                self.endpoint.clear();
                self.subscription.clear();
            }
            PushEvent::Failed { operation, message } => {
                self.error = format!("{operation:?}: {message}");
            }
            PushEvent::Route(route) => {
                self.route = route.url;
                self.path = route.path;
                self.routes += 1;
            }
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<PushAction>, _: &mut ()) -> Result<bool, String> {
        let push = self.push.as_ref().ok_or("Push has not started")?;
        match target.action {
            // Inside the tap's input event, as Safari requires.
            PushAction::Subscribe => push.subscribe(VAPID_KEY),
            PushAction::Unsubscribe => push.unsubscribe(),
            PushAction::Badge => {
                self.badge = push.set_app_badge(2);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn report_error(&mut self, message: String) {
        self.error = message;
    }

    fn started(&mut self, intents: IntentSender<PushIntent>) {
        self.push = Some(WebPush::start(intents, PushIntent::Push));
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("states", self.states);
        observation.fact("support", self.support.as_str());
        observation.fact("permission", self.permission.as_str());
        observation.fact("endpoint", self.endpoint.as_str());
        observation.fact("subscription", self.subscription.as_str());
        observation.fact("route", self.route.as_str());
        observation.fact("path", self.path.as_str());
        observation.fact("routes", self.routes);
        observation.fact("badge", self.badge);
        observation.fact("error", self.error.as_str());
    }
}

#[cfg(target_arch = "wasm32")]
stobie_ui_egui::web_handle!(App, |_| App::default());
