# Typed icon qualification

The [implementation/UI contract](../typed-icons-plan.md) owns the bounded scope.
Artwork and licensing live with the [SVG source](../../crates/polyorama-ui-egui/assets/icons/README.md);
[component documentation](../ui-guides/components.md#typed-icons-and-action-presentations)
owns the public API and compiled consumer example.

## Design decision

Task and reference: compose an ordinary desktop action group using Polyorama's
analytical reference tokens, with quiet pane tools and a named primary action.
The vocabulary uses original monochrome 24-unit geometry, two-unit round strokes,
16-point token artwork and consistent square alignment.

First visual pass: native action presentations clearly distinguished text,
icon-only and labelled actions, selected tools, disabled Undo and keyboard focus.
The ordinary toolbar unnecessarily placed its primary Save below a short row of
tools despite available width. The single bounded revision puts Save in the same
wrapping row with token spacing. The adversarial long-label fixture retains both
a useful regular slot and a smaller measured/eliding slot.

The inspected vocabulary and high-contrast 150% narrow fixture show coherent
stroke weights, recognisable concepts, aligned centres, legible labels and an
independent primary-action hierarchy. The primary foreground changes with the
owning theme; decorative artwork never supplies an action name. Label minimum
width deliberately reserves measured ellipsis; a caller must provide it, wrap or
choose explicit icon-only/overflow. No further visual variation was selected.

State inspection also found that the analytical theme aliases hover and selection
fills, producing identical hover/press pixels. A shared action-state correction
adds a token-coloured inset active outline, distinct from outer keyboard focus,
and Primary hover changes its outer border. Foregrounds, allocations and resting
text-only appearance remain unchanged. This is a contract repair rather than a
new icon-specific interaction model.

Behavioural checks: focused regressions establish pointer/keyboard/AccessKit
activation, disabled and toggle semantics, unique logical instances, clipped
bounds, omitted fully hidden observations and pane/domain metadata. Icon-only
creates no text attempts; labelled actions measure their actual reserved region.
Physical native/browser reports and final canonical verification are retained
with the owning pull request and the evidence manifest below.

## Baseline decision

All five original fixtures retain **zero changed pixels**, equal metadata and
semantics, and identical measured text observations. Their only structural
change is `coverage.native_text_controls` increasing by four, for the additional
Gallery catalogue entries. Only those five text JSON files were updated. The
three new fixtures retain the inspected action, vocabulary and narrow/enlarged
captures. Frozen evaluation seed fixtures, assertions and scoring are unchanged.
The verifier remains read-only and never approves expected output automatically.

## Platform and preview limits

Native proof uses Linux Xvfb with GL/llvmpipe. Browser proof uses pinned Chromium,
WebGPU/SwiftShader and physical mouse/keyboard input on the production WASM
Gallery, including display scales 1, 1.25 and 2. Story/configuration hooks arrange
fixed fixtures and read current targets; they do not inject action completion.

AccessKit parity and keyboard evidence extend component coverage. This package
makes no new screen-reader/platform qualification claim. Eframe 0.36.1 still has
no supported browser AccessKit adapter; the existing
[platform matrix](../accessibility-integration-report.md) remains authoritative.

The configured `publish-dev-preview` run reached launcher readiness, with the
new allowlisted `/gallery/` route. A fresh browser profile received HTTP 403 from
the private gateway before loading application assets. No applicable authenticated
session was available; no credential copying or access-policy change occurred.
That HTTPS observation remains failed/unverified. The maintained local browser
harness supplies application acceptance evidence separately.

An initial headless browser probe delivered working input/semantics but black
capture pixels. Those images were rejected, and the maintained smoke now fails
on blank/flat output and owns the declared Linux headful Xvfb route. Failed probe
records remain in ignored `.tools/runtime/`; they are not accepted evidence.

## Inspected artefacts

- [Dark toolbar](browser-toolbar-dark.png) and [light compact toolbar](browser-toolbar-light-compact.png).
- [Hover](browser-hover.png), [held press](browser-pressed.png),
  [keyboard focus](browser-keyboard-focus.png), [focused press](browser-focused-pressed.png)
  and [disabled explanation](browser-disabled-tooltip.png).
- [Dark high-contrast narrow/150%](browser-long-narrow-high-contrast.png), plus the
  [light high-contrast baseline](../ui-snapshots/expected/icon-long-high-contrast/visual.png).
- [Vocabulary](browser-vocabulary.png), [1.25 display scale](browser-vocabulary-display-1.25.png)
  and [2 display scale](browser-vocabulary-display-2.png), plus the
  [light compact baseline](../ui-snapshots/expected/icon-vocabulary-light/visual.png).

The [browser observations](browser-observations.json) retain fixture/configuration,
input feedback, audits, coverage, actual display scale, rendered pixel statistics,
image hashes and warmed idle evidence. PNGs were opened and inspected, including
simultaneous focus/press and all display-scale captures. Hover/press differ within
the same 32×32 semantic target (96 of 1024 pixels; maximum channel difference 99),
not merely in unrelated page pixels. Native observations and captures accompany
this record. These visual probes identify their provisional source/build explicitly;
the owning PR retains exact committed-candidate canonical/CI and landing evidence.

Native [presentations](native-presentations.png), [toolbar](native-toolbar.png)
and [narrow high-contrast](native-narrow-high-contrast.png) captures have their
[observations](native-observations.json) retained separately. The narrow probe
predates the shared state-outline repair; its label/artwork layout and input path
are unchanged. Presentations/toolbar were recaptured after that repair. The
common icon-size token is generated behind `DesignTokens::icon_size()` so existing
public token struct literals remain compatible as well as `ActionButtonSpec`.

Design decision: accepted after the toolbar re-critique and state-contract repair.
The foreground, proportion, baseline alignment, spacing and state cues support
the declared action hierarchy in ordinary and adversarial fixtures. Remaining
limits are the explicitly unverified HTTPS gateway session and unchanged platform
accessibility support described above.
