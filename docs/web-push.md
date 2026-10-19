# Web Push

An installable Stobie web application can receive Web Push notifications: its
generated service worker shows a notification for every push and routes a
tapped notification back into the application, and the application reads the
permission, subscribes, reads its subscription and sets the app badge through
`stobie_ui_egui::web_push`, with no JavaScript of its own. Sending pushes is
the application server's job: it holds a VAPID key pair, stores the
subscriptions the application sends it and encrypts each message to a
subscription's push service (RFC 8030, 8291 and 8292). Stobie has no server
part.

## Opt in

Make the page installable, as [Production browser builds](browser-startup.md#installable-web-applications)
describes, and add one tag to its `<head>`:

```html
<meta name="stobie-web-push" content="enabled">
```

`stobie build-web` and production packaging then append push handling to the
page's `sw.js`, and the package manifest's `pwa` field records
`"webPush": true`. Without the tag the worker is the caching worker alone,
byte for byte. Any other `content` fails the build, and so does the tag on a
page that links no manifest, because push needs the installable page's
service worker.

## What the service worker does

Every push shows a notification. Browsers require it: a subscription is
`userVisibleOnly`, and Safari revokes the permission of a site whose push
shows none, even while the application is open in the foreground. The
worker reads the payload as a declarative push message:

```json
{
  "web_push": 8030,
  "mutable": true,
  "notification": {
    "title": "Scout",
    "body": "Five new papers",
    "navigate": "https://example.org/app/?item=scout",
    "tag": "item-scout",
    "app_badge": "2"
  }
}
```

- `title`, `body`, `tag`, `lang` and `dir` become the notification's. The
  `notification` object alone, without `web_push`, is read the same way.
- `navigate` is the notification's route. It must be inside the worker's
  scope, the directory of the page; a relative URL resolves against the
  scope, and a missing one or one outside the scope opens the application at
  its scope.
- `app_badge`, a whole number or a string of digits, inside `notification`
  or beside it, sets the application's icon badge; 0 clears it.
- A payload that is not such an object, or has no title, still shows a
  notification titled with the application's name (its `<title>` or
  `application-name`) that opens it.

Send `"mutable": true`. Safari 18.4 and later read a declarative message
themselves and pass it to the worker's `push` handler only when it is
mutable; the worker then shows the same notification. Older browsers deliver
every payload to the handler. One payload therefore serves both.

Keep a payload well under 4 KB, the limit of Apple's push service; send an
identifier the application can look up rather than content.

A tapped notification closes, and the worker:
- focuses an open window of its scope, if there is one, and posts it the
  route;
- otherwise remembers the route in Cache Storage
  (`stobie-notification-route`) and opens a window at it. The application
  reads and removes the remembered route when it starts, if it is less than
  two minutes old, because WebKit has sometimes opened such a window at the
  application's start URL instead
  ([WebKit bug 263687](https://bugs.webkit.org/show_bug.cgi?id=263687)).

The worker contains no inline script and evaluates nothing, so it runs under
a strict Content-Security-Policy such as `default-src 'none'; script-src
'self' 'wasm-unsafe-eval'; worker-src 'self'; ...`, which the verification
serves it with.

## Use it from the application

Start `WebPush` in `Application::started`, keep it in the application, and
handle its results as an intent. Every result arrives through the
application's `IntentSender`, so it applies like any [event from
outside](application-composition.md#receive-events-from-outside).

```text
use stobie_ui_egui::web_push::{PushEvent, WebPush};

pub enum Intent { Push(PushEvent), /* ... */ }

fn started(&mut self, intents: IntentSender<Intent>) {
    self.push = Some(WebPush::start(intents, Intent::Push));
}

fn apply(&mut self, intent: Intent, _: &mut ()) -> Result<bool, String> {
    match intent {
        Intent::Push(PushEvent::State(state)) => {
            // Support, permission and the current subscription, on every
            // launch: send the subscription to the server each time.
        }
        Intent::Push(PushEvent::Subscribed(subscription)) => {
            // Send subscription.json to the server.
        }
        Intent::Push(PushEvent::Route(route)) => {
            // Open what route.url or route.path names.
        }
        // Unsubscribed, Failed { operation, message }
        _ => {}
    }
    Ok(true)
}

fn perform(&mut self, target: ActionTarget<Action>, _: &mut ()) -> Result<bool, String> {
    match target.action {
        // A button the user taps: subscribe inside the tap.
        Action::TurnOnNotifications => self.push()?.subscribe(VAPID_PUBLIC_KEY),
        Action::TurnOffNotifications => self.push()?.unsubscribe(),
    }
    Ok(false)
}
```

- `WebPush::start(intents, map)` connects to the page's service worker. Once
  it is active it delivers `PushEvent::State`: `support`, `permission` and
  the current `subscription`. If a tapped notification opened the
  application, `PushEvent::Route` follows, and later taps deliver it while
  the application is open. An unsupported browser gets `State` at once.
- `support()` and `permission()` read the present state. `permission()` is
  `None` without notifications.
- `subscribe(key)` takes the server's VAPID public key in base64url (65
  bytes, an uncompressed P-256 point), asks for permission if needed and
  delivers `Subscribed` or `Failed`. `PushSubscription::json` is the
  browser's `toJSON()`, with the endpoint and the `p256dh` and `auth` keys
  the server encrypts to. A subscription made with another key must be
  removed with `unsubscribe()` first.
- `unsubscribe()` delivers `Unsubscribed`; tell the server too. `refresh()`
  delivers a fresh `State`.
- `set_app_badge(count)` shows a count on the icon, and 0 clears it; it
  returns whether the browser has badges. Badges usually count something
  the application knows, such as unread items, so set it whenever that
  changes, as well as from a payload's `app_badge`.
- `NotificationRoute` carries the absolute `url` and the `path` after the
  scope, such as `?item=scout`.
- Native builds report push as unsupported: `State` arrives at once with
  `PushSupport::Unsupported`, and the operations deliver `Failed`.
- Dropping `WebPush` stops listening for routes.

## iPhone and iPad

- Push works from iOS and iPadOS 16.4, only in an application added to the
  Home Screen and opened from there, whose manifest has `display`
  `standalone` (Stobie's does). In Safari itself `support()` is
  `Unsupported`, with a reason that says to add the application to the Home
  Screen.
- Subscribe from a tap. Safari subscribes, and shows its permission prompt,
  only when `PushManager.subscribe` is called during the user's input event.
  The host performs an action from a pointer tap or click inside that event,
  and `subscribe` calls `PushManager.subscribe` before it returns, so call
  it directly in `perform`, not after awaiting anything. An action that
  VoiceOver activates is performed in a later pass, outside the event, so
  Safari may refuse it; a keyboard activation or an agent's invocation is
  not a tap either.
- Show a notification for every push, as the worker does. A push that shows
  none counts against the site, and Safari revokes its permission.
- Safari never reports a changed or expired subscription
  (`pushsubscriptionchange`), so send the subscription in every launch's
  `State` to the server, which should replace what it holds for the device.
- Apple's push service takes payloads up to 4 KB.
- The badge shows only with notification permission granted.

## Verification

- Unit tests in `stobie-build` cover the opt-in tag and the worker it adds.
- `tools/tests/service-worker-push.test.mjs` runs the generated worker in
  Node against stand-in browser objects: declarative and bare payloads, the
  badge, unreadable payloads, routes outside the scope, and a tap with and
  without an open window.
- `tools/web-push-smoke.mjs`, in the outside-app verification stage, runs an
  application built from `tools/web-push-app/lib.rs` in Chromium under a
  strict Content-Security-Policy. A physical click subscribes, inside the
  click's input event, with the decoded key; a relaunch reads the
  subscription; pushes delivered through DevTools show their notifications,
  including an unreadable one, as the worker's recorded `showNotification`
  calls show; and the worker's routes reach the open application and,
  through the remembered route, a window opened at the scope.
- No test contacts a push service. `PushManager.subscribe` is replaced by a
  stand-in with a local endpoint, and every host but the loopback address
  fails to resolve in the browser. Chromium cannot click a system
  notification under automation, so the smoke calls the worker's own
  `openRoute`, which the `notificationclick` handler calls; the handler is
  covered by the Node tests.

Chromium does not stand in for an iPhone: the permission prompt from a tap,
delivery through Apple's push service, a notification on the lock screen, a
tap opening the Home Screen application at the route, and the badge need a
device check.
