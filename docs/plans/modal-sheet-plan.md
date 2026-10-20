# Modal dialog, sheet and confirm

Status: complete
Delivery: [PR #117](https://github.com/robchristie/stobie/pull/117)

## Outcome and boundaries

An application asks for a task before anything else: a form, such as a
routine's editor, or a question, such as whether to delete it or sign out.
Stobie had egui's `Modal`, but no recipe with a title and close control, no
layout that becomes a bottom sheet on a phone, no confirm, and no host rules:
egui's modal layer blocks the pointer and Tab, but leaves arrow-key focus,
shortcuts, agents and assistive technology able to reach what is behind it,
and returns focus nowhere when it closes. Applications built confirmations
inline, as a two-step button, and forms inline in the page. Stobie now owns
a dialog and sheet recipe, a confirm recipe, and the host's modal rules for
focus, agents, shortcuts, observation and the AccessKit tree. Applications
own whether a dialog is open, its content and the actions that open and
close it.

Destructive emphasis, a scrim token, a fixed footer for a form's actions, a
labelled text-field recipe, stacked dialogs (one over another), non-modal
popovers, transitions and a device run on iOS are later work.

## Implementation and UI acceptance contract

Representative task: on a phone, open a routine's editor as a bottom sheet,
change its name and prompt, turn its notification over and save; on a
desktop, do the same in a centred dialog; delete the routine after a
confirm, or keep it. The title leads, the close control is a quiet icon at
its end and the work's action is the one accent; the confirm's question
leads, its message explains, and cancel comes first. Reference: the
platform sheets and alerts of iOS and Android, the WAI-ARIA Authoring
Practices' modal dialog and alert dialog patterns, and the other recipes'
planar surfaces and boundaries.

- Recipes: `PresentationContext::dialog(ui, key, &DialogSpec, availability,
  content)` and `PresentationContext::confirm(ui, key, &ConfirmSpec,
  availability)`, with the hosted `Frame::dialog(ui, p, key, &DialogSpec,
  content)`, which triggers dismiss, and `Frame::confirm(ui, p, key,
  &ConfirmSpec)`, which triggers confirm or cancel. `DialogSpec::new(title,
  dismiss)` is centred; `.sheet()`, `.full_height()`, `.backdrop_dismisses()`
  and `.width()` change it. `ConfirmSpec::new(title, message, confirm,
  cancel)` and `.confirm_emphasis()`.
- Layout: `DialogPlacement::for_layout`, typed policy in layout points. A
  centred dialog is `DIALOG_WIDTH` (560) wide at the reference font scale,
  a confirm `CONFIRM_WIDTH` (400), never wider than the screen less its
  margin. A sheet is a bottom sheet across the screen below
  `PaneWidthClass::Wide` (720) and centred from it, as tall as its content
  up to the screen's height less the application bar's, so the top of the
  page stays in view; full height, it covers the screen. The title is a
  fixed header, at least the close control's hit target tall, with the close
  control at its end, and taller content scrolls beneath it.
- Surface: the panel surface with a subtle border over a scrim of the canvas
  colour at partial opacity. The scrim takes the pointer and dismisses only
  when the spec asks; a confirm's never does.
- Safe areas and the keyboard: a dialog sits in egui's content rectangle,
  which the web shell keeps clear of the safe areas (`viewport-fit=cover`
  with the PWA template's padding) and, with `fitViewport`, of the
  on-screen keyboard through the visual viewport (`--stobie-bottom-inset`
  drops to 0 while it shows), so a bottom sheet rests above the home
  indicator and above the keyboard.
- Fields: a form's fields are `NativeTextEdit`s through
  `PresentationContext::native`, which on a touch screen become the native
  field over the sheet, as on iOS.
- Host rules (`host/modal.rs`): focus moves to the dialog's first control
  when it opens, a confirm's cancel; focus that lands behind it returns to
  where it was in it; Tab wraps inside it; when it closes focus returns to
  its opener, the control clicked or focused as it opened, or to the first
  dialog's opener when a dialog takes another's place. The host observes
  and binds only the dialog and what it presents; every other action is
  disabled with the reason `The dialog "<title>" is open`, so agents' and
  shortcuts' invocations are refused. A plugin hides every other top-level
  layer in the AccessKit tree, and AccessKit requests for hidden nodes are
  dropped.
- Semantics: `UiRole::Dialog`, named by its title, and
  `UiRole::AlertDialog`, named by its title and described by its message,
  each modal in AccessKit and `dialog` or `alertdialog` with `aria-modal` in
  the ARIA layer. The parity audit requires a dialog to be modal.
- States: open and closed; focus in a field, on Save, on Close, on cancel
  and on confirm; a dismiss that is disabled or hidden; a phone, a tablet
  and a desktop; a sheet opened from a pane of the navigation recipe in
  each of its layouts; 150% text; light, dark and high contrast. Excluded:
  loading and error states inside the form (application-owned) and a
  dialog over a dialog.

Calibration question: does each dialog read as a task over the page, with
its title and the next action obvious and the way out plain, and is nothing
behind it reachable? Smallest probe: Gallery `dialog/sheet` in dark
comfortable at 1440 by 900 and in light compact on a 390 by 844 phone, and
`dialog/confirm` in light compact at 1440 by 900 and in light high contrast
at 150% on the phone. Evidence owner:
[Dialog, sheet and confirm evidence](../modal-sheet-evidence.md). Exit:
focused host regressions at phone and desktop widths and over the
navigation recipe, a browser journey with keyboard, pointer and touch,
inspected baselines and an accepted two-pass critique, with at most one
refinement.

## UI review record

```text
Task and reference: edit a routine in a sheet on a phone and a centred dialog
  on a desktop, and confirm its deletion; platform sheets and alerts, the
  WAI-ARIA modal and alert dialog patterns, the other recipes' surfaces.
Behavioural checks: passed (22 hosted and 3 Gallery regressions; the browser
  journey on WebGPU and WebGL2).
Presentation checks: passed (four fixtures, two-pass critique).
Design decision: accepted without refinement.
Main reason and remaining trade-off: the title and the work's action lead and
  the page behind recedes at every width; there is no destructive tone or
  scrim token yet, and a long form scrolls its Save away.
Exact candidate evidence links: the evidence owner's rendered evidence.
```

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Roles, recipes, host modal rules and ARIA | Complete | 21 hosted regressions, a navigation regression at three widths, and the parity audit |
| Gallery, browser journey, documentation and baselines | Complete | [Evidence](../modal-sheet-evidence.md), four inspected baselines, `tools/dialog-browser-smoke.sh` |
| Candidate qualification | Complete | `cargo xtask verify` on the candidate; review, CI and merge retained in the PR |

## Closeout

The dialog and sheet recipe, the confirm recipe, the host's modal rules, the
two roles in semantics, AccessKit and the ARIA layer, the Gallery stories
with four fixtures, the browser journey, the compilable API example and the
documentation are complete. Final review, CI and merge qualification remain
with the owning pull request; this completed product state does not assert
that it has merged.

## Later work

- A destructive emphasis for confirm, and a scrim token.
- A fixed footer for a form's actions, so a long form keeps Save in view.
- A labelled text-field recipe: a form in a sheet presents each field
  through `PresentationContext::native` with a `NativeTextEdit`, an AccessKit
  label and an observed node by hand.
- A dialog opened with no opener, as an agent or the application opens one,
  returns focus nowhere; a named return target would let the application
  choose.
- Facts an application observes itself (`Application::observe`) are not yet
  filtered while a dialog is open, and text coverage counts native controls
  behind it.
- A device run on iOS: a sheet against the home indicator and the keyboard.
- Stacked dialogs, non-modal popovers and transitions.
