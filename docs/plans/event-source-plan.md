# Events from outside the host's passes

Status: complete
Delivery: [PR #93](https://github.com/robchristie/stobie/pull/93)

Framework work item 3 of the [iOS PWA spike report](../ios-pwa-spike-report.md#framework-work):
an async event source that wakes the host. Bokkie, the first consumer, needs a
browser WebSocket client that reconnects, catches up from a sequence number and
feeds an `AppHost` application. PWA output (item 4) is a separate plan.

## Problem

A socket, timer or fetch delivers data outside a pass. Before this work an
application could push it in only through a hand-written
`#[wasm_bindgen] impl WebHandle` block calling
`WebHost::with_host_mut(|host| host.submit(intent))`, as the Gallery does, and
natively it had to manage its own thread and `egui::Context` handle. Nothing
documented how such an event should wake the host or what it costs.
`PassContext` had no delayed repaint, so timed work in `prepare` had to schedule
its wake-up from `ui`.

## Decision

**A queue of intents owned by `AppHost`, a cloneable `IntentSender` that wakes
the host, and an `Application::started` hook that hands the application its
sender.** Transports stay in the application.

- `IntentSender<I>` holds a weak reference to the host's queue and the egui
  context. `send` pushes the intent and calls `Context::request_repaint` when
  the queue was empty, so a burst of events wakes one pass. It returns
  `HostStopped(intent)` once the host is dropped. It is `Send` and `Sync` when
  the intent is `Send`; on wasm32 the same code runs on the page's thread.
- The host applies queued intents at the start of a pass, before `prepare`, in
  the order sent, through `Application::apply`. eframe's `App::logic` hook also
  applies them, which it calls before every pass and, natively, while the window
  is minimised. The pass that applies them presents them, so they record no
  `HostRepaint`: an idle application wakes for exactly the events that arrive.
  Errors reach `report_error` and show in the same pass.
  `CompletedPass::sent_intents` counts them.
- While the viewport is hidden (a minimised or occluded window, or a hidden
  browser page) eframe runs `logic` but no pass: on the web it falls back from
  `requestAnimationFrame` to a `setTimeout` throttled to about once a second.
  The host still applies sent intents then, and records that a pass has yet to
  present them. eframe runs no pass when a page is shown again unless focus
  changes, and none when a native window stops being occluded, so while hidden
  with unpresented changes the host schedules a recheck every 250 ms
  (`HIDDEN_RECHECK`) until a visible pass presents them. A hidden application
  with nothing new schedules nothing.
- `Application::started(&mut self, IntentSender<Self::Intent>)` runs once in
  `AppHost::new`, before the first pass. `AppHost::intent_sender` returns more
  senders for page hooks and tests. `with_host_mut` and `submit` remain for
  page hooks that need a synchronous result.
- `PassContext::request_repaint_after` matches `Frame::request_repaint_after`.

Alternatives considered:

- *An application-owned receiver that the host polls through a trait method.*
  Every application would have to store and return it, and a forgotten return
  would silently drop events. The host owning the queue guarantees the drain.
- *Applying intents directly from the sender on wasm32*, as `submit` does,
  which would skip the pass for an intent that changes nothing. The sender has
  no access to the runner, the runner may be borrowed, and it would make the
  two platforms differ. Transports filter traffic instead.
- *A WebSocket client in Stobie.* Which sockets, reconnect policy and protocol
  are transport policy, which [browser-startup.md](../browser-startup.md#consuming-applications)
  keeps outside Stobie core. The client lives in an example to copy.
- *A one-repaint wake* (`request_repaint_after` with a tiny delay, which egui
  answers with one pass instead of two). It would halve passes per streamed
  token, but every other Stobie repaint uses egui's two-pass convention, which
  exists to settle frame-delayed layout such as scroll areas following new
  content. Not changed here; see [Later work](#later-work).

## Example

`apps/live-feed` is a workspace application, built natively and for the web.
Its feed events are `{"seq": N, "text": "..."}`.

- The browser transport, `src/socket.rs`, is a `web-sys` WebSocket client. It
  connects to `<feed URL>?after=<seq>` with the highest sequence number
  received, forwards only newer events (so a replayed overlap or heartbeat wakes
  nothing), reports connection changes as intents, and reconnects after 250 ms,
  doubling to 8 s and resetting on open. Its callbacks hold weak references and
  are created once, because a callback that reconnects must not drop itself.
  Dropping it, as the host does when it stops, closes the socket and clears the
  timer.
- The native transport, `src/stdin.rs`, reads one event per line of standard
  input on a thread, the shape of a native socket client.
- The application publishes `connection`, `last_sequence`, `received`, `missed`,
  `shown` and `latest` facts.

## Acceptance

- Host unit tests: an idle host asks for no pass; an intent sent from another
  thread wakes it; the next pass presents it with no host repaint reason and
  only egui's settling pass follows; intents sent before a pass join it in
  order; an error shows in that pass; an intent that changes nothing costs its
  passes and no more; a dropped host refuses sends; `prepare` can schedule a
  later pass; an intent applied while hidden schedules rechecks until a
  visible pass presents it, and a hidden host with nothing new asks for
  nothing.
- Example unit tests: parsing, duplicates, gaps, the bounded tail, standard
  input lines in order, and Clear.
- `tools/live-feed-smoke.sh native`: the release binary under Xvfb, driven
  through standard input and observed through its automation socket. Idle
  between events, each event presents in at most three passes, and the closed
  source is reported.
- `tools/live-feed-smoke.sh browser`, on WebGPU in the `browser` stage and on
  WebGL2 in the `webgl2` stage: Chromium with Playwright's `routeWebSocket` as a
  mock feed server. The client connects to `feed?after=0`, receives events,
  stays idle, repaints for each event, does not repaint for a replayed event or
  a heartbeat, reconnects after the server closes the connection with
  `after=4`, catches up on the events published meanwhile with none missed or
  duplicated, and receives later events. With nothing focused, it hides the
  page (emulating `document.hidden` and dispatching `visibilitychange`), sends
  an event, checks that nothing presents it, shows the page and expects it
  presented with no input. That step failed on the first reviewed head and
  passes with the recheck.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Framework sender, queue, hook and delayed `prepare` repaint | Complete | Host unit tests |
| Hidden-viewport recheck, from independent review of the first head | Complete | Host unit test; hidden-page browser smoke step |
| Live Feed example, smokes and verification wiring | Complete | Example tests; native and browser smokes on WebGPU and WebGL2 |
| Documentation | Complete | Composition guide section "Receive events from outside"; browser guide; README; example README |

## Observations

- egui answers each immediate repaint request with two passes ("to give some
  things time to settle"), so one event costs two passes: the one that applies
  it and egui's settling pass. The local browser smoke measured exactly two per
  event, and none for a filtered replay or heartbeat.
- Constructing an `AppHost` for a kittest harness needs the harness's context,
  which exists only after the harness is built, so tests build the harness with
  a placeholder host and replace it.
- A useful `Debug` for `HostStopped` cannot require `Debug` intents, because
  application intents rarely derive it; it prints no intent.

- Independent review of the first head found the hidden-page defect by
  reading eframe's source; the smoke reproduced it only once the canvas lost
  focus, because a focused canvas makes eframe repaint on every visibility
  change.

## Later work

- Measure whether a single-pass wake for sent intents is safe for scroll areas
  and text layout, and adopt it if so: it would halve the passes a streamed
  reply costs.
- A native WebSocket example, which needs a client crate; Bokkie is web first.
