# Layout and recipes

Status: complete
Delivery: [PR #127](https://github.com/robchristie/stobie/pull/127)

A chat application built on Stobie, with a household's settings forms, a
conversation list and a routine editor, met a set of layout and recipe gaps
and worked around each locally:

- a heading and a control could not share a row (the heading took the whole
  row and the control was squeezed to a pixel or pushed out of sight);
- a scroll area pushed the reader down when older rows arrived above them,
  and nothing kept a footer at the foot of a short pane;
- bounded text scrolled out of view failed the text audit outside a dock
  pane, and partly visible headings and labels in a dock pane published no
  content;
- local times had only sentence-initial relative days ("Today at…") and no
  way to know when the local day changes;
- there was no choice or segmented control, a bound action's visible label
  was always its action's label, and short labels ("Tue") could not be
  presented;
- a hovered row looked selected (one colour served hover, quiet hover and
  selection), muted text on the selection fill was not validated, a hovered
  quiet button looked toggled on, a disabled toggle lost its on state and
  icon-only controls were below a touch target;
- there was no labelled text field, no avatar that is itself a focusable
  target, no Copy on a Markdown code block, and Copy inside a message bubble
  in a long scrolled conversation never settled;
- the navigation recipe's header held only Back, a title and the detail
  toggle, and navigation panes gave their content the whole width;
- an application followed Dark whatever the system said, and the browser's
  text size never reached `font_scale`;
- a confirm had no destructive emphasis, and a sheet had no footer action
  that stays in reach while a long form scrolls;
- robchristie/stobie#104's three gaps: integer arguments beyond what a
  JSON number carries exactly, a refused argument's binding, and hidden
  controls' IDs.

## Decisions

- **Heading rows.** `PresentationContext::heading_row` (and
  `Frame::heading_row`) presents a heading with `HeaderControl`s at its end.
  The controls keep their whole size and the heading takes the room left,
  eliding with its full text kept for assistive technology. When the heading
  would have less than its least width (its whole text, or three minimum hit
  targets), the controls take an end-aligned line of their own, running on to
  further lines, so no control is squeezed or pushed out of sight. Icon-only
  controls take the touch target; labelled controls keep the control height,
  centred on the same line.
- **The navigation header slot.** `Frame::navigation_with` takes
  `NavigationOptions`: an avatar, a subtitle under the title and header
  controls of the content pane, laid out as a heading row between Back and
  the detail toggle. Each pane's body is inset by the header's own inline
  spacing unless `with_body_inset(pane, false)` turns it off, so content no
  longer touches a phone's edges.
- **Anchored scrolling and a pane footer.** `AnchoredScrollArea` presents
  keyed rows (`ScrollRows::row`) and keeps the first wholly visible row at
  the same place when rows are inserted above it, in either scroll
  direction, and keeps the end when the reader is at it with
  `stick_to_bottom`. `ScrollRows::footer` puts a block at the foot of a
  short pane and after a long list, measured in the same pass, with no
  spacer from the previous frame. A Load older control out of view stays
  invocable through its action.
- **Text in visible-only regions.** A bounded heading or label that is
  partly visible in a visible-only region publishes its content, and bounded
  text wholly out of view is not observed (unless it failed), so long
  scrolling pages outside a dock pane audit cleanly. Text outside any
  visible-only region is audited as presented, as before.
- **Local time.** `LocalTime::relative_day_in_sentence` and
  `relative_day_and_time_in_sentence` give "today at 3:30 PM" and "on
  Saturday at 9:41 AM"; `LocalTime::until_next_day` gives the time until the
  next local midnight on the zone's clock, counting 23- and 25-hour days at
  daylight-saving changes, for a single scheduled repaint.
- **Segmented choice.** `PresentationContext::segmented_choice` (and
  `Frame::segmented_choice`) presents a `SegmentedChoiceSpec`, `single`
  (a radio group, entered by Tab at its chosen option and moved by arrows)
  or `multiple` (toggles), of `ChoiceOption`s, each a bound action with an
  argument, its own label and an optional short label. Options wrap whole in
  a narrow column. `BoundAction::with_label` and `with_short_label` give one
  bound control its own name and visible label (the weekday on a shared
  "Runs on" action), and short labels are presented whole at least at the
  minimum hit size.
- **State colours.** Every theme has its own hover colour, distinct from
  selection and from the raised surface; `surface.hover` and the quiet
  button's hover use it, so a hovered row or quiet button never looks
  current or toggled on. Muted text on hover, selection and pressed fills
  is validated at 4.5:1, and a component that paints muted text on a fill
  too close to it turns it primary. A disabled toggle keeps its on state;
  disabled quiet chrome stays muted.
- **Icon touch targets.** An icon-only action takes a square target of
  `icon.touchTargetSize` (44 points) in both densities, with its visual
  chrome kept at the control height, centred; heading rows, the navigation
  header, the composer's Send and the code-block Copy fit it.
- **Labelled text field.** `PresentationContext::text_field` (and
  `Frame::text_field`) presents a `TextFieldSpec`: a visible caption that
  names the field, an optional hint, description and error (shown,
  described, marked invalid and announced), single or multiline with a
  row limit, and password fields that never publish or accept their text
  through observation or agents. Agents set a field's text by its ID.
  Stobie's own forms and the Gallery's dialogs use it.
- **Avatar items and strip.** `avatar_item` presents an avatar as one
  navigation item named by its whole caption, at least the touch target,
  selectable and disabled with a reason; `avatar_strip` lays a row of them
  in a horizontal scroll that publishes what it shows. An agent opens a
  person through their avatar.
- **Copy on code blocks.** `markdown_with_code_copy` presents Markdown with
  a named copy control in a strip above each code block, keyed by the
  block's position so a growing reply keeps each control and its
  confirmation; one block is named by the label and several by their
  numbers. Narrow columns wrap the code beneath its control.
- **A bubble's width while partly in view.** A message bubble hugs its
  content by measuring its painted shapes and keeping the width for later
  passes (the recipe for content that hugs its width: egui cannot know a
  width before layout, so measuring and refitting once is the design, not a
  missing primitive). The reported failure to settle was diagnosed: content
  out of view skips its paint, so a bubble partly in view measured only its
  visible part, such as a Copy below text scrolled away, narrowed, wrapped
  its text into view and measured again without end. A bubble now measures
  only while wholly in view and keeps its width otherwise; its texts' line
  counts and heights, recorded whether painted or not, still trigger a refit
  when the content changes.
- **System appearance and text size.** `UiPreferences::system()` follows
  the system's light or dark appearance and, in a browser, the text size:
  the page's root font size against 16 px and, in WebKit, the system body
  font size (which follows Dynamic Type on iOS) against 17 px, bounded to
  Stobie's range and read again when the page regains focus, is shown again
  or is resized. `Application::preferences` defaults to it.
  `UiPreferences::default()` is unchanged (Dark), so an application that
  names it keeps its look. The template's page follows the system's
  appearance too.
- **Destructive emphasis.** `ActionEmphasis::Destructive` draws normal
  chrome with the label, artwork and border in the error colour; a press
  keeps the hover fill, because the error colour on the selection fill
  measures 4.46:1 in Dark. Token validation checks the error colour on the
  raised and hover surfaces in every theme (at least 5.04:1).
  `ConfirmSpec::confirm_emphasis(ActionEmphasis::Destructive)` uses it.
- **A dialog's footer action.** `DialogSpec::footer(action)` presents a
  primary action at the footer's end, below the scrolling content and above
  nothing, so Save stays in reach on a long form in a sheet. It comes after
  the content and before Close in focus order, takes the dialog's
  availability, and a hidden footer reserves no room. `DialogResponse`
  reports it and `Frame::dialog` triggers it. The body's scroll is bounded
  to the room between the title and the footer, and the dialog's measured
  height is clamped to its room so an overflowing body settles.
- **robchristie/stobie#104.** `ActionArgument::try_integer` refuses an
  integer beyond ±(2^53 − 1), which a JSON number cannot carry exactly, with
  `InexactInteger`, and `ActionArgument::integer` panics on one in release
  builds too. A control whose argument its action's contract refuses stays
  discoverable with that value and refuses invocation, while the action's
  own binding still takes any value the contract allows. A hidden
  control's empty response is identified within its presentation, so two
  presentations hiding the same action and argument no longer share an
  egui ID.
- **Non-exhaustive enums.** `UiRole`, `TextOverflow`, `TextComponentKind`,
  `TextLayoutError`, `Invocation` and `AccessKitMismatch`, which have gained
  variants before, are `#[non_exhaustive]`. A read-only survey of the other
  known consumer, emuella-viewer at `f2ef8b1` (pinned to `8bfcdd6`), found
  no use of any of them, no glob import of Stobie's UI crate, and no
  exhaustive match on `ActionEmphasis` or literal of `DialogSpec` or
  `DialogResponse`, so the new variant and fields break neither it nor the
  chat application, which matches `UiRole` only with `==` and `matches!`.

## Design review contract

- **Task.** Settle a household's routines and conversations on a phone and a
  desktop: edit a routine in a sheet and save it while the form is scrolled;
  choose how it repeats and on which days; delete it from a confirm; read a
  conversation's header (avatar, name, status, Pin, Hide, Edit profile, the
  details toggle); open a pinned person from a strip; load older messages
  without losing one's place; copy a code block from a reply; fill a sign-up
  form with an error.
- **Hierarchy.** The heading or title first, its controls quieter at the
  row's end; the primary action (Save) the one filled control in its view;
  a destructive action visibly different from its cancel without
  out-shouting the question; hover quieter than selection; an error in the
  error colour next to its field.
- **Reference.** The existing approved fixtures at the base (`main`
  `745c10b`) for every changed story, and the platform conventions of a
  phone's settings forms and messaging applications (segmented controls,
  44-point targets, a sheet's fixed action).
- **States.** Normal, narrow (390 points), 150% text, high contrast, light
  and dark; selected, hovered, pressed, disabled with a reason, toggled on
  and off; error and hint on fields; an unavailable option; a long title
  that elides; controls that take their own line; a short pane and a long
  list; a partly visible bubble at the top of a scrolled conversation.
- **Exclusions.** Drop-down select controls, date and time pickers, a
  pull-to-refresh gesture, colour per destructive severity, and device
  checks.

Gallery stories: `heading/controls`, `navigation/header`, `scroll/load-older`,
`choice/segmented`, `form/text-fields`, `avatars/strip`, `markdown/code-copy`,
and the changed `dialog/sheet` and `dialog/confirm`; new fixtures
`heading-controls-phone-light`, `navigation-header-phone-dark`,
`scroll-load-older-dark`, `choice-segmented-light`,
`choice-segmented-phone-high-contrast`, `form-text-fields-dark`,
`avatars-strip-phone-light` and `markdown-code-copy-dark`.

## Critique

First pass, by a reviewer who saw only the task, the hierarchy, the reference
captures and the candidates (seven changed fixtures and the eight new ones):

- Noticed first: each story's heading or question, then its one filled
  action (Save) or its selected option; in the confirm, Delete is now an
  outlined error-coloured button that no longer out-shouts the question.
- Competes: a toggled-on, focused icon control beside a long title is louder
  than the title; a saturated avatar colour draws the eye ahead of the
  current person.
- Harder: in the rows story the icon-only Remove sat about 6 points below
  the labelled Open and Pinned, and in the composer the labelled Dictate sat
  5 to 6 points above Attach and Send (the 44-point targets took the row's
  height, and labelled controls kept the top); the labelled Edit profile in
  a heading row likewise sat above the toggle beside it. Rows, headers and
  composers are about 12 points taller. A code block's copy control takes a
  strip above the code. A bubble partly scrolled out of view at the top of
  `scroll/load-older` is drawn at the widest a bubble may be. The sheet's
  footer has no rule above it. Icon-only Pin and Hide do not say what they
  do. The high-contrast phone fixture of `choice/segmented` cuts the story
  off at the viewport, and its unavailable option's lighter fill reads as
  prominent.

Second pass, with the rationale: the misalignments are defects and were
revised once. A heading row, the navigation header and the composer now
place a labelled control at the control height centred on the place an
icon-only control's target gives the row (`control_place`), with host tests
in `heading_tests.rs` and `composer_tests.rs`; the Gallery's rows story sets
its `ui.horizontal_wrapped` row to the touch target, which the components
guide now tells an application mixing the two in its own row to do. The
re-critique of the revised captures accepted the alignment in every row,
composer and heading row. It noted that an icon-only control's chrome is
centred in its 44-point target, so the visible gap before it is wider than
the row's spacing where the chrome is narrower than the target (about 15
points against 8 in the rows story at 100%, even at 150%); that is the
target's cost, kept rather than letting targets overlap. The taller rows are
the cost of a 44-point target, which the task asks for. The copy strip keeps
the code's first line clear of the control at any width, with the control at
the block's top end where a reader looks for it.
The wide partly visible bubble is the settling fix's deliberate state (a
bubble measures only while wholly in view); it narrows when scrolled into
view. A sheet's footer is separated by spacing, as the confirm's is, and the
body scrolls beneath its fixed title above it. Icon meanings, the Gallery's
own clipping at a 900-point viewport and the disabled chrome are existing
treatments outside this change. No further revision.

Decision record:

```text
Task and reference: settle routines and conversations on a phone and a
  desktop (sheet, confirm, segmented choice, heading rows, navigation header,
  avatar strip, load older, code copy, sign-up form); the approved fixtures
  at main 745c10b and phone settings and messaging conventions
Behavioural checks: passed (host tests for every recipe; the reader's-place
  and text-size browser smokes)
Presentation checks: passed after one revision (control alignment in rows,
  headers and the composer); no semantic, AccessKit or text audit finding
Design decision: accepted
Main reason and remaining trade-off: each recipe replaces an application's
  workaround with a measured, observable control at a 44-point target; rows
  and headers are taller, a partly visible bubble is wide until in view, and
  a code block's copy takes a strip of its own
Exact candidate evidence links: tests/ui-snapshots/expected (the fixtures
  named above)
```

## Delivery

| Increment | Status |
| --- | --- |
| Local time in sentences and the next local day | Complete |
| Hover colour, muted text on fills, action states and icon touch targets | Complete |
| #104's three gaps and non-exhaustive enums | Complete |
| Partly visible and wholly clipped text in visible-only regions | Complete |
| System appearance and browser text size | Complete |
| Labelled text field | Complete |
| Heading rows, navigation header slot and inset pane bodies | Complete |
| Avatar items and strip | Complete |
| Segmented choice and per-control labels | Complete |
| Code-block Copy and the bubble's partly visible width | Complete |
| Anchored scroll area, pane footer and browser smoke | Complete |
| Destructive emphasis and a dialog's footer action | Complete |
| Gallery stories, fixtures, guides, critique and full verification | Complete |

## Evidence

- Host tests for each recipe: heading rows and the navigation header
  (`host/heading_tests.rs`, including a labelled control centred with an
  icon-only one); anchored scrolling, the footer, and bubbles that settle
  while partly in view and in a long conversation with Copy inside
  (`host/scroll_tests.rs`); segmented choice (`host/choice_tests.rs`);
  text fields (`host/text_field_tests.rs`); avatar items and the strip;
  code-block Copy; partly visible text (`host/content_tests.rs`); state
  colours and touch targets (`components/action_button.rs`,
  `xtask/src/tokens.rs`); the dialog footer (`host/dialog_tests.rs`: Save in
  reach and in the sheet while a long form scrolls at phone width, Tab order
  field, Save, Close, one save per click, an agent invoking it, a centred
  dialog's footer at its end, a hidden footer reserving nothing); local
  time, including daylight-saving days of 23 and 25 hours; system
  preferences; #104's integer, binding and ID tests.
- `tools/scroll-browser-smoke.sh`, run by `cargo xtask verify`, loads older
  messages in headful Chromium in both directions of scrolling and checks
  that the reader's row stays where it was.
- The text-size browser smoke checks that the page's root font size reaches
  `UiPreferences::system()`.
- Inspected snapshot baselines for every changed and new fixture under
  `tests/ui-snapshots/expected`, with no semantic, AccessKit or text audit
  findings.
- `cargo xtask verify`.

## Size

`cargo xtask web-size` on this branch against the sizes recorded at the base
by [`fonts-and-emoji-plan.md`](fonts-and-emoji-plan.md), in bytes:

| Package | WASM raw before | WASM raw after | WASM Brotli before | WASM Brotli after |
| --- | ---: | ---: | ---: | ---: |
| Gallery (browser production) | 13,044,438 | 13,280,625 (+236,187) | 3,566,333 | 3,611,928 (+45,595, +1.3%) |
| Lab (browser production) | 13,347,593 | 13,351,324 (+3,731) | 3,586,487 | 3,588,163 (+1,676) |
| `stobie new` template | 11,363,599 | 11,372,723 (+9,124) | 3,208,133 | 3,210,293 (+2,160) |

An application that uses none of the new recipes grows by about 2 KB
compressed; the Gallery's growth is mostly its seven new stories.

## What an application does on its pin move

- Add a wildcard arm to any exhaustive `match` on the six non-exhaustive
  enums.
- An application that relied on `Application::preferences` defaulting to
  Dark now follows the system; return `UiPreferences::default()` to keep
  Dark.
- Content in `Frame::navigation` panes is now inset; drop a local inset, or
  turn it off per pane.
- Replace local workarounds with the recipes: `heading_row` for a heading
  with controls, `navigation_with` for a header with an avatar, subtitle and
  controls, `AnchoredScrollArea` for loading older rows and a pane footer,
  `segmented_choice` and per-control labels for choices, `text_field`,
  `avatar_item` and `avatar_strip`, `markdown_with_code_copy`,
  `ActionEmphasis::Destructive` for a confirm that loses data, and
  `DialogSpec::footer` for a sheet's Save.
- Icon-only controls are now 44 points square; a test that asserted 32 may
  need its bounds updated.

## Limits

- A bubble first presented partly in view, or taller than its view, keeps
  the most width it may take until it is wholly in view, then hugs its
  content.
- The browser text size is read from computed styles; a browser setting
  that changes neither the root font size nor WebKit's system body font,
  such as page zoom, is not a text size. No device check is claimed.
- A dialog has one footer action; a footer with several actions is left for
  a later need.
