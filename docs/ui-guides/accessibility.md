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
The Emuella viewer is not hosted by `AppHost` and has no layer. The
[plan](../aria-mirror-plan.md) records why AccessKit, not the semantic
snapshot, is the source.

- **Source.** An egui plugin takes `platform_output.accesskit_update` in its
  output hook, before eframe's web runner discards it. egui generates the tree
  only on passes it already runs, and the layer never requests a repaint.
- **Elements.** The layer is a `div` after the canvas. It has `opacity: 0` and
  `pointer-events: none`, but it is neither `display: none` nor `aria-hidden`.
  Each element is absolutely positioned at its node's bounds, so a pointer at
  an element's centre reaches the canvas and the same control. A node's
  `author_id`, which Stobie recipes set to the semantic ID, becomes the element
  ID `stobie-a11y:<semantic ID>`. Unnamed layout containers are flattened into
  their parent.
- **Updates.** Each pass is diffed with the previous one by element ID. Only
  changed attributes, text, geometry and child order are written, and a pass
  whose tree is unchanged writes nothing. Hover repaints, for example, write
  nothing.
- **Root.** The canvas becomes `role="application"`, named by the application
  identity, and owns the layer through `aria-owns`. eframe's hidden text agent
  is named "Text entry".

The layer is read-only in this increment. Activation through the layer, focus
synchronisation and the values of text inputs and splitters are planned work.

| `UiRole` | AccessKit role | ARIA in the layer |
| --- | --- | --- |
| `Application` | egui root | `application` on the canvas, named by the application |
| `ApplicationBar`, `Toolbar` | As the recipe sets it | `toolbar` when its node has the AccessKit toolbar role; otherwise flattened |
| `Button` | `Button` | `button`; a toggle adds `aria-pressed`; a disabled control adds `aria-disabled` and its reason in `aria-description` |
| `NavigationItem` | `Button` with current state | `button` with `aria-current` |
| `RadioButton` | `RadioButton` | `radio` with `aria-checked` |
| `ComboBox` | `ComboBox` | `combobox` with its value as text and `aria-expanded` (`false` when unset) |
| `TextInput`, `MultilineTextInput` | `TextInput`, `MultilineTextInput` | `textbox`; multiline adds `aria-multiline` |
| `Slider` | `Slider` | `slider` with `aria-valuenow`, `aria-valuemin` and `aria-valuemax` |
| `Tab` | `Tab` in the dock's `TabList` | `tab` with `aria-selected`, inside `tablist` |
| `Splitter` | `Splitter` | `separator` with orientation and value attributes |
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
  writes only what changed;
- in Record Desk, a status change reaches its polite live region.

This proves the browser accessibility tree only. It is not screen-reader
qualification.

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
