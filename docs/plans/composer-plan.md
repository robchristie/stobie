# Composer recipe and hidden actions

Status: complete
Delivery: [PR #PENDING](https://github.com/robchristie/stobie/pull/PENDING)

Framework work item 6 of the [iOS PWA spike report](../ios-pwa-spike-report.md#framework-work):
a chat application needs a multiline composer that grows with its message,
places Send beside it, handles Return on a phone and a desktop, and lets
agents set the draft and send it. `NativeTextEdit`
([iOS native text](ios-native-text-plan.md)) gives the platform's own field
on a touch device, but an application still built the composer around it by
hand: its height, its controls, its Return handling and its focus after a
send. The same item found that `Frame::action` drew an action whose
availability is `Hidden` as a disabled button: the button component only
asserted, in debug builds, that its caller had filtered hidden actions.

## Decisions

- **Hidden actions present nothing.** `present_action_button`, and so
  `PresentationContext::action`, `icon_action`, `action_with_icon`,
  `action_with_content` and `bound_action`, and the hosted `Frame::action`,
  `icon_action` and `toggle`, draw no control and publish no node for a
  hidden availability; they return an empty response with the control's
  egui ID. The debug assertion is gone. Discovery still lists the capability
  with `rendered: false`, and invoking it is refused as `unavailable`, as
  before.
- **A composer recipe.** `Frame::composer(ui, presentation, key, spec,
  draft, edit)` presents a `ComposerSpec`: a field name, an optional hint,
  `max_rows` (default `COMPOSER_DEFAULT_MAX_ROWS`, six), the send control and
  optional leading and trailing controls. It emits `edit(text)` for each
  change, triggers send with the draft as its text argument when sent, and
  triggers the other controls when clicked.
  `PresentationContext::composer` is the low-level form over a `&mut String`
  and an availability closure. Both return a `ComposerResponse`.
- **Controls take an icon or a label.** `ComposerControl::icon`, `label` and
  `icon_label` choose the existing `ActionButtonContent`, so artwork replaces
  a label without other changes. Leading and trailing controls are quiet and
  borderless; send is primary. Each keeps the token hit height and sits at
  the bottom of the field.
- **The application owns the draft.** The send action takes a text argument
  (the composer panics on another contract). `availability_with(send,
  Text(draft))` decides whether this draft can be sent: hidden (for example,
  empty, leaving a microphone alone) or disabled with a reason (for example,
  while sending). A draft over `max_length` disables send with the
  contract's reason. The application clears the draft in `perform_with`; a
  refusal returns an error and the draft stays.
- **Growth.** `NativeTextEdit` gains `max_rows`, `frame` and `return_key`.
  With `max_rows` the field sits in a scroll area that grows with the text up
  to that many lines and then scrolls; its response, interaction rectangle,
  AccessKit bounds and native registration are the part shown, so the native
  field overlays only that. Its hint shows on one elided line, and the
  AccessKit placeholder keeps the whole hint. The composer's controls follow
  the field's height one pass later, with one repaint.
- **The native field wraps where egui does.** A registration's text
  rectangle was egui's `text_clip_rect`, which is only as wide as the text
  laid out, so an active native field's right padding was the field's width
  less its longest line: after a newline, the platform's field wrapped
  earlier than egui and overflowed. The text rectangle now spans the field
  less its margins, for every `NativeTextEdit`. Its line height was the
  font's row height (17.25 points for body text), but egui places a text
  edit's rows on whole points (two lines take 34): the native text overflowed
  and showed a scroll bar. A multiline field now registers the line height of
  egui's rows, and `max_rows` measures that many of them.
- **Return.** With a keyboard, Return sends and Shift+Return types a
  newline; Cmd or Ctrl+Return also sends. Return does nothing while send is
  disabled or hidden. Holding Return sends once. An active native field on
  a touch device keeps the platform's Return, which types a newline; the send
  control sends, and a hardware keyboard's Ctrl or Cmd+Return still sends.
  The iOS keyboard's Return therefore never sends: a phone sends with Send,
  as Messages does.
- **Focus.** A press on send leaves the field focused, and the field takes
  focus after every send, so typing continues and an active native field
  stays active, keeping a phone's keyboard up. Other controls take focus as
  any button does.
- **Observation.** The field is a `MultilineTextInput` node named by the
  spec, whose value is the draft; agents set it with `set-text`. The send
  control carries the draft as its argument, so `invoke SEND --id ID` sends
  the draft shown and `invoke SEND --value TEXT` sends that text, through the
  control carrying it or the action's own binding. A refused send's receipt
  carries the application's error.
- **Compatibility.** Additive. `Frame::action`, `icon_action`, `toggle`,
  `ActionButtonSpec`, `action_button`, `NativeTextEdit` and `ActionKey`,
  which emuella-viewer uses, keep their signatures; the only behaviour
  change is that a hidden action draws nothing where a release build drew a
  disabled button and a debug build panicked. No dependency is added, so the
  web package changes only by the recipe's own code.

## Design review contract

- **Task.** Write a message to a contact, over several lines, and send it;
  attach a file or dictate instead; on a phone, with the on-screen keyboard
  up throughout.
- **Hierarchy.** The draft leads. Send is the one primary control, present
  only when there is something to send; Attach and the microphone are quiet
  and borderless at the edges. Controls stay beside the last line as the
  field grows.
- **Reference.** A familiar chat composer: a rounded field with `+` at the
  left and a microphone or Send at the right; Stobie's text field, action
  button and focus-ring treatments.
- **States.** Empty (hint, Send hidden), one line, grown lines, past the line
  limit (scrolling), a long localised hint in a narrow column, Send disabled
  with a reason, focused (focus-ring border), dark and light, standard and
  high contrast, 100% and 150% text.
- **Exclusions.** Attachments' own presentation, dictation, mentions, rich
  text, typing indicators and a send-in-progress spinner.

## Acceptance

- Host tests (`stobie-ui-egui`, `host::composer_tests`): the field grows a
  line at a time to exactly its limit and then scrolls; Return and Cmd+Return send
  and keep focus; Shift+Return types a newline; the send control sends and
  keeps focus; an empty draft cannot be sent and Return types nothing; a
  disabled send (in flight, or over `max_length`) keeps its reason and
  ignores Return; a refused send keeps the draft; hidden controls take no
  place; agents set the text and send by ID and by value, a refused send
  keeps the draft and an empty draft is not sent; and hidden plain actions
  (`action`, `icon_action`, `toggle` and `PresentationContext::action`) draw
  nothing, publish no node and stay discoverable but refused.
- `NativeTextEdit` tests: a field registers its text at its full width less
  its margin; a limited field grows to its rows and no further, registers
  the part shown and a line height whose lines fill egui's text; a frameless field paints no frame; a limited field's hint is one
  elided line with the whole hint as its placeholder.
- Outside application (`tools/composer-app`, run by
  `tools/composer-smoke.mjs` in the outside-app stage): natively and in
  Chromium, `stobie set-text` sets the draft, `invoke` sends it by Send's ID
  and by value, a send refused at the daily limit keeps the draft, and an
  empty draft is refused. In headful Chromium with a keyboard: typing,
  Shift+Enter for a newline, Enter to send and a press on Send, with the
  field keeping focus. With a touch screen at a phone's width: a tap makes
  the field the platform's own, Return types a newline, the platform's field
  wraps two lines without overflowing, a tap on Send sends,
  the platform's field stays focused and empty, and Ctrl+Enter sends.
- Gallery `composer/chat` and `composer/long-narrow`, with inspected fixtures
  `composer-chat-dark` and `composer-long-high-contrast` (light high contrast
  at 150% and narrow): no semantic or text audit findings.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Hidden actions and the composer recipe | Complete | Host and `NativeTextEdit` tests |
| Outside application, browser and agent checks | Complete | `tools/composer-smoke.mjs` in the outside-app stage, natively, in Chromium with a keyboard and with a touch screen |
| Gallery, documentation, critique and full verification | Complete | Two stories and fixtures, guides and `cargo xtask verify` |

## Design review

```text
Task and reference: write and send messages in the Gallery composer stories
  and the outside-composer application; a familiar chat composer and
  Stobie's text field and button treatments as reference.
Behavioural checks: passed (host tests; stobie natively and in Chromium;
  headful keyboard and touch journeys).
Presentation checks: passed in composer-chat-dark (dark, comfortable, 100%)
  and composer-long-high-contrast (light, high contrast, 150%, narrow).
Design decision: accepted after one revision.
Main reason and remaining trade-off: the touch capture showed the active
  native field wrapping earlier than egui and scrolling inside a two-line
  field; the registration's text width and line height are fixed. The first critique found the narrow
  column's long hint wrapped over four lines, pushing the field tall while
  empty, and the third example clipped at the story's foot. The hint is now
  one elided line with the whole hint kept for assistive technology, and the
  examples fit. A field past its limit shows the top of its next line in its
  bottom padding, which scrolls with the text; it reads as a cue that the
  field scrolls and is kept. At 150% in a
  narrow column the Dictate label leaves the field narrow; icons for the
  microphone (a consumer's choice) widen it.
Exact candidate evidence links: the two composer fixtures under
  tests/ui-snapshots/expected; the outside application's captures were not
  retained.
```

## iOS device checklist

For a consumer's device check of its composer, on an iPhone home-screen app
built on this revision:

1. **First tap.** One tap on the empty composer raises the keyboard; the
   hint is the application's ("Message Ada").
2. **Return.** The keyboard's Return key types a newline and never sends; the
   field grows a line at a time, the controls staying beside the last line.
3. **Limit.** Past the line limit the field stops growing and scrolls; the
   controls stay in reach above the keyboard.
4. **Send.** Tap Send: the message sends, the field clears and the keyboard
   stays up with the caret in the field, ready for the next message.
5. **Refused.** With the service refusing a send (such as a daily limit), tap
   Send: the draft stays as typed and the application shows its error.
6. **Empty.** With the draft empty, Send is hidden (or disabled, as the
   application chooses), and the trailing control, such as a microphone,
   stands alone.
7. **Dictation and paste.** Dictate a sentence and paste a paragraph: each
   arrives once, the field grows, and Send sends it.
8. **Hardware keyboard.** With a keyboard attached, Cmd+Return sends.
9. **Look.** Light and dark: the field's text, hint, caret and selection use
   the application's font and colours, and nothing doubles or shifts when
   the field activates.

## Closeout

Hidden actions draw nothing anywhere the action recipes present them, and a
composer recipe gives chat applications a growing native field with Send,
leading and trailing controls, Return and focus conventions for keyboards and
touch screens, and agent routes through `set-text` and `invoke`. Full
canonical verification passed on the committed implementation; review, CI and
merge evidence stays with the owning pull request.

## Later work

- An active native field past its limit scrolls itself, and egui's scroll
  offset does not follow it: when the field returns to egui, egui shows its
  own offset, at its caret, which each native edit leaves at the end.
- A desktop screen reader editing the composer's field in the ARIA layer
  types a newline on Return, as in any layer textarea, rather than sending.
- Device results for the checklist above, run by the first consumer.
