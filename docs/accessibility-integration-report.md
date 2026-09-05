# End-user accessibility integration evidence

Status: platform-independent candidate verified; one exact Linux GNOME/Orca
environment directly qualified

## Evidence identity

- baseline source:
  `0e725f5a97a6d99a6bc4c961dfc05b4e9252ba1d`;
- native actual-AT evidence source:
  `17618d5553e845a97dbba38ad44ca994fc117928`;
- human-operated Linux qualification executable source:
  `fb6c9f3773a88a87a5bf5be7da8453c8a89c6c24`;
- exploration date: 4 September 2026, Australia/Adelaide;
- host: Arch Linux rolling, kernel `7.1.3-arch1-1`, x86-64;
- session: SSH TTY, `XDG_SESSION_TYPE=tty`, no X11 or Wayland display;
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)`, Cargo 1.97.1;
- Node/npm: 25.8.2 / 11.11.1; `wasm-bindgen-cli 0.2.127`;
- framework: crates.io eframe, egui and egui-wgpu 0.36.1, winit
  0.30.13, and AccessKit 0.24.1; and
- baseline graphics evidence: the earlier design-system qualification used
  native GL/Mesa llvmpipe and browser WebGPU. This TTY has no current renderer
  identity and does not inherit that earlier qualification.

The locked crate checksums are
`eframe 2afc0cbcdb6896b7bfb1dbbebaf7b9af9635ff38fd01e89bdd0174c1717b1857`,
`egui c977ac91dfaa651633fd9722e4ce9ccb32cda4c748b89a5cb57e504036e37c13`,
`egui-winit 9327fc8edef2c57db9bcbcacc82c8a4b1e8cc64cd41a67db4419dcf643d88c83`,
and
`accesskit d3b7f7f85a7e5f68090000ed7622545829afd484d210358702ae4cb97dd0c320`.

## Exploration question and representative probe

Question:

> Can egui/eframe 0.36.1 expose Polyorama's existing AccessKit tree to native
> and browser assistive technologies without a second authoritative UI or
> application-state model?

The selected probe is one current application frame containing an enabled
application action, a disabled action and reason, a dock tab, an adjustable
splitter, a selected materialised result, and an image viewport whose semantic
context names its pane, active tool, selection and available actions. The
framework tree is compared with the bounded observational `UiSnapshot`; the
snapshot remains neither a retained widget tree nor an action/state authority.

## Route observations and decisions

| Route | Exact configuration or observation | Result | Decision |
| --- | --- | --- | --- |
| Native eframe/winit AccessKit | Workspace eframe 0.36.1 with feature `accesskit`, which enables `egui-winit/accesskit` and `accesskit_winit 0.32.x` | Eframe initialises the platform adapter, enables egui tree generation on `InitialTreeRequested`, forwards AT actions as egui input, repaints for activation/action only, and submits each generated tree update through `update_if_active` | Retain and implement; it consumes the existing immediate-mode frame semantics without another Polyorama model |
| Browser AccessKit through stock `WebRunner` | Eframe 0.36.1 `web/app_runner.rs:394` destructures `accesskit_update: _` with `not currently implemented`; no web AccessKit adapter is present in the locked graph | The application cannot configure, intercept or forward the generated update through this private runner path | Reject for this stack; retained upstream blocker |
| Eframe `web_screen_reader` | Optional Web Speech synthesis speaks `PlatformOutput::events_description()` only when egui's `screen_reader` option is true | No browsable roles/tree, virtual cursor, platform-AT focus, or AT-originated action route; it is not an AccessKit adapter | Reject as proof of browser accessibility; do not mislabel synthesised event speech as browser AT integration |
| Custom DOM/ARIA mirror | Would require application-owned DOM projection and browser event plumbing outside stock eframe | Could create duplicate semantic ownership and exceeds the bounded upstream probe | Out of scope: the objective explicitly forbids expanding into a custom DOM or accessibility framework merely to avoid recording this limitation |

The native path answers **yes**. The browser path answers **no for the stock
0.36.1 runner**. The coherent browser outcome is therefore the retained blocker,
not a replacement accessibility tree.

## Retained browser blocker reproduction

Run from the exact source revision:

```sh
cargo tree -e features -i eframe
cargo tree -e features -i egui-winit
rg -n 'accesskit_update: _.*not currently implemented' \
  /home/rob/.cargo/registry/src/index.crates.io-*/eframe-0.36.1/src/web/app_runner.rs
rg -n 'accesskit_winit' Cargo.lock
```

At the baseline, neither eframe nor egui-winit enables its native `accesskit`
feature, `Cargo.lock` has no `accesskit_winit`, and the source probe returns:

```text
394:            accesskit_update: _,        // not currently implemented
```

Enabling the native feature is deliberately not presented as changing this Wasm
path. The implementation candidate must retain this probe result and must not
claim browser screen-reader support.

## Current environment and actual assistive technology

The host still has no interactive desktop accessibility environment:

- `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_CURRENT_DESKTOP` and
  `DESKTOP_SESSION` are absent;
- Orca, Accerciser, AT-SPI registry/launcher, Speech Dispatcher, espeak-ng and
  their user services are absent;
- no system Firefox, Chromium/Chrome or browser driver is installed; and
- the repository's isolated Ubuntu compatibility environment can run
  Playwright Chromium and Xvfb/llvmpipe, but contains no desktop accessibility
  bus, screen reader or speech stack.

The exact release binary was therefore exercised with actual Orca 50.2,
AT-SPI2 2.60.6 and Speech Dispatcher 0.12.1 in a disposable rootless Arch Linux
container using Xvfb and Mesa llvmpipe. Orca discovered the application through
AT-SPI, generated speech for the complete representative workflow, and observed
dynamic state. The exact configuration, transcript, state checkpoints,
artefact hashes and limitations are retained in the
[Linux Orca/Xvfb evidence](accessibility-integration-evidence/linux-orca-xvfb.md).

That virtual display, synthetic input and recorded speech pipeline remain a
partial result on their own. A subsequent persistent Debian 13 VM combined a
repeatable guest-native probe with a human-operated GNOME 48 remote-login
desktop. The exact release binary completed the automated state/input journey,
and the human operator confirmed that Orca 48.1 audibly read the workflow over
RDP. The environment, attestation and artefact identities are retained in the
[Debian GNOME/Orca RDP evidence](accessibility-integration-evidence/linux-gnome-rdp.md).
This directly qualifies that exact combination, not Linux generally.

## Platform qualification matrix

| Environment | Status | Actual AT evidence | Limitation and smallest next action |
| --- | --- | --- | --- |
| Linux native, Arch/Xvfb/llvmpipe | Partially qualified | Actual Orca 50.2 and Speech Dispatcher completed the representative workflow against the exact release binary; [retained evidence](accessibility-integration-evidence/linux-orca-xvfb.md) | Repeat on a human-operated X11 or Wayland desktop, retain heard speech or braille and exact desktop/backend identity, then decide the direct support claim |
| Linux native, Debian 13/GNOME 48/RDP/Orca 48.1 | Qualified for this exact environment | The repeatable VM probe completed the workflow and a human operator confirmed audible Orca output in the GNOME remote-login desktop; [retained evidence](accessibility-integration-evidence/linux-gnome-rdp.md) | Keep the claim limited to this versioned remote-desktop configuration; qualify other distributions, desktops, local-seat sessions and Orca versions independently |
| Windows native | Unavailable in the current environment | None | Run the exact candidate on versioned Windows with NVDA or Narrator and retain the workflow transcript/tree evidence |
| macOS native | Unavailable in the current environment | None | Run the exact candidate on versioned macOS with VoiceOver and retain the workflow observations |
| Browser, any OS/AT pair | Blocked by a retained reproduction | None | A future supported upstream web adapter or separately authorised architecture must first deliver roles, focus and actions; then qualify each browser/OS/AT combination independently |

One exact human-operated Linux environment is directly qualified and the Arch
virtual environment remains partially qualified with actual Orca. Compilation,
`egui_kittest`, semantic snapshots, browser automation and an accessibility-
tree dump alone remain insufficient.

## Qualification workflow

Every future claimed environment must exercise and record:

1. locate the application and current status;
2. move through application bar, dock tabs and pane content;
3. activate an application action and discover a disabled action with its
   reason;
4. select and activate a pane, then adjust a splitter;
5. focus an image viewport and identify pane/activity, tool, camera link,
   selection, state and actions;
6. select a result through the bounded list and inspect it;
7. perform one annotation or viewport action without a pointer; and
8. observe a dynamic loading, availability, selection or tool change.

Evidence must name OS, AT, browser where applicable, graphics/backend, exact
source revision, steps, observations, failures and retained tree/transcript/
recording/screenshots. Non-qualified rows remain explicit until that evidence
exists.

## Implementation and verification status

The candidate enables eframe's native `accesskit` feature and locks
`accesskit_winit 0.32.2`. The architecture gate now fails if that feature is
removed. Eframe therefore owns native platform-adapter activation and update
submission; Polyorama continues to emit its existing immediate-mode AccessKit
tree and does not add another state or widget model.

The shared action component distinguishes momentary controls from pressed
tool/link modes. Generated semantics expose ordinary buttons without selected
or toggled state, mode controls with a deliberate toggled state, and
selection-only tabs, result rows, thumbnails and Canvas nodes without a false
checkable state. The AccessKit parity audit rejects any unexpected toggled
state across the complete audited control set.

Each image viewport now has a stable Canvas node. Its current description
includes active/inactive pane state, selected tool, camera link, image-space
centre and scale, selected result/annotation, relevant shared worker state, and
the actions currently available. AccessKit custom actions are translated to
the existing typed application and pane-intent paths for transports that
deliver them. The pinned Linux AT-SPI adapter does not enumerate or dispatch
that custom-action mechanism, so the actual Orca workflow uses the registered
toolbar controls as its non-pointer route and makes no native Canvas-action
claim. A focused viewport has a visible token-derived focus ring. The
observational `UiSnapshot` carries the same semantic identity and action set
without becoming authoritative.

The five evidence axes remain deliberately separate:

| Axis | Candidate result | What it proves and does not prove |
| --- | --- | --- |
| Framework semantic tests | Pass | A production-path representative frame contains the expected application action, disabled reason, dock tabs, splitters, bounded selected result and viewport; the snapshot and generated AccessKit nodes have matching stable identity, role, state, bounds and actions, with no duplicate owners. These tests exercise AccessKit's framework transport, not Linux AT-SPI custom-action delivery |
| Keyboard tests | Pass | Deterministic repeated-frame Tab/Shift+Tab traversal reaches application actions, splitters, the active tab, pane tools and viewport; tool shortcuts and result selection update viewport context through existing commands/intents |
| Platform-adapter integration | Native pass at build/architecture level; browser blocked | The native dependency route is compiled and guarded. Stock eframe 0.36.1 still discards browser AccessKit updates, so no browser adapter claim follows |
| Automated tree and physical input | Pass within the stated limits | Generated AccessKit updates, bounded virtualisation, dock move/restoration, dynamic state and action routing pass automated tests. Native Xvfb/llvmpipe and Playwright Chromium/WebGPU physically exercise keyboard and pointer workflows, and deterministic UI/text checks pass. These are not actual AT sessions |
| Actual assistive technology | One exact native environment qualified; browser blocked | The repeatable Debian VM journey plus human-confirmed audible Orca output directly qualify Debian 13/GNOME 48/RDP/Orca 48.1. Arch/Xvfb remains partial, and no claim extends to other native environments or browsers |

On 4 September 2026 the candidate passed `cargo xtask verify` with npm's
unrelated registry audit request disabled after the registry request stalled.
The canonical gate itself completed format checks, strict native and Wasm
clippy, all 177 tests, architecture checks, native and Wasm release builds,
five deterministic UI fixtures, Chromium browser smokes for both applications,
and Xvfb/llvmpipe native smokes for both applications. The runtime evidence is
retained under the ignored local directory
`.tools/runtime/verification-evidence`; pull-request evidence must record the
exact candidate commit because a commit cannot contain its own identifier.

This result completes the platform-independent implementation and directly
qualifies the exact Debian 13/GNOME 48/RDP/Orca 48.1 environment. The former
human-operated Linux acceptance blocker is closed for that combination.
Windows/NVDA or Narrator, macOS/VoiceOver, other Linux configurations and each
future browser adapter combination remain independently unqualified.
