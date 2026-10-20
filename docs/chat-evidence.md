# Chat evidence

## Experience and design decision

Task and reference: scan a conversation list for who wrote last and what is
unread, open one conversation and read it back and forth, then turn a
setting off. Compare with the composer and avatar recipes beside it, the
navigation item's current state, and chat clients whose own messages sit
at the end of the line on an accented fill and whose lists show an avatar,
a name, a preview, a time and an unread dot.

Behavioural checks: passed. Recipe regressions cover the switch's name,
`checked` state and AccessKit `Switch` role with its toggled state, a label
that stays in view when off and elides beside a whole track when narrow, a
disabled switch that keeps its state and reason, and colours where the on
track is never the quiet hover fill, every state stands out at 3:1 from
every row fill, and a disabled track never stands out more than an enabled
one. Conversation-row regressions cover the name, the description (unread
words, preview, time and action, in that order), `ConversationRowTime`,
`ConversationRowTitle` and `ConversationRowPreview` under a
`ConversationRow` parent with no content text, the current row as
`selected` and `aria-current`, never toggled, a narrow row eliding its
title and preview but keeping its time, the hit size without a preview,
time or avatar, a disabled row's reason, an unread dot that stands out at
3:1 from every row fill, muted text kept at 4.5:1 on the current and hovered
fills, and a hovered fill that differs from the current one. Hosted
regressions, at phone and desktop widths, check that each row carries its
conversation's ID and opens its own conversation by pointer, Tab and Enter;
that agents discover each row as its own capability and open a conversation
by its argument; that a switch turns over by pointer, Space and Enter; that
bubbles sit on their sides, hug short messages, wrap long ones within 85%
of the column and their token width, publish a `Section` and an AccessKit
group named by the sender, and put their words in observation as content
text; and that a bubble fits again when its text, its column, or the
density, contrast or text size that sets it changes. Every hosted test
passes the semantic, text and AccessKit parity audits.

The browser journey, `tools/chat-browser-smoke.sh`, runs the production
Gallery in headful Chromium with physical input. At desktop width in dark
it reads the four rows (names, current state, arguments and descriptions),
the two switches and the bubbles from observation, checks that own bubbles
end where other bubbles start and that each is within 85% of the column,
finds the messages' words as content text, and finds in the ARIA layer
`aria-current` on the rows, `role="switch"` with `aria-checked`, and one
named group per message. It hovers and clicks a row, which opens and reads
that conversation, moves to the next row with Tab and opens it with Enter,
and turns the Notifications switch over by pointer, Space and Enter, checking
`aria-checked` follows. It then checks light compact at desktop width, light
at a regular width and dark at phone width (the list above the
conversation), and the long story in dark and in light high contrast at
150%: elided names with whole semantics, whole times, the archived row
disabled with its reason, the disabled switch with its reason and
`aria-disabled`, a wrapped long message and short messages on one line,
after wheel-scrolling the narrow column. An idle Gallery presents no frame.

The first browser run found two faults before any baseline was accepted. A
bubble kept the width it measured in one density after another set its text
wider, so "Thank you!" wrapped; it measured its glyphs' ink rather than the
laid-out line. A bubble now measures to the end of its laid-out lines and
measures again when its text's line count or height changes, and a hosted
regression changes density, contrast and text size. A hovered row had the
quiet action hover fill, which every theme gives the selection's colour, so
a hovered row looked current; a hovered row now takes the raised surface.

Presentation checks: passed. First critique of `chat-conversation-dark` and
`chat-long-high-contrast`, before reading the rationale: the conversation
reads as two voices at once, the person's own messages on the teal tint at
the end and the other party's on the quiet raised surface at the start; the
Markdown reply's list and link sit comfortably inside its bubble, and code
in the long story keeps its own darker block. In the list, names lead, the
preview is quieter, the time is small at the end, and the accent dots pick
out the two unread conversations without competing with the current row,
which keeps its fill and marker. The switches state themselves by track and
thumb with their labels in view. What competes: in light high contrast at
150%, the disabled "Archive old conversations" switch had a solid
near-black track, the darkest mark in the column, so a setting the person
cannot change drew more attention than the ones they can. Reading the
rationale did not answer that, so one refinement gave a disabled switch a
raised track with a decorative outline and thumb, the thumb still at the
end when on. Re-critique accepted it: the disabled switch now reads as
quieter than its neighbours in both themes, and its state still shows by
the thumb's position. Remaining trade-offs: in a wide single column (the
regular-width story) the time and unread dot sit far from the name, as in
other chat clients; a bubble shows no sender or time, which the application
presents beside it when it wants them; the long story's own bubbles reach
under the scroll area's floating scroll bar at the column's end, as any
end-aligned content does.

Design decision: accepted after that one refinement.

## Rendered evidence

- [Desktop, dark comfortable](../tests/ui-snapshots/expected/chat-conversation-dark/visual.png).
- [Regular width, light compact](../tests/ui-snapshots/expected/chat-conversation-light/visual.png).
- [Phone width, dark comfortable](../tests/ui-snapshots/expected/chat-long-dark/visual.png).
- [Phone width, light high contrast at 150%](../tests/ui-snapshots/expected/chat-long-high-contrast/visual.png).
- [The composer with the `Send` and `Attach` icons](../tests/ui-snapshots/expected/composer-chat-dark/visual.png).

## Size

No dependency was added; the lockfiles are unchanged. Measured against
`8bae37ce`, the Gallery's release WASM after wasm-bindgen grew from
11,892,519 to 11,947,889 bytes (+55,370, +0.5%; 4,214,829 to 4,236,492
gzip, +21,663; 2,983,857 to 2,999,703 Brotli, +15,846). After
`wasm-opt -Oz` it grew from 9,292,042 to 9,340,278 bytes (+48,236, +0.5%;
Brotli +12,591, +0.4%). Both figures include the four recipes and the two
Gallery stories.

## Baseline decision and qualification

The four new fixtures' visual, semantic and text artefacts and the two
changed composer pictures were inspected outside `expected/` and accepted
explicitly. The other fixtures change only as recorded in the
[snapshot guide](../tests/ui-snapshots/README.md). Frozen evaluation tasks,
fixture definitions and scoring are unchanged. Verification never updates
baselines.

## Platform limits

The browser journey proves the browser accessibility tree and physical
input in Chromium only; it is not screen-reader qualification. Native
AccessKit output is covered by the hosted regressions' parity audit, not by
a native journey.
