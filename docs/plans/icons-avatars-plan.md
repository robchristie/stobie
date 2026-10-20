# Chat icons and avatars

Status: complete
Delivery: [PR #PENDING](https://github.com/robchristie/stobie/pulls)

## Outcome and boundaries

A chat application needs icons for send, attach, voice input, copy and a
person, and avatars for people and for characters such as assistants. Stobie
owns the icon compiler, which gains curves, five icons drawn in its own
style under its Apache-2.0 licence, and one avatar recipe: a person's
initials or glyph, an application-supplied shape in a solid colour with a
face, or an image clipped to either, at header, list and strip sizes.
Applications own their shapes, colours, images and image loading, and name
each avatar's person or thing beside it. Interactive avatars (a strip of
pinned people that open conversations), status rings and presence dots,
image decoding, and chat bubbles, rows and switches are separate work.

## Implementation and UI acceptance contract

Representative task: scan a list of people and assistants and recognise each
at a glance by face, shape and colour or by initials, then see the same
avatar beside a conversation's title. Names lead; avatars are recognisable
marks that never compete with the name or with actions. Reference: the
typed icon vocabulary and chat clients whose assistant avatars are soft
coloured shapes with small eyes, and whose people show initials or a photo.

- Icon compiler: absolute `Q`, `C` and unrotated `A` alongside `M`, `L`,
  `Z` and circles; quadratics and arcs become cubics (arcs in pieces of at
  most a quarter turn); refuse relative, shorthand and rotated forms, bad
  flags, zero radii and any computed control outside the view box; keep
  the polyline form for straight paths so existing geometry is unchanged.
  The painter draws cubics with egui's flattening and keeps round caps and
  joins. New icons: `Send`, `Attach` (paperclip), `Mic`, `Copy`, `User`;
  the composer's `+` is the existing `Add`.
- Avatar: `AvatarSpec` (figure, `AvatarSize`, optional `SizedTexture`),
  `PresentationContext::avatar`, low-level `avatar` and `paint_avatar`.
  Person: up to two initials (first letter or digit of the first and last
  words, upper case) in semibold on the raised surface with a decorative
  border, shrunk to fit and centred by ink; the `User` glyph when there are
  no initials or the fonts cannot draw them. Shape: an `AvatarOutline`
  (start plus at least two line, quadratic or cubic segments in the unit
  square; `const fn new` fails the build when invalid, `try_new` returns an
  error for data) filled by ear clipping so concave outlines fill exactly,
  with a hairline in the fill colour for smooth edges; built-in circle,
  rounded square and hexagon; `AvatarFace::Eyes` in near-black or white by
  contrast with the fill. Image: fills the outline, cropped to a centred
  square, with a decorative border; the figure stands in while it is
  `None`.
- Sizes are tokens: `avatar.headerSize` 32, `avatar.listSize` 40,
  `avatar.stripSize` 56, grown with text to 150%.
- Semantics: decorative, like icons; no node, no measured text, initials
  unobserved; the response senses hover only.
- States: ordinary, every size, long and localised names, names without
  drawable initials, empty name, image and image loading, narrow, 150% text,
  light, dark and high contrast. Excluded: hover, press, focus, selected and
  disabled (an avatar does not act), error (an image failure shows the
  figure, as loading does) and empty lists (application-owned).

Calibration question: do the shapes read as a family of friendly characters
beside quiet, legible people, at list size, in both themes? Smallest probe:
Gallery `avatars/figures` in dark comfortable, light compact and narrow
light high contrast at 150%. Evidence owner:
[Avatar evidence](../avatar-evidence.md). Exit: focused regressions,
inspected baselines and accepted two-pass critique, with at most one
refinement.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Curve compiler and five icons | Complete | 6 compiler and 2 painter regressions; `generate-icons.py --check` |
| Avatar recipe, tokens and scoped API | Complete | 10 recipe and 2 Gallery regressions; token generation check |
| Gallery, documentation and baselines | Complete | [UI evidence](../avatar-evidence.md), three inspected baselines |
| Candidate qualification | Complete | `cargo xtask verify` on the candidate; review, CI and merge retained in the PR |

## Closeout

The curve compiler, five icons, avatar recipe and tokens, scoped API, the
Gallery story with three fixtures, and the documentation are complete. One
design refinement (larger, wider-set eyes) was accepted on re-critique.
Final review, CI and merge qualification remain with the owning pull
request; this completed product state does not assert that it has merged.
Interactive avatars, presence and status marks, and the chat recipes remain
separate work.
