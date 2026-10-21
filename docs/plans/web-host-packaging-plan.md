# Web host and packaging

Status: complete
Delivery: [PR #122](https://github.com/robchristie/stobie/pull/122)

## Outcome and boundaries

Four gaps a chat application built on Stobie met when it hosted its own web
package:

- New-tab links opened without an opener only when the application started
  through `WebHost`. An application that ran eframe's web runner with an
  `AppHost`, or without one, handed every linked page `window.opener`.
- `stobie --version`, and so `stobie new`'s pin, named no commit when the
  CLI was installed with `cargo install --git` on a host without the `git`
  command, such as a container image: the build script asked `git` alone.
- `WebPush::start` on a page with no service worker waited for
  `navigator.serviceWorker.ready` for ever, and the application received no
  `State`.
- The documentation said to retain earlier versioned assets only through the
  reference server's `--retain-directory`, which says nothing to an
  application server that serves the package from one directory.

Out of scope: a retention helper in the build or the CLI (the guidance is
enough, since content-addressed names never collide); any change to the
service worker's caching.

## Decisions

- Every `AppHost` installs the new-tab plugin, and each Stobie link component
  installs it when it asks for a new tab, since `Context::add_plugin` is
  idempotent and a plugin added mid-pass still sees that pass's output.
  `install_new_tab_links(ctx)` is public for an application with neither.
  `WebHost` no longer installs it separately.
- The CLI build script asks `git` first and otherwise reads the checkout's
  files (`HEAD`, the loose ref or `packed-refs`, and a linked worktree's
  `gitdir` and `commondir`), so Cargo's own checkout, which its Git library
  makes, names its commit. The checkout's top level is two directories above
  the crate and must hold the Git directory, so vendored source names no
  commit. `STOBIE_SOURCE_REVISION` still overrides, for a source tree that is
  not a checkout.
- The startup loader publishes `window.__stobieServiceWorker`, a promise of
  its registration record, before the application starts. `WebPush::start`
  awaits it and, when the loader registered no worker and the page has no
  registration of its own, delivers `State` with `Unsupported` and a reason;
  `support()` then reports the same. A page whose own code registers its
  worker still gets `State` once that worker is active.
- Asset retention is guidance in `docs/browser-startup.md`, "Keeping earlier
  packages' assets": what to keep, what never to keep and for how long, from
  what the service worker does at activation.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Opener protection for every entry point | Complete | Native tests that an `AppHost`, a reading-text link and a Markdown link install the plugin, and a same-tab link does not; the existing reading-text and Markdown browser smokes check `noopener` in the browser |
| The CLI's commit without Git | Complete | Four unit tests of the file reader; `cargo install --git` without `git` on `PATH` |
| `WebPush::start` without a service worker | Complete | A native test of the reason, a loader test of the promise, and a Web Push smoke step that serves the page without its manifest link |
| Earlier packages' assets | Complete | Guidance and the service worker's comment |
| Candidate qualification | Complete | `cargo xtask verify` and `cargo xtask web-size` on the reviewed head; review, CI and merge retained in the PR |

## Evidence

- `cargo install --locked --git file://<checkout> --rev <commit> stobie-cli`
  with a `PATH` holding no `git` command, in a fresh target directory:
  at this branch's first commit `stobie --version` names that commit; at
  `2964cad` (before) it names none.
- Web package sizes from `cargo xtask web-size` (WASM raw / Brotli, bytes),
  against `2964cad`: Gallery 12155584 / 3046856 (-237 / +710), Lab
  12459697 / 3069921 (-332 / +172), template 10475398 / 2688246
  (-720 / -375).

## Limits

- Chromium alone runs the browser checks; Safari's and Firefox's handling
  of `noopener` and of a page without a service worker are not exercised.
- The no-Git build was checked by hand, not in CI.
