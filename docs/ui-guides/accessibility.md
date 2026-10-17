# Accessibility and semantics

Egui/AccessKit provides the framework semantics where available. Stobie
augments it with `UiSnapshot`: stable IDs, current bounded geometry, complete
names and descriptions, role, enabled/focused/selected state, actions,
disabled reason, pane/domain reference, text selectability and measured-text
observations. The snapshot is an observation of the current frame, not a
second application tree.

The native applications compile eframe's AccessKit adapter, which consumes the
same egui tree exercised by semantic tests. Adapter compilation, framework
semantics and keyboard proof are independent evidence axes; none establishes
working screen-reader support by itself. Claim end-user support only for an
OS/assistive-technology combination whose representative workflow is retained
in the
[accessibility integration evidence](../accessibility-integration-report.md).
Stock eframe 0.36.1 provides no browser AccessKit adapter and discards the tree
update in `WebRunner`. Stobie's own [browser ARIA layer](#browser-aria-layer)
takes that update first and mirrors it into the page. Do not substitute
eframe's Web Speech event output for a browser accessibility tree.

Custom interactive controls must expose a usable role and name, current state,
supported action, focusability, visible focus and a token minimum hit target.
Derive IDs from stable pane/domain identity. For example, dock tabs and
splitters use `PaneId` and `DockNodeId`; actions use their `ActionTarget`
semantic ID. Never target stale pixel coordinates in tests or automation.
Nodes that contain a supported text-selection surface set `text_selectable`;
measured-text observations retain the corresponding `TextInteraction` so a
physical test can resolve the current allocation before dragging.
Each visible string has one AccessKit owner. Standalone content uses its text
node; label/value pairs expose adjacent label and value nodes without repeating
the pair on an aggregate container; interactive chrome owns its painted text.

## Browser ARIA layer

In the browser, `WebHost` mirrors each completed pass's AccessKit tree into
hidden DOM elements with ARIA semantics. Every `AppHost` web application gets
it without application code. `Application::web_accessibility_layer` opts out.
An application that runs its own eframe loop instead of `AppHost`, such as
the [Emuella viewer](https://github.com/robchristie/emuella-viewer), has no
layer. The
[plan](../plans/aria-mirror-plan.md) records why AccessKit, not the semantic
snapshot, is the source.

- **Source.** An egui plugin takes `platform_output.accesskit_update` in its
  output hook, before eframe's web runner discards it. egui generates the tree
  only on passes it already runs, and the layer never requests a repaint.
- **Elements.** The layer is a `div` after the canvas. It has `opacity: 0` and
  `pointer-events: none`, but it is neither `display: none` nor `aria-hidden`.
  Each element is absolutely positioned at its node's bounds, so a pointer at
  an element's centre reaches the canvas and the same control. A node's
  `author_id`, which Stobie recipes set to the semantic ID, becomes the element
  ID `stobie-a11y:<semantic ID>` and its `data-stobie-id`, the ID the
  inspection interface uses. Unnamed layout containers are flattened into
  their parent.
- **Hit targets.** An enabled control with a click action, and an editable
  text field, is marked `data-stobie-hit` and takes pointer input at its
  centre pixel only: a control through a one-pixel child element, and a text
  field, whose `input` or `textarea` cannot hold one, through its own element
  clipped to that pixel. egui reports a widget's whole rectangle, even where a
  list, scroll area or pane clips it, so only a control whose centre is
  visible within those has one. A control whose centre the visible part of a
  later control also covers has none either, because egui would give that
  control the click; a
  dock splitter's grab area, registered before its panes, lies beneath them. Playwright's default `click()` and Lantern's strict
  role/name targets hit-test that centre, so they now reach the control. The
  layer dispatches every pointer, mouse, touch, wheel and context-menu event
  that lands on the pixel to the canvas again, at the same coordinates, with
  hit targets briefly disabled so eframe's own hit test finds the canvas. It
  stops the canvas hearing that the pointer left. File drags and drops are
  forwarded too. egui therefore receives the same input as without the layer:
  a click there is a physical click, and hover, drags and wheel starting there
  behave as before. Over that pixel the cursor keeps the default shape rather
  than egui's; Safari's trackpad gesture events are not forwarded; and a touch
  that starts there stops reaching the canvas if its control's element is
  removed mid-touch. A popup or menu drawn over a control does not remove its
  target, so close popups before plain-clicking controls beneath them.
  Clicking a text field focuses it in egui, as a real click does; eframe's
  text agent then takes the field's name, so a role/name target for the field
  matches two text boxes until focus leaves. Type into a field by role and
  name without clicking it first (Lantern's `type` focuses the layer field
  itself), or address it by `data-stobie-id`.
- **Updates.** Each pass is diffed with the previous one by element ID. Only
  changed attributes, text, geometry and child order are written, and a pass
  whose tree is unchanged writes nothing. Hover repaints, for example, write
  nothing.
- **Root.** The canvas becomes `role="application"`, named by the application
  identity, and owns the layer and eframe's text agent through `aria-owns`.
  The application role makes
  screen readers pass keys to egui, which handles Tab, arrows and shortcuts.
  The layer can still be read in browse mode. Destroying the host removes the
  layer and restores the canvas.
- **Activation.** A `click` event on a layer element (not on its hit target)
  becomes an AccessKit click request for its node on egui's next pass. That covers a screen reader's
  default action, `element.click()` and Playwright's `dispatchEvent('click')`.
  Native assistive technology uses the same route. egui clicks only an enabled
  widget that senses clicks, and a triggered action still rechecks its
  availability before `perform`. A disabled control therefore does nothing; it
  carries `aria-disabled` and its reason in `aria-description`. A hidden
  capability renders no widget, so it has no element.
- **Focus.** DOM focus stays on the canvas, which owns keyboard and IME input
  together with eframe's text agent. The canvas's `aria-activedescendant` names
  egui's focused element, so a Tab move is announced. Focusable controls carry
  `tabindex="-1"`. When assistive technology moves DOM focus onto one, the
  layer requests egui focus for its node and returns DOM focus to the canvas.
  A layer text field instead keeps DOM focus (see Values). While egui edits
  text, eframe's text agent holds DOM focus. It is named after the focused
  field, so typing after clicking "Record title" edits that field.
- **Values.** Text fields are hidden `input` and `textarea` elements whose
  `value` mirrors egui's text, so browsers and Playwright snapshots expose it.
  A focused layer field keeps DOM focus and can be edited in place, as
  `locator.fill()` and typing do. Each `input` event becomes an AccessKit
  `SetValue` request with the field's whole text. The host applies it as
  typed input: it focuses the egui field, selects its text and pastes the new
  text over it. The application therefore sees its ordinary change, and a
  value egui does not take is restored after the pass. Tab and Escape return
  DOM focus to the canvas and reach egui as the same keys. Cmd or Ctrl chords,
  other than the field's own selection, clipboard, undo and caret keys, reach
  egui as application shortcuts while focus stays in the field. Password
  fields are never edited this way. Combo boxes
  expose their current text. Sliders expose `aria-valuenow`, minimum and
  maximum. Splitters expose the first pane's share as a percentage within
  10–90%, because Chromium ignores `aria-valuetext` on `separator`.
- **Touch.** iOS raises the keyboard only for a focus made while a touch is
  dispatched, and eframe focuses its text agent in the frame after egui
  focuses a field. A tap, one touch that egui will take as a click, that ends
  on an enabled, editable text field therefore focuses the text agent at
  once, from a capture `touchend` listener. The field is the topmost
  interactive element at that point in the last pass, as egui decides
  clicks. eframe's `pointerup` handler focuses the canvas, so the same tap's
  touch `pointerup`, where a browser sends it after `touchend`, stops in the
  document's capture phase; egui takes the release from eframe's `touchend`.
  In the pass
  in which egui takes focus from the tap, its text edit emits no IME output
  yet, so the layer supplies the field's rectangle and eframe keeps the agent
  focused. Where a popup that egui draws over a field comes earlier in the
  tree, the layer still takes the field for the tap's target, egui gives the
  popup the click, and the keyboard opens and closes again.
- **Native fields.** A field shown with `NativeTextEdit` carries
  `data-stobie-native`, spelling, autocorrect and sentence capitalisation
  (unless the application turns them off). A tap on it moves its element
  out of the transparent container into a visible host over the canvas,
  leaving a hidden placeholder in its place. Until the next pass it takes no
  pointer input, so eframe's `touchend` still finds the canvas under the tap
  and releases egui's press. Its layer parent owns it through
  `aria-owns`, so it stays in its region. The layer styles it from the `Ui`'s
  visuals and font, with text under 16 CSS pixels scaled so that iOS does not
  zoom, and focuses it inside the gesture, with the caret where egui's
  layout puts the tap. egui focuses the field through AccessKit and, while
  the field has DOM focus, counts the application focused. It stops drawing
  the text, caret and selection while the field is active. Edits reach egui
  as `SetValue`, as in other layer fields. The field returns to the layer
  when it loses DOM focus to anything but the canvas, or egui's focus leaves
  it; when eframe focuses the canvas for a touch while egui keeps the field,
  the field takes focus back. Only a tap activates it:
  mouse clicks, Tab and assistive technology use it as an ordinary layer
  field. While it is active, window and visual viewport resize and scroll
  events move its host without a pass, and popups that egui draws over it are
  hidden beneath it. Return in a single-line native field does not reach
  egui. The [plan](../plans/ios-native-text-plan.md) records
  the design.
- **Shortcuts.** An action button's registered shortcut is its
  `aria-keyshortcuts`. In the browser a Cmd shortcut lists both forms, such
  as `Control+S Meta+S`; native AccessKit gives the platform's form.
- **Limits.** The layer is positioned from the canvas's client rectangle on
  each pass. A page that scrolls, or transforms an ancestor, without a pass
  leaves it misaligned until the next pass. It is not a substitute for a text
  editor's selection and caret semantics.

| `UiRole` | AccessKit role | ARIA in the layer |
| --- | --- | --- |
| `Application` | egui root | `application` on the canvas, named by the application |
| `ApplicationBar`, `Toolbar` | As the recipe sets it | `toolbar` when its node has the AccessKit toolbar role; otherwise flattened |
| `Button` | `Button` | `button` with `aria-keyshortcuts` for its shortcut; a toggle adds `aria-pressed`; a disabled control adds `aria-disabled` and its reason in `aria-description` |
| `NavigationItem` | `Button` with current state | `button` with `aria-current` |
| `RadioButton` | `RadioButton` | `radio` with `aria-checked` |
| `ComboBox` | `ComboBox` | `combobox` with its value as text and `aria-expanded` (`false` when unset) |
| `TextInput`, `MultilineTextInput` | `TextInput`, `MultilineTextInput` | Focusable `input` or `textarea` (implicit `textbox`) whose `value` is the current text |
| `Slider` | `Slider` | `slider` with `aria-valuenow`, `aria-valuemin` and `aria-valuemax` |
| `Tab` | `Tab` in the dock's `TabList` | `tab` with `aria-selected`, inside `tablist` |
| `Splitter` | `Splitter`, valued in percent | Focusable `separator` with orientation, `aria-valuenow` (10–90) and `aria-valuemin`/`aria-valuemax` |
| `Pane` | `Region` named by the tab title | `region` landmark |
| `Viewport` | `Canvas` | `group` with `aria-roledescription="viewport"` |
| `ScrollArea` | egui container | Flattened |
| `ResultRow`, `ThumbnailCell` | `ListBoxOption` inside `ListBox` | `option` with `aria-selected`, `aria-posinset` and `aria-setsize`, inside `listbox` |
| `Status` | `Status`, live polite | `status`, a polite live region. A readout that follows the pointer declares live off (`aria-live="off"`) |
| `StatusChip` | `Label` | Text; never a live region |
| `Section` | As the recipe sets it | `group` when named; otherwise flattened |
| Section heading (no `UiRole`) | `Heading`, level 2 | `heading` with `aria-level` |
| Content text (no `UiRole`) | `Label` | Text |

Other AccessKit roles map to their ARIA equivalents, such as `checkbox`,
`menuitem` and `dialog`. Unknown roles become generic elements.

`tools/aria-layer-browser.mjs` checks each application in Chromium:

- the CDP accessibility tree against reviewed expectations in
  `tools/aria-layer-expectations.json`;
- axe-core, with no serious or critical violation;
- an idle application presents no frame and writes nothing;
- hover frames write nothing;
- a coordinate click at an element's centre changes the same control and
  writes fewer changes than the layer has elements;
- every enabled clickable control shown on the canvas is the topmost element
  at its centre, as Lantern computes it, and the canvas receives a real
  pointer's moves at a hit target without hearing it leave;
- a plain Playwright `click()` activates enabled controls in each
  application;
- activation through a click event, `element.click()` and Playwright's forced
  coordinate click gives the same outcome as a physical click. In Record Desk
  this is confirmed through application-interface facts. A disabled control
  writes nothing, and a capability hidden in a pane has no element there;
- Tab keeps DOM focus on the canvas and moves `aria-activedescendant`;
- in Record Desk, focusing a text field through the layer then typing edits
  it, `locator.fill()` edits a single-line and a multiline field as typing
  would, a status change reaches its polite live region, and destroying the host
  restores the canvas;
- in Record Desk with touch, a tap on the title focuses eframe's text agent
  before the next animation frame. A tap on the native notes field focuses
  its element there, shown and topmost over the field and outside the layer
  container, with one notes textbox in its region. A later drag on it is not
  claimed for egui, and it takes focus back from the canvas while egui keeps
  it. Inserted text reaches egui, the active field presents no frames while idle, and a tap on the
  title returns it to the layer.

This proves the browser accessibility tree only. It is not screen-reader
qualification. `tools/aria-layer-orca/run.sh` runs Record Desk under actual
Orca in a container; the
[integration evidence](../accessibility-integration-report.md#browser-aria-layer-with-orca)
records its result and limits.

## Review checks

Inspect the current snapshot by stable action, role, pane or domain reference.
Require finite, positive node rectangles within the root; unique IDs; existing
parents; and disabled reasons only on disabled nodes. Compare the snapshot with
the AccessKit tree using the built-in parity audit. These must agree:

- role, name and enabled state;
- selection and description;
- click and adjust actions;
- a status's live setting;
- a row's list box container;
- bounds.

Keyboard proof is not optional for buttons, tabs, splitters, bounded result
selection or viewport alternatives. Long visible labels may elide, but their
semantic names and tooltip/description must retain the complete text. Automated
tree inspection may verify adapter input but cannot replace actual
assistive-technology use. Report every unqualified or unavailable platform
accurately rather than inferring support from compilation or snapshots.

## Qualification evidence

Define observable acceptance before a qualification run: the exact source and
binary, OS/desktop/session, graphics backend, AT and adapter versions; the
representative [workflow](../accessibility-integration-report.md#qualification-workflow);
and the expected names, focus, state, actions and dynamic announcements. Retain
step-linked observations, failures and artefact identities. Actual Orca must
consume platform accessibility events while the workflow operates Stobie;
retain its speech output/transcript or a recording (or the equivalent braille
output). A tree walk, adapter compilation or application-generated speech alone
does not establish that path. A speech-generation log alone also does not prove
audible delivery when audible output is part of the claimed result.

Automated input and capture can provide actual-AT evidence. Do not require a
human operator solely because a test is automated; require a human observation
only when an explicitly agreed criterion calls for it or the needed observation
cannot be obtained otherwise. A missing observation calls for safe in-scope
investigation and an explicit unproved result, without inventing a new approval
gate. Existing authority still governs live side effects. Limit qualification
to the environment and behaviour actually observed; a virtual-display result
does not qualify an untested desktop or local seat.

The retained qualification matrix and historical manual observations remain
unchanged by this contract. Requalification needs new evidence and a deliberate
report update; this guidance neither upgrades partial results nor extends
platform support.
