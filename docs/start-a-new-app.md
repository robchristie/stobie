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
applications to it. The install needs no `git` command: Cargo fetches the
commit with its own Git library, and the CLI reads the commit from the
checkout's files. A CLI built from a source tree that is not a Git checkout,
such as an unpacked archive, names no commit unless `STOBIE_SOURCE_REVISION`
holds the full commit when it builds; `stobie --version` shows the commit it
names. Native applications need a graphical session and a
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
| `web/icons/` | Placeholder icons for the installable page, to replace |
| `README.md` | These commands, for the next reader |

The application declares its actions, state, UI and handlers; the
[composition guide](application-composition.md) explains each part, and
the [`notes` example](../crates/stobie-ui-egui/examples/notes.rs) adds undo
and redo. `stobie new --imaging` also adds the image layer: `stobie-imaging`,
`stobie-runtime`, `stobie-render-wgpu` and the UI crate's `image-rendering`
feature, described in [Compose an image application](image-application-composition.md).

The application follows the system's light or dark appearance and, in the
browser, its text size, because it leaves `Application::preferences` at its
default, `UiPreferences::system()`; the page's own colours in
`web/index.html` follow the same setting. Override `preferences` to offer a
choice, as the [composition guide](application-composition.md) describes.

A page served with a Content-Security-Policy that leaves `'unsafe-inline'`
out of `style-src` must allow the `<style>` element in `web/index.html` by
its hash, or move its rules into a stylesheet file. Stobie's own code styles
the page only through the CSSOM, so it needs nothing more; the
[accessibility guide](ui-guides/accessibility.md#browser-aria-layer) has the
details.

### A browser-only application

`stobie new my-app --browser-only` writes an application that runs only in
the browser. It has no `src/main.rs`, and `Cargo.toml` declares eframe and the
UI crate's `eframe` feature only for `wasm32`, so natively the library builds
without a window system or GPU, for `cargo test`. Build and serve it with
`stobie build-web` and `stobie serve` as above, and operate it with `stobie
attach web`.

### Inside an existing Cargo workspace

`stobie new apps/my-app --member` (with `--path` for another directory)
writes an application as a member of the Cargo workspace above it: its
`Cargo.toml` has no `[workspace]` or `[profile.release]`, and it has no
`rust-toolchain.toml`, because Cargo takes those from the workspace. It
refuses a directory with no workspace above it. Then:

- add the directory to `[workspace] members` in the workspace's `Cargo.toml`
  (`stobie new` prints the line);
- set the profile for web builds there, since Cargo reads profiles only from
  the workspace root (see [Build profiles](#build-profiles));
- run `stobie` from the application's directory or pass `--manifest-path`.
  Cargo looks for `.cargo/config.toml` from the directory it runs in and its
  parents, so a `--stobie-path` patch written beside the application applies
  only when Cargo runs there; move it to the workspace's `.cargo/` to apply it
  to every member.

`stobie build-web` builds the library for `wasm32-unknown-unknown` in release
mode, checks that the `wasm-bindgen` CLI is exactly the version of the
`wasm-bindgen` crate the project locks (and prints the install command when
it is not), runs it into `web/pkg/`, and writes the shared startup loader,
`web/browser-startup.js`. Because the page links `manifest.webmanifest`, it
also writes that manifest and a service worker, `web/sw.js`, so the page
installs as a web application, including on an iPhone's home screen. All of
these are generated; the template's `.gitignore` leaves them out. The page's
`<head>` names the application, colours it and lists its icons; the
[browser startup guide](browser-startup.md#installable-web-applications)
describes the tags and the service worker. `stobie serve` serves `web/` on `http://127.0.0.1:8080/`
without caching. The host prefers WebGPU and falls back to WebGL2;
`?renderer=webgl2` forces it.

For hosting, `stobie build-web --production` also writes a package with
Brotli and gzip representations and immutable, content-addressed asset
directories, by default in `target/web-production/`. `--variant Oz` or `O3`
runs Binaryen 131's `wasm-opt`. The [browser startup guide](browser-startup.md#hosting-and-publication)
says how to serve and publish it. An application with a Web Worker builds each
web root and packages them together with `stobie package-web`. Both print each
packaged application's sizes in bytes, raw and with Brotli, for its WebAssembly
and for everything it serves; `--json` gives them as `size` (`build-web`) or
`sizes` (`package-web`), which the package's `manifest.json` records per file.

### Build profiles

`stobie build-web` builds with Cargo's `release` profile, `--debug` with
`dev`, and `--profile NAME` with any other, such as a profile for web builds
in the workspace's `Cargo.toml`:

```toml
[profile.web]
inherits = "release"
lto = "thin"
codegen-units = 1
```

```sh
stobie build-web --production --profile web
```

A production package's `sourceIdentity` records the profile's name, the
workspace's `Cargo.toml` when the application is a member of a workspace rooted
elsewhere, and any `CARGO_PROFILE_*` environment variables. All of these change
its content identity, so packages built with different settings never share
asset directories. Profile settings in `.cargo/config.toml` are not recorded;
set them in `Cargo.toml` instead.

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
variant; return an error there, because it always arrives with a value:

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

`range_control(ui, id, parent, label, &mut value_f32, minimum..=maximum,
action, p.tokens())` is the counterpart for a number: declare it
`argument = "number", minimum = 1, maximum = 20, integer = true` and trigger
`ActionArgument::Number(f64::from(value))`. `ActionArgument::Text(..)` and
`ActionArgument::Boolean(..)` carry the other kinds. A text value usually comes
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

## Present an action on each row

When each row of a list offers the same action, such as Open on each
person, declare the action with an argument for the row's ID and present it
once per row as a `BoundAction`. Each row's control then carries its ID: a
click performs the action for that row, agents discover one control per row
carrying its ID, and assistive technology hears a distinct name per row
("Open Ada Lovelace"):

```text
#[action(label = "Open", description = "Open the person with this ID",
         argument = "number", minimum = 1, maximum = 999, integer = true)]
Open,
```

```text
for person in &self.people {
    // The row itself, named by its title, selects the person.
    frame.list_row(ui, &mut p, BoundAction::new(PeopleAction::Select, person.key),
        ListRowSpec { title: &person.name, detail: Some(&person.role) });
    frame.action(ui, &mut p,
        BoundAction::new(PeopleAction::Open, person.id).with_subject(&person.name),
        ActionEmphasis::Normal);
}
```

`icon_action` and `toggle` take a `BoundAction` too. `perform_with` receives
the row's ID as the argument. Implement `availability_with` to decide each
row's availability and to hide an ID that names no row, so that a click or
an agent acting on a removed row is refused before `perform_with`:

```text
fn availability_with(&self, target: ActionTarget<PeopleAction>,
                     argument: &ActionArgument) -> Availability {
    match argument.as_number().and_then(|id| self.person(id as u32)) {
        Some(_) => Availability::Enabled,
        None => Availability::Hidden,
    }
}
```

Agents choose a row by the value its control carries:

```sh
stobie discover my-app.open              # one per row, such as `my-app.open = 2`
stobie invoke my-app.open --value 2      # the row carrying 2
stobie invoke my-app.select --argument ada
```

## What agents can read

`stobie observe` lists each control's role, name, state and value and the
facts your application publishes, and leaves out geometry. `stobie observe
--content` adds the page content in view, such as headings, labels, note text,
Markdown and links, each as an excerpt of at most 256 characters; `stobie wait
content --value TEXT` waits for it to appear. Wrap anything observation must
not publish in `withhold_content_text`; see
[content text](application-interface.md#content-text). To let an agent confirm
an effect it cannot read from a bounded excerpt, publish a bounded fact for it
in `Application::observe`.

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

## Fonts and licence notices

`stobie-ui-egui` embeds Source Sans 3 and the monochrome Noto Emoji, both
under the SIL Open Font License 1.1, so its `license` field is
`Apache-2.0 AND OFL-1.1`. It does not enable egui's `default_fonts` feature;
the application chooses egui's bundled fonts (Hack, Ubuntu Light, an older
Noto Emoji and an emoji icon font) through its own dependencies. `stobie new`
keeps them through eframe's `default_fonts` feature, and depends on `egui`
without its default features, so that feature is the one switch. To leave
them out, remove it; monospace text then uses Source Sans 3. Leaving them out
makes the template's compressed WASM about 680 KB smaller.
`cargo tree -e features -i epaint_default_fonts` shows what still enables
them.

A build that is distributed must carry each embedded font's licence and
copyright notice. The
[bundled-font record](../crates/stobie-ui-egui/assets/fonts/README.md) lists
Stobie's faces, their licence files and notices; egui's fonts carry their own
in the `epaint_default_fonts` crate. Emoji draw in one colour, the text colour,
because egui cannot draw colour glyphs.

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
