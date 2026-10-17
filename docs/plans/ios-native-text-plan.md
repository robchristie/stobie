# Native text input on iOS

Status: active
Next action: The owner confirms, on a build of the latest head, a single-tap keyboard after an app switch and after rotation, and that touches at the caret keep the iOS field; land only after they pass.

Framework work item 1 of the [iOS PWA spike report](../ios-pwa-spike-report.md#framework-work):
the composer of a chat client must raise the keyboard on the first tap and
edit like a native iOS field. This plan covers that item only. Message
selection and copy, emoji and fonts, PWA output and the async event source are
separate items in the same report.

## Problem

The spike's evidence, on an iPhone 12 Pro Max with iOS 26.6.2 and Safari 26.6.1
([typing in detail](../ios-pwa-spike-report.md#typing-in-detail)):

- **Two taps.** iOS raises the keyboard only for a `focus()` made while a user
  gesture is being dispatched. eframe 0.36.1 focuses its hidden text agent in
  `AppRunner::handle_platform_output`, in the animation frame after egui gives
  the field focus. eframe's own
  iOS workaround in its `touchend` handler blurs and refocuses the agent only
  when the agent already has focus. That needs a frame between `touchstart`
  and `touchend`. On the iPhone, the spike's device log shows the first tap
  focusing the agent once and never refocusing it: the agent was not focused
  before `touchend`. The second tap refocuses it, and the keyboard opens
  120 ms later. Chromium runs frames between Playwright's `touchstart` and
  `touchend`, which is why emulation never showed the problem.
- **Caret, loupe and predictions.** iOS caret gestures and the loupe act on the
  agent, a one-pixel input with a transparent caret, not on the text egui
  draws. eframe also clears the agent's value on blur and on every input type
  other than insertion, so iOS predicts and autocorrects without context.

## Decision

**A Stobie host workaround for the first tap, and an opt-in native field for
the composer, both in the browser ARIA layer. No eframe change now.**

Rationale:

- At `touchend` eframe cannot tell whether the touch is over a text field.
  egui decides that in the next pass. A correct eframe fix needs either a
  pass inside the touch handler or a hit test of the previous pass's
  widgets. `PlatformOutput::mutable_text_under_cursor` is the closest signal,
  and eframe ignores it (its TODO cites egui issue 4569), but it reflects the
  pointer of the last pass. A quick tap often ends before any pass sees the
  press.
- Stobie pins eframe 0.36.1 from crates.io. An upstream change helps only
  after an egui release and a version bump. Patching eframe in Stobie would
  make every consumer carry the patch.
- The ARIA layer already holds, for each completed pass, every editable text
  field's visible rectangle in egui's registration order, which is the same
  order egui uses to decide what a click hits. The native field also needs a
  hook at `touchend`, so both parts share one place.

The cost is that the workaround works only where the layer runs: `AppHost`
web applications that keep `Application::web_accessibility_layer` on. The
spike's chat client is one.

An upstream report is still worth making. eframe could focus its agent in
`touchend` when the previous pass's `mutable_text_under_cursor` was set,
which fixes taps that last longer than a frame. That is outward-facing, so it
is the owner's call (see [Later work](#later-work)).

## Design

### First-tap keyboard

The layer listens for `touchstart` and `touchend` in the capture phase on the
document, so it runs before eframe's bubbling `touchend` on the document.

- A **tap** is one trusted touch that ends within 6 points of where it began
  and lasts under 0.8 s. That is inside egui's click limits, so egui will
  treat it as a click. The listener ignores the layer's own forwarded copies,
  which are untrusted.
- At the end of a tap, the layer converts the touch to egui points and asks the
  last projected tree which interactive element is topmost there
  (`aria::text_field_at`): the last one in egui's order whose visible rectangle
  contains the point. If that element is an enabled, editable text field, the
  layer focuses eframe's text agent at once, with `preventScroll`, inside the
  gesture. iOS then raises the keyboard.
- egui focuses the field in the next pass as usual. eframe's
  `handle_platform_output` finds the agent focused already and leaves it.
  eframe's own `touchend` blur and refocus still runs after the layer's
  listener and does no harm.
- In the pass in which egui takes focus from the tap, the text edit emits no
  IME output yet, because egui assigns IME ownership at the start of a pass.
  eframe reads that as "not editing", blurs the agent and focuses the canvas
  for one frame, which would close the keyboard again. While egui's focus is
  on the tapped field, the layer's output hook supplies the field's rectangle
  as IME output, for at most the two shown passes after the tap.
- eframe's `pointerup` handler, on the document, focuses the canvas and runs
  a pass at once. Where a browser sends a touch's `pointerup` after its
  `touchend`, that would blur the field the tap has just focused, outside the
  gesture. After a `touchend` that focused a field, the layer stops the same
  tap's touch `pointerup` in the capture phase. egui still receives the
  release, because eframe's `touchend` sends it too. eframe sends it only if
  its hit test finds the canvas under the touch, or a pass has already seen
  the press, so a native field moved over the tap takes no pointer input
  until the next pass. Where `pointerup` comes first,
  as in Chromium, eframe's pass has already focused the agent, and nothing
  is stopped. The first device run of this branch shows that iOS sends
  `pointerdown`, `touchstart`, `pointerup`, `touchend`, as Chromium does, so
  on iOS the stop never acts.
- If egui does not focus the field, because a popup egui draws over it comes
  earlier in the tree, eframe blurs the agent in the next pass and the
  keyboard closes again. The layer's hit targets have the same limit.

### Native composer mode

`NativeTextEdit` is an opt-in recipe for a single-line or multiline field,
used where the application would add an `egui::TextEdit`. It builds the
`TextEdit` itself, so it knows the field's font, margin and colours. It also
registers the field for the current pass, with a style taken from the `Ui`'s
visuals, which Stobie derives from the design tokens. Natively, and in a
browser until the field is activated, it is an ordinary egui field.

In the browser, the layer projects a registered field as before, as a
`textarea` or `input` in the transparent container, with
`data-stobie-native`. It gives the field spelling, autocorrect and
sentence capitalisation, and records the field's absolute, clipped geometry
and its active style (`AriaTree::native`).

- **Activation.** A tap on a native field (the first-tap test above) activates
  it inside the gesture. The layer:
  1. puts a hidden placeholder in the element's place in the layer;
  2. adds the element's ID to the `aria-owns` of its layer parent, so the
     accessibility tree keeps it in its region;
  3. moves the element into a visible sibling host over the canvas;
  4. styles it from the registered style: the application's font (Source Sans 3,
     registered as a `FontFace` from the bytes egui uses), size, line height,
     padding, text, caret, selection and placeholder colours, a transparent
     background and no border;
  5. focuses it, then asks egui to focus the field through AccessKit `Focus`.

  The element is moved before it is focused, because moving a focused element
  blurs it. iOS then owns the caret, loupe, selection, autocorrect context,
  dictation, paste and IME composition. The element holds the whole text, so
  predictions see the text before the caret.
- **Leaving the container.** The host is a fixed element with the canvas's
  geometry, `overflow: hidden` and `pointer-events: none`, outside the layer's
  `opacity: 0`, `contain: strict`, `user-select: none` container. Only the
  active element in it takes pointer input. The placeholder stands in for the
  element in every placement the diff makes, so the layer's structure is
  unchanged.
- **Geometry.** Each pass re-reads the canvas's client rectangle and
  writes the element's geometry and clip only when they change. While a field
  is active, window `resize`, `scroll` and `orientationchange`, and
  `visualViewport` `resize` and `scroll`, re-read the canvas's rectangle and
  move the host without a pass. Keyboard and rotation also resize the canvas
  when the page sizes it from the visual viewport, which runs a pass. Text
  smaller than 16 CSS pixels is drawn at 16 and scaled down, because iOS
  zooms the page to focus smaller text.
- **Painting.** While its field is active, `NativeTextEdit` gives egui a
  transparent text colour and hint, and turns off the caret, its blink and the
  selection fill. egui keeps the frame, background and focus stroke, and
  still lays out the text, so the field keeps the height egui gives it. The
  pass after activation is the first to paint this way, so for one frame
  egui's text is drawn beneath the element's.
- **Editing.** Each `input` event on the element becomes an AccessKit
  `SetValue` request with its whole text, as for any layer field. The host
  applies it as typed input, and the application sees ordinary changes. A
  value set by the application, such as a cleared draft after Send, reaches
  the element through the layer's usual text update. Composition text reaches
  egui as it is typed.
- **Focus hand-off.**
  - From the layer to egui: the AccessKit `Focus` request at activation.
    eframe counts the application focused only while the canvas or its agent
    has DOM focus, and egui reports a field focused only in a focused
    viewport, so the layer's input hook marks the input focused while the
    native field has DOM focus.
  - Caret: a programmatic focus puts the caret at the end. At activation the
    layer places it where egui would, from the galley egui laid out for the
    field in the last pass (`NativeTextRegistration::char_at`). egui cannot
    place it itself: when a tap's events reach it in one pass, eframe's
    `PointerGone` comes with them and egui does not count the field as
    hovered.
  - Keys: Tab and Escape return DOM focus to the canvas and reach egui. Cmd and
    Ctrl chords reach egui as shortcuts, as in other layer fields. Return types
    a newline in a multiline field.
  - Deactivation: when the element loses DOM focus to anything but the
    canvas, or when egui's focus leaves the field after it had it. eframe's
    `pointerup` focuses the canvas for any touch on it, including one on the
    field's edge just outside the element. If egui still has the field, the
    field takes DOM focus back. A tap elsewhere on the canvas deactivates
    the field because egui gives up the field's focus on the click. The
    layer then puts the element back in the container,
    restores its layer style, focuses the canvas, so that eframe gives the
    agent focus if egui focused another text field, and clears the active
    state. A field that is no longer presented or no longer native
    deactivates too.
  - Only a tap activates. Mouse clicks, Tab and focus that egui requests
    itself edit through eframe's agent as before. Assistive technology that
    moves DOM focus onto a native field edits it in place in the layer, as
    before. Desktop browsers and screen readers therefore behave as they did.
- **Limits.** Popups that egui draws over an active field are hidden beneath
  the element. A page that scrolls without a pass or a viewport event leaves
  the element misaligned until the next pass. egui's caret is not updated
  from the element's selection on deactivation; it stays at the end of the
  text. Each edit reaches egui as select-all and paste, which leaves egui's
  cursor at the end, so a field taller than its scroll area scrolls to its
  end on every edit. Return in a single-line native field does not reach
  egui, so an application cannot submit on Return from it.

## Acceptance

- `cargo xtask verify` passes on the branch.
- Unit tests cover:
  - the tap test and `text_field_at`: topmost wins; read-only, disabled, clipped
    or covered fields are excluded;
  - the projection of native fields: the marker, absolute and clipped
    geometry, the style from the registration and the 16-pixel guard;
  - `NativeTextEdit`: it registers per pass, a tap's caret position comes
    from egui's layout, and while active egui paints no visible text or
    caret and requests no frames.
- A Chromium touch check with Record Desk, whose notes field becomes a native
  field. During each tap it holds animation frames and keeps touch pointer
  events from the page, so that, as on the iPhone, no egui pass handles the
  press first. A tap on Record title focuses eframe's agent during
  `touchend`, and the agent keeps focus while egui takes it. A tap on Record
  notes focuses the visible element during `touchend`, over the field's
  rectangle and outside the layer container, with the caret at the tap, and
  the accessibility tree still has one "Record notes" textbox. Nothing claims
  a later drag on the active field for egui, so eframe saw the tap end. The
  field takes focus back from the canvas while egui keeps it, and Escape
  returns it to the layer. Inserted text reaches egui. An active field presents no frames while idle.
  The field lies above eframe's text agent. A tap elsewhere puts the element
  back in the layer.
- The owner's device results pass every item in the
  [device checklist](#device-checklist).

## Device results

First run, 17 October 2026, on the spike's iPhone 12 Pro Max (iOS 26.6.2),
built from `f1beca8`. The owner reported: "This update looks good. Now a
single tap opens the ios keyboard and UI shows ios copy. paste etc. popup."
The diagnostics log (local, `.spike-runtime/device-logs/device-default.jsonl`,
session `a385e072`) shows:

- one tap on the composer from the conversation view: the native field
  focused 4 ms after `touchend` and the keyboard opened 145 ms later (item 1,
  first case only);
- typing and backspace as `insertText` and `deleteContentBackward` on the
  native field, ten touches inside it and a `select` event (item 4);
- touch and pointer order as `pointerdown`, `touchstart`, `pointerup`,
  `touchend`;
- a defect: a 1-second press on the canvas outside the field's element made
  eframe's `pointerup` focus the canvas while egui kept the field. The field
  returned to the layer and eframe's agent took over editing. The layer now
  refocuses the field in that case; the touch check covers it.

Item 1's repeats after closing the keyboard, an app switch and rotation, and
items 2, 3 and 5 to 10, were not reported in this run.

Second run, 17 October 2026, same device, built from `f6de9cb` (the device
loaded package assets `3f1ad5ff…`, built from that head). The owner
reported items 2, 3 and 5 to 10 as passing ("1. Yes 2. Working well now
3. Yes 4. Yes 5. yes 6. All good 7. All good 8. All good", numbered in the
order they were asked). The log (session `605ce61a`, standalone) shows:

- three single-tap activations after the keyboard had closed, each opening
  the keyboard (item 1, repeat after closing the keyboard);
- eight touches on the canvas beside the active field: each time eframe's
  `pointerup` took focus, the field took it back within 3 ms, and the
  keyboard stayed up (the first run's defect, fixed);
- `insertLineBreak` from Return and `insertFromPaste` (item 7);
- no orientation change or app switch in this build's sessions, so item 1's
  repeats after rotation and an app switch have no logged evidence;
- a further defect: one touch landed on eframe's one-pixel text agent, which
  eframe keeps at egui's caret and which lay above the native host. Editing
  moved to the agent until the next tap. The host now goes after the agent,
  above it; the touch check covers it.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Plan and decision | Complete | This plan |
| First-tap keyboard workaround | Prototype complete | Unit tests; Chromium touch check, which fails without the `touchend` focus and without the IME cover |
| Native composer mode | Prototype complete | Unit tests; Chromium touch check, which fails if the moved field takes pointer input before eframe's `touchend` |
| Device acceptance | Items 2 to 10 and item 1 after closing the keyboard pass; item 1 after an app switch and rotation outstanding | [Device results](#device-results) |

## Device checklist

Build the spike client (`/nvme/development/stobie-ios-spike`) against this
branch, with its composer switched to `NativeTextEdit`. Serve it with
`tools/serve-tailscale.sh`, reinstall the home-screen app, and record
pass or fail with a note for each item. The page diagnostics log focus and
input events.

1. **First tap.** From the conversation view, one tap on the composer raises
   the keyboard. Repeat after closing the keyboard, after an app switch and
   after rotating.
2. **Caret placement.** The caret lands where you tap in existing text, in the
   middle of a word and at the end.
3. **Loupe and caret drag.** Touch and hold, then drag. The loupe shows the
   composer's text, and the caret follows your finger without lag or offset.
   Try the space-bar trackpad too.
4. **Selection.** Double-tap a word. The handles and the Cut, Copy and Paste
   menu appear, and the selection matches the text.
5. **Autocorrect context.** Type "teh" then a space; it becomes "the". Then type
   the first word of a sentence and check QuickType predicts from the text
   before the caret. Delete a word with backspace and keep typing: predictions
   continue from the remaining text.
6. **Dictation.** Dictate a sentence. The words appear once, in order, and
   Send sends them.
7. **Paste.** Paste a line copied from Notes, at the caret and over a
   selection.
8. **Look.** The composer's text, placeholder, caret and selection use the
   app's font and colours in light and dark mode. Nothing doubles or shifts
   when the field activates or deactivates. The page does not zoom.
9. **Leaving.** Tap the transcript: the keyboard closes and egui draws the
   draft again. Tap Send: the message sends, the composer clears and the
   keyboard closes, because egui moves focus off a field on a press
   elsewhere.
10. **Geometry.** With the keyboard up, the composer stays over its frame
    while you scroll the transcript, while the keyboard's height changes
    (emoji keyboard, QuickType bar) and after rotation.

## Later work

- Report the two-tap behaviour to egui with the spike's event sequence, and
  propose focusing the agent in `touchend` from `mutable_text_under_cursor`.
  Remove the first-tap workaround when eframe makes it unnecessary.
- Copy the element's selection back into egui's cursor on deactivation.
- Hide an active field under popups that egui draws over it.
