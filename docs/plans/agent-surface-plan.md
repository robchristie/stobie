# Agent surface and the ARIA layer

Status: complete
Delivery: [PR #131](https://github.com/robchristie/stobie/pull/131)

A chat application built on Stobie, with long settings screens, a
conversation list, a composer and confirmations, met a set of gaps in what
agents, host tests and the browser ARIA layer could do, and worked around
each locally:

- an agent could not reach a field below the fold of a region observed only
  in view: `discover` did not find it, the protocol had no reveal, and one
  `set-text` reported completed while the field stayed empty; host tests
  could not act on far-off controls either, and used a very tall harness;
- `discover` and `invoke` straight after a page loaded answered
  `missing_observation` until the first publication, and only `wait`
  retried;
- an agent invoking an unlisted row learnt no reason for the refusal;
- the composer's field had an opaque `presentation.<hex>` ID;
- one action and value presented by two controls at once (a run in two
  lists, a Cancel beside a dialog's close control) resolved as `ambiguous`;
- a heading row's controls ignored their bound action's own label and short
  label, and a code block's Copy control was renamed when a second block
  arrived, so a focused control's name changed under it;
- Back, and a dialog or confirm closing, returned focus to an opener that had
  gone, so egui published a focus outside the accessibility tree (kittest
  panicked; the browser layer lost focus);
- observation did not say which theme a page was drawn in;
- host tests had no supported harness sharing egui's context with the
  `AppHost`, and kittest's own 8-point margin moved breakpoints;
- an application was not told when its page was hidden;
- Web Push had no native double for host tests;
- the ARIA layer needed `style-src 'unsafe-inline'`, named eframe's text
  agent only from `aria-label`, and its browser check no longer tapped a
  plain egui text edit (eframe's own text agent) on touch;
- typing in headless Chromium, and filling password fields, had no documented
  route;
- a `TMPDIR` inside the checkout broke the sandboxed browser smokes, and a
  deep checkout's `TMPDIR` made a socket test exceed the Unix socket path
  limit.

## Decisions

- **Agents reveal a node by its semantic ID.** `REVEAL_CAPABILITY`
  (`stobie.reveal`) is an application-scope capability with a text argument,
  offered by every `AppHost` application that enables inspection. It scrolls
  the innermost scroll area holding the node until the node is wholly in
  view, or partly in view once scrolling can do no more, and is rejected with
  a reason when no pass presents the node, when it sits behind an open
  dialog, or when it cannot be scrolled into view; a reveal inside an open
  dialog scrolls the dialog. It lives in observation, so recipes, reading
  text and application nodes all get it through one `reveal_at` hook. The
  Rust client (`Client::reveal`), the Node client (`reveal`) and the CLI
  (`stobie reveal ID`) offer it, and discovery for an invocation that names
  an `id` that is not presented reveals it and discovers again, failing with
  the reveal's reason.
- **A reveal settles over a few passes.** egui places a node at its scrolled
  position one pass after next, so the host waits up to two passes for the
  node's rect to settle and scrolls again at most four times. A node within
  half a point of its clip counts as in view.
- **Set-text completes only when the text applied.** The host compares the
  field's value in the published tree with the edit and checks that the pass
  recorded it; an edit that did not apply, for example because its field
  vanished first, is rejected with a reason instead of completing. A
  single-line field that already holds the text with its line breaks turned
  into spaces, as egui turns them, took it.
- **Stale-target rejections stay.** An invoke made as the view re-presents is
  still rejected with `stale_target` or "is no longer presented": loud, typed
  and retryable.
- **Controls out of view stay out of observation.** Visible-only regions and
  the observation budgets rely on leaving wholly clipped controls and reading
  text out; revealing by ID is the route to them.
- **Clients wait for the first publication.** `observe`, `query` and
  `discover` in the Rust and Node clients retry `missing_observation` until
  their deadline, as `wait` already did.
- **Host tests reveal through `AppHost::reveal`.** It works in any harness;
  `AppHost::reveal_outcome` reports the result.
- **Every control that presents an action is its own binding.** The host no
  longer folds two controls presenting the same action and value into one
  binding, so each can be queried, discovered and invoked by its own ID.
  `CapabilityDescription::performs_same_as` says when two bindings perform
  the same thing: the same capability, pane, argument, target meaning,
  availability and semantic invocability; text entry, links and copying
  never count. `select_invocation_target`, the Rust client's `discover_one`
  and capability-enabled wait, and the Node client's `invoke` resolve
  equivalent bindings to the least semantic ID unless the caller named one
  of them. Controls that do different things are still `ambiguous`.
- **A header control uses its bound action's own labels.** `HeaderControl`
  measures, shows and names its control from `BoundAction::visible_label` and
  `accessible_name`, so `with_label` and `with_short_label` take effect in a
  heading row and the navigation header.
- **A code block's Copy name does not change as blocks arrive.** The first
  block's control is named by the label alone and later blocks by the label
  and their position ("Copy code", "Copy code 2"); `CodeCopySpec::name`
  keeps its signature and ignores the count.
- **An application can name its composer.**
  `PresentationContext::composer_with_id` and `Frame::composer_with_id` give
  the field the application's ID and its controls that ID with `.leading`,
  `.trailing` and `.send`. `composer` keeps its scoped IDs. The ID is not
  derived from the send action, because several composers can share one.
- **A hidden argument is refused without a reason.** Hidden means the
  application does not present that argument, so a reason would reveal what
  is hidden. An application that wants the refusal explained returns
  `Availability::Disabled` with a reason from `Application::availability_with`.
- **Focus returns to the nearest control when its opener has gone.** Back, a
  closing `Frame::dialog` and a confirm return focus to their opener only
  while the current pass presents it as an enabled control in the
  accessibility tree; otherwise to the control on the opener's layer whose
  centre is nearest where the opener's was (a removed row hands focus to the
  row that took its place, or the row before it), and otherwise nowhere.
  Back also restores focus when a level closed under the focused control.
- **The host never publishes a focus outside the accessibility tree.** When
  no modal is showing and an update names a focused node it does not
  contain, the host publishes the root as focused for that pass.
- **Observation carries the drawn theme.** The host adds the framework facts
  `stobie.theme` (`light` or `dark`) and `stobie.contrast` (`standard` or
  `high`), `THEME_FACT` and `CONTRAST_FACT`, after the application's own
  facts, from the preferences and system appearance the pass was drawn
  with. They do not count against the application's 256-fact limit.
- **Host tests run an application as eframe does.** The `test-support`
  feature exposes `testing::host_harness` (and `host_harness_with`), which
  runs an `Application` on an `AppHost` in the egui_kittest harness's own
  context, so sent intents and repaint requests wake the harness. Each step
  reports visibility and applies sent intents as `App::logic` does, and runs
  no UI pass while hidden. It avoids kittest's `eframe` feature, so
  browser-only applications can use it. egui_kittest (MIT OR Apache-2.0,
  already the crate's test harness) becomes an optional dependency.
- **The harness lays the application out at exactly its size.** It presents
  over the whole viewport instead of kittest's 8-point margin, so a
  breakpoint such as `PaneWidthClass`'s 720 points flips at the harness
  width.
- **Visibility is a hook with a value.** `Application::visibility_changed`
  receives `HostVisibility` once per change, before the intents sent since,
  and `AppHost::visibility` reads it. It follows egui's
  `ViewportInfo::visible()`: a hidden page, or a minimised or occluded
  window; losing focus is not hiding. In the browser the host listens for
  `visibilitychange` itself (removed on `destroy()`), because eframe repaints
  only when canvas or text-agent focus changes. The wake records no new
  `HostRepaint` variant, which would break exhaustive matches.
- **Web Push has a native double.** Under `test-support`,
  `web_push::PushDouble::install` stands in for the browser on the test's
  thread: it records each call as a `PushCall`, answers `start` and `refresh`
  with a `State` the test sets, and leaves `Subscribed`, `Unsubscribed`,
  `Failed` and `Route` to `deliver`. Without it, native builds still report
  push unsupported.
- **The ARIA layer styles itself through the CSSOM.** Element styles are set
  through `style.cssText`, and the hit-target and native-field rules live in
  a constructed `CSSStyleSheet` adopted into `document.adoptedStyleSheets`,
  so the layer runs under `style-src 'self'` with no `'unsafe-inline'` and
  no nonce. eframe's text agent and Stobie's startup loader already style
  through the CSSOM; a page allows only its own inline `<style>` (the
  template's, by hash or as a file). A browser without adoptable stylesheets
  falls back to `<style>` elements. The ARIA layer check serves every page
  under that policy and fails on any reported violation.
- **eframe's text agent is named as a browser names the field.** The layer
  takes the agent's name from the field's `aria-labelledby` targets before
  its `aria-label`, so a field labelled with `Response::labelled_by` names the
  agent after its visible label.
- **Touch coverage of eframe's own text agent uses the Gallery's password
  field.** It is egui's own `TextEdit`, never replaced by a native field, so
  a tap on it must focus eframe's text agent inside `touchend`; Record
  Desk's title, now a native field, keeps covering the native route.
- **Agents fill password fields by typing.** `stobie.set-text` still never
  sets a password. An agent focuses the field with a click or tap and types
  with the keyboard; the Gallery check proves the masked value grows by what
  was typed.
- **Typing in Chromium needs a font, not a headful browser.** Reproduced:
  without a font that fontconfig finds, Chromium drops text typed into
  hidden inputs, headless or headful, and with the repository's fonts.conf
  headless typing works through every route. `docs/agent-cli.md` documents
  `FONTCONFIG_FILE` and `set-text`, which needs neither.
- **Sandboxed smokes get a private `TMPDIR`.** The UI sandbox sets
  `TMPDIR=/tmp`, its private directory, so a host `TMPDIR` inside the
  checkout no longer breaks it; the outside-app smoke keeps its application
  outside the checkout whatever `TMPDIR` says. `tools/test-rust.py` keeps the
  checkout's `.tools/t` only while its path leaves room for the tests' Unix
  sockets, and otherwise uses a private directory under `/tmp`, removed after
  the run.
- **The dark theme's selection colour (from the layout recipes).** The
  layout recipes darkened Dark's `colour.palette.selection` from `#164d52`
  to `#123f43` when hover gained its own colour, because muted text
  (`#9daab3`) on the old fill measured 3.98:1 and on the new one measures
  4.86:1, above the 4.5:1 floor the token tests now hold. The other themes'
  selection colours are unchanged.

## Delivery

| Part | Status | Evidence |
| --- | --- | --- |
| Reveal, set-text that fails loudly, clients that wait | Landed | `host/reveal_tests.rs`; `stobie-client` tests; `tools/tests/application-client.test.mjs`; `tools/stobie-cli-smoke.mjs` sets a field below the fold in a 1280x400 browser through both clients |
| Bindings, header labels, Copy names, composer ID, hidden refusal | Landed | `host/row_tests.rs`, `host/dialog_tests.rs`, `presentation/heading_tests.rs`, `host/copy_tests.rs`, `presentation/composer_tests.rs`; `stobie-protocol` and both clients' equivalence tests |
| Focus return and the theme facts | Landed | `host/focus_return_tests.rs` (at the base revision, 7 of its first 9 tests failed, 5 with kittest's focus panic); `host/theme_fact_tests.rs` |
| Host harness, visibility and the Web Push double | Landed | `testing/tests.rs`; Live Feed's host tests and browser smoke (hide and show with nothing focused) |
| ARIA layer under `style-src 'self'`, text agent naming, touch and password coverage, sandbox `TMPDIR` | Landed | `tools/aria-layer-browser.mjs` under the policy for the Lab, Gallery and Record Desk; `aria/tests.rs`; `tools/tests/test_xvfb_display.py`; `tools/tests/test_rust_tests.py` |

## Evidence

Each gap was reproduced at the base revision (`f1d535e`) before its fix: a
host test that fails there, a browser run, or a source line.

- Reveal: a visible-only field below the fold was absent and `set-text` on
  its ID gave `no_match`; a `set-text` on a field gone before the edit
  applied completed with no effect; a kittest click on a clipped control did
  nothing. The CLI smoke asserts the notes field is absent before the
  reveal.
- Bindings: the second control presenting an action and value had no
  binding of its own; header controls measured and were named as plain
  icons; the first Copy was renamed "Copy code 1".
- Focus: the new tests panicked with "Focused ID ... is not in the node
  list" for Back after the opener left the list and for a confirm that
  removed its own opener.
- Harness: the base harness laid the application out at
  `[[8 8] - [712 472]]` in a 720x480 harness; intents sent from the
  application did not wake it.
- CSP: under `style-src 'self'` plus its own hash, Record Desk's layer never
  appeared, with style-src-attr and style-src-elem violations.
- `TMPDIR`: the Lab ARIA check failed with EROFS for a `TMPDIR` inside the
  worktree and ENOENT for one under the host's `/tmp`; the socket test
  failed with "path must be shorter than SUN_LEN" from a deep worktree.

## Size

Measured with `cargo xtask web-size` in the production stage.

## What an application changes

- Use `testing::host_harness` under the `test-support` feature instead of a
  copied harness, and drop size padding added for kittest's margin.
- Use `AppHost::reveal` in host tests instead of a tall harness, and drop
  wheel-scrolling or retry loops for `missing_observation` in smokes.
- Implement `Application::visibility_changed` instead of listening for
  `visibilitychange`; read `stobie.theme` instead of publishing a theme fact
  (the host's fact overwrites an application's fact under the same key).
- Use `PushDouble` instead of recording Web Push calls through its own seam.
- Name the composer with `composer_with_id`; a second action kept only to
  avoid `ambiguous` may go.
- A page with a strict `style-src` drops `'unsafe-inline'` and allows only
  its own `<style>`.
- Discovery now lists `stobie.reveal`, every observation carries the two
  theme facts, and an action presented by two controls has two bindings;
  tests that count capabilities, facts or bindings exactly change.
- With several code blocks the first Copy control is named "Copy code", not
  "Copy code 1"; a `HeaderControl` whose bound action has `with_label` or
  `with_short_label` is named and measured by them; and a `set-text` whose
  field did not take the text is rejected where it used to complete. Tests
  that assert the old names or outcomes change.

## Limits

- A reveal scrolls only the innermost scroll area; a node clipped by an
  outer one too may settle partly in view or fail. A virtualised row never
  laid out is "not presented".
- Equivalent bindings are recognised within one discovery page of up to
  eight; beyond that they are reported ambiguous as before.
- Code-copy numbering is by position, so a block inserted before another
  renumbers the later ones; appending never does.
- The focus fallback is chosen by geometry on the opener's layer; an
  application cannot name one. The ARIA layer's fallback was checked by
  projecting the AccessKit update natively, not in a browser.
- Native visibility depends on winit's occlusion support; no native smoke
  hides a window.
- Safari before 16.4 lacks adoptable stylesheets, so a strict `style-src`
  blocks the layer's fallback `<style>` there. Only `style-src` was
  checked. The layer's accessible-name rule approximates the browser's.
- The password route is proven with a tap and the keyboard in the Gallery;
  a mouse click then typing is documented but not run on a password.
