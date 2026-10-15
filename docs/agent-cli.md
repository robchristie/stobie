# The `stobie` command

`stobie` lets an agent with a shell launch or attach to a Stobie application,
natively or in a browser, read its UI compactly and operate it through the
[application interface](application-interface.md). It needs no Stobie source
or documentation at run time: `stobie --help`, `stobie COMMAND --help` and
`stobie capabilities --json` describe every command, flag, exit status and
output schema.

Build it with `cargo build --release -p stobie-cli` (the binary is
`target/release/stobie`), or install it with
`cargo install --path crates/stobie-cli`. It is a separate package from every
application: no application or WASM build depends on it, and
`cargo xtask architecture` enforces that.

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
the session when the application exits. `stop NAME` ends it. The
application's output is kept in `NAME.log` beside the sessions.

The native window must be mapped and not covered: eframe runs queued
invocations only while it draws. On Linux the application needs a display,
such as an X server or Xvfb.

### Browser

Serve the web build, open it in a Chromium that exposes DevTools, with
`?automation=1` in its URL, then attach. With Lantern:

```sh
lantern browser start --graphics webgpu --json        # or swiftshader
lantern browser endpoint ID                           # prints endpoint=http://127.0.0.1:PORT
lantern open 'http://127.0.0.1:8080/?automation=1' --endpoint http://127.0.0.1:PORT
stobie attach web --cdp http://127.0.0.1:PORT --page 'http://127.0.0.1:8080/?automation=1'
stobie observe --session web
```

`--page` selects the page whose URL is exactly the one given or, failing
that, the only page whose URL starts with it; `--target ID` names a DevTools
target. Each request evaluates only the page's fixed dispatcher,
`window.__STOBIE_AUTOMATION.request`, with `Runtime.evaluate`. A page opened
without `?automation=1` has no dispatcher, and `stobie` says so. The DevTools
client is Lantern's `lantern-core`, pinned to an exact revision.

## Selectors

Commands that name controls take `--id`, `--role`, `--name`,
`--capability`, `--pane`, `--domain JSON` or `--selector JSON`. All given
properties must match. A positional argument fills the obvious field:
`invoke CAPABILITY`, `query ID`, `discover CAPABILITY`, `set-text ID TEXT`.
Roles are snake case, such as `button`, `text_input`, `combo_box`, `tab` and
`result_row`. Add `--id` or `--pane` when a capability has several controls.

## Commands

- `observe` outlines the latest observation: application facts, collections,
  audits, then one line per node with its role, name, state, value, stable
  `#ID` and `[capabilities]`, indented by containment. Capabilities without a
  rendered control follow as `other capabilities`. `--limit` and `--offset`
  page through long outlines.
- `query` and `discover` list nodes or capabilities with `--limit` and
  `--cursor`. `discover` shows each capability's availability, argument
  contract and shortcut.
- `invoke CAPABILITY [--value VALUE]` discovers one target, checks the value
  against its contract (text, `true`/`false`, a number or a choice ID or label; or
  `--value-json`), sends the mutation once, waits for its receipt and then for
  the observation that presents its effect, and reports the facts that
  changed. `--request-id` makes a retry safe; `--no-wait` returns once the
  invocation is admitted.
- `set-text ID TEXT` replaces a text field's text through the same edit route
  as typing (`-` reads standard input; `--text-file PATH` reads a file).
- `wait CONDITION` polls until a condition holds: `present`, `absent`,
  `enabled`, `capability_enabled`, `focused`, `selected`, `checked`, `name`,
  `value`, `status`, `selection_changed`, `fact` (`--key K --value V`) or
  `audits_clear`, with selector flags; `--condition JSON` gives the whole
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
leave out in `omitted`, such as node geometry and measured text observations
(`observe --include-text` adds them). Budgets are tested: Record Desk's default
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
| 1 | The application rejected or could not match the request, or an invocation was rejected |
| 2 | The command line or a request was malformed |
| 3 | No application answered, or it restarted |
| 4 | A deadline passed, or a mutation's outcome is uncertain: read its receipt, do not repeat it |
| 5 | Evidence is partial, such as a capture whose screenshot failed |

## The Node client

`tools/application-client.mjs` remains the JavaScript client the Playwright
test harness uses, including its physical input adapters. A parity smoke
(`tools/stobie-cli-smoke.sh`) runs the same scenarios through it and through
`stobie`, natively and in the browser, and requires the same outcomes.
