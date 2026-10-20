# Avatar evidence

## Experience and design decision

Task and reference: scan a list of people and assistants and recognise each
at a glance by face, shape and colour or by initials, then see the same
avatar beside a conversation's title. Compare with Stobie's typed icon
vocabulary, whose stroke and proportion the person glyph shares, and with
chat clients whose assistants are soft coloured shapes with small eyes and
whose people show initials or a photo.

Behavioural checks: passed. Recipe regressions cover the size tokens and
their growth to 150% (ignoring smaller and non-finite scales); initials from
the first and last words' first letters or digits, through punctuation,
accents, `ß` and names with none; outline validation for constants and data,
including non-finite and out-of-square points and too few segments, and a
`const` outline that fails to build; ear clipping that fills a concave
outline exactly in either winding, drops straight runs, and returns nothing
or bounded triangles for degenerate and self-crossing outlines without
panicking; a concave blob filled in its colour with the flattened outline's
exact area and two upright eyes in the contrasting colour; initials centred
by their ink and shrunk to fit; the person glyph for empty, emoji-only,
punctuation-only and CJK names; an image clipped to the circle (area within
2% of πr²) with its centred-square crop for wide and tall images; an empty
image ignored; empty and non-finite rectangles painting nothing; and the
scoped call allocating its scaled square while publishing no node and no
text. Gallery regressions check both themes' text and AccessKit audits,
that captions are measured while initials are painted but not observed, that
the photo fills two outlines, and that a narrow column at 150% elides long
captions without an audit finding. Icon regressions check the compiler
(quarter-turn arcs, exact quadratics, both sweeps, scaled radii, closed
curves and fourteen refusals, including a move with no segment) and that
`Attach`, `Mic` and `User` paint stroked cubics with round joins while
`Send` and `Copy` stay straight. In the first render of the icon vocabulary
the paperclip's inner loop closed up at 16 points, so its loops were widened
to radii of 2, 4 and 6 before that baseline was accepted.

Presentation checks: passed. First critique of the dark comfortable story
and the narrow light high-contrast column at 150%, before reading the
rationale: the coloured shapes are noticed first and read as one family; the
people's discs are quiet and the same in each theme, with initials legible
and the glyph clearly a stand-in; the photo sits in its outline with a fine
edge; captions lead each row. What competes: nothing beside the shapes'
colour, which is the application's. What became harder: at list size the
eyes were dots set close together, so the faces read as marks rather than
faces. Reading the rationale did not answer that, so one refinement made
each eye a twelfth of the avatar wide and a sixth tall, with centres a
quarter of the avatar apart. Re-critique accepted it: the faces now read at
header size. Remaining trade-offs: a fill close to the surface (the slate
triangle in dark mode) has a weak edge, because the application owns its
colours and the recipe adds no ring to a shape; initials take only the first
and last words, so names written family name first show the family initial
first; a name with any glyph the fonts lack falls back to the glyph rather
than to partial initials.

Design decision: accepted after that one refinement.

## Rendered evidence

- [Dark comfortable](../tests/ui-snapshots/expected/avatars-figures-dark/visual.png).
- [Light compact](../tests/ui-snapshots/expected/avatars-figures-light/visual.png).
- [Narrow light high contrast at 150%](../tests/ui-snapshots/expected/avatars-figures-high-contrast/visual.png).
- [Icon vocabulary with the five new icons](../tests/ui-snapshots/expected/icon-vocabulary-light/visual.png).

## Size

No dependency was added; the lockfiles are unchanged. Curved icons add
static geometry, and the avatar recipe and story add code only.

## Baseline decision and qualification

The three new fixtures' visual, semantic and text artefacts and the changed
icon vocabulary were inspected outside `expected/` and accepted explicitly.
The other fixtures change only as recorded in the
[snapshot guide](../tests/ui-snapshots/README.md). Frozen evaluation tasks,
fixture definitions and scoring are unchanged. Verification never updates
baselines.

## Platform limits

Avatars are decorative and add no accessible owner, so they add nothing to
screen-reader qualification. There is no browser or native journey: an
avatar has no interaction, and the browser snapshots render it through the
production WebGPU path.
