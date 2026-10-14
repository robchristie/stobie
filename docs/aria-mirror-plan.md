# Hidden ARIA layer for hosted web applications

Status: active
Next action: Land increment 1, the read-only mirror with role mapping, incremental updates and live status.

## Outcome and boundaries

A Stobie web application renders into a WebGPU canvas, so the browser has no
accessibility tree for it. Screen readers and general browser agents
(Playwright role locators, accessibility-tree agents) can only click blind
coordinates. Every `AppHost` web application now gets a framework-owned hidden
DOM layer with ARIA semantics, without application code. Applications can opt
out. The Lab, the Gallery and Record Desk get it through `AppHost`. The Emuella
viewer is not on `AppHost`, so it is out of scope.

The version 1 inspection protocol, capability IDs, semantic IDs, UI snapshot
baselines and existing smokes stay unchanged. `?automation=1` remains the only
way to enable the automation dispatcher. The layer offers no route beyond what a
user can already do with a pointer or keyboard.

## Source decision

The two candidate sources were the semantic `UiSnapshot` and egui's own AccessKit
`TreeUpdate`.

| Criterion | `UiSnapshot` | AccessKit update |
| --- | --- | --- |
| Authoritative model | Agent-facing observation | The framework tree that native assistive technology already consumes; the parity audit keeps it consistent with the snapshot |
| Stable identity | Semantic IDs | Stobie sets `author_id` to the semantic ID on every audited control and on content labels. Other nodes use egui IDs, which are stable for unchanged UI |
| Values and text | Neither: no value field, and content, headings and property rows publish no node | Text edit values, numeric slider and splitter values, and every label's text |
| Native AccessKit and application interface | Needs a separate role and state mapping | Identical to native by construction. Activation is an AccessKit action request, which native assistive technology already uses |
| Cost | Already computed every pass | Tree generation on passes that already run. egui 0.36.1 always compiles AccessKit, so there is no new crate |

**Decision: the AccessKit update is the only source of the DOM tree.** The
snapshot stays the agent-facing model and is not read by the layer. Adding body
text to the snapshot would change baselines and agent queries. Projecting the
snapshot would also need a second role and state mapping beside the native one.

The update is reachable without patching eframe. `egui::Plugin::output_hook`
runs at the end of every `Context::end_pass`, before eframe's web runner
discards `platform_output.accesskit_update`. The accessibility report's earlier
"cannot intercept" finding covered only the runner's private path. On the web,
the host calls `Context::enable_accesskit`. AccessKit generation then runs only
on passes that egui already runs, so an idle application costs nothing.

Gaps in Stobie's AccessKit tree are fixed at the producing recipe or host, not
in the mirror, so that native assistive technology gains the same fixes:

- status lines become `Status` with polite live state;
- dock panes become named regions;
- headings get the heading role;
- result rows get a list box container;
- combo boxes get their current value.

The parity audit is extended where a UiRole gains an expected AccessKit form.

## Design

- **Where.** The projection and diff are platform-independent and unit-tested
  (`stobie_ui_egui::aria`). The DOM applier is a wasm-only plugin
  (`host/aria_dom.rs`) that `WebHost::start` installs before the first pass.
  Applications opt out with `Application::web_accessibility_layer`. It uses only
  `web-sys` features that eframe already enables.
- **Container.** The layer is a `div` sibling placed after the canvas, absolutely
  positioned over the canvas. It has `pointer-events: none` and `opacity: 0`, so
  it does not render, but it is neither `display: none` nor `aria-hidden`. Each
  element is absolutely positioned at its node's bounds. A coordinate click at
  an element's centre therefore reaches the canvas and the same egui widget.
  Real mouse and touch input is unaffected.
- **Root.** The canvas becomes `role="application"`, named by the application's
  identity. It owns the layer through `aria-owns`.
- **Diffing.** The update is converted into a desired element tree keyed by DOM
  ID: `author_id` when present, otherwise the egui node ID.
  - Unnamed generic containers are flattened into their parent.
  - Attributes, text, geometry and child order are compared with the last
    applied state, and only differences are written. A frame whose tree is
    unchanged writes nothing.
  - Reordered children keep the longest run in their old relative order; only
    the others move.
  - Discarded passes are skipped.
  - The layer never requests a repaint.
  - Tests count writes with a `MutationObserver`, so the product needs no
    counter.
- **Focus.** DOM focus stays on the canvas, and the canvas's
  `aria-activedescendant` names the focused node's element. eframe reads
  keyboard input only while the canvas itself (or its text agent) has DOM
  focus, and it refocuses the canvas after each pass. Moving DOM focus into the
  layer would therefore break canvas keyboard and IME input. A focused text
  edit moves DOM focus to eframe's text agent, as today.
- **Activation.** A `click` on a layer element (from a screen reader's default
  action, `element.click()` or Playwright's `dispatchEvent('click')`) queues an
  AccessKit `Click` request for that node. A plugin `input_hook` adds it to
  egui's next input, with one recorded repaint request. egui marks a widget
  clicked only when it is enabled and senses clicks. That click then follows
  the existing path: `Frame` triggers, and the host rechecks availability
  before `perform`. A disabled control does nothing and exposes
  `aria-disabled` with its reason as `aria-description`. A hidden capability
  renders no widget, so it has no element.
- **Playwright route.** The layer's elements cannot be hit targets without
  intercepting real pointer input. Playwright's default actionability check
  therefore reports that the canvas intercepts pointer events. There are two
  supported routes, documented in the agent guide:
  - `locator.dispatchEvent('click')` is the semantic route;
  - `locator.click({ force: true })` is a physical click at the control's
    centre.

  Both are tested.

## Role mapping

The accessibility guide documents the full table, from `UiRole` through
AccessKit to ARIA. Summary:

| Element | ARIA |
| --- | --- |
| Application root | `application` on the canvas |
| Buttons and toggles | `button`; toggles add `aria-pressed` |
| Navigation item | `button` with `aria-current` |
| Radio button | `radio` |
| Combo box | `combobox` with its value |
| Text inputs | `textbox`, with `aria-multiline` for multiline input |
| Slider | `slider` with its values |
| Tabs | `tab` inside the dock's `tablist` |
| Splitter | focusable `separator` with its value |
| Pane | `region` named by its title |
| Viewport | `group` with an `aria-roledescription` |
| Result rows and thumbnails | `option` inside a `listbox`, with position and set size where known |
| Status | `status`, a polite live region |
| Status chip | Plain text; never a live region |
| Section | `group` |
| Headings | `heading` |
| Content | Text |

## Increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 1. Read-only tree: role mapping, incremental updates, live status, opt-out | In progress | Projection and diff unit tests. The parity audit is extended to status, pane and list containers, with a hosted Lab audit. `tools/aria-layer-browser.mjs` passes for all three applications. |
| 2. Activation and focus synchronisation | Planned | |
| 3. Values, documentation and the general-agent probe | Planned | |

Each increment lands through review in order, rebased on the previous merge.

## Acceptance

- In Chromium, Record Desk, the Gallery and the Lab expose correct roles,
  names, states and values. Evidence:
  - CDP `Accessibility.getFullAXTree` compared against reviewed expectations;
  - axe-core, pinned in `package-lock.json`, with no serious violations, or
    with justified exclusions.
- `element.click()` and the documented Playwright route invoke the same
  capability as a physical click.
  - A disabled control reports its reason and does nothing.
  - A hidden capability has no element.
- Focus moves are reflected through `aria-activedescendant`, and status changes
  are announced through a polite live region. Canvas keyboard input still
  works, and the existing keyboard smokes pass unchanged.
- DOM writes track tree changes, not frames. An idle application performs no
  DOM writes and no repaints.
- A fresh agent, using only a browser tool and accessibility-tree interaction,
  does the following in Record Desk. The outcome is verified through the
  application interface, and its stumbles are fixed.
  - selects a record;
  - toggles Reviewed;
  - applies the change, then undoes it.
- The PR reports the startup benchmark (Lab and Gallery) and WASM sizes, before
  and after.
- `cargo xtask verify`, including the production stage, passes.
- If Orca with Chromium can run on this host, a short Record Desk check records
  what was heard. Otherwise the documents state that limit. Automated
  accessibility-tree proof is not screen-reader qualification.

## Findings

- Semantic fixes at the producing recipe also help native assistive technology:
  - status lines are `Status` with polite live state;
  - dock pane bodies are named regions;
  - section headings are level-2 headings;
  - result rows, thumbnails and Record Desk records sit in list boxes with
    position and set size;
  - the Gallery's appearance combo boxes are named.
- The Lab's coordinate readouts and decoding count change with the pointer or
  worker progress. They declare live off, so they do not flood a screen reader.
- CDP reports neither set position nor a non-focusable separator's value. The
  check reads position from the DOM. Splitter values need focusable elements,
  which arrive with focus handling in increment 2.
- axe-core reports only moderate best-practice findings: no `main` landmark, no
  level-one heading, and content outside landmarks.

## Later work

- Editing text through the layer (handing focus to egui's text agent) is the
  next increment unless increment 3 shows that it fits cleanly.
