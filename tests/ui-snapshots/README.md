# Deterministic UI snapshots

[`fixtures.json`](fixtures.json) is the versioned, closed fixture manifest for
Stobie's selected visual, semantic and text baselines. Each fixture pins its
gallery story, viewport, data seed, appearance, contrast, density, font scale,
width class, bundled font set and pinned browser-WebGPU/SwiftShader renderer
contract.

Build the browser package once, then use an explicit ignored output directory:

```sh
cargo xtask build-web
cargo xtask ui list --output-dir .tools/runtime/ui-list
cargo xtask ui render --fixture application-shell-dark --output-dir .tools/runtime/ui-render
cargo xtask ui inspect --fixture application-shell-dark --output-dir .tools/runtime/ui-inspect
cargo xtask ui audit-text --all --output-dir .tools/runtime/ui-audit
cargo xtask ui verify --output-dir .tools/runtime/ui-verify
```

For safety, UI output must be a dedicated directory beneath the repository's
ignored `.tools/` tree. A versioned ownership marker is required before the
tool will replace a non-empty directory; source, baseline and arbitrary user
directories are rejected before any recursive cleanup.

Every command writes a versioned JSON `summary.json`; `list`, `render`,
`inspect` and `audit-text` retain their corresponding machine-readable
artefacts. `verify` compares pixels at zero tolerance and compares canonical
metadata, semantic snapshots and text observations structurally.

Each `text.json` includes `coverage` with measured component and native control
counts plus excluded categories. The `audit-text` summary retains coverage per
fixture and states the bounded meaning of a pass. Missing coverage, failed component attempts or counts
inconsistent with the observations fail verification. Attempts are recorded
independently of the visible observation subset, so filtering cannot erase a
failed request. Empty findings mean
“Every observed Stobie text component passed”, not “Every visible string was
structurally audited”. Counts cover the submitted layout pass, including clipped
controls and gallery chrome; ordinary labels remain excluded. The Gallery counts
only the catalogue entries in view, so a story added below them changes no
other fixture's count. See the
[design language](../../docs/design-language.md) for the denominator and native-widget
boundary.

The verifier is deliberately read-only with respect to `expected/`. It has no
approval or update mode. A mismatch writes a fixture-specific bundle under
`<output>/failures/` containing expected and actual metadata, semantic, text
and visual artefacts, machine-readable diffs, a visual diff and capture logs.
When capture or comparison cannot produce an artefact, the same bundle records
that category as explicitly unavailable and retains every artefact and log
that was produced. Audit findings are serialised before the command reports
failure, so `audit-text` remains diagnostic rather than collapsing into a
capture error.
CI invokes this same verifier through `cargo xtask verify` and uploads the
ignored verification evidence when a gate fails.

Baseline changes are ordinary reviewed source changes. Generate candidate
artefacts outside `expected/`, inspect every affected visual and semantic/text
diff, and copy only the deliberately accepted files into the checked-in tree.
CI must never perform that operation.

The icon fixtures pin the production action presentations in dark comfortable
mode, the vocabulary in light compact mode, and narrow labelled actions in light
high contrast at 150%. Physical hover/press, keyboard, toggle/disabled and display
scale probes run through `tools/icon-actions-browser-smoke.mjs` and the Linux
native companion in full verification. Catalogue growth changes Gallery chrome
and native-control coverage only where the new entry is in view; review those
differences alongside any deliberate artwork change before updating the original
fixtures. Frozen seed tasks and their
assertions remain unchanged.

Navigation fixtures add the ordinary dark sidebar, light compact state/count
examples, and narrow light high contrast at 150%. The physical
`tools/navigation-browser-smoke.sh` and Linux native companion run through full
verification. The browser journey exercises actual Tab/Enter/Space, pointer
hover/press, consumer selection/count changes, disabled and zero-count rows,
large counts, long label/badge elision, scrolling and bounded idle behaviour.
Successful partially clipped text is measured by the recipe but omitted from
Gallery's fully visible audited subset; the pass coverage inventory still counts
those attempts and retains every failed measurement. Clipped row interaction
semantics remain visible with current bounds.

Status-chip fixtures add task rows/detail in dark comfortable mode, all four
tones on parent surfaces in light compact mode, and localised narrow ellipsis in
light high contrast at 150%. Physical `tools/status-chip-browser-smoke.sh` and
its Linux native companion exercise standalone selection/copy input, inert chip
parent activation, stable current status updates and bounded idle behaviour.
The browser journey verifies actual clipboard content for a fully elided localised
label, and scrolling replaces clipped owners while retaining every attempt.
Rust regressions prove native egui selection/copy and AccessKit ownership/parity.

Reading fixtures add a conversation in dark comfortable mode, with a reply of
ten lines and two links, and a narrow light high-contrast column at 150% with
unbroken and localised text, a wrapping link label, a bare URL, a disabled
link and text crossing the scroll area's fold. The physical
`tools/reading-text-browser-smoke.sh` runs through full verification: it
copies a whole multi-paragraph reply by drag selection, opens links in a new
tab by pointer and by Enter and Space after focusing through the ARIA layer
while the Gallery page and its state stay, checks that a disabled link opens
nothing, and scrolls the narrow column. Its Linux native companion,
`tools/reading-text-native-smoke.sh`, selects the reply with xdotool and
clicks a link, whose URL reaches the platform's default browser through a
`BROWSER` recorder. Adding the two catalogue entries
raised every earlier fixture's native-control count from 36 to 38 and changed
nothing else.

Row fixtures add four people in dark comfortable and light compact modes, each
row a list row with Open, Pinned and Remove beside it and the first person
pinned so that their Remove is disabled, and a narrow light high-contrast
column at 150% with long and localised titles and details elided, a disabled
row and rows crossing the scroll area's fold. Each control's node carries its
person's ID and is named after them. The physical journeys through these
controls, in Chromium's ARIA layer and through `stobie`, run in
`tools/row-actions-smoke.mjs` during the outside-app stage. Adding the two
catalogue entries raised every earlier fixture's native-control count from 38
to 40, and changed one pixel of `status-chip-treatments-light` by one colour
step (at 863, 140) as the glyph atlas grew; nothing else changed.

Markdown fixtures add a digest reply in dark comfortable mode (a heading,
four numbered linked titles with notes, a short list, a quote and a code
block) and a narrow light high-contrast column at 150% with an unbroken
identifier, wrapping code, unsafe links and raw HTML kept as text, nested
lists and, below the fold, a table as text and a reply still arriving. The
physical `tools/markdown-browser-smoke.sh` runs through full verification
on WebGPU and WebGL2: it copies a whole list item across its link by drag
selection, opens links inside the text in a new tab by pointer and by Enter
and Space after focusing through the ARIA layer, finds the headings with
their levels in the ARIA layer, checks that unsafe links and a disabled
region open nothing, and scrolls to the unfinished tail. Adding the two
catalogue entries raised every earlier fixture's native-control count from
40 to 42 and changed one pixel each of `navigation-long-high-contrast` (at
824, 423) and `status-chip-treatments-light` (at 863, 140) by one colour
step as the glyph atlas grew; nothing else changed.

Composer fixtures add chat composers in dark comfortable mode: an empty draft
whose send is hidden, leaving the microphone alone, a one-line draft ready to
send and a draft grown to three lines. A narrow light high-contrast column at
150% adds a long localised hint elided to one line, a draft past its four-line
limit, scrolled within the field, and a disabled send while a message is
sending. The physical journeys, typing and sending in headful Chromium with a
keyboard and a touch screen and through `stobie` natively and in the browser,
run in `tools/composer-smoke.mjs` during the outside-app stage. Adding the two
catalogue entries raised every earlier fixture's native-control count from 42
to 44 and changed one pixel each of `navigation-long-high-contrast` (at 824,
423) and `navigation-states-light-compact` (at 706, 294) by one colour step
as the glyph atlas grew; nothing else changed.

Avatar fixtures add people, shapes with faces and images at every size in
dark comfortable and light compact modes, and a narrow light high-contrast
column at 150% where long names elide. Avatars are decorative, so the
fixtures carry no avatar node and measure only headings and captions; there
is no physical journey, because an avatar does not act. Adding the catalogue
entry raised every earlier fixture's native-control count by one (from 44
to 45, and the composer fixtures' from 47 to 48) and changed one pixel of
`navigation-states-light-compact` (at 706, 294) by one colour step as the
glyph atlas grew. The five new icons (`Attach`, `Copy`, `Mic`, `Send` and
`User`) changed `icon-vocabulary-light`'s picture and added five measured
captions to it; nothing else changed.

Chat fixtures add a conversation list beside a conversation in dark
comfortable mode at desktop width, with an unread, a current and plain rows,
switches on and off, and messages on both sides holding text, Markdown, a
list and a link; the same story in light compact mode at a regular width,
where the list stacks above the conversation; and a narrow column in dark
comfortable mode and in light high contrast at 150% with long and localised
names and previews, a row without a preview, an archived row, a long switch
label, a disabled switch, and bubbles holding a long link and code below the
fold. Rows and switches are measured with their own text kinds and publish
their names, previews, times, unread words and states in semantics; the
messages' words are content text. The physical `tools/chat-browser-smoke.sh`
runs through full verification: it reads rows, switches and bubbles through
observation and the ARIA layer, opens a conversation by pointer and by Tab
and Enter, turns a switch over by pointer, Space and Enter, and scrolls the
narrow column. Adding the two catalogue entries raised every earlier
fixture's native-control count by two (from 45 to 47, and the composer
fixtures' from 48 to 50) and changed one pixel of
`rows-actions-light-compact` (at 538, 309) by one colour step as the glyph
atlas grew. The composer's send and attach controls now draw the `Send` and
`Attach` icons, which changed the pictures of `composer-chat-dark` and
`composer-long-high-contrast`; nothing else changed.

Copy fixtures add copy controls in dark comfortable mode, beside an
invitation code, icon-only under a message, label-only with a long German
label and confirmation, and disabled with a reason; a narrow light
high-contrast column at 150% holds them all. The fixtures show the ready
faces, so they carry no status node, and no button carries the text it
copies. The physical journeys, copying by mouse, touch screen, Enter after
focusing through the ARIA layer, a click on the layer element and an agent
in headful Chromium with clipboard permission, and a refused write, run in
`tools/copy-browser-smoke.sh` through full verification on WebGPU and
WebGL2. Adding the catalogue entry raised every earlier fixture's
native-control count by one (from 47 to 48, and the composer fixtures' from
50 to 51) and changed, by one colour step as the glyph atlas grew, one pixel
each of `avatars-figures-high-contrast` (at 717, 568) and
`rows-actions-light-compact` (at 538, 309) and two of
`diagnostics-high-contrast` (at 703, 516 and 699, 522); nothing else changed.

Responsive navigation fixtures add the conversation list, the open
conversation and its details as three columns in dark comfortable mode at
the Gallery's Desktop width; the same story in light compact mode at the Wide
width, where the shown details take the conversation's place beside the
list with Back; and the phone story's root in light high contrast at 150%,
a list of long and localised destinations with counts and an archived one,
under an eliding title. Panes publish as `Pane` nodes named by their titles
under the story, the open row is `selected`, and the conversation's pane
and its nodes carry its domain reference. The physical
`tools/responsive-navigation-browser-smoke.sh` runs through full
verification: it resizes between the columns, the split and the stack with
the same conversation open, opens rows, goes back by pointer and by Enter
with focus following, toggles the details, scrolls the phone list and
returns to its place, and checks that no history entries are pushed.
Adding the two catalogue entries raised every earlier fixture's
native-control count by two (from 48 to 50, and the composer fixtures' from
51 to 53) and changed, by one colour step as the glyph atlas grew, one pixel
each of `avatars-figures-high-contrast` (at 717, 568) and
`navigation-long-high-contrast` (at 752, 208); nothing else changed.

Dialog fixtures add the routine editor as a centred dialog in dark
comfortable mode at 1440 by 900, and as a bottom sheet across a 390 by 844
phone in light compact mode, each over the routine it edits under a scrim;
and the confirm before deleting that routine, centred, in light compact mode
at 1440 by 900 and in light high contrast at 150% on the phone, where its
two buttons still share a row. Their text observations are the dialog's
own: the title, field captions, schedule and buttons, or the question, its
message and Keep routine then Delete; nothing behind the scrim is measured.
The editor's fixtures show the caret in the focused Name field. Captures
ask for reduced motion, the Gallery follows the browser's preference, and
under reduced motion Stobie holds the caret steady, so the caret no longer
depends on when in its blink the capture is taken.
The physical
`tools/dialog-browser-smoke.sh` runs through full verification: it checks
the centred dialog and the bottom sheet's geometry, that focus starts in the
first field, that Tab and Shift+Tab stay inside and wrap, typing into both
fields, that the scrim and an agent's invocation reach nothing behind, the
ARIA layer holding only the modal dialog, Escape, Save and Close by keyboard
and pointer with focus returned to the opener, the confirm's focused cancel,
Escape and Enter, deletion by keyboard and pointer, taps on a touch screen,
and that the page is idle afterwards. Adding the two catalogue entries
raised every earlier fixture's native-control count by two (from 50 to 52,
and the composer fixtures' from 53 to 55) and changed, by one colour step as
the glyph atlas grew, two pixels of `diagnostics-high-contrast` (at 703, 516
and 699, 522) and one of `navigation-long-high-contrast` (at 752, 208);
nothing else changed.

Emoji fixtures show the `typography/emoji` story, the seven emoji in
reading, semibold, secondary and monospace text and the sequences, in light
comfortable mode and in dark high contrast at 150% and narrow width. Every
fixture's font identity names the monochrome Noto Emoji 3.002 face that
`stobie-ui-egui` embeds. Adding the face changed, by one colour step as the
glyph atlas changed, one pixel each of `application-shell-dark` (at 937, 362)
and `navigation-responsive-dark` (at 357, 311); nothing else changed.
