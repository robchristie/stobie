# Fonts, licences and emoji

Status: complete
Delivery: [PR #124](https://github.com/robchristie/stobie/pull/124)

`stobie-ui-egui` declared Apache-2.0 alone although it embeds Source Sans 3
under the SIL Open Font License 1.1, and it turned on egui's
`default_fonts` feature, which embeds four more fonts under three more
licences, with no way for an application to leave them out. Those fonts
also cover only part of today's emoji: of sixteen emoji typed into a chat
application built on Stobie on a phone, seven (🧠 🤗 🫣 🛑 🤣 🪿 🌧) drew as
boxes. This plan makes the licence statement accurate, lets an application
omit egui's fonts, and adds the current monochrome Noto Emoji to Stobie's
font families.

## Decisions

- **The crate's licence names the fonts.** `stobie-ui-egui`'s `license` is
  `Apache-2.0 AND OFL-1.1`. Its own code is Apache-2.0; the three faces it
  embeds are OFL-1.1. `assets/fonts/README.md` is the bundled-font record:
  each file, its licence file, its copyright notice and its SHA-256, which
  `tests/font_notices.rs` checks against the files.
- **egui's fonts are the application's choice.** The workspace's `egui`
  dependency sets `default-features = false`, so the UI crate no longer
  enables `epaint_default_fonts` (Hack, Ubuntu Light, Noto Emoji 1.05 and an
  emoji icon font, under MIT with the Bitstream Vera licence, the Ubuntu
  Font Licence and OFL-1.1). An application enables them through eframe's
  `default_fonts` feature, as `stobie new` still does, or leaves them out;
  without them the `Monospace` family uses Source Sans 3 Regular. The
  template now depends on `egui` without its default features, so eframe's
  feature is its one switch; the template README and the
  [new-application guide](../start-a-new-app.md) say how to use it and that
  a distributed build ships each font's notice.
  `cargo xtask architecture` checks that the UI crate's normal dependencies,
  with and without all its features, never resolve `epaint_default_fonts`.
  Cargo applies a member's `default-features = false` only when the
  workspace dependency also sets it, so the setting lives in the workspace.
- **Noto Emoji 3.002, monochrome, as a static regular instance.** The face
  comes from google/fonts (`ofl/notoemoji/NotoEmoji[wght].ttf`), the
  canonical published build. Its `OFL.txt` declares no Reserved Font Name,
  so a Modified Version may keep the name. egui draws one weight per face,
  so the crate embeds the static `wght=400` instance made by fontTools'
  instancer: 886,628 bytes, 522 KB compressed with Brotli, about half the
  variable font's 1.09 MB. Its licence text ships beside it, unmodified, as
  `NotoEmoji-OFL.txt`; Adobe's `LICENSE.md` stays byte for byte as
  published, so the two licences are two files.
- **Emoji follow the text face.** In each named family the order is the
  text face, Noto Emoji, then egui's proportional faces when compiled in.
  Noto Emoji also maps digits, `#`, `*` and the space, so it must not come
  first. `Proportional` and `Monospace` add Noto Emoji after their existing
  faces. The face takes egui's own emoji scale, 0.81 of the text size.
- **The face is always embedded.** No feature gates it: it is the coverage
  the crate promises, and a feature would be off for an application that
  disables the UI crate's defaults.
- **Colour emoji are closed: egui 0.36 cannot draw them.** epaint 0.36.1
  rasterises a glyph by reading its outline (`src/text/font.rs`,
  `outline_glyphs().get(...)`), filling it in white
  (`set_paint(... WHITE)`), and storing the result as coverage in the font
  atlas (`src/image.rs`, `from_white_alpha`), which the text colour tints.
  It has no path for COLR layers or CBDT bitmaps. A COLRv1 glyph's base
  outline is empty and a CBDT glyph has no outline, so both draw nothing:
  `tests/colour_emoji.rs` sets a one-glyph COLRv1 subset and a one-glyph
  CBDT subset of Noto Color Emoji as the only face, and shows 😀 mapped,
  shaped with an advance, and given no atlas image. The same file shows
  Stobie's emoji reaching the atlas as coverage only (every texel grey).
  Colour emoji need a DOM text layer or a colour glyph path in egui, which
  is a separate piece of work.
- **Sequences draw better than expected.** egui shapes text with harfrust,
  which applies the face's ligatures, so a ZWJ family (👨‍👩‍👧), a flag (🇦🇺)
  and a ZWJ flag (🏳️‍🌈) each draw as one glyph. A skin tone (👍🏽) draws
  as its untoned emoji, because the monochrome face maps the toned sequence
  to the base glyph. A keycap (1️⃣) draws as its digit and ❤️ as the text
  face's heart, because the text face maps those characters first.
  `typography::tests::emoji_sequences_draw_one_monochrome_glyph` holds this.
- **Fixtures stay out of the package.** The colour test fonts live in the
  repository's `tests/fixtures/colour-emoji/`, with their licence, source
  and commands, so they are never in a package directory that an
  application's notices check scans.

## Design review contract

- **Task.** Read and write messages that contain emoji, in every text role
  an application uses: reading text, control labels, secondary text and
  technical monospace text, at 100% and 150%, light and dark.
- **Hierarchy.** An emoji sits in the line as a word does: the same colour as
  its text, no taller than the line, on the text's baseline, never louder
  than the words beside it.
- **Reference.** egui's own emoji face at the same scale; how a messaging
  application on a phone sets emoji inline, which is in colour.
- **States.** Each of the seven reported emoji in reading, semibold label,
  secondary and monospace roles; ZWJ, flag, toned and keycap sequences;
  narrow wrapping; light and dark; 100% and 150%.
- **Exclusions.** Colour emoji, emoji pickers, and emoji larger than the
  text (an emoji-only message shown large).

The Gallery story `typography/emoji` shows each state.

## Critique

First pass, by a reviewer who saw only the task, the reference and the two
captures (light at 100%; dark high contrast at 150%, narrow):

- Noticed first: the stop sign, a solid disc, is the loudest mark in every
  role, and the solid heart likewise; in the semibold role the outline emoji
  read lighter than the words.
- Competes: solid and outline emoji differ in weight; in the monospace role
  emoji sit in wide cells and look detached; the flag tiles read as badges.
- Harder: at 100% the brain, hug and peek emoji are hard to tell apart and
  the laugh emoji is not recognisable; the family is tiny, the flags are
  lettered tiles, the skin tone is dropped and the keycap is a bare digit;
  muted emoji almost vanish. No boxes or missing glyphs, line spacing intact,
  orderly wrapping at 150%.

Second pass, with the rationale: the weight differences, the lettered flag
tiles and the dropped tone are the monochrome face's own design, and the
heart is the text face's glyph. The emoji are already 1.25 times the text's
cap height (0.81 of a 1.02 em glyph against a 0.66 em cap height), as egui
sized its own emoji face, so scaling them up would push them past the line;
detail at 100% is limited by about ten pixels on a standard-density screen,
and a phone's screen has two to three times the pixels. Muted emoji take the
muted text colour, as the hierarchy intends. Monospace cells are the
monospace face's advance. No revision.

Decision record:

```text
Task and reference: emoji inline in each text role; egui's own emoji face
Behavioural checks: passed (host tests; the story's text is selectable)
Presentation checks: passed (typography-emoji-light,
  typography-emoji-dark-high-contrast; no text audit finding)
Design decision: accepted
Main reason and remaining trade-off: every reported emoji now draws, in the
  text's colour and line; monochrome flags, tones and keycaps stay
  simplified, and small emoji at 100% on a standard-density screen lose
  detail
Exact candidate evidence links: tests/ui-snapshots/expected/typography-emoji-*
```

## Acceptance

- Host tests (`stobie-ui-egui`): each of the seven emoji maps first to Noto
  Emoji in both named families, `Proportional` and `Monospace`, and draws as
  one glyph (`typography::tests::every_family_draws_the_spike_report_emoji`);
  with egui's fonts left out, Stobie's faces alone cover every family and
  text still comes from the text face
  (`stobie_faces_alone_cover_every_family_without_eguis_default_fonts`); the
  sequences draw as described above; colour glyphs draw nothing and Stobie's
  emoji reach the atlas as coverage only (`tests/colour_emoji.rs`); the
  bundled-font record matches the files and the crate's licence
  (`tests/font_notices.rs`).
- `cargo xtask architecture`: the UI crate never resolves
  `epaint_default_fonts`.
- The tracked-files policy admits the Noto Emoji face at its exact size and
  the colour fixtures under `tests/fixtures/`.
- Gallery story `typography/emoji` and its test; the snapshot fixtures'
  font identity names Noto Emoji.
- Web package size, before and after (`cargo xtask web-size`), below.
- `cargo xtask verify`.

## Size

`cargo xtask web-size` at the base revision and with this change, in bytes.
The WASM carries the fonts, so it carries the whole change:

| Package | WASM raw before | WASM raw after | WASM Brotli before | WASM Brotli after |
| --- | ---: | ---: | ---: | ---: |
| Gallery (browser production) | 12,155,637 | 13,044,438 (+888,801) | 3,046,707 | 3,566,333 (+519,626, +17.1%) |
| Lab (browser production) | 12,459,697 | 13,347,593 (+887,896) | 3,069,921 | 3,586,487 (+516,566, +16.8%) |
| `stobie new` template | 10,475,763 | 11,363,599 (+887,836) | 2,688,025 | 3,208,133 (+520,108, +19.3%) |

The face adds about 520 KB compressed. The template built with egui's fonts
left out (eframe without `default_fonts`) measured 9,944,753 bytes raw and
2,526,201 Brotli: 682 KB smaller than with them, and 162 KB smaller than
before this change, with wider emoji coverage.

## What an application does on its pin move

- Ship Noto Emoji's licence and notice with any distributed build:
  `assets/fonts/NotoEmoji-OFL.txt`, verbatim, and the copyright notice
  `Copyright 2013 Google LLC`, alongside the Source Sans 3 notice it already
  ships.
- If it keeps egui's fonts, nothing else changes. To leave them out, drop
  eframe's `default_fonts` feature and depend on `egui` with
  `default-features = false`, then remove their notices.
- Emoji draw in the text colour; a design that expects colour emoji needs
  the follow-up work above.
