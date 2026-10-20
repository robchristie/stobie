# Responsive navigation evidence

## Experience and design decision

Task and reference: on a phone, scan a conversation list, open one, read
it, show its details and go back to the list; on a desktop, do the same with
everything in view and hide the details again; resize between them without
losing the open conversation. Compare with the chat recipes' list and
bubbles, the navigation item's current state, the application bar's height,
and chat and mail clients whose list, message and detail panes collapse into
a stack with Back on a phone.

Behavioural checks: passed. Hosted regressions, through `AppHost` with
kittest, cover the layout policy at each breakpoint and just below it
(stack below 720, split from 720, columns from 1040); a phone
that shows one level at a time, the list as its root, the conversation
pushed with Back and the details pushed above it, Back popping one level
at a time; a tablet that shows the list beside the conversation, the shown
details taking the conversation's place with Back; a desktop that shows
the three side by side with no Back and hides the details with the toggle;
a resize from desktop to tablet and phone and back that keeps the open
conversation, its shown details and its row's current state; a list that keeps its scroll position across levels and
layouts; keyboard focus that moves to Back when a level opens over the
focused row and returns to that row on Back, by Enter, while an agent's
change takes no focus; agents that read the open item's domain reference
open another conversation by its argument, or by the open action's own
binding while the phone's list is covered, and go back in each layout; and panes that are AccessKit regions named by their titles,
holding their headers' controls and their bodies' nodes. The open row is
`selected` and `aria-current`; the details toggle is a toggle whose pressed
state follows the application. A Gallery regression checks the panes each
Gallery width shows. Every hosted test passes the semantic, text and
AccessKit parity audits.

The browser journey, `tools/responsive-navigation-browser-smoke.sh`, runs
the production Gallery in headful Chromium with physical input on WebGPU
and on WebGL2. At the Desktop width in dark it reads the three panes from
observation, their domain reference, the rows' current state and parents,
and finds in the ARIA layer a region per pane holding its rows,
`aria-current` on the open row and `aria-pressed` on the toggle; it opens
another conversation by pointer and hides its details. It then narrows to
the Wide width (the split), shows the details and goes back; narrows to the
Narrow width (the stack), goes back to the list, opens a conversation, finds
focus on Back, goes back with Enter and finds focus on the row again; and
widens to the desktop with nothing open, where the content asks for a
choice. The Gallery's fixture state proves each resize kept what was open
and counts each open, Back and toggle. It checks light compact at the Wide
width, then the phone story in light high contrast at 150%: elided
destinations with whole names in semantics, an elided title beside Back
with the whole title on its region, the archived row disabled with its
reason, and a wheel-scrolled list that keeps its place after a conversation
below the fold is opened and closed. No history entries are pushed, and an
idle Gallery presents no frame.

The first browser run found one fault before any baseline was accepted.
Opening a conversation from the phone's list removed the focused row, so
keyboard focus was lost and Tab started again at the top of the page. The
recipe now moves focus to Back when a level opens over focus and returns it
to the control that opened the level on Back, and a hosted regression walks
the phone and tablet levels by Enter.

Presentation checks: passed. First critique of
`navigation-responsive-dark`, `navigation-responsive-light-compact` and
`navigation-responsive-high-contrast`, before reading the rationale: each
width reads as the same application. On the desktop the list, the
conversation and its details stand as three columns with headers of one
height and subtle edges between them; the open row keeps the current fill
and marker, and the conversation's title heads the middle pane with the
details toggle, pressed, at its end. In the split the shown details take the
conversation's place, and Back at the start of their header is plain without
competing with the title. On the phone the list is the whole column under an
eliding title; names lead, counts sit at the end, and the archived row is
quieter. What competes: in dark the pressed details toggle is the only
accent in the headers, drawing the eye about as much as the current row; it
states a state the reader can change, so it was kept. Reading the rationale
raised nothing the pictures contradicted. Remaining trade-offs: the
detail's property rows stack their labels above their values at the
detail's 320 points; in the split the conversation is hidden while its
details show, as in mail clients' narrow layouts; the story's frame leaves
space below short panes, as the Gallery frames every story.

Design decision: accepted without refinement.

## Rendered evidence

- [Desktop, dark comfortable: three columns](../tests/ui-snapshots/expected/navigation-responsive-dark/visual.png).
- [Wide, light compact: the split with the details and Back](../tests/ui-snapshots/expected/navigation-responsive-light-compact/visual.png).
- [Phone, light high contrast at 150%: the list root](../tests/ui-snapshots/expected/navigation-responsive-high-contrast/visual.png).

## Size

No dependency was added; the lockfiles are unchanged. Measured against
`2513b796`, the Gallery's release WASM after wasm-bindgen grew from
11,991,475 to 12,054,075 bytes (+62,600, +0.5%; 4,247,858 to 4,268,898
with `gzip -9`, +21,040; 3,004,240 to 3,014,755 with Brotli 11, +10,515).
After `wasm-opt -Oz` (Binaryen 131) it grew from 9,511,400 to 9,566,161
bytes (+54,761, +0.6%; Brotli 2,902,156 to 2,915,493, +13,337, +0.5%). Both
figures include the recipe, the bound destinations, focus following and the
two Gallery stories.

## Baseline decision and qualification

The three new fixtures' visual, semantic and text artefacts were inspected
outside `expected/` and accepted explicitly. The other fixtures change only
as recorded in the [snapshot guide](../tests/ui-snapshots/README.md).
Frozen evaluation tasks, fixture definitions and scoring are unchanged.
Verification never updates baselines.

## Platform limits

The browser journey proves the browser accessibility tree and physical
input in Chromium only; it is not screen-reader qualification, and it does
not prove the iOS edge swipe, which leaves the page as for any page because
the recipe pushes no history entries. Native AccessKit output is covered by
the hosted regressions' parity audit, not by a native journey.
