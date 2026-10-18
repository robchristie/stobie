# Live Feed

A small Stobie application that receives numbered events from outside its
passes and is idle between them. It is the reference for feeding a socket,
timer or fetch into an `AppHost` application; the
[composition guide](../../docs/application-composition.md#receive-events-from-outside)
describes the route.

- `src/feed.rs`: the events and what the application keeps of them. An event
  is `{"seq": 4, "text": "..."}`; sequence numbers start at 1.
- `src/lib.rs`: the application. `Application::started` gives its
  `IntentSender` to the transport, and each event applies as an ordinary
  intent at the start of the next pass.
- `src/socket.rs`: the browser transport, a WebSocket client. It connects to
  `<feed URL>?after=<seq>` with the highest sequence number it has received,
  forwards only newer events, and reconnects after 250 ms, doubling to 8 s.
- `src/stdin.rs`: the native transport, a thread that reads one event per
  line of standard input, as a native socket client's thread would.

Run it natively and type lines, or pipe them in:

```sh
cargo run -p live-feed
```

For the browser, `cargo xtask build-web` writes `apps/live-feed/web/`. The
page connects to `feed` beside it, or to the WebSocket URL given as
`?feed=<url>`, so serve it with a feed server that speaks the format above.
`tools/live-feed-smoke.sh browser` runs it in Chromium against a mock server
that drops the connection, and checks the catch-up; `tools/live-feed-smoke.sh
native` drives the native build through standard input. Both check that the
application paints nothing while idle and repaints when an event arrives.
