# Build profiles, scaffolding and web size

Status: complete
Delivery: [PR #PENDING](https://github.com/robchristie/stobie/pull/PENDING)

A chat application built on Stobie met five gaps in the build and scaffolding
path: `stobie build-web` built only with `release` or `dev`, so a smaller web
profile could not be chosen or recorded; an application meant only for the
browser still built eframe, winit and wgpu natively; an action enum needed at
least one variant; `stobie new` wrote only a standalone workspace; and nothing
measured the web package's size.

## Decisions

- **A named profile, recorded.** `stobie build-web --profile NAME` (and
  `stobie_build::build_with_profile`) builds with any Cargo profile, such as
  a `[profile.web]` that inherits `release`; `--debug` remains `dev` and the
  two conflict. A production package's `sourceIdentity` records the profile's
  name as before, plus the workspace root's `Cargo.toml` when the package is a
  member of a workspace rooted elsewhere (where Cargo reads profiles) and any
  `CARGO_PROFILE_*` environment. A standalone release build without overrides
  records exactly what it did before, so its content identity is unchanged.
  `BuildOptions` keeps its fields: a new field would break a literal
  outside the crate.
- **Browser-only applications.** `stobie new --browser-only` omits
  `src/main.rs` and declares eframe and the UI crate's `eframe` feature only
  under `cfg(target_arch = "wasm32")`; natively the UI crate builds with
  `default-features = false`, so `cargo test` needs no window system. The
  composition guide documents the same shape for applications written by
  hand.
- **An empty action enum.** `#[derive(Action)]` accepts an enum with no
  variants: it declares no actions, and the application's `availability` and
  `perform` are `match target.action {}`. The `trybuild` case that rejected it
  is removed and a host test runs such an application.
- **Joining an existing workspace.** `stobie new --member` writes a member of
  the Cargo workspace above the directory: no `[workspace]`, profile or
  `rust-toolchain.toml`, a README section on profiles and `.cargo/config.toml`
  discovery, and a next step naming the `members` line to add. It refuses a
  directory without a workspace above it. Editing the workspace's manifest is
  left to its owner.
- **Size as a report.** `stobie_build::size` reads a package's
  `manifest.json` and sums, per application, its `.wasm` and everything it
  serves, raw, Brotli and gzip. `stobie build-web --production` and
  `stobie package-web` print and return the sizes. `cargo xtask web-size`
  builds the repository's production package (the Lab and the Gallery) and a
  template application from `stobie new`, writes a JSON report and a Markdown
  table, and compares with `--baseline`. CI's `production` and `outside-app`
  stages write the same report to their evidence and step summary. No
  threshold is stated, so it is not a gate.

## Delivery

| Increment | Status |
| --- | --- |
| Named profile in `build-web`, recorded in the source identity | Complete |
| `stobie new --browser-only` and its documentation | Complete |
| An action enum with no variants | Complete |
| `stobie new --member` and its documentation | Complete |
| Size report in `stobie-build`, the CLI, `cargo xtask web-size` and CI | Complete |

## Evidence

- Host tests: profile selection and its conflict with `--debug`; the profile
  identity, including an unchanged identity for a plain release build; a
  member's workspace manifest found through `cargo metadata`; every
  combination of `--imaging`, `--browser-only` and `--member` fills every
  placeholder and is `rustfmt`-clean; Cargo's own reading of each generated
  manifest (eframe only for `wasm32` when browser-only; a member joins its
  workspace); the enclosing workspace search; the size report from a written
  package and its baseline comparison; an empty action enum hosted with
  inspection.
- `tools/outside-app-smoke.sh` now also creates a browser-only application
  (its native `cargo tree` has no eframe, winit or wgpu; `cargo test`,
  clippy and `stobie build-web` pass) and a member of a generated workspace
  built with `stobie build-web --production --profile web`, whose package
  records the profile and the workspace manifest.
- `cargo xtask web-size` on this branch, variant `none`, in bytes (no runtime
  code changed, so these are the baseline for later changes):

  | Package | App | WASM raw | WASM Brotli | Total raw | Total Brotli |
  | --- | --- | ---: | ---: | ---: | ---: |
  | browser-production | gallery | 12155821 | 3046146 | 12348740 | 3073237 |
  | browser-production | lab | 12460029 | 3069749 | 12675902 | 3101686 |
  | template | size-template | 10476122 | 2688458 | 10672957 | 2720371 |

- The profile's effect, for a template application as a workspace member:
  the workspace's default `release` profile gave 12257801 bytes of WASM
  (2761776 Brotli); `--profile web` with thin LTO and one codegen unit gave
  10468160 (2688446 Brotli), 14.6% smaller raw and 2.7% smaller with Brotli,
  with different content identities.

## Limits

- Profile settings given in `.cargo/config.toml` are not recorded in the
  source identity; the guide says to set them in `Cargo.toml`.
- A browser-only application with `--imaging` still builds wgpu natively
  through `stobie-render-wgpu`; it has no eframe or winit.
