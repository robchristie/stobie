# Bundled typography faces

Source Sans 3 regular and semibold are unmodified TrueType files from Adobe's
[Source Sans repository](https://github.com/adobe-fonts/source-sans), release
branch revision `87b37a2daaed80fcb8e8ccb0085c4d72ddade12e`.

They are distributed under the **SIL Open Font License 1.1**, with Adobe's
copyright notice and the reserved font name `Source` retained in
[LICENSE.md](LICENSE.md). This licence permits bundling and embedding these
unmodified fonts with the application. The fonts retain their original licence;
the surrounding Polyorama Rust implementation remains Apache-2.0.

| Upstream file | SHA-256 |
| --- | --- |
| `TTF/SourceSans3-Regular.ttf` | `4644c81b86ec9caaa76b634889968ed3c4f4f52f054855933acc7c2b21e53b0f` |
| `TTF/SourceSans3-Semibold.ttf` | `a3f4f8dcf343a8f24dc61951de93f3ba1558b15cd250ba24af8a40e957081b7d` |
| `LICENSE.md` | `56af9b9c6715597e458284a474dc118a50a4150e9d547c70f7b4a33c3e6a9328` |

The named regular and semibold families retain egui 0.36.1's bundled
proportional fallback chain, including its emoji coverage. Font installation is
additive and idempotent. Call `install_typography_fonts`, or the design-system
application function that calls it, before the first egui pass; egui activates
new fonts at the next pass. Consumers that later replace all font definitions
must include these named families in their replacement.
