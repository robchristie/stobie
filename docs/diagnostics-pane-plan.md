# Diagnostics pane visibility

Status: complete

Delivery: [Polyorama PR #41](https://github.com/robchristie/polyorama/pull/41).

Starting revision: `83ae311b1fe14df04f57b3738defcddb5dca417f`.
Owner: Polyorama product repository.
Scope: the Lab's Diagnostics pane, generic canonical dock transitions, product
documentation and focused native/browser regressions.

## Objective and design contract

The representative user closes Diagnostics to give the remaining dock more
space, saves and restores the closed layout, then reopens Diagnostics without
losing analytical state. The existing application bar and dock are the visual
reference. View/Panels is the discoverable action; its Diagnostics item shows
the current state. The same action has a keyboard shortcut and semantic name.
The pane reopens beside Inspector. Normal and narrow layouts, focus, an open
and closed toggle, saved restoration and repeated reopening must remain usable.
Loading and error states introduce no new control behaviour; no new pane type or
exact former split restoration is in scope.

## Invariants and evidence

- The serialisable `Workspace` remains the only dock model. The application
  chooses which pane may close; the core keeps open and closed IDs disjoint and
  refuses to remove the last docked pane or reopen an existing instance.
- Existing valid layouts remain accepted. Only Diagnostics may be absent from
  the Lab dock when its ID is in `closed_optional_panes`.
- Focused Rust tests cover transitions and persistence. The canonical
  `cargo xtask verify` path exercises native and browser interactions, builds,
  architecture and UI snapshots.

## Closeout

Core and Lab regressions cover unique open/closed transitions and persisted
analytical state. Physical native and browser journeys cover closing, saved
restoration and reopening; the browser journey also covers a narrow viewport.
Full CI, independent review and landing evidence belong to the delivery pull
request.
