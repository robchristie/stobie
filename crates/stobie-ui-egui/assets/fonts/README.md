# Bundled typography faces

`stobie-ui-egui` embeds three font files. All three are under the **SIL Open
Font License 1.1** (`OFL-1.1`). The rest of the crate is Apache-2.0, so the
crate's `license` field is `Apache-2.0 AND OFL-1.1`. An application that
redistributes a build carrying these faces must ship each face's licence text
and copyright notice in its third-party notices.

| File | Face | Licence file | Copyright notice (name ID 0) | SHA-256 |
| --- | --- | --- | --- | --- |
| `SourceSans3-Regular.ttf` | Source Sans 3 Regular | [LICENSE.md](LICENSE.md) | Adobe's, as in `LICENSE.md` | `4644c81b86ec9caaa76b634889968ed3c4f4f52f054855933acc7c2b21e53b0f` |
| `SourceSans3-Semibold.ttf` | Source Sans 3 Semibold | [LICENSE.md](LICENSE.md) | Adobe's, as in `LICENSE.md` | `a3f4f8dcf343a8f24dc61951de93f3ba1558b15cd250ba24af8a40e957081b7d` |
| `NotoEmoji-Regular.ttf` | Noto Emoji Regular 3.002 | [NotoEmoji-OFL.txt](NotoEmoji-OFL.txt) | `Copyright 2013 Google LLC` | `1ada997ccb67bf2404828e25528e5257d6db10c80287e77684abf6737a5d198f` |

The licence texts are upstream files, byte for byte:

| File | SHA-256 |
| --- | --- |
| `LICENSE.md` | `56af9b9c6715597e458284a474dc118a50a4150e9d547c70f7b4a33c3e6a9328` |
| `NotoEmoji-OFL.txt` | `500bb1ccf43df7bbb522112f9133a52b16e1c35e809632f5d8609b179152de5b` |

`tests/font_notices.rs` checks this table against the files.

## Source Sans 3

The regular and semibold files are unmodified TrueType files from Adobe's
[Source Sans repository](https://github.com/adobe-fonts/source-sans), release
branch revision `87b37a2daaed80fcb8e8ccb0085c4d72ddade12e`, files
`TTF/SourceSans3-Regular.ttf` and `TTF/SourceSans3-Semibold.ttf`.
`LICENSE.md` is that repository's licence, with Adobe's copyright notice and
the Reserved Font Name `Source`. Because of the Reserved Font Name, these files
must stay unmodified: no subsetting, instancing or renaming.

## Noto Emoji

`NotoEmoji-Regular.ttf` is the monochrome Noto Emoji, version 3.002, from the
[google/fonts repository](https://github.com/google/fonts), file
`ofl/notoemoji/NotoEmoji[wght].ttf` at revision
`b979dba422e445492b0eb9951ac52ee0b4d648c3` (SHA-256
`de6c18832938afc99caf132b39d6a30a19bac7f2e812e28db2535b4608d27551`).
`NotoEmoji-OFL.txt` is that directory's `OFL.txt`, unmodified.

The upstream file is a variable font with a weight axis from 300 to 700.
egui draws one weight per face, so the bundled file is its static regular
instance, made with fontTools 4.60.1:

```text
fonttools varLib.instancer "NotoEmoji[wght].ttf" wght=400 --static \
  --no-recalc-timestamp -o NotoEmoji-Regular.ttf
```

The command is deterministic and gives the hash in the table above. The
instance is about half the size of the variable font, compressed. It is a
Modified Version under the licence, which allows one: `OFL.txt` declares no
Reserved Font Name, so the face keeps the name `Noto Emoji`. Its name table,
version and copyright notice are the upstream ones.

## How the families use the faces

`install_typography_fonts` adds the faces to egui's font definitions. Each
family lists its faces in this order, and egui draws each character from the
first face that maps it:

| Family | Faces |
| --- | --- |
| Stobie Source Sans 3 Regular (named) | Source Sans 3 Regular, Noto Emoji, then egui's proportional fallbacks |
| Stobie Source Sans 3 Semibold (named) | Source Sans 3 Semibold, Noto Emoji, then egui's proportional fallbacks |
| `Proportional` | Source Sans 3 Regular, then egui's proportional faces, then Noto Emoji |
| `Monospace` | egui's monospace faces, then Source Sans 3 Regular, then Noto Emoji |

Noto Emoji follows the text face because it also maps digits, `#`, `*` and
the space; text characters must come from the text face. egui's own faces are
present only when the application enables egui's `default_fonts` feature.
In the `Proportional` and `Monospace` families they come before Noto Emoji, so
there an emoji that egui's older emoji face maps draws from that face. Every
Stobie text role except the monospace technical role uses a named family.

Noto Emoji draws each emoji in one colour, the text colour. egui 0.36 cannot
draw colour glyphs: its rasteriser fills glyph outlines only. The crate's
`tests/colour_emoji.rs` shows a colour font's glyph drawing nothing.

## Omitting egui's default fonts

Stobie no longer enables egui's `default_fonts` feature, which embeds Hack,
Ubuntu Light, an older Noto Emoji and an emoji icon font. An application that
needs none of them turns it off in its own dependencies: it depends on `eframe`
with `default-features = false` and without the `default_fonts` feature, and
on `egui`, if it names egui directly, with `default-features = false`. The
`Monospace` family then draws with Source Sans 3 Regular. An application that
keeps egui's fonts must also ship their notices.

Font installation is additive and idempotent. Call `install_typography_fonts`,
or the design-system application function that calls it, before the first
egui pass; egui activates new fonts at the next pass. An application that
later replaces all font definitions must include these named families in its
replacement.
