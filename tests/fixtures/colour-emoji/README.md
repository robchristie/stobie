# Colour emoji fixtures

Two one-glyph colour fonts that `crates/stobie-ui-egui/tests/colour_emoji.rs`
uses to show how egui draws colour emoji. They are test fixtures only: no
Stobie crate embeds them, and they sit outside every crate's directory so that
no application's font notices need to name them.

Both hold only 😀 (U+1F600), subset from Noto Color Emoji
[v2.051](https://github.com/googlefonts/noto-emoji/releases/tag/v2.051)
(commit `8998f5dd683424a73e2314a8c1f1e359c19e8742`) with fontTools 4.60.1:

```sh
pyftsubset fonts/Noto-COLRv1.ttf --unicodes=U+1F600 --output-file=Noto-COLRv1-1F600.ttf
pyftsubset fonts/NotoColorEmoji.ttf --unicodes=U+1F600 --output-file=NotoColorEmoji-1F600.ttf
```

| File | Colour format | Upstream SHA-256 | Fixture SHA-256 |
| --- | --- | --- | --- |
| `Noto-COLRv1-1F600.ttf` | COLRv1 vector layers | `0ae57fe58645638523ba35f388d93739d292539a9acb84df5700c81b1e1a28d2` | `6f1ab7a8ffdf1787cf27b12b235596f300d2568426735e705db1e71e209461c1` |
| `NotoColorEmoji-1F600.ttf` | CBDT bitmaps | `72a635cb3d2f3524c51620cdde406b217204e8a6a06c6a096ff8ed4b5fd6e27b` | `7a1e7a34b187587e4444861cd91875ed9510fcf9720617034b139bf854c79d1a` |

Noto Color Emoji is Copyright 2022 Google Inc. (its fonts' name tables) and is
distributed under the SIL Open Font License 1.1, which names no Reserved Font
Name. [OFL.txt](OFL.txt) is the upstream `fonts/LICENSE`, unmodified
(SHA-256 `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2`).
The subsets are Modified Versions under that licence.
