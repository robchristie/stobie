# The `stobie` command

`stobie` creates and builds Stobie applications, and lets an agent with a
shell launch or attach to one, natively or in a browser, read its UI
compactly and operate it through the
[application interface](application-interface.md). It needs no Stobie source
or documentation at run time: `stobie --help`, `stobie COMMAND --help` and
`stobie capabilities --json` describe every command, flag, exit status, wait
condition and the JSON output envelope.

Install it from a pinned commit with
`cargo install --locked --git https://github.com/robchristie/stobie --rev COMMIT stobie-cli`,
or from a checkout with `cargo install --path crates/stobie-cli` or
`cargo build --release -p stobie-cli` (the binary is `target/release/stobie`).
It records the commit it was built from: `stobie --version` prints
`stobie VERSION (commit COMMIT)`, and `capabilities --json` reports it as
`source_revision`. It is a separate package from every
application: no application or WASM build depends on it or on its build
library, `stobie-build`, and `cargo xtask architecture` enforces that.

## Create and build applications

- `new NAME` creates a standalone application in its own Cargo workspace,
  depending on Stobie through git pinned to the CLI's commit
  (`--stobie-rev COMMIT` pins another). `--imaging` adds the image layer,
  `--browser-only` leaves out the native entry point and native eframe,
  `--member` writes a member of the Cargo workspace above the directory, and
  `--stobie-path CHECKOUT` writes `.cargo/config.toml` to build against a
  local checkout. [Start a new app](start-a-new-app.md) walks through it.
- `build-web` builds an application for the browser: a
  `wasm32-unknown-unknown` build with the `release` profile (`--debug` for
  `dev`, `--profile NAME` for another), `wasm-bindgen` into `web/pkg/` and the
  shared startup loader beside `index.html`. It fails, with the install
  command, unless the `wasm-bindgen` CLI is exactly the version of the
  `wasm-bindgen` crate the project locks. `--production` also packages the
  build with compressed, immutable assets, records the profile in the
  package's source identity and reports its sizes; see the
  [browser startup guide](browser-startup.md) and
  [Build profiles](start-a-new-app.md#build-profiles).
- `serve [DIR]` serves a built web root, or a `--production` package (its
  applications at `/NAME/`; `/NAME` redirects there), on `127.0.0.1` without
  caching, for local preview; it does not negotiate compressed
  representations. `--port 0` picks a free port; `--json` prints its URL.
- `package-web NAME=DIR...` packages already built web roots, such as an
  application and its worker, into one production directory, and reports
  each application's sizes.

Stobie's own `cargo xtask build-web`, its production build and Record Desk's
`build-web.sh` use the same implementation. `cargo xtask web-size` reports the
sizes of the repository's production package and of a template application
built with `stobie new`, raw and with Brotli and gzip; `--baseline` compares
with an earlier report. CI's `production` and `outside-app` stages write the
same report to their evidence and step summary. It is a report, not a gate.

## Reach an application

Each command that addresses an application takes one of:

- `--session NAME`: a session that `launch` or `attach` recorded. Without any
  of these flags, `$STOBIE_SESSION` or, when there is only one, the only
  session is used.
- `--socket PATH`: a native application started with
  `STOBIE_AUTOMATION_SOCKET=PATH`.
- `--cdp URL` with `--page URL` or `--target ID`: a browser page opened with
  `?automation=1`, through Chromium's DevTools endpoint.

Sessions live in `$XDG_RUNTIME_DIR/stobie` (or `/tmp/stobie-UID`), a directory
private to the user. `stobie sessions` lists them.

### Native

```sh
stobie launch desk --manifest-path consumers/record-desk/Cargo.toml
stobie observe
stobie stop desk
```

`launch NAME` builds the application with Cargo in release mode
(`--manifest-path PATH`, `--package PKG`, `--bin NAME` or `--example NAME`,
`--debug`), or runs a
built program given after `--`. It creates a fresh private socket directory,
starts the program under a detached supervisor with
`STOBIE_AUTOMATION_SOCKET` set, and returns once the application answers and
has published its first observation. The supervisor removes the directory and
the session when the application exits. It holds a lock in the session
directory, which the application inherits; `stop`, `sessions` and `launch`
treat a session as running only while that lock is held, so a session left
behind by a killed supervisor is removed without signalling a PID that may
since belong to another process. `stop NAME` ends it; for an
attached browser session it only forgets the session and leaves the page
running. The
application's output is kept in `NAME.log` beside the sessions.

The native window must be mapped and not covered: eframe runs queued
invocations only while it draws. On Linux the application needs a display,
such as an X server or Xvfb.

### Browser

Serve the web build (`stobie serve`), open it in a Chromium that exposes
DevTools, with `?automation=1` in its URL, then attach. Any Chromium started
with `--remote-debugging-port=PORT` works. With Lantern:

```sh
stobie serve web/ &                                   # http://127.0.0.1:8080/
lantern browser start --graphics webgpu --json        # or swiftshader
lantern browser endpoint ID                           # prints endpoint=http://127.0.0.1:PORT
lantern open 'http://127.0.0.1:8080/?automation=1' --endpoint http://127.0.0.1:PORT
stobie attach web --cdp http://127.0.0.1:PORT --page 'http://127.0.0.1:8080/?automation=1'
stobie observe --session web
```

`--page` selects the page whose URL is exactly the one given or, failing
that, the only page whose URL starts with it; `stobie` does not add
`?automation=1`, so the URL must already carry it. `--target ID` names a DevTools
target. Each request evaluates only the page's fixed dispatcher,
`window.__STOBIE_AUTOMATION.request`, with `Runtime.evaluate`. A page opened
without `?automation=1` has no dispatcher, and `stobie` says so. On a page
that starts through the shared startup loader, as `stobie new` generates,
`attach` waits for the dispatcher until `--timeout` while the page loads and
the application starts, and other commands say that it has not started yet.
`attach` also waits, within the same timeout, for a page to match its
selector, because Chromium can list a page it has just opened before showing
its URL; other commands fail at once when no page matches. When its
startup failed, both report the failure, for example "No supported
renderer". Without GPU acceleration, Chromium may need
`--use-angle=swiftshader --enable-unsafe-swiftshader` for WebGL2. The DevTools
client is Lantern's `lantern-core`, pinned to an exact revision.

## Selectors

Commands that name controls take `--id`, `--role`, `--name`,
`--capability`, `--pane`, `--domain JSON`, `--argument VALUE` or
`--selector JSON`. All given properties must match. `--argument` selects the
control that carries a value, such as a row's ID: a JSON number, boolean or
quoted string, or else the text given. A positional argument fills the obvious field:
`invoke CAPABILITY`, `query ID`, `discover CAPABILITY`, `set-text ID TEXT`.
Roles are snake case, such as `button`, `text_input`, `combo_box`, `tab`,
`link` and `result_row`. Add `--id` or `--pane` when a capability has several controls.

## Commands

- `observe` outlines the latest observation: application facts, collections,
  audits, then one line per node with its role, name, state, value, the
  argument it carries, stable `#ID` and `[capabilities]`, indented by
  containment. A text field's value
is its text and a combo box's is its selected option's label. Capabilities without a
  rendered control follow as `other capabilities`. `--limit` and `--offset`
  page through long outlines. `--content` adds the presented content text:
  each heading, content label, reading text, rendered Markdown block, list
  marker and link, in presentation order, as an excerpt of at most 256
  characters that ends in `…` when cut, or `withheld` where the application
  withholds it. Only text in view is published, and a password never is; see
  [content text](application-interface.md#content-text).
- `query` and `discover` list nodes or capabilities with `--limit` and
  `--cursor`. `discover` shows each capability's availability, argument
  contract and shortcut, and what a value-taking capability expects. While
  a modal dialog is open, every action it does not present is disabled with
  the reason `The dialog "<title>" is open` (see
  [dialogs](ui-guides/interactions.md#dialogs)).
- `invoke CAPABILITY [--value VALUE]` discovers one target, checks the value
  against its contract (text, `true`/`false`, a number or a choice ID or label; or
  `--value-json`), sends the mutation once, waits for its receipt and then for
  the observation that presents its effect, and reports the facts that
  changed and up to six nodes whose name, state or value `changed`, or that
  were `added` or `removed` (`--json` lists up to 32). A choice value may also be its option's
  label, in any case. When a capability is presented on several rows, the
  value chooses the row's control that carries it, or else the action's own
  binding; a row's control needs no value. Controls that perform the same
  thing, such as one row's Open in two lists, are one choice, and the one
  with the least ID runs. `--request-id` makes a retry safe; `--no-wait` returns once the
  invocation is admitted.
- `set-text ID TEXT` replaces a text field's text through the same edit route
  as typing (`-` reads standard input; `--text-file PATH` reads a file). A
  composer's field takes its draft this way, and its send control carries
  the draft, so `invoke SEND --id ID` then sends it; see
  [composing and sending](ui-guides/interactions.md#composing-and-sending).
- `invoke stobie.open-link --id ID` opens a link, a `link` node whose value
  is its URL, in a new browser tab (natively, the default browser), so the
  application stays open. The outline tags links `[open-link]`.
- `invoke stobie.copy-text --id ID` copies a copy control's text to the
  clipboard, as pressing it does; the control's polite `status` node then
  carries its confirmation or failure. Neither the outline nor discovery
  shows the text: the target's meaning is a text revision. The outline tags
  copy controls `[copy-text]`.
- `wait CONDITION` polls until a condition holds: `present`, `absent`,
  `enabled`, `capability_enabled`, `focused`, `selected`, `checked`, `name`,
  `value`, `status`, `selection_changed`, `fact` (`--key K --value V`),
  `audits_clear` or `content` (`--value TEXT`: a published content excerpt
  contains the text; withheld text never matches), with selector flags; `--condition JSON` gives the whole
  condition. Unknown condition fields are rejected.
- `capture --output PNG` writes a screenshot between two observations: the
  browser canvas through DevTools, or a launched native window through X11
  (`xdotool` and ImageMagick's `import`).
- `receipt ID` reads a mutation's receipt; `cancel ID` cancels a queued one.
- `capabilities` prints the command registry as JSON without connecting.

`--timeout MS` (default 15000) bounds each command, including its waits.

## Output

Text is the default. `--json` prints one JSON object whose first fields are
`schema_version`, `command` and `ok`, in a fixed order, followed by the
command's payload. Lists say how many entries there are, whether they are
`truncated`, and the cursor or offset to continue from. Outputs list what they
leave out in `omitted`, such as node geometry and text layout measurements
(`observe --include-text` adds the measurements: geometry, line counts and
overflow, not the strings unless `--content` is also given) and presented
content (`observe --content` adds a `content` array of each text's
`component`, `role` and `content`). Budgets are tested: Record Desk's default
`observe` stays under 8 KB and the Lab's under 16 KB.

Errors go to standard error. With `--json` they are
`{"schema_version", "command", "ok": false, "error": {"code", "message",
"hint", "request_id"?, "observation"?}}`; otherwise `error: CODE: message`
and `next: hint`. A rejected invocation is a completed exchange: its receipt
is printed on standard output with status 1.

## Exit statuses

| Status | Meaning |
| --- | --- |
| 0 | Success |
| 1 | The application rejected or could not match the request, an invocation was rejected, or a build or capture failed |
| 2 | The command line or a request was malformed |
| 3 | No application answered, or it restarted |
| 4 | A deadline passed, or a mutation's outcome is uncertain: read its receipt, do not repeat it |
| 5 | Evidence is partial, such as a capture whose screenshot failed |

## The Node client

`tools/application-client.mjs` remains the JavaScript client the Playwright
test harness uses, including its physical input adapters. A parity smoke
(`tools/stobie-cli-smoke.sh`) runs the same scenarios through it and through
`stobie`, natively and in the browser, and requires the same outcomes.
