# Web Push in the installable web output

Status: complete
Delivery: [PR #__PR__](https://github.com/robchristie/stobie/pull/__PR__)

An installable Stobie web application needs Web Push: a notification for
every push, a tapped notification routed back into the application, and a
way for the application to read the permission, subscribe inside a tap (as
iOS requires), read its subscription and set the app badge without its own
JavaScript. Before this, the generated service worker had only `install`,
`activate` and `fetch` handlers and no extension point: `sw.js` is a
generated name, so an application's own worker was dropped, and a scope
holds one worker.

## Decisions

- **Opt in from the page's head.** `<meta name="stobie-web-push"
  content="enabled">` beside the installable page's other tags, which the
  [PWA output](pwa-output-plan.md) already makes the single source of the
  application's identity. Any other value fails the build, and so does the
  tag on a page that links no manifest. Without it the worker is the caching
  worker byte for byte: the push handlers live in their own template,
  `crates/stobie-build/web/service-worker-push.js`, appended only on opt-in
  (`pwa::push_service_worker`; `pwa::service_worker` keeps its signature and
  output, and `Declaration::service_worker` chooses between them).
  The package manifest's `pwa` field records `"webPush": true` only when set,
  so existing packages are unchanged.
- **One payload shape for every browser: the declarative push message.**
  The `push` handler reads `{"web_push": 8030, "notification": {...}}`, or the
  `notification` object alone: `title`, `body`, `tag`, `lang`, `dir`,
  `navigate` and `app_badge` (inside `notification` or beside it, a number or
  a string of digits, since sources differ). It always shows a notification;
  an unreadable payload, or one without a title, is titled with the
  application's name and opens it. A server sends `mutable: true` so that
  Safari 18.4 and later pass the message to the handler too.
- **Routes stay inside the scope.** `navigate` resolves against the scope; a
  missing route or one outside the scope opens the scope.
- **A tap focuses and posts, or remembers and opens.** With a window of the
  scope open, the worker focuses it and posts `{stobie: "notification",
  route}`; if focusing fails it still posts. Otherwise it stores the route in
  Cache Storage and opens a window at it, and the application reads and
  removes the stored route at launch if it is under two minutes old. The
  stored route, not the launch URL, is the launch signal: WebKit has opened
  such windows at the start URL (bug 263687), and a launch URL cannot tell a
  tap from an ordinary deep link.
- **The bridge is `stobie_ui_egui::web_push`**, not a new crate: the host's
  crate already owns the browser entry points and `IntentSender`. `WebPush::start`
  in `Application::started` takes the sender and a mapping to the
  application's intent, and every result (`State`, `Subscribed`,
  `Unsubscribed`, `Failed`, `Route`) arrives as an intent, like any event
  from outside. `subscribe` calls `PushManager.subscribe` before returning,
  using the registration found at start, so a tap's action performs it inside
  the input event. Keys are checked as 65-byte uncompressed P-256 points
  before any browser call. Native builds report push as unsupported. The
  browser calls go through `js_sys::Reflect` where an API may be absent (no
  `Notification` in Safari outside a Home Screen application), with web-sys
  features only for the service worker container, registration, message
  event and URL.
- **Verification without a push service.** The outside-app stage builds a
  second `stobie new` application whose library is
  `tools/web-push-app/lib.rs` and whose page opts in, and
  `tools/web-push-smoke.mjs` drives its production package in Chromium
  under a strict Content-Security-Policy. `PushManager.subscribe` is replaced
  by a stand-in with a local endpoint, and every host but the loopback
  address fails to resolve. Pushes arrive through DevTools
  (`ServiceWorker.deliverPushMessage`), and the smoke records the worker's
  `showNotification` and badge calls as they settle: Chromium's
  `getNotifications` drops a notification its message centre has not yet
  displayed, so reading it straight after a push raced (see Observations). Chromium cannot click a system
  notification under automation, so the smoke calls the worker's own
  `openRoute`, and Node tests run the `notificationclick` handler itself.

## Acceptance

- Unit tests (`stobie-build`): the opt-in tag, its errors, the caching
  worker unchanged without it and followed by the push handlers with it, and
  packaging that records `webPush`.
- Node tests (`tools/tests/service-worker-push.test.mjs`, checks stage): the
  generated worker against stand-in browser objects: declarative and bare
  payloads, badges set and cleared, unreadable payloads and invalid badges,
  routes outside the scope, a tap with an open window (focus and post, also
  when focus fails) and without one (remember and open).
- Unit tests (`stobie-ui-egui`): key decoding, route paths, and a native
  `WebPush` reporting unsupported through the host's sender.
- Browser smoke (`tools/web-push-smoke.mjs`, outside-app stage): a physical
  click subscribes inside the click's input event (`window.event` is the
  pointer event and the page has user activation) with
  `userVisibleOnly` and the decoded key; a relaunch reads the subscription;
  a declarative push and an unreadable one show their notifications, with
  the declared title, body, tag and route, and the badge set where Chromium
  has one; the
  worker's route reaches the open application, and a window opened at the
  scope after a tap receives the remembered route once; no
  Content-Security-Policy violation and no page error.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Opt-in push worker in `stobie-build` | Complete | Unit and Node tests |
| `web_push` bridge in `stobie-ui-egui` and the Web Push guide | Complete | Unit tests, browser smoke |
| Browser verification | Complete for Chromium | Web Push smoke in the outside-app stage; no iPhone run (see [Device check](#device-check)) |

## Device check

Not run. Chromium covers the worker, the bridge and subscribing inside a
click, but not Safari's rules. An iPhone check, with the Home Screen
application and a real server, should confirm:
- `support()` is unsupported in Safari and supported from the Home Screen;
- a tap on a button that calls `subscribe` raises the permission prompt and
  subscribes, and a VoiceOver activation of the same button, performed in a
  later pass, is refused or not;
- a push, through Apple's push service, shows its title and body on the
  lock screen and sets the badge from `app_badge`;
- a tap with the application closed opens it at the route, or at its start
  URL with the route delivered from the stored record;
- a tap with the application open in the background focuses it and
  delivers the route;
- the subscription read at each launch matches what the server holds.

## Observations

- The first smoke runs read `registration.getNotifications()` right after a
  DevTools push and found nothing, although the worker's `showNotification`
  had resolved. A push delivered two seconds later, or after an earlier one,
  was listed. Chromium synchronises that list with the notifications its
  platform has displayed, and drops any not yet displayed, so an early read
  removes the notification. The smoke now records the worker's own calls.
- Headless Chromium reports notification permission as `denied` and keeps
  it so even after Playwright grants it; the verification runs headful.
- In Chromium the subscribe call ran during the click's `pointerup`
  dispatch, with user activation, confirming that the host performs a
  tapped action inside the input event. Safari's stricter rule is the
  device check's.

## Later work

- The device check above (owner).
- Notification actions (buttons), images and `requireInteraction` from the
  declarative message are not read.
- Clearing a notification or the badge when the application opens the
  route is left to the application.
