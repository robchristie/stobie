# Linux Orca/Xvfb accessibility evidence

Status: partially qualified with actual assistive technology; not a direct
end-user platform claim

A later human-operated Debian GNOME/RDP session directly qualified its exact
environment; see the
[Debian GNOME/Orca RDP evidence](linux-gnome-rdp.md). The Arch/Xvfb result below
remains partial and independently useful as the richer retained Orca
transcript.

## Evidence identity

- source revision: `17618d5553e845a97dbba38ad44ca994fc117928`;
- release binary SHA-256:
  `1e956845d195aaf75b76c14d137772b222d8f0ff48a24224cd27c803b3104ec1`;
- session: 4 September 2026, 05:08:59–05:11:28 UTC;
- host: Arch Linux rolling, Linux `7.1.3-arch1-1`, x86-64, rootless Podman
  6.0.0;
- container: Arch Linux build `20260222.0.493200`, image
  `docker.io/library/archlinux:base`, digest
  `sha256:bb1e5dd58eb79755e736ac530292074f4408572c0fbc4306cd62b431fdf356f0`;
- assistive technology: Orca 50.2-1, AT-SPI2 2.60.6-1, Speech Dispatcher
  0.12.1-3 and espeak-ng 1.52.0-1;
- display/input: Xvfb 21.1.24-1 at 1440×900×24 and xdotool
  4.20260303.1-1; and
- graphics: eframe/wgpu OpenGL on Mesa 26.2.2 llvmpipe, reported as
  `llvmpipe (LLVM 22.1.8, 256 bits)`.

The exact host release binary was mounted read-only into the container. The
session ran under `dbus-run-session`; enabled GNOME toolkit accessibility;
started the application; changed
`org.gnome.desktop.a11y.applications screen-reader-enabled` from false to true
so AccessKit observed the canonical AT-SPI activation transition; and then
started Orca with its debug transcript directed to the evidence directory.
Speech Dispatcher was live throughout. A separate PyGObject AT-SPI walk was
retained as supporting evidence only; it was not treated as a substitute for
Orca.

## Representative workflow result

The workflow completed with exit status 0. Focus checkpoints were resolved
from the application's bounded observational snapshot, while all user-facing
observations below came from actual Orca speech output.

| Step | Input and retained state | Orca observation |
| --- | --- | --- |
| Locate application and status | AT-SPI listed `analytical-workspace-lab`; flat review returned to the application bar | `Screen reader on`; later `Analytical Workspace Lab … Workspace ready · 0 decoded thumbnails · 0 decoding` |
| Application action | Tab focused Save layout; Space created a 1,358-byte persisted layout with SHA-256 `f4c2202ab97005ae47223b5ea5c75dc9254b9905843435ee5150718163b70dfb` | `Save layout`; `button`; `Persist the current workspace layout` |
| Disabled action and reason | Orca+Control+Left moved object navigation through Redo to the non-Tabbable disabled Undo control | `Undo`; `button`; `Undo the most recent change; unavailable: History is empty` |
| Dock and splitter | Tab traversed three splitters and Primary View; Right adjusted splitter 1 | `Vertical splitter`; `Resize adjacent dock panes`. The workspace hash changed from `40f5bb36d8d13921` to `3ef14346c59e6d9c` and undo depth from 0 to 1 |
| Pane and viewport | Enter activated Primary View; Tab reached its toolbar and Canvas | `Primary View viewport`; `canvas`; the complete active-pane, Navigate-tool, linked-camera, no-selection and available-action description |
| Dynamic tool state | Shift+Tab reached Polygon; Space selected it; Tab returned to the Canvas | Polygon was a `toggle button` and Orca announced `pressed`, then described `active tool: Polygon` when the viewport regained focus |
| Non-pointer viewport action | Shift+Tab reached Fit view; Space invoked it; Tab returned to the Canvas | `Fit view`; `button`; the viewport scale changed from `256.00` to `238.03 image pixels per screen point`, was spoken by Orca, and undo depth rose from 1 to 2 |
| Bounded result selection | Tab reached Results, Enter activated it, and Tab reached materialised row 1; Space selected it | `#0000001; 81006, 42945; 86.2%; Target`; `selected`; Where Am I repeated the item as `2 of 21` materialised rows, without a checkable-state announcement |
| Inspect selection | Tab reached Inspector and Enter activated it; Orca flat review traversed its selection content | `Inspector`; `Selection`; `Result`; `#1`; `Position`. The same live AT-SPI tree exposed position `81006.0, 42945.0`, confidence `86.23%` and category `Target` |

The non-pointer viewport operation deliberately used the registered **Fit
view** toolbar button. Although the Canvas carries AccessKit custom-action
metadata for transports that support it, the pinned
`accesskit_atspi_common 0.18.1` adapter neither enumerates nor dispatches those
custom actions through AT-SPI. The Linux result therefore proves the discoverable
Canvas context plus the toolbar alternative; it does not claim that Orca can
invoke the Canvas custom actions themselves.

The final supporting tree contained 92 children under one application frame,
including 21 materialised result list items for a logical collection of one
million rows. It exposed exactly one selected row and described every image
viewport with result 1 without materialising the complete result set.
Momentary actions were ordinary buttons, only tool/link modes were toggle
buttons, Canvas nodes had no checkable state, and the complete Orca transcript
contained no `checked` or `not checked` speech for result rows.

Representative Orca speech excerpts, with timestamps from the retained debug
transcript, are:

```text
05:08:59.928447  Screen reader on.
05:09:09.452454  Save layout
05:09:12.836724  Undo
05:09:12.838784  Undo the most recent change; unavailable: History is empty.
05:09:17.483867  Vertical splitter.
05:09:36.064252  Primary View viewport
05:09:46.891614  Scientific scalar image; active pane; active tool: Polygon; … scale: 256.00 …
05:09:54.618308  Scientific scalar image; active pane; active tool: Polygon; … scale: 238.03 …
05:10:45.447596  #0000001; 81006, 42945; 86.2%; Target.
05:10:46.727380  selected
05:11:13.837403  Inspector
05:11:17.159683  Selection
05:11:18.690514  Result
05:11:22.519584  Position
05:11:24.825621  Analytical Workspace Lab … Workspace ready 0 decoded thumbnails · 0 decoding
```

## Artefact manifest

The full local evidence was retained under the ignored
`.tools/runtime/accessibility-at-probe` directory. The checked-in transcript
and observations above are the durable subset. The raw artefact identities are:

| Artefact | Bytes | SHA-256 |
| --- | ---: | --- |
| `orca-debug.log` | 8,182,105 | `9d3e09051de4ade090bb901034ac55d258aff5c314a203cb15399b2d5463e796` |
| `atspi-tree.txt` | 14,542 | `9f5fc47f2eab50ac98e1e08bf41fb993468cfd617b1960638664ec6d4d6aaf82` |
| `workflow-steps.log` | 4,331 | `8dbcb5c2a79911c8fd4e7022f7df73a25422f4e640cf93e83613b649fe6a8cba` |
| `workflow-state.jsonl` | 47,899 | `c65d7c8b691541275be964ff40c8bf5348698d5d113641225945b5bd4724eba8` |
| `probe.png` | 1,039,119 | `f87a9edefb72ff66f13e982daa3ba01338ff1ae3352976ddda80f8eeeb3049c4` |

The AT-SPI client emitted two non-fatal environment warnings: the Arch
AT-SPI2 package lacked libXRes pointer-monitor support, and AccessKit did not
provide the optional `/org/a11y/atspi/cache` object. Direct accessible-object
queries, focus/state events, Orca speech generation and the complete workflow
continued successfully.

## Qualification boundary

This is actual Orca use against the exact native AccessKit adapter, not an
automated tree-only claim. It remains **partial** because it used a rootless
container, Xvfb, synthetic keyboard input and a recorded speech pipeline rather
than a human-operated X11 or Wayland desktop with heard speech or braille.
Therefore it does not establish a general Linux end-user support claim.

The smallest remaining native session is to run the same candidate and
workflow on a real Linux X11 or Wayland desktop with a human Orca user, record
the distribution, desktop, Orca/AT-SPI2/Speech Dispatcher versions and graphics
backend, and retain the observed speech or braille result. Windows, macOS and
all browser combinations remain independently unqualified.
