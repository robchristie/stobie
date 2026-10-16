# {{title}}

A [Stobie](https://github.com/robchristie/stobie) application, created with
`stobie new`. It runs natively and in the browser, and agents can read and
operate it with the `stobie` command.

- `src/lib.rs` holds the application: its actions, state, UI and handlers.
  Start there; the [composition guide](https://github.com/robchristie/stobie/blob/{{stobie_rev}}/docs/application-composition.md)
  explains each part.
- `src/main.rs` is the native entry point; the library's `web_handle!` is the
  browser one.
- `web/` holds `index.html` and `bootstrap.js`. `stobie build-web` writes
  `web/pkg/` and `web/browser-startup.js` beside them.

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
