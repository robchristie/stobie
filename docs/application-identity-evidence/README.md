# Application identity calibration evidence

The retained candidate source is `a3eb8463eb9459be3376595d6d071f41c3eb4726`.
It repairs the enlarged-text gallery entry from
`ab86d64f7805b01d43b70154aa73318a6365d9ca`, which extended checkpoint
`8f0d92292df67ebbb4e639754a82f7fffb9b24c7`;
the original comparison base is `d469d74a14f0bc4494fdfc44504f12462ee50841`.
The user explicitly approved the five candidate baseline images and associated
native-control coverage updates on 2026-09-07. This records source qualification,
not a release or deployment. Bokkie's application-owned values and comparison evidence remain in
Bokkie's repository.

The calibration question was whether the intended single-colour foreground
reaches emitted glyph paint, and whether one bounded application identity can
resolve the same values for native widgets and custom components. The smallest
probe was a real action galley plus native button rectangle across authored
modes; the exit condition was executable foreground/resolver evidence before
broader styling. That probe passed, including idle primary, pointer-pressed
primary, pointer-pressed ordinary action and disabled primary labels. The
retained implementation resolves foreground before text layout and preserves
rich-text painting behaviour.

## Verification result

[Canonical verification](canonical-verification.log) passed governance,
formatting, generated-token drift, native and Wasm Clippy, workspace tests,
architecture boundaries, release native/Wasm builds and both browser smokes.
The gallery has 9 passing tests and the UI crate has 61. The new gallery test
renders the real reference shell and open workbench for three authored themes
in all four mode/contrast combinations with no text audit findings.

**Historical pre-approval verification failed:** all five previous
visual baselines differed. All five retain identical semantic and metadata snapshots and identical
measured-text observations, with empty audits. Text coverage now honestly counts
the native workbench entry: 26 native controls instead of 25, and the explicit
`native_button_text` exclusion. This deliberate coverage delta also requires
baseline acceptance, supplied explicitly by the user on 2026-09-07. The accepted
images and text coverage records now replace those five files pairs under
`docs/ui-snapshots/expected/`. The [summary](summary.json)
records pixel counts and the exact Wasm identity; [artefact hashes](artefact-hashes.json)
bind the retained evidence files.

The canonical command stops before native smokes when visual baselines fail.
Both native smokes ran separately and passed at the preceding `ab86d64`
checkpoint: [native verification](native-verification.log). The affected gallery
native smoke ran again at `a3eb846` and passed:
[entry repair native verification](entry-native-verification.log),
[current native gallery capture](gallery-native.png). These establish Linux GL/llvmpipe
functional behaviour, not physical-GPU performance or additional assistive
technology qualification.

| Fixture | Candidate | Difference | Expected |
| --- | --- | --- | --- |
| Application shell | [Actual](snapshots/application-shell-dark/actual.png) | [Diff](snapshots/application-shell-dark/diff.png) | [Approved](../ui-snapshots/expected/application-shell-dark/visual.png) |
| Diagnostics high contrast | [Actual](snapshots/diagnostics-high-contrast/actual.png) | [Diff](snapshots/diagnostics-high-contrast/diff.png) | [Approved](../ui-snapshots/expected/diagnostics-high-contrast/visual.png) |
| Narrow tabs | [Actual](snapshots/tabs-narrow-wide/actual.png) | [Diff](snapshots/tabs-narrow-wide/diff.png) | [Approved](../ui-snapshots/expected/tabs-narrow-wide/visual.png) |
| Splitter states | [Actual](snapshots/splitter-states/actual.png) | [Diff](snapshots/splitter-states/diff.png) | [Approved](../ui-snapshots/expected/splitter-states/visual.png) |
| Keyboard focus | [Actual](snapshots/button-keyboard-focus/actual.png) | [Diff](snapshots/button-keyboard-focus/diff.png) | [Approved](../ui-snapshots/expected/button-keyboard-focus/visual.png) |

The differences include the workbench entry control, token-derived native
foregrounds and fills, and corrected primary/pressed treatment. The user accepted these
exact candidates and their coverage updates before they were copied into the
baseline tree. Comparison thresholds remain unchanged.

## Live workbench journey

The existing release browser gallery ran at 1440×900 with WebGPU. Lantern
inspected its live page and retained captures; a small temporary Playwright
driver supplied canvas pointer coordinates, since Lantern's selector clicks
do not address individual egui controls. No product automation API was added.
This complete journey ran at `ab86d64`; the page reloaded that checkpoint's
canonical Wasm after its build, and no source edits occurred during the journey.
The subsequent `a3eb846` change is confined to the bounded entry layout and
honest native-button inventory; it does not change authoring or export behaviour. Browser console and page errors were empty.

The journey opened the workbench, selected Neutral graphite, edited its dark
decorative border from `#333338` to `#2B2B2E`, and copied the complete four-mode
[ThemeColours JSON](workbench/exported-theme.json) from the actual browser
clipboard. An invalid near-white primary foreground on its near-white primary
background then failed at 1.09:1: the preview retained the last valid identity
and the export button became disabled. The journey also toggled the analytical
reference comparison, selected Warm paper in light mode, and returned to
Neutral graphite on the production button story.

- [Graphite on production buttons](workbench/graphite-components.png)
- [Rejected edit and disabled export](workbench/invalid-edit.png)
- [Analytical reference comparison](workbench/reference-comparison.png)
- [Warm paper on the same reference shell](workbench/warm-light.png)

## Enlarged-text entry repair

Review of the first candidate caught clipping in the new sidebar entry at 150%
text: its horizontal row had to accommodate both “Stories” and the full button
name. The current source puts the button on its own wrapping row. A regression
probe at 100%, 125% and 150% verifies that both the response and emitted text
remain within the 228-point sidebar content area, the label is not elided, and
AccessKit retains the complete button name. The native-button inventory remains
explicitly outside structural text measurement.

The regenerated [150% high-contrast candidate](snapshots/diagnostics-high-contrast/actual.png)
shows the complete entry within the sidebar. All five candidate bundles were
regenerated from the `a3eb846` release Wasm, rather than reusing the earlier
images. Their unchanged text observations and intentional inventory delta are
recorded in the historical machine-readable summary. These five replacements
were subsequently accepted explicitly by the user.

The retained export is a development example, not Bokkie's theme. It is not an
approved visual baseline. Runtime theme validation proves bounded text,
focus/selection and opacity contracts; meaningful control-boundary contrast
still requires application checks because the analytical reference preserves
its historical border values. Bokkie owns the corresponding graphite checks.

The retained logs and summary describe the pre-approval checkpoint. Final
canonical verification, exact-revision review and landing evidence are recorded
in [Polyorama #33](https://github.com/robchristie/polyorama/pull/33). Bokkie owns
final consumer integration and the complete objective’s closeout.
