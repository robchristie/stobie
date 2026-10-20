# Dialog, sheet and confirm evidence

## Experience and design decision

Task and reference: on a phone, open a routine's editor as a bottom sheet,
change its name and prompt, turn its notification over and save; on a
desktop, do the same in a centred dialog; delete the routine after a
confirm, or keep it. Neither people nor agents can reach what is behind
either. Compare with the platform sheets and alerts of iOS and Android (a
sheet along the bottom on a phone, a centred modal on a desktop, cancel
before the destructive choice), the WAI-ARIA Authoring Practices' modal
dialog and alert dialog patterns, and the analytical surfaces and
boundaries of the other recipes.

Behavioural checks: passed. Twenty-one hosted regressions, through `AppHost`
with kittest, cover focus moving to the first control when a dialog opens;
Tab and Shift+Tab staying inside and wrapping; an arrow key unable to move
focus behind it; Escape and the close control dismissing it with focus
returned to the opener; a press on the scrim, or on a control behind it,
doing nothing; agents reaching only what the dialog presents, every other
action disabled with the reason `The dialog "<title>" is open` and an
invocation of one rejected as `unavailable`; a Save that a long form has
scrolled out of view staying enabled, invocable and reachable by its
shortcut; a shortcut for an action behind it doing nothing; observation holding only the dialog; AccessKit hiding
every layer behind it and marking it modal; the sheet as a bottom sheet on a
phone, a full-height sheet covering it, the sheet centred on a desktop and a
centred dialog fitting a phone; two `NativeTextEdit` fields in a sheet
taking typing and `stobie.set-text`; a dialog whose dismiss is disabled
staying open; and a confirm that focuses cancel, cancels on Return and on
Escape, confirms by keyboard and pointer, offers agents its two choices and
fits a phone. A further regression opens a sheet from a pane of the
responsive navigation recipe at phone, tablet and desktop widths and finds
focus contained, Back and the list unreachable by agents and absent from
observation, and focus returned to its opener. Gallery regressions check
both stories' observations and that typing reaches the sheet's first field
once it opens. Every hosted test passes the semantic, text and AccessKit
parity audits, which now also require a dialog to be modal.

The browser journey, `tools/dialog-browser-smoke.sh`, runs the production
Gallery in headful Chromium with physical input on WebGPU and on WebGL2. At
1440 by 900 it opens `dialog/sheet` and finds a dialog centred within a
point and no wider than 560 points, focus on Name and an ARIA layer holding
only a `dialog` with `aria-modal` and its five controls; a click on the
scrim leaves it open; it types into Name and Prompt, walks Tab and
Shift+Tab six steps each and finds the same five controls in order with
wrapping; and an agent finds the Gallery's workbench action disabled with
the dialog's reason and its invocation rejected. Escape closes the dialog
the story opened; Enter on its opener, focused through the ARIA layer,
opens it again, and Escape, the pointer on Save and the pointer on Close
each close it with focus back on the opener. On `dialog/confirm` it finds
cancel focused and an `alertdialog` holding only Keep routine and Delete;
Escape and Enter on cancel keep the routine with focus back on its opener,
Tab reaches Delete and wraps, Enter on Delete deletes and a click on Delete
deletes. An idle Gallery then presents no frame. On a 390 by 844 touch
screen the sheet meets the bottom and spans the width; a tap on Prompt,
then on Name, makes each the active native field, topmost over the sheet,
and typing there reaches it; a tap on Close dismisses.

The first browser runs found two faults in the evidence, not the recipe.
Typing reached no field because the sandbox had no fonts, so eframe's text
agent never focused; the journey's wrapper now gives Chromium the
repository's fonts. A dialog that a story opens itself has no opener, so
Escape returns focus nowhere; the journey now opens each dialog again from
its opener before checking focus return, and the
[interactions guide](ui-guides/interactions.md#dialogs) states the rule.

Presentation checks: passed. First critique of `dialog-sheet-dark`,
`dialog-sheet-phone-light`, `dialog-confirm-light` and
`dialog-confirm-phone-high-contrast`, before reading the rationale: each
reads as a dialog over the page, not as more page. The scrim dims the
Gallery enough that the surface's title and fields lead, while the routine
behind stays recognisable. The editor's title heads a planar surface with a
quiet close icon at its end; captions sit above full-width fields, the
schedule reads as a value and Save is the one accent. On the phone the sheet
spans the width and rests on the bottom edge with its top border showing
where it starts; the page above remains visible. The confirm's question
leads, its message wraps beneath, and Keep routine then Delete end the row;
at 150% in high contrast the buttons still share the row with solid edges.
What competes: the focused Name field's ring is the brightest edge in the
dark dialog, which is the focus it reports; in the confirm the focus ring on
Keep routine and Delete's primary fill compete, which marks the safe choice
as focused and the asked-about action as the primary one. What became
harder: Delete has no destructive tone, so only its label says it destroys;
the switch row sits a few points in from the captions, as the switch recipe
pads its row. Reading the rationale raised nothing the pictures
contradicted. Remaining trade-offs: there is no destructive emphasis or
scrim token yet; Save is the application's content rather than a fixed
footer, so a long form scrolls its Save away; on a compact phone the
sheet's padding is the section spacing, 8 points in compact density.

Design decision: accepted without refinement.

## Rendered evidence

- [Desktop, dark comfortable: the routine editor centred](../tests/ui-snapshots/expected/dialog-sheet-dark/visual.png).
- [Phone, light compact: the editor as a bottom sheet](../tests/ui-snapshots/expected/dialog-sheet-phone-light/visual.png).
- [Desktop, light compact: the confirm](../tests/ui-snapshots/expected/dialog-confirm-light/visual.png).
- [Phone, light high contrast at 150%: the confirm](../tests/ui-snapshots/expected/dialog-confirm-phone-high-contrast/visual.png).

## Size

No dependency was added; the lockfiles are unchanged. Measured against
`45ce41b3`, the Gallery's release WASM after wasm-bindgen grew from
12,054,251 to 12,150,226 bytes (+95,975, +0.8%; 4,269,109 to 4,299,348
with `gzip -9`, +30,239; 3,014,748 to 3,034,449 with Brotli 11, +19,701).
After `wasm-opt -Oz` (Binaryen 131) it grew from 9,566,152 to 9,645,397
bytes (+79,245, +0.8%; Brotli 2,915,384 to 2,935,722, +20,338, +0.7%). Both
figures include the two recipes, the host's modal rules and the two Gallery
stories.

## Baseline decision and qualification

The four new fixtures' visual, semantic and text artefacts were inspected
outside `expected/` and accepted explicitly. The first CI run found the two
editor fixtures depending on the caret's blink: locally the capture fell in
its off phase, and on the runner in its on phase. Reduced motion, which
captures ask for, now holds the caret steady, and the Gallery follows the
browser's preference, so both fixtures were captured again with the caret
shown, inspected and accepted. The other fixtures change only
as recorded in the [snapshot guide](../tests/ui-snapshots/README.md).
Frozen evaluation tasks, fixture definitions and scoring are unchanged.
Verification never updates baselines.

## Platform limits

The browser journey proves the browser accessibility tree, physical input
and the native field's activation in Chromium only; it is not screen-reader
qualification. iOS is not proven on a device: the sheet sits in egui's
content rectangle, which the web shell keeps clear of the safe areas and,
with `fitViewport`, the on-screen keyboard, but no device run has checked a
sheet against the home indicator or the keyboard. Native AccessKit output is
covered by the hosted regressions' parity audit, not by a native journey.
