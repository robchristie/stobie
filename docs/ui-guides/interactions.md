# Interactions

Every user-visible capability has an entry in an application-owned enum that
implements the framework `ActionKey` trait, normally through `#[derive(Action)]`.
Its generic `ActionSpec<A>` supplies the label, description, optional compact
label, shortcut and scope; the key supplies the stable external ID. The derive
declares these once per variant, checks shortcuts and identities at compile
time and lists every action in `ActionSet::ALL`. Actions are semantically
invocable unless they declare `physical_only` with a reason. An action that
chooses a value declares one typed `argument`, which discovery publishes and
the host validates before `Application::perform_with` receives it; it cannot
have a shortcut. Controls, shortcut
routing, semantic metadata, tests and physical targeting must share that typed
identity; do not
create a parallel string action name or add application capabilities to
`stobie-ui-egui`.

Bind a control to an `ActionTarget<A>`: application actions have no pane
target, while pane and active-pane actions have a stable `PaneId`. Shared UI
components remain generic over `A`; semantic snapshots retain only a
`SemanticActionId` derived from `ActionKey::stable_id` so diagnostic consumers
do not need the originating application enum. Respect
`Availability`: enabled, disabled with an observable reason, or hidden are
different states. Disabled controls retain clear semantic explanation and do
not silently consume an unavailable action. A hidden action presents nothing:
`Frame::action`, `icon_action`, `toggle` and `PresentationContext::action`
draw no control and publish no node for it, so a caller need not filter it.
Its capability stays discoverable with `rendered: false`, and invoking it is
refused as `unavailable`.

## Actions on rows

When one action acts on each row of a list, such as Open on each person,
Remove on each invitation or Pinned on each item, present it once per row as a
`BoundAction`: the action together with the argument its control carries,
normally the row's stable ID (`BoundAction::new(Action::Open, person.id)`),
and the row's name as its subject (`.with_subject(&person.name)`). Declare the
action's `argument` as for any value-taking action. Pass the bound action
wherever a hosted `Frame` takes a target: `action`, `icon_action`, `toggle`,
`list_row`, `conversation_row`, `switch`, `trigger` and `availability`. Do not hand-build a button, observe
it by hand or call `trigger_with` from a row's click.

- **Identity.** The control's identity combines its key with the argument, so
  each row's control has its own stable semantic ID wherever the row moves.
  Use the row's domain ID, never its position, label or a counter.
- **Names.** A button's accessible name is the action's label followed by the
  subject ("Open Ada Lovelace"); its visible label stays the action's unless
  the control gives its own with `with_label`, which then names it ("Edit
  Morning news"), or a shorter one to show with `with_short_label`. A list
  row is named by its title. Each row's control therefore has a distinct name
  in AccessKit, the snapshot and the browser ARIA layer. A conversation row is
  named by its title and a switch by its label in the same way.
- **Semantics.** The control's node publishes the argument as `UiNode.argument`
  (the JSON value an invocation would send). The host binds each such control
  as its own capability carrying that argument, and keeps the action's own
  binding, which takes any value its contract allows, unless a control without
  an argument presents the action.
- **Availability.** Implement `Application::availability_with(target,
  argument)` to decide availability for one row, such as a disabled Remove on
  a pinned person, and to hide an argument that names nothing, such as a
  removed row. Controls carrying an argument, their bindings, and every
  action performed with an argument use it; the default is the target's
  availability.
- **Dispatch.** A click, Enter or Space, an AccessKit click and a semantic
  invocation all reach `Application::perform_with` with the row's argument,
  after the host checks the contract and `availability_with` again.

A row removed before its action runs is refused, never performed for another
row. A click on a row that an earlier intent of the same pass removed is
refused by `availability_with` when it hides the row's argument. A discovered
target whose row has gone no longer matches (`no_match`); one admitted before
the row went is refused when it runs (`unavailable` when `availability_with`
hides its argument, otherwise `stale_target`). A target whose control now
carries another argument is `stale_target`. A control whose argument its
contract refuses is discovered disabled with that argument and the reason,
beside the action's own binding, and invoking it is `unavailable`. Invoking
the action's own binding with a removed row's ID is refused by
`availability_with` when it hides that ID. A custom row that paints itself can carry the same contract: set its
node's `actions` and `argument` and trigger the bound action with
`Frame::trigger`.

## Segmented choices

A segmented choice presents each option as a control bound to one action with
that option's argument, so everything above applies to each option.

- **Keyboard.** A single choice is one Tab stop: the chosen option, or the
  first enabled one when none is chosen. Left and Up move to the previous
  enabled option, Right and Down to the next, wrapping, and Home and End to
  the first and last; each move chooses the option it reaches, as in the
  WAI-ARIA radio group pattern. Space, Enter and a click choose the focused
  option. Each option of a multiple choice is its own Tab stop, toggled by
  Space, Enter or a click; arrows do nothing there.
- **Agents.** Each option is its own capability carrying its argument, as
  each row's control is. Invoking it performs the action with that argument;
  a disabled or hidden option is refused as its control is.

## Composing and sending

A chat-style message is composed in a composer
([recipe](components.md#composer)): the application owns the draft, and a
send action that takes a text argument carries it.

- **The draft.** Keep it in application state. `Frame::composer` presents it
  and emits the given intent with the new text for each edit; clear it in
  `perform_with` when the send succeeds. A send the application refuses
  returns an error and leaves the draft as it was, so nothing typed is lost.
- **Availability.** `availability_with(send, Text(draft))` decides whether
  this draft can be sent: hide send for an empty draft to let the trailing
  control (such as a microphone) stand alone, or disable it with a reason,
  such as while the previous message is still sending. A draft over the
  action's `max_length` disables send with the contract's reason.
- **Return.** With a keyboard, Return sends and Shift+Return types a newline;
  Cmd or Ctrl+Return also sends. Return does nothing while send is disabled,
  hidden or the draft is empty: it neither sends nor types a newline. On a
  touch device a tap makes the field the platform's own text field, whose
  Return key types a newline; the send control sends. A hardware keyboard's
  Cmd+Return still sends there.
- **Focus.** Pressing the send control does not take focus from the field,
  and the field keeps focus after any send, so typing continues and a phone's
  keyboard stays up. Other composer controls take focus as any button does.
- **Agents.** The field is a `multiline_text_input` named by the composer, so
  `stobie set-text FIELD TEXT` sets the draft through the same edit route as
  typing. The send control carries the draft as its argument: `stobie invoke
  SEND --id ID` sends the draft shown. `--value TEXT` sends that text,
  through the send control when it carries it and otherwise through the
  action's own binding, under the same availability. A refused send's receipt
  carries the application's error, and the draft stays.

## Copying to the clipboard

A copy control ([recipe](components.md#copy)) copies a given text when
pressed; it is not an action, because copying changes no application state.
Browsers let a page write to the clipboard only inside a user's gesture, and
Safari on iOS only inside the gesture's own event, so the web host writes in
that event rather than on a later frame:

- **Tap or click.** eframe runs the pass that handles a pointer release
  inside the browser's `pointerup` handler, and the host's clipboard route
  calls `navigator.clipboard.writeText` at the end of that pass, so the write
  is inside the tap.
- **Keyboard.** A key press reaches egui only on the next frame, so the host
  writes inside the `keydown` itself when Enter or Space reaches the canvas
  while a copy control has keyboard focus, with the text that control
  presented in the latest pass. The pass that then sees the press does not
  write again.
- **Assistive technology.** A click on a copy control's ARIA layer element,
  as VoiceOver's double tap or a screen reader's activation sends, writes
  inside that click in the same way; the layer's request still reaches egui.
- **Agents and AccessKit actions.** `stobie invoke stobie.copy-text --id ID`
  and AccessKit actions from outside the layer press the control on the next
  pass, outside any gesture. Chromium allows that write while the page holds
  clipboard permission, and a browser that refuses it makes the control show
  its failure.

The browser's answer settles the control: success shows the confirmation,
refusal the failure. Natively the platform clipboard takes the text through
egui and the control confirms. Only copy controls write this way; selectable
text keeps egui's own copy.

## Dialogs

A dialog, sheet or confirm ([recipe](components.md#dialogs-sheets-and-confirms))
is modal: while it is presented, it is the whole of what people, assistive
technology and agents can reach. The host enforces this; the application only
presents it while its state says it is open.

- **Focus in.** When it opens, focus moves to its first control: a form's
  first field, or a confirm's cancel. Focus that lands behind it, as an
  arrow key can move egui's focus, goes back to where it was in the dialog.
- **Tab.** Tab and Shift+Tab move through the dialog's controls only, and
  wrap. The close control comes last, after the content.
- **Escape and the close control.** Both trigger the dismiss action, as a
  confirm's Escape triggers cancel, while that action is enabled, from a
  focused field too. An open popup, such as a combo box's list, closes on
  Escape first.
- **Focus back.** When the dialog closes, focus returns to the control that
  opened it: the one clicked, or focused, as it opened. A dialog that opened
  with no such control, as one an agent or the application opened, returns
  focus nowhere. A dialog that takes another's place, such as a confirm
  after a form, returns focus to the first one's opener.
- **Pointer.** Controls behind the scrim take no pointer input; a press on
  the scrim dismisses only a dialog that asks for it.
- **Agents and shortcuts.** The host observes and binds only the dialog and
  what it presents. Every other action is disabled while it is open, with
  the reason `The dialog "<title>" is open`, so an invocation is rejected as
  `unavailable` and a shortcut for it does nothing; the actions the dialog
  presents remain invocable, with their shortcuts, including one whose
  control a long form has scrolled out of view.
- **Assistive technology.** Everything behind the dialog is hidden from the
  AccessKit tree, and requests aimed at it are dropped; in the browser the
  ARIA layer holds only the dialog, a `dialog` or `alertdialog` with
  `aria-modal`.

## Keyboard and pointer behaviour

Use the registered shortcut only in its valid scope. Pane shortcuts require an
active pane and non-command shortcuts yield to a focused text field (an egui
`TextEdit` or an observed, focused text input), not to a focused button or
viewport. Plain Enter and Space also yield to a focused control they activate,
but reach a focused observed viewport. A hosted application's shortcuts
dispatch through `AppHost`: modifiers must match exactly, and an unavailable
shortcut leaves its key press unconsumed, so a Shift chord never falls through
to its plain counterpart. Tabs support roving Left/Right/Home/End plus
Enter/Space activation. Splitters expose the same 0.05 adjustment through
keyboard and semantic increment/decrement paths as they do through the gesture
command path.

Use stable IDs and current semantic geometry for physical automation. During a
drag, preserve the original press position when delayed drag recognition is
involved, and commit only the completed gesture. Repaint only for a recorded
reason: interaction, state transition or scheduled work—not an unconditional
frame loop. Reduced motion removes custom animation timing rather than hiding
state changes, and holds a focused field's caret steady instead of blinking.

Text selection is an explicit interaction, not a consequence of a text role.
Selectable measured text uses egui's label-selection state so drag, multi-label
selection and copy remain platform-consistent. A component with an owning click
or drag gesture keeps its text inert; expose the same technical value in the
Inspector when selection would compete with that gesture.
