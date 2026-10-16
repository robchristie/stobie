# Start a new app

A Stobie application lives in its own repository and depends on Stobie
through git. The `stobie` command creates it, builds it for the web and
operates it. Stobie is not on crates.io and has no releases; you pin an exact
commit.

## Install the tools

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --git https://github.com/robchristie/stobie --rev COMMIT stobie-cli
cargo install --locked wasm-bindgen-cli --version 0.2.127
```

`COMMIT` is the full 40-character Stobie commit you want, such as the head of
`main`. The `stobie` binary remembers it, and `stobie new` pins new
applications to it. Native applications need a graphical session and a
wgpu-compatible backend; browser builds run in a browser with WebGPU or
WebGL2.

## Create, run and build

```sh
stobie new my-app
cd my-app
cargo test
cargo run --release
stobie build-web
stobie serve
```

`stobie new` writes a standalone Cargo workspace:

| Path | What it is |
| --- | --- |
| `Cargo.toml` | The package, its own `[workspace]`, git dependencies on Stobie pinned to one revision, and `wasm-bindgen` pinned for the web |
| `rust-toolchain.toml` | The Rust toolchain Stobie is built with, plus the wasm target |
| `src/lib.rs` | A small notes application on the framework host, with tests |
| `src/main.rs` | The native entry point |
| `web/index.html`, `web/bootstrap.js` | The browser page and its bootstrap |
| `README.md` | These commands, for the next reader |

The application declares its actions, state, UI and handlers; the
[composition guide](application-composition.md) explains each part, and
the [`notes` example](../crates/stobie-ui-egui/examples/notes.rs) adds undo
and redo. `stobie new --imaging` also adds the image layer: `stobie-imaging`,
`stobie-runtime`, `stobie-render-wgpu` and the UI crate's `image-rendering`
feature, described in [Compose an image application](image-application-composition.md).

`stobie build-web` builds the library for `wasm32-unknown-unknown` in release
mode, checks that the `wasm-bindgen` CLI is exactly the version of the
`wasm-bindgen` crate the project locks (and prints the install command when
it is not), runs it into `web/pkg/`, and writes the shared startup loader,
`web/browser-startup.js`. Both are generated; the template's `.gitignore`
leaves them out. `stobie serve` serves `web/` on `http://127.0.0.1:8080/`
without caching. The host prefers WebGPU and falls back to WebGL2;
`?renderer=webgl2` forces it.

For hosting, `stobie build-web --production` also writes a package with
Brotli and gzip representations and immutable, content-addressed asset
directories, by default in `target/web-production/`. `--variant Oz` or `O3`
runs Binaryen 131's `wasm-opt`. The [browser startup guide](browser-startup.md#hosting-and-publication)
says how to serve and publish it. An application with a Web Worker builds each
web root and packages them together with `stobie package-web`.

## Operate it with `stobie`

```sh
stobie launch app --manifest-path Cargo.toml
stobie observe
stobie set-text my-app.draft 'Buy stamps'
stobie invoke my-app.add
stobie stop app
```

Every action you declare is discoverable and invocable this way, natively and
in the browser, without inspection code. In the browser, open the page with
`?automation=1` in a Chromium that exposes DevTools, then `stobie attach`.
[The `stobie` command](agent-cli.md) describes both.

## Add an action with an argument

An action can take a value that its control or an agent supplies: text, a
boolean, a number or a choice. Declare it on the variant; the variant itself
carries no data:

```text
#[action(label = "Sort notes", description = "Choose the order of the notes",
         argument = "choice", choices = SortOrder)]
Sort,
```

The argument is `"text"` (with an optional `max_length`), `"boolean"`,
`"number"` (with `minimum` and `maximum`, and `integer = true` for whole
numbers) or `"choice"` (with `choices = Type`). An action with an argument
cannot have a shortcut, because a key press supplies no value. A choice type
lists its options once, with a stable ID and a label for each:

```text
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortOrder { #[default] Oldest, Newest, Alphabetical }

impl ChoiceArgument for SortOrder {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Oldest, "oldest", "Oldest first"),
        (Self::Newest, "newest", "Newest first"),
        (Self::Alphabetical, "alphabetical", "A to Z"),
    ];
}
```

Handle it in `Application::perform_with`, which receives the validated value,
and pass every other action on to `perform`. `perform` must still match the
variant; reject it there, because it always arrives with a value:

```text
fn perform_with(&mut self, target: ActionTarget<NoteAction>, argument: ActionArgument,
                state: &mut ()) -> Result<bool, String> {
    match target.action {
        NoteAction::Sort => {
            // Other kinds: as_text(), as_boolean(), as_number(), each an Option.
            let order: SortOrder = argument.as_choice()?;
            let changed = order != self.sort;
            self.sort = order;
            Ok(changed)
        }
        _ => self.perform(target, state),
    }
}
```

In the pane, present the control for the current value and trigger the action
with the value the user chose. `ui` borrows the application immutably, so the
control edits a copy:

```text
let mut sort = self.sort;
let options: Vec<(SortOrder, &'static str)> =
    SortOrder::CHOICES.iter().map(|(value, _, label)| (*value, *label)).collect();
stobie_ui_egui::choice_control(ui, SemanticUiId::new("my-app.sort"), SemanticUiId::pane(pane),
    "Sort", &mut sort, &options, NoteAction::Sort, p.tokens());
if sort != self.sort {
    frame.trigger_with(NoteAction::Sort, ActionArgument::choice(sort));
}
```

`range_control` is the counterpart for a number. A text value usually comes
from a text field instead: agents fill it with `stobie set-text` and invoke a
plain action, as the template's draft and Add do. Clicks and agents then take
the same route, and `stobie discover my-app.sort` shows the contract:

```sh
stobie invoke my-app.sort --value alphabetical    # the option's ID or its label
```

Publish the value as a fact in `observe` (`observation.fact("sort",
self.sort.choice_id())`) so that agents can wait for it. The `Action` derive's
and `Application`'s rustdoc (`cargo doc --open` in your application) give the
exact contracts.

## What agents can read

`stobie observe` lists each control's role, name, state and value and the
facts your application publishes. It does not export page content such as
headings or note text, and it leaves out geometry. To let an agent confirm an
effect, publish a bounded fact for it in `Application::observe`.

Address controls by their capability IDs (`my-app.add`) or by the semantic
IDs you give them (`my-app.draft`). Generated presentation IDs, such as
`#presentation.300df68a0e3a837e`, can differ between native and browser
hosts.

## Without GPU acceleration

On a machine with no usable GPU, such as a virtual machine or a CI runner,
Chromium may block WebGL2 as well as WebGPU, and the page then reports "No
supported renderer". Start Chromium with
`--use-angle=swiftshader --enable-unsafe-swiftshader` for software WebGL2. A
native application on a software-only X server may need `WGPU_BACKEND=gl`.
`stobie attach` waits for a page that is still loading its application, and
reports the page's own startup failure when it fails.

## Upgrade Stobie

Every Stobie dependency in `Cargo.toml` names the same `rev`. To upgrade,
replace it everywhere with the new full commit and run `cargo update`.
Install the `stobie` CLI from the same commit, so its loader, template and
inspection protocol match. Stobie is pre-1.0; read the commits between the two
revisions for API changes.

## Build against a local Stobie checkout

To try unmerged Stobie changes, patch the git source with a local checkout in
`.cargo/config.toml`, one line per Stobie crate `Cargo.toml` names:

```toml
[patch."https://github.com/robchristie/stobie"]
stobie-core = { path = "/path/to/stobie/crates/stobie-core" }
stobie-ui-egui = { path = "/path/to/stobie/crates/stobie-ui-egui" }
```

`stobie new my-app --stobie-path /path/to/stobie` writes this file for you.
Cargo still fetches the pinned revision to resolve the patch, so the pin must
be a commit GitHub has. Stobie's own CI does exactly this: its `outside-app`
stage creates an application outside the checkout, patches it to the source
under test, builds it natively and for the web, and drives it with `stobie`
natively and on WebGPU and WebGL2 (`tools/outside-app-smoke.sh`).

## Inside the Stobie repository

[Record Desk](../consumers/record-desk/README.md) is a larger application in
its own Cargo workspace. It lives in this repository and uses `path`
dependencies so every change is tested against it; its README says which parts
of its setup are specific to that.
