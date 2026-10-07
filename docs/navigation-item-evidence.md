# Navigation-item evidence

## Experience and design decision

Task and reference: navigate Home, Tasks, Needs attention, Activity and Settings
with application-owned counts, selection and destination content. Compare the
[shared analytical icon/action reference](typed-icons-evidence/browser-toolbar-dark.png).
The candidate retains planar chrome, aligned artwork and labels, small secondary
counts and one strong current-destination cue.

Behavioural checks: focused tests and actual native/browser input passed for
pointer, Tab, Enter/Space, AccessKit Click, disabled rows, focus without selection,
consumer selection/count updates, identity, current parity, clipping and idle.
Navigation changes only selection; a separate application-owned completion
action updates the outstanding task count.
The Gallery applies pending navigation intents after coherent presentation of
one pass, then repaints the changed state.

Presentation checks: ordinary and adversarial rendered images inspected; complete
semantic text and measured allocations retained. The strict Gallery text audit
covers its fully visible allocation subset. Production measurements retain
partially clipped galleys separately from their effective clips; shared inventory
counts all attempted labels/badges and retains every failed observation.

Design decision: accepted after the marker refinement and a contrast invariant
repair. Initial ordinary review found
clear hierarchy, consistent starts and balanced count badges. Held press plus
keyboard focus crowded the leading current marker. Moving the marker inward
separates it from the inset press outline while retaining the existing icon slot.
Re-critique found current, hover, press and focus distinguishable; narrow 150%
text keeps readable rows, explicit elision and bounded badges. Scrolling can put
the current destination outside the visible subset, as ordinary application
scrolling requires. No collapsed mode is supplied. A validated application theme can alias hover
fill and the selection indicator; unselected press now uses the validated
selection pair, with an inset outline and no current marker. Its focused
regression proves unchanged current semantics.

## Inspected rendered evidence

- [Native states and count update](navigation-item-evidence/native-states.png)
  with [native observations](navigation-item-evidence/native-observations.json).
- [Ordinary dark sidebar](ui-snapshots/expected/navigation-sidebar-dark/visual.png).
- [Light compact disabled/zero states](ui-snapshots/expected/navigation-states-light-compact/visual.png).
- [Light high-contrast narrow/150%](ui-snapshots/expected/navigation-long-high-contrast/visual.png).
- [Keyboard focus before activation](navigation-item-evidence/browser-focus.png),
  [current hover](navigation-item-evidence/browser-selected-hover.png) and
  [current, held press and focus](navigation-item-evidence/browser-selected-pressed.png).
- [Unselected hover](navigation-item-evidence/browser-unselected-hover.png),
  [unselected held press](navigation-item-evidence/browser-unselected-pressed.png)
  and [independent count update](navigation-item-evidence/browser-count-update.png).
- [Dark high contrast](navigation-item-evidence/browser-dark-high-contrast.png)
  and [narrow 150% scrolled badge elision](navigation-item-evidence/browser-light-high-contrast-150-scrolled.png).

The [browser observation record](navigation-item-evidence/browser-observations.json)
retains source context, WASM hash, renderer, fixed configuration, semantic targets,
counts, audit coverage, input steps and image hashes. It identifies the local
pre-commit capture context; final qualification and exact reviewed/merged revision
identities belong to the owning PR's verification and landing record.

## Baseline decision and qualification

The eight existing fixtures retain byte-identical PNGs, equal metadata and
semantics, and identical measured observations. Only
`coverage.native_text_controls` increases by three for the new catalogue entries.
Three new fixtures were captured outside `expected/`, inspected, then explicitly
accepted as reviewed source. Frozen seed stories, assertions, scoring and
existing fixture definitions are unchanged. Verification never updates baselines.

`cargo xtask verify` passed at `837306806763d839534440ae6fb7d995095f8e6d` in the configured bundled Linux
UI environment, including all 11 zero-tolerance fixtures and physical native and
browser journeys. Verification on the final PR head is retained by GitHub CI.

Focused navigation regressions, Gallery's representative appearance matrix and
the executable consumer rustdoc cover the production recipe. Canonical
`cargo xtask verify` also exercises both physical navigation scripts and all 11
zero-tolerance snapshot fixtures. Qualification results and logs are retained
with the owning PR; the scripts bind their reports to source and binary/WASM
identities. No screenshot is used as a substitute for interaction or semantics.

## Platform and preview limits

Native proof uses Linux Xvfb with GL/llvmpipe. Browser proof uses Chromium with
WebGPU/SwiftShader and physical Playwright pointer/keyboard input. Story hooks
arrange fixed inputs and read observations; they do not trigger navigation.
These are AccessKit-semantic and keyboard-tested results. No new actual
screen-reader or platform qualification is claimed; eframe's unsupported browser
AccessKit adapter remains in the maintained platform matrix.

The configured `publish-dev-preview` route reached readiness and exposed Gallery's
allowlisted path. Its fresh browser session was denied by the private gateway
(HTTP 403); that HTTPS browser observation remains failed/unverified. The local
browser harness established application acceptance independently. No gateway
access or credential policy was changed. The task-owned preview is stopped during
cleanup using its exact generation guard.
