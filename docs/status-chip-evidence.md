# Compact status-chip evidence

## Experience and design decision

Task and reference: scan task names with concise application-owned status, then
read the selected task detail heading using the same production chip. Compare
with the existing analytical wrapping status badge and
[navigation-row badges](ui-snapshots/expected/navigation-states-light-compact/visual.png).
Names/headings lead; status is secondary information, distinct from actions.

Behavioural checks: passed. Nine focused production-API regressions cover measured
allocation and overflow, icon reservation at 150%, actual opaque paint/contrast,
stable current values/metadata, AccessKit parity (including adverse mutations),
full selection/copy of genuinely elided ASCII/non-ASCII text, inert parent
click/drag, current clipped owners/coverage and repeated-pass replacement.
Gallery also checks narrow/enlarged task-row clipping and parent AccessKit parity,
including omission of hidden rows and their informational children. It checks all
three authored application palettes in all four
appearance/contrast modes and both densities. The executable consumer rustdoc
maps application-owned states to explicit tone and optional artwork.

Presentation checks: passed. The ordinary task/detail, compact parent treatments,
narrow/localised ellipsis and high-contrast enlarged captures were opened and
inspected for hierarchy, alignment, legibility and visual balance. The first
critique found consistent text/icon starts and useful adjacent status, but the
rectangular outline too closely resembled an action button. One refinement uses
a generated pill radius, bounded by half the visual height, retaining the opaque
panel/primary-text pair and measured geometry. Re-critique accepted the pill:
regular status text and compact informational chrome remain secondary, with
balanced icon/label spacing and recognisable complete status meaning.
No coloured text or tinted compositing is needed. The boundary supplements text.
The enlarged parent fixture uses a wide surface to inspect all 18 chips; the
separate narrow fixture deliberately probes clipping and truncation limits.

Design decision: accepted after that one refinement. A parent narrower than the
measured useful minimum must wrap, scroll or relocate the chip. Ellipsis/tooltips
retain the full value; this is a compact label recipe, not a message reader.
There is no new interaction, workflow enum or announcement policy.

## Rendered and physical evidence

- [Task rows and detail heading](ui-snapshots/expected/status-chip-tasks-dark/visual.png).
- [Light compact tones and parent surfaces](ui-snapshots/expected/status-chip-treatments-light/visual.png).
- [Narrow high-contrast localised labels at 150%](ui-snapshots/expected/status-chip-long-high-contrast/visual.png).
- [Actual hovered task row](status-chip-evidence/browser-row-hover.png) and
  [selected task/detail](status-chip-evidence/browser-row-selected.png).
- [Current status update](status-chip-evidence/browser-updated.png).
- [Dark high contrast](status-chip-evidence/browser-dark-high-contrast.png) and
  [light high contrast at 150% on all three parents](status-chip-evidence/browser-light-high-contrast-150.png).
- [Selected elided localised text](status-chip-evidence/browser-long-copy.png) and
  [scrolled current observations](status-chip-evidence/browser-scrolled.png).
- [Native task/status update](status-chip-evidence/native-status-chips.png).

[Browser observations](status-chip-evidence/browser-workflow.json) retain physical
mouse/keyboard targets, full clipboard assertions, state updates, configurations,
WASM/image hashes, browser version and settled idle proof. Hooks only select
fixed stories/appearance and read current observations; they do not activate
rows or change status. The actual browser clipboard contains the complete
elided German label. The inert chip click activates its parent once; Enter then activates that focused
parent through the same path, with a visible token focus ring. Returning
to the first task and applying the separate consumer action changes Active to
Completed at its unchanged logical identity, without stale Active observations.

[Native observations](status-chip-evidence/native-workflow.json) retain actual
xdotool selection/Copy input, inert parent activation, selected detail, consumer
status update, binary/image hashes and settled idle proof. The native journey
exercises clipboard input; full native label-copy output is asserted directly
by egui regression tests rather than by an external X11 clipboard reader.

These exploratory captures identify their pre-commit source context and dirty
state honestly. Final committed-candidate journeys and canonical verification
retain exact revision/input identities in ignored runtime evidence and the
owning PR, avoiding self-referential source revisions in checked-in evidence.

## Baseline decision and qualification

All 14 fixture text audits passed. The 11 existing fixtures retain byte-identical
pixels, metadata and semantics, and identical measured observations. Only the
native-control denominator increases from 33 to 36 for the three catalogue
entries. All three new fixture visuals and semantic/text artefacts were inspected
outside expected/ and accepted explicitly. Frozen evaluation tasks, fixture
definitions, assertions and scoring remain unchanged. Verification never updates
baselines.

Canonical qualification passed with `cargo xtask verify` at
`dd10a1253f86778dd0c3b2f3adedb0a3f914be96`, including generated tokens,
format/lint/tests, native/WASM release builds, architecture and documentation,
14 zero-tolerance fixtures, existing application paths and both new physical
status-chip journeys. Its result and exact independent review/CI/merge identities
are retained in [PR #53](https://github.com/robchristie/polyorama/pull/53).

## Platform and preview limits

Local canonical qualification uses the configured bundled Linux libraries and a
headful Xvfb display, matching the CI browser mode. Direct headless invocation
first lacked libatk; supplying the library path started it but timed out waiting
for initial useful content. The focused packaged-startup probe passed in the
maintained headful environment, then full qualification used that environment.
These failed host observations do not establish a product headless-support claim.

Native proof uses Linux Xvfb with GL/llvmpipe. Browser proof uses Chromium with
WebGPU/SwiftShader. This package establishes semantic, geometry, text and physical
input behaviour in those environments; it adds no actual screen-reader or
physical-GPU performance qualification. Stock eframe's missing browser AccessKit
adapter remains documented in the maintained accessibility platform matrix.

The configured private HTTPS preview reached readiness and exposed the allowlisted
Gallery route, but its fresh test-browser session received gateway HTTP 403. That
browser observation remains failed/unverified. The existing local browser harness
establishes the application acceptance independently; gateway access policy was
not changed. Cleanup stops only the task-owned preview generation.
