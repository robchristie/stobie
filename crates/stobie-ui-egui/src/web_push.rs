//! Web Push for a browser application: support and permission, subscribing
//! inside a tap, the current subscription, the app badge, and the routes of
//! tapped notifications.
//!
//! The service worker that shows the notifications is the one `stobie
//! build-web` writes for an installable page that declares
//! `<meta name="stobie-web-push" content="enabled">`; see the
//! [Web Push guide](https://github.com/robchristie/stobie/blob/main/docs/web-push.md).
//! [`WebPush::start`] in [`Application::started`](crate::Application::started)
//! connects to it, and every result arrives later as a [`PushEvent`] through
//! the application's [`IntentSender`], like any event from outside a pass.
//! Native builds report push as unsupported.

use std::fmt;
use std::rc::Rc;

use crate::IntentSender;

/// Whether this build and browser can receive Web Push.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushSupport {
    Supported,
    /// Push is unavailable, for the reason given, such as a native build, an
    /// insecure page, or Safari outside a Home Screen application.
    Unsupported(String),
}

impl PushSupport {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// The browser's notification permission for the page's origin.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NotificationPermission {
    /// Not yet asked; subscribing asks.
    Default,
    Granted,
    /// Refused; only the browser's settings can change it.
    Denied,
}

/// A push subscription, to send to the application's server.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PushSubscription {
    /// The push service's URL for this subscription.
    pub endpoint: String,
    /// The subscription as the browser serialises it (`PushSubscription.toJSON`):
    /// `{"endpoint", "expirationTime", "keys": {"p256dh", "auth"}}`.
    pub json: String,
}

/// Where a tapped notification asked the application to go.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationRoute {
    /// The absolute URL, always inside the service worker's scope.
    pub url: String,
    /// The part of `url` after the scope, such as `?bot=scout` or
    /// `bots/scout`; empty for the scope itself.
    pub path: String,
}

/// Support, permission and the current subscription, read together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PushState {
    pub support: PushSupport,
    /// `None` when the browser has no notifications.
    pub permission: Option<NotificationPermission>,
    pub subscription: Option<PushSubscription>,
}

/// The operation a [`PushEvent::Failed`] reports.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PushOperation {
    Subscribe,
    Unsubscribe,
    Refresh,
}

/// A result from [`WebPush`], delivered through the application's sender.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PushEvent {
    /// After [`WebPush::start`], once the service worker is active, and after
    /// each [`WebPush::refresh`]. Unsupported browsers report it at once.
    State(PushState),
    Subscribed(PushSubscription),
    Unsubscribed,
    Failed {
        operation: PushOperation,
        message: String,
    },
    /// A notification was tapped: at launch, when the tap opened the
    /// application, or while it is open.
    Route(NotificationRoute),
}

/// The application's connection to Web Push. Create it with [`Self::start`]
/// and keep it for as long as the application should hear tapped
/// notifications; dropping it stops listening.
pub struct WebPush {
    deliver: Rc<dyn Fn(PushEvent)>,
    #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
    web: Rc<web::Shared>,
}

impl fmt::Debug for WebPush {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebPush")
            .field("support", &self.support())
            .finish_non_exhaustive()
    }
}

impl WebPush {
    /// Connect to the page's service worker and deliver every result as
    /// `event(result)` through `intents`. Call it from
    /// [`Application::started`](crate::Application::started).
    ///
    /// It delivers [`PushEvent::State`] once the service worker is active,
    /// with the current subscription: send that to the server on every
    /// launch, because Safari never reports a changed subscription. If a tap
    /// on a notification opened the application, [`PushEvent::Route`]
    /// follows; later taps deliver it while the application is open.
    pub fn start<I: 'static>(
        intents: IntentSender<I>,
        event: impl Fn(PushEvent) -> I + 'static,
    ) -> Self {
        let deliver: Rc<dyn Fn(PushEvent)> = Rc::new(move |result| {
            // A stopped host hears nothing more.
            let _ = intents.send(event(result));
        });
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            let web = web::Shared::start(deliver.clone());
            Self { deliver, web }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        {
            let push = Self { deliver };
            (push.deliver)(PushEvent::State(PushState {
                support: push.support(),
                permission: None,
                subscription: None,
            }));
            push
        }
    }

    /// Whether this browser can receive Web Push now.
    pub fn support(&self) -> PushSupport {
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            web::support()
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        {
            PushSupport::Unsupported("Web Push is available only in the browser build".into())
        }
    }

    /// The notification permission now, or `None` without notifications.
    pub fn permission(&self) -> Option<NotificationPermission> {
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            web::permission()
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        {
            None
        }
    }

    /// Subscribe with the server's VAPID public key, `application_server_key`
    /// in base64url (65 bytes, an uncompressed P-256 point). The browser asks
    /// for permission if it has not. Delivers [`PushEvent::Subscribed`] or
    /// [`PushEvent::Failed`].
    ///
    /// Call it from [`Application::perform`](crate::Application::perform)
    /// for an action the user tapped or clicked: the host performs such an
    /// action inside the browser's input event, and Safari subscribes only
    /// there. It fails while the service worker is not yet active, before the
    /// first [`PushEvent::State`]. A subscription made with another key must
    /// be removed with [`Self::unsubscribe`] first.
    pub fn subscribe(&self, application_server_key: &str) {
        let key = match decode_application_server_key(application_server_key) {
            Ok(key) => key,
            Err(message) => return self.failed(PushOperation::Subscribe, message),
        };
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            if let Err(message) = self.web.subscribe(&key) {
                self.failed(PushOperation::Subscribe, message);
            }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        {
            let _ = key;
            self.unsupported(PushOperation::Subscribe);
        }
    }

    /// Remove the current subscription, if any; delivers
    /// [`PushEvent::Unsubscribed`] or [`PushEvent::Failed`]. Tell the server
    /// too.
    pub fn unsubscribe(&self) {
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            if let Err(message) = self.web.unsubscribe() {
                self.failed(PushOperation::Unsubscribe, message);
            }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        self.unsupported(PushOperation::Unsubscribe);
    }

    /// Read support, permission and the subscription again; delivers
    /// [`PushEvent::State`] or [`PushEvent::Failed`].
    pub fn refresh(&self) {
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            if let Err(message) = self.web.refresh() {
                self.failed(PushOperation::Refresh, message);
            }
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        (self.deliver)(PushEvent::State(PushState {
            support: self.support(),
            permission: None,
            subscription: None,
        }));
    }

    /// Show `count` on the application's icon, or clear it for 0. Returns
    /// whether the browser has app badges; Safari shows them for a Home
    /// Screen application with notification permission.
    pub fn set_app_badge(&self, count: u64) -> bool {
        #[cfg(all(target_arch = "wasm32", feature = "eframe"))]
        {
            web::set_app_badge(count)
        }
        #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
        {
            let _ = count;
            false
        }
    }

    fn failed(&self, operation: PushOperation, message: String) {
        (self.deliver)(PushEvent::Failed { operation, message });
    }

    #[cfg(not(all(target_arch = "wasm32", feature = "eframe")))]
    fn unsupported(&self, operation: PushOperation) {
        if let PushSupport::Unsupported(message) = self.support() {
            self.failed(operation, message);
        }
    }
}

/// Decode a VAPID public key from base64url, with or without padding (the
/// standard alphabet is accepted too), and check that it is an uncompressed
/// P-256 point.
fn decode_application_server_key(text: &str) -> Result<Vec<u8>, String> {
    let invalid = || "the application server key is not base64url".to_owned();
    let mut bits = 0u32;
    let mut count = 0;
    let mut bytes = Vec::with_capacity(65);
    for character in text.trim().trim_end_matches('=').bytes() {
        let value = match character {
            b'A'..=b'Z' => character - b'A',
            b'a'..=b'z' => character - b'a' + 26,
            b'0'..=b'9' => character - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return Err(invalid()),
        };
        bits = (bits << 6) | u32::from(value);
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    // A final group of one character cannot hold a byte; leftover bits must be zero.
    if count >= 6 || bits != 0 {
        return Err(invalid());
    }
    if bytes.len() != 65 || bytes[0] != 4 {
        return Err(format!(
            "the application server key has {} bytes; a VAPID public key is an uncompressed P-256 point of 65 bytes starting with 4",
            bytes.len()
        ));
    }
    Ok(bytes)
}

/// The part of `url` after `scope`, when `url` is inside it.
#[cfg_attr(not(all(target_arch = "wasm32", feature = "eframe")), allow(dead_code))]
fn route_path(url: &str, scope: &str) -> Option<String> {
    url.strip_prefix(scope).map(str::to_owned)
}

#[cfg(all(target_arch = "wasm32", feature = "eframe"))]
mod web {
    use std::cell::RefCell;
    use std::rc::Rc;

    use js_sys::{Function, Promise, Reflect};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::{JsFuture, spawn_local};
    use web_sys::{MessageEvent, ServiceWorkerContainer, ServiceWorkerRegistration};

    use super::{
        NotificationPermission, NotificationRoute, PushEvent, PushOperation, PushState,
        PushSubscription, PushSupport, route_path,
    };

    /// Where the service worker leaves a tapped notification's route for the
    /// window it opens (`service-worker-push.js`).
    const ROUTE_CACHE: &str = "stobie-notification-route";
    const ROUTE_KEY: &str = "__stobie/notification-route";
    /// A remembered route older than this belongs to an earlier tap.
    const ROUTE_LIFETIME_MS: f64 = 120_000.0;

    /// The container's `message` listener, removed when dropped.
    type Listener = (ServiceWorkerContainer, Closure<dyn FnMut(MessageEvent)>);

    pub(super) struct Shared {
        deliver: Rc<dyn Fn(PushEvent)>,
        registration: Rc<RefCell<Option<ServiceWorkerRegistration>>>,
        listener: Option<Listener>,
    }

    impl Drop for Shared {
        fn drop(&mut self) {
            if let Some((container, listener)) = &self.listener {
                let _ = container.remove_event_listener_with_callback(
                    "message",
                    listener.as_ref().unchecked_ref(),
                );
            }
        }
    }

    fn global(name: &str) -> JsValue {
        Reflect::get(&js_sys::global(), &JsValue::from_str(name)).unwrap_or(JsValue::UNDEFINED)
    }

    fn present(value: &JsValue) -> bool {
        !value.is_undefined() && !value.is_null()
    }

    fn container() -> Option<ServiceWorkerContainer> {
        let navigator = global("navigator");
        let container = Reflect::get(&navigator, &JsValue::from_str("serviceWorker")).ok()?;
        present(&container).then(|| container.unchecked_into())
    }

    pub(super) fn support() -> PushSupport {
        if global("isSecureContext").as_bool() != Some(true) {
            return PushSupport::Unsupported(
                "Web Push needs a secure page: HTTPS, or localhost while developing".into(),
            );
        }
        if container().is_none() {
            return PushSupport::Unsupported("this browser has no service workers".into());
        }
        if !present(&global("PushManager")) || !present(&global("Notification")) {
            return PushSupport::Unsupported(
                "this browser has no Web Push; on iPhone and iPad, add the application to the Home Screen and open it from there".into(),
            );
        }
        PushSupport::Supported
    }

    pub(super) fn permission() -> Option<NotificationPermission> {
        let notification = global("Notification");
        if !present(&notification) {
            return None;
        }
        let permission = Reflect::get(&notification, &JsValue::from_str("permission")).ok()?;
        match permission.as_string()?.as_str() {
            "granted" => Some(NotificationPermission::Granted),
            "denied" => Some(NotificationPermission::Denied),
            _ => Some(NotificationPermission::Default),
        }
    }

    /// An error's name and message, such as `NotAllowedError: Permission denied`.
    fn describe(error: &JsValue) -> String {
        let field = |name: &str| {
            Reflect::get(error, &JsValue::from_str(name))
                .ok()
                .and_then(|value| value.as_string())
                .filter(|value| !value.is_empty())
        };
        match (field("name"), field("message")) {
            (Some(name), Some(message)) => format!("{name}: {message}"),
            (None, Some(message)) => message,
            (Some(name), None) => name,
            (None, None) => error.as_string().unwrap_or_else(|| format!("{error:?}")),
        }
    }

    fn call(target: &JsValue, method: &str, arguments: &[JsValue]) -> Result<JsValue, JsValue> {
        let function: Function = Reflect::get(target, &JsValue::from_str(method))?.dyn_into()?;
        let arguments: js_sys::Array = arguments.iter().collect();
        function.apply(target, &arguments)
    }

    async fn settle(promise: Result<JsValue, JsValue>) -> Result<JsValue, String> {
        let promise: Promise = promise
            .and_then(|value| value.dyn_into())
            .map_err(|error| describe(&error))?;
        JsFuture::from(promise)
            .await
            .map_err(|error| describe(&error))
    }

    fn subscription(value: &JsValue) -> Option<PushSubscription> {
        if !present(value) {
            return None;
        }
        let endpoint = Reflect::get(value, &JsValue::from_str("endpoint"))
            .ok()?
            .as_string()?;
        // JSON.stringify uses PushSubscription.toJSON.
        let json = js_sys::JSON::stringify(value).ok()?.as_string()?;
        Some(PushSubscription { endpoint, json })
    }

    fn scope_of(registration: Option<&ServiceWorkerRegistration>) -> String {
        match registration {
            Some(registration) => registration.scope(),
            // The loader registers the worker for the page's directory.
            None => web_sys::Url::new_with_base("./", &location())
                .map(|url| url.href())
                .unwrap_or_default(),
        }
    }

    fn location() -> String {
        Reflect::get(&global("location"), &JsValue::from_str("href"))
            .ok()
            .and_then(|href| href.as_string())
            .unwrap_or_default()
    }

    fn route(url: JsValue, scope: &str) -> Option<NotificationRoute> {
        let url = url.as_string()?;
        let path = route_path(&url, scope)?;
        Some(NotificationRoute { url, path })
    }

    async fn current_state(registration: &ServiceWorkerRegistration) -> Result<PushState, String> {
        let manager = Reflect::get(registration, &JsValue::from_str("pushManager"))
            .map_err(|error| describe(&error))?;
        let subscription_value = settle(call(&manager, "getSubscription", &[])).await?;
        Ok(PushState {
            support: support(),
            permission: permission(),
            subscription: subscription(&subscription_value),
        })
    }

    /// The route the worker left for the window a tap opened, if it is
    /// recent; reading it removes it.
    async fn remembered_route(scope: &str) -> Option<NotificationRoute> {
        let caches = global("caches");
        if !present(&caches) {
            return None;
        }
        let key = format!("{scope}{ROUTE_KEY}");
        let cache = settle(call(&caches, "open", &[ROUTE_CACHE.into()]))
            .await
            .ok()?;
        let response = settle(call(&cache, "match", &[key.as_str().into()]))
            .await
            .ok()?;
        if !present(&response) {
            return None;
        }
        let _ = settle(call(&cache, "delete", &[key.as_str().into()])).await;
        let record = settle(call(&response, "json", &[])).await.ok()?;
        let at = Reflect::get(&record, &JsValue::from_str("at"))
            .ok()?
            .as_f64()?;
        let age = js_sys::Date::now() - at;
        if !(0.0..=ROUTE_LIFETIME_MS).contains(&age) {
            return None;
        }
        route(
            Reflect::get(&record, &JsValue::from_str("route")).ok()?,
            scope,
        )
    }

    impl Shared {
        pub(super) fn start(deliver: Rc<dyn Fn(PushEvent)>) -> Rc<Self> {
            let registration = Rc::new(RefCell::new(None));
            let support = support();
            let listener = container().map(|container| {
                // A tap while the application is open: the worker posts the route.
                let deliver = deliver.clone();
                let known = registration.clone();
                let listener =
                    Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                        let data = event.data();
                        let kind = Reflect::get(&data, &JsValue::from_str("stobie")).ok();
                        if kind.and_then(|kind| kind.as_string()).as_deref() != Some("notification")
                        {
                            return;
                        }
                        let scope = scope_of(known.borrow().as_ref());
                        if let Some(route) = Reflect::get(&data, &JsValue::from_str("route"))
                            .ok()
                            .and_then(|url| route(url, &scope))
                        {
                            deliver(PushEvent::Route(route));
                        }
                    });
                let _ = container
                    .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref());
                (container, listener)
            });
            let shared = Rc::new(Self {
                deliver: deliver.clone(),
                registration: registration.clone(),
                listener,
            });
            if !support.is_supported() {
                deliver(PushEvent::State(PushState {
                    support,
                    permission: permission(),
                    subscription: None,
                }));
                return shared;
            }
            let container = shared
                .listener
                .as_ref()
                .map(|(container, _)| container.clone());
            spawn_local(async move {
                let Some(container) = container else { return };
                // `ready` settles once a worker for this page is active.
                let ready = match container.ready() {
                    Ok(ready) => JsFuture::from(ready).await,
                    Err(error) => Err(error),
                };
                let active: ServiceWorkerRegistration = match ready {
                    Ok(value) => value.unchecked_into(),
                    Err(error) => {
                        return deliver(PushEvent::Failed {
                            operation: PushOperation::Refresh,
                            message: describe(&error),
                        });
                    }
                };
                registration.replace(Some(active.clone()));
                if let Some(route) = remembered_route(&active.scope()).await {
                    deliver(PushEvent::Route(route));
                }
                deliver(match current_state(&active).await {
                    Ok(state) => PushEvent::State(state),
                    Err(message) => PushEvent::Failed {
                        operation: PushOperation::Refresh,
                        message,
                    },
                });
            });
            shared
        }

        fn active(&self) -> Result<ServiceWorkerRegistration, String> {
            if let PushSupport::Unsupported(reason) = support() {
                return Err(reason);
            }
            self.registration.borrow().clone().ok_or_else(|| {
                "the service worker is not active yet; try again in a moment".to_owned()
            })
        }

        pub(super) fn subscribe(&self, key: &[u8]) -> Result<(), String> {
            let registration = self.active()?;
            let manager = Reflect::get(&registration, &JsValue::from_str("pushManager"))
                .map_err(|error| describe(&error))?;
            let options = js_sys::Object::new();
            let set = |name: &str, value: &JsValue| {
                Reflect::set(&options, &JsValue::from_str(name), value)
                    .map_err(|error| describe(&error))
            };
            set("userVisibleOnly", &JsValue::TRUE)?;
            set(
                "applicationServerKey",
                &js_sys::Uint8Array::from(key).buffer().into(),
            )?;
            // Called now, inside the input event, not after an await.
            let promise = call(&manager, "subscribe", &[options.into()]);
            let deliver = self.deliver.clone();
            spawn_local(async move {
                deliver(match settle(promise).await {
                    Ok(value) => match subscription(&value) {
                        Some(subscription) => PushEvent::Subscribed(subscription),
                        None => PushEvent::Failed {
                            operation: PushOperation::Subscribe,
                            message: "the browser returned no subscription".into(),
                        },
                    },
                    Err(message) => PushEvent::Failed {
                        operation: PushOperation::Subscribe,
                        message,
                    },
                });
            });
            Ok(())
        }

        pub(super) fn unsubscribe(&self) -> Result<(), String> {
            let registration = self.active()?;
            let deliver = self.deliver.clone();
            spawn_local(async move {
                let result = async {
                    let manager = Reflect::get(&registration, &JsValue::from_str("pushManager"))
                        .map_err(|error| describe(&error))?;
                    let current = settle(call(&manager, "getSubscription", &[])).await?;
                    if present(&current) {
                        settle(call(&current, "unsubscribe", &[])).await?;
                    }
                    Ok::<_, String>(())
                }
                .await;
                deliver(match result {
                    Ok(()) => PushEvent::Unsubscribed,
                    Err(message) => PushEvent::Failed {
                        operation: PushOperation::Unsubscribe,
                        message,
                    },
                });
            });
            Ok(())
        }

        pub(super) fn refresh(&self) -> Result<(), String> {
            let support = support();
            if !support.is_supported() {
                (self.deliver)(PushEvent::State(PushState {
                    support,
                    permission: permission(),
                    subscription: None,
                }));
                return Ok(());
            }
            let registration = self.active()?;
            let deliver = self.deliver.clone();
            spawn_local(async move {
                deliver(match current_state(&registration).await {
                    Ok(state) => PushEvent::State(state),
                    Err(message) => PushEvent::Failed {
                        operation: PushOperation::Refresh,
                        message,
                    },
                });
            });
            Ok(())
        }
    }

    pub(super) fn set_app_badge(count: u64) -> bool {
        let navigator = global("navigator");
        let (method, arguments) = if count == 0 {
            ("clearAppBadge", vec![])
        } else {
            ("setAppBadge", vec![JsValue::from_f64(count as f64)])
        };
        if !Reflect::get(&navigator, &JsValue::from_str(method))
            .is_ok_and(|value| value.is_function())
        {
            return false;
        }
        let promise = call(&navigator, method, &arguments);
        // A refused badge is not an error the application can act on.
        spawn_local(async move {
            let _ = settle(promise).await;
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A VAPID public key in base64url, as a server publishes it.
    const KEY: &str =
        "BEl62iUYgUivxIkv69yViEuiBIa-Ib9-SkvMeAtA3LFgDzkrxZJjSgSnfckjBJuBkr3qBUYIHBQFLXYp5Nksh8U";

    #[test]
    fn a_vapid_key_decodes_from_base64url_or_base64() {
        let key = decode_application_server_key(KEY).unwrap();
        assert_eq!((key.len(), key[0]), (65, 4));
        let standard = KEY.replace('-', "+").replace('_', "/");
        assert_eq!(
            decode_application_server_key(&format!("{standard}=")).unwrap(),
            key
        );
        assert!(
            decode_application_server_key("BEl6*")
                .unwrap_err()
                .contains("base64url")
        );
        // Trailing bits that are not zero mean a truncated or altered key.
        assert!(decode_application_server_key(&format!("{}V", &KEY[..KEY.len() - 1])).is_err());
        let short = decode_application_server_key("BAAA").unwrap_err();
        assert!(short.contains("has 3 bytes"), "{short}");
        let compressed = KEY.replacen('B', "A", 1);
        assert!(decode_application_server_key(&compressed).is_err());
    }

    #[test]
    fn a_route_is_the_part_after_the_scope() {
        let scope = "https://app.test/notes/";
        assert_eq!(
            route_path("https://app.test/notes/?bot=scout", scope).as_deref(),
            Some("?bot=scout")
        );
        assert_eq!(route_path(scope, scope).as_deref(), Some(""));
        assert_eq!(route_path("https://app.test/other/", scope), None);
    }
}
