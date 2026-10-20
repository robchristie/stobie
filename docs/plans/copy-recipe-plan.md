# Copy recipe

Status: complete
Delivery: [PR #112](https://github.com/robchristie/stobie/pull/112)

The second increment of [content text in observation](content-text-observation-plan.md).
An application that shows its user a code, a link or a message often offers a
button that copies it: an invitation code to send to someone else, for
example. Stobie had no such recipe, so a consumer made the text selectable
and left the user to select it, which a phone makes awkward, or wrote its own
button, which met two problems: Safari on iOS lets a page write to the
clipboard only inside the user's gesture, and egui's `copy_text` output is
written on a later frame; and nothing told assistive technology or an agent
that the copy happened.

## Decisions

- **A recipe, not an action.** `PresentationContext::copy(ui, key,
  &CopySpec)` presents a button that copies `spec.text`. Copying changes no
  application state, so it needs no `ActionKey`, intent or `perform`; the
  application gives the text and four strings of its own (label,
  confirmation, failure and an optional description). Stobie adds no words.
- **Writes inside the gesture.** The web host installs a clipboard plugin
  that takes each pass's writes in its output hook and calls
  `navigator.clipboard.writeText` itself. eframe runs the pass that handles a
  pointer release inside its `pointerup` handler (as it does for its own
  copy and cut events, for Safari), so a tap or click writes inside the
  gesture. A key press and an ARIA layer click reach egui only on the next
  frame, so for those the host writes inside the event itself, with the text
  the focused or clicked copy control presented in the latest pass, and the
  pass that sees the press does not write again. Agents and AccessKit
  actions from outside the layer press the control on the next pass, outside
  any gesture: Chromium allows that with clipboard permission, and a browser
  that refuses shows the failure honestly. Natively the platform clipboard
  takes the text through egui's output, and the control confirms, because it
  cannot observe a failure.
- **Outcome and announcement.** Success shows a check and the confirmation
  for two seconds (`COPY_CONFIRMATION`); refusal shows a warning and the
  failure for five (`COPY_FAILURE`); new text drops an outcome. The button
  keeps its label as its accessible name throughout, so a voice-control
  user's "click Copy code" still works while the face reads "Copied"; a
  polite AccessKit status beside it, present and empty before any outcome so
  screen readers announce the change, takes the message. Snapshots publish
  the status node only while it has a message.
- **Steady width.** The face is as wide as the widest of its label,
  confirmation and failure, so a confirmation never moves its neighbours.
  `ButtonFace` and `present_button` became the shared button presentation
  for action buttons and copy controls.
- **The text is never published.** The control is a `Button` node named by
  its label. Agents discover it as the framework capability
  `stobie.copy-text`, whose target meaning is `text-revision:N`, a counter
  kept with a hash of the text in memory only: it moves on when the text
  changes, so an old target is `stale_target`, and it reveals nothing about
  the text. An application that must not publish a secret withholds the
  content text that shows it and keeps the copy control.
- **Content text.** Empty text and text given a zero-size slot now publish no
  content, and the application interface guide says that inside a
  visible-only region (each dock pane) a partly visible heading or label
  publishes nothing until it is wholly visible, as increment 1 found.

## Design review contract

- **Task.** Copy an invitation code to send to someone, copy a message, and
  copy a link in a German interface; on a phone with a tap, at a desk with a
  pointer or keyboard, with a screen reader, and as an agent.
- **Hierarchy.** The copied text leads; the copy control is quiet beside or
  under it, a label-only control is a normal button, and an icon-only one is
  borderless. Its confirmation replaces the label in place.
- **Reference.** Familiar copy buttons in messaging and developer tools
  (a copy glyph and "Copy", turning to a check and "Copied" for a moment);
  Stobie's action button, icon and focus-ring treatments.
- **States.** Ready, hover, pressed, keyboard focus, copied, refused,
  disabled with a reason, hidden; icon-only, label-only and both; a long
  localised confirmation; dark and light, standard and high contrast, 100%
  and 150% text, narrow.
- **Exclusions.** Copying rich text or images, reading the clipboard,
  a DOM text layer for native selection, and a toast or snackbar.

## Acceptance

- Host tests (`stobie-ui-egui`): each face in each state matches AccessKit,
  keeps its width and never publishes the text; the status region is polite,
  always present and named by the message; a disabled control carries its
  reason and copies nothing; a hidden one takes no space and publishes
  nothing (`presentation::copy::tests`). Through `AppHost`
  (`host::copy_tests`): a press copies and confirms with a status, which
  returns to the label after two seconds; the keyboard copies from the
  focused control; agents discover `stobie.copy-text` with a text revision
  and invoke it; a disabled or changed target copies nothing
  (`unavailable`, `stale_target`); new text drops a confirmation; and the
  text never reaches a snapshot or discovery. Clipboard unit tests cover a
  later write replacing an earlier outcome, writes between passes and the
  native route. Content text: empty and zero-size text publish no content.
- Browser (`tools/copy-browser-smoke.sh`, in full verification on WebGPU and
  WebGL2), in headful Chromium with clipboard permission granted: a mouse
  click writes inside its trusted `pointerup`, Enter on a control focused
  through the ARIA layer inside its trusted `keydown`, a click on the
  control's layer element inside that click, and a touch-screen tap inside
  its trusted `pointerup`; each writes once, the clipboard holds the text,
  and the snapshot and the ARIA layer show one polite status that ends after
  its time. An agent copies through the application interface and discovery
  never carries the text; a disabled control writes nothing; a refused write
  shows the failure for five seconds; the page goes idle.
- Gallery `copy/actions`, with inspected fixtures `copy-actions-dark` and
  `copy-actions-high-contrast` (light high contrast at 150% and narrow): no
  semantic, AccessKit or text audit findings.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Recipe, clipboard route and agent capability | Complete | Host and clipboard tests |
| Browser gesture checks | Complete | `tools/copy-browser-smoke.sh` on WebGPU and WebGL2 |
| Gallery, documentation, critique and full verification | Complete | One story, two fixtures, guides and `cargo xtask verify` |

## Design review

```text
Task and reference: copy a code, a message and a localised link in Gallery
  copy/actions, by pointer, touch, keyboard, layer click and agent; familiar
  copy buttons and Stobie's action button treatments as reference.
Behavioural checks: passed (host tests; headful Chromium with mouse, touch,
  keyboard, layer click, agent and a refused write).
Presentation checks: passed in copy-actions-dark (dark, comfortable, 100%)
  and copy-actions-high-contrast (light, high contrast, 150%, narrow), and
  in the browser captures of copied, keyboard-copied and refused states.
Design decision: accepted after one revision.
Main reason and remaining trade-off: the first critique found the browser
  route wrote on the frame after a key press or a screen reader's
  activation, outside the event, where Safari refuses; the host now writes
  inside those events too. The second critique accepted the faces: the
  confirmation replaces the label in place without moving anything, and the
  status announces it once. An icon-only control shows a failure only as a
  warning glyph (its tooltip still names the control), so the guide
  recommends a face with the label where a failure must be read.
Exact candidate evidence links: the two copy fixtures under
  tests/ui-snapshots/expected; the browser captures stay in ignored
  .tools/runtime evidence.
```

## iOS device checklist

For a consumer's device check of its copy controls, on an iPhone home-screen
app built on this revision:

1. **Tap.** Tap a copy control: its face shows the confirmation for about two
   seconds, then its label; paste into Notes, which receives exactly the
   text.
2. **Twice.** Tap it again during the confirmation: it copies again and the
   confirmation restarts; nothing beside it moves.
3. **VoiceOver.** With VoiceOver on, swipe to the control (it reads its
   label as a button), double-tap: VoiceOver announces the confirmation, and
   the clipboard holds the text.
4. **Hardware keyboard.** With a keyboard attached, Tab to the control and
   press Space or Return: it copies.
5. **New text.** When the application replaces the text (a new code), the
   control shows its label and copies the new text.
6. **Look.** Light and dark: the glyphs, label and confirmation use the
   application's font and colours, and the focus ring is clear.

## Closeout

Applications present a copy control with `PresentationContext::copy`: it
writes inside the user's gesture in the browser, confirms or reports a
refusal in place and through a polite status, and lets agents copy through
`stobie.copy-text` without ever publishing the text. Full canonical
verification passed on the committed implementation; review, CI and merge
evidence stays with the owning pull request.

## Later work

- Device results for the checklist above, run by the first consumer.
- Safari's answer to an agent's copy, which runs outside any gesture, is not
  yet observed; the control shows the failure if Safari refuses.
- A native platform's clipboard failure is not observable through egui, so a
  native control always confirms.
