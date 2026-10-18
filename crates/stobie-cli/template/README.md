# {{title}}

A [Stobie](https://github.com/robchristie/stobie) application, created with
`stobie new`. It runs natively and in the browser, and agents can read and
operate it with the `stobie` command.

- `src/lib.rs` holds the application: its actions, state, UI and handlers.
  Start there; the [composition guide](https://github.com/robchristie/stobie/blob/{{stobie_rev}}/docs/application-composition.md)
  explains each part.
- `src/main.rs` is the native entry point; the library's `web_handle!` is the
  browser one.
- `web/` holds `index.html`, `bootstrap.js` and placeholder icons in
  `web/icons/`. `stobie build-web` writes `web/pkg/`, `web/browser-startup.js`,
  `web/manifest.webmanifest` and `web/sw.js` beside them.

## Install it

The page is an installable web application. Its `<head>` names it (`<title>`
and `apple-mobile-web-app-title`), colours it (`theme-color`) and points at its
icons; `stobie build-web` writes the web app manifest and service worker from
those tags, so change them there and replace the PNGs in `web/icons/` at the
same sizes. Browsers install, and service workers run, only over HTTPS or on
`localhost`. On an iPhone, open the page in Safari and choose Share, then Add
to Home Screen.

## Run it

Native (needs a graphical session):

```sh
cargo run --release
```

Browser (needs the `wasm-bindgen` CLI at exactly the version in `Cargo.toml`;
`stobie build-web` says how to install it):

```sh
stobie build-web
stobie serve
```

Then open <http://127.0.0.1:8080/> in a browser with WebGPU or WebGL2. Add
`?renderer=webgl2` to force WebGL2.

## Operate it with `stobie`

```sh
stobie launch app --manifest-path Cargo.toml
stobie observe
stobie set-text {{name}}.draft 'Buy stamps'
stobie invoke {{name}}.add
stobie stop app
```

In the browser, open the page with `?automation=1` in a Chromium started with
`--remote-debugging-port=9222`, then
`stobie attach web --cdp http://127.0.0.1:9222 --page 'http://127.0.0.1:8080/?automation=1'`.
`stobie --help` describes every command.

## Add an action with an argument

An action can take a value that its control or an agent supplies. In outline,
for a choice (add `ActionArgument` and `ChoiceArgument` to the
`stobie_ui_egui` imports in `src/lib.rs`):

```text
// 1. Declare it: the variant carries no data. Arguments are "text" (max_length),
//    "boolean", "number" (minimum, maximum, integer) or "choice" (choices = Type).
#[action(label = "Sort notes", description = "Choose the order of the notes",
         argument = "choice", choices = SortOrder)]
Sort,

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortOrder { #[default] Oldest, Newest, Alphabetical }
impl ChoiceArgument for SortOrder {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Oldest, "oldest", "Oldest first"),
        (Self::Newest, "newest", "Newest first"),
        (Self::Alphabetical, "alphabetical", "A to Z"),
    ];
}

// 2. Handle the value in Application::perform_with; `perform` still matches
//    the variant but never receives it without a value.
fn perform_with(&mut self, target: ActionTarget<NoteAction>, argument: ActionArgument,
                state: &mut ()) -> Result<bool, String> {
    match target.action {
        NoteAction::Sort => { self.sort = argument.as_choice()?; Ok(true) }
        _ => self.perform(target, state),
    }
}

// 3. Present a control in the pane and trigger the action with its value.
let mut sort = self.sort;
let options: Vec<(SortOrder, &'static str)> =
    SortOrder::CHOICES.iter().map(|(value, _, label)| (*value, *label)).collect();
stobie_ui_egui::choice_control(ui, SemanticUiId::new("{{name}}.sort"), SemanticUiId::pane(pane),
    "Sort", &mut sort, &options, NoteAction::Sort, p.tokens());
if sort != self.sort { frame.trigger_with(NoteAction::Sort, ActionArgument::choice(sort)); }
```

An action with an argument has no shortcut, and its `perform` arm returns an
error. For the other kinds, `as_text()`, `as_boolean()` and `as_number()` each
return an `Option`, and controls trigger `ActionArgument::Text(..)`,
`ActionArgument::Boolean(..)` or `ActionArgument::Number(f64)`. A number is
declared `argument = "number", minimum = 1, maximum = 20, integer = true`
and presented with `range_control(ui, id, parent, label, &mut value_f32,
1.0..=20.0, action, p.tokens())`. A text value usually comes from a text field
that agents fill with `stobie set-text`. Publish the value as a fact in `observe` so agents can wait for
it, then run `stobie invoke {{name}}.sort --value alphabetical`. The
[guide](https://github.com/robchristie/stobie/blob/{{stobie_rev}}/docs/start-a-new-app.md#add-an-action-with-an-argument)
explains each step.

## Test

```sh
cargo test
```

## Stobie version

Stobie is not on crates.io. `Cargo.toml` depends on it through git, pinned to
revision `{{stobie_rev}}`. To upgrade, replace that revision in every Stobie
dependency and run `cargo update`. To build against a local Stobie checkout
instead, create `.cargo/config.toml`:

```toml
[patch."https://github.com/robchristie/stobie"]
stobie-core = { path = "/path/to/stobie/crates/stobie-core" }
stobie-ui-egui = { path = "/path/to/stobie/crates/stobie-ui-egui" }
```

Patch every Stobie crate `Cargo.toml` names. Cargo still fetches the pinned
revision, so it must exist on GitHub.
