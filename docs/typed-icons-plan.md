# Typed icons and shared action presentations

Status: complete
Delivery: [Polyorama PR #50](https://github.com/robchristie/polyorama/pull/50)

## Outcome and boundaries

`polyorama-ui-egui` owns a public typed icon vocabulary and reusable vector painter,
plus text-only, icon-only and leading icon+label presentations of its existing
typed actions. Preserve `ActionButtonSpec` literals, logical instance identity,
capability targets, availability, response handling and toggle semantics. Application
intent/command ownership and event-driven repaint remain unchanged. Bokkie,
dependency updates, navigation-item recipes and application redesign are excluded.

## Implementation and acceptance contract

- Use project-authored monochrome geometric artwork under Apache-2.0, a shared
  normalised coordinate system and stroke treatment; no fonts, network fetches,
  runtime icon names or application/branded artwork. Theme/token resolution owns
  foreground, icon sizing and component geometry; hit geometry is independent.
- Add presentation APIs without adding fields to `ActionButtonSpec`. Text-only
  retains its measured centred label. Leading icons reserve fixed scaled artwork
  and token spacing before measuring one start-aligned, eliding label. A narrow
  control retains its icon and declared label slot down to padding + icon + gap + measured ellipsis;
  it never implicitly changes to icon-only. Icon-only uses a square minimum hit
  target and records no measured label.
- Complete action names, descriptions and disabled reasons remain authoritative
  in snapshots and AccessKit. Icons are decorative paint with no semantic owner.
  Icon-only always provides a useful tooltip. Preserve pointer/keyboard activation,
  independent visible focus, toggle state, multiple logical instances, clipped
  targets, omitted fully hidden nodes and pane/domain metadata.
- Test representative vector geometry/foregrounds, pointer and keyboard actions,
  disabled/toggle state, identity, clipping and semantic parity, and narrow label
  measurement. Compile maintained consumer examples. Complete final
  `cargo xtask verify`, native and browser paths, independent review and CI.

## UI acceptance and bounded calibration

Task: compose an ordinary desktop toolbar with recognisable actions and a clear
primary action. Reference: Polyorama's analytical token appearance and existing
text action recipes. The intended hierarchy uses quiet icon tools for frequent
local actions, a leading icon+label for a named primary action, and restrained
monochrome artwork. No decorative cards or competing accent colours.

Question: do the shared proportions and reserved label slot remain optically
balanced at ordinary sizes, enlarged text and narrow widths? Smallest probe:
Gallery vocabulary plus one action group in dark comfortable and light compact,
then high contrast at 150% with a long/narrow label. Evidence owner:
`docs/typed-icons-evidence/`. Exit after a bounded two-pass critique (one revision
and re-critique if needed) establishes crisp strokes, balanced centres/spacing,
legible names, hierarchy and distinguishable states. Preserve contradictory
findings if a preference remains unresolved.

Required states: default, hover, pointer press, selected toggle, disabled with
reason and keyboard focus; light/dark and high contrast, compact/comfortable,
100–150% font scale and narrow/long labels. Loading, empty, partial and error
application data are outside these momentary/toggle control fixtures; status
artwork is included in the vocabulary. Gallery calls production APIs. Frozen
UI evaluation tasks remain unchanged; review any deliberate overflow baseline
change separately before accepting it.

## Closeout

The public vocabulary, vector painter, all three shared action presentations,
Gallery compositions and dock overflow adoption are complete. Existing action
and token struct literals remain compatible. The shared active/focus outlines
and Primary hover treatment resolve the reference palette's state ambiguity.

Focused regression coverage includes 108 UI tests and 11 Gallery tests, with
native/WASM builds, the compiled consumer example, inspected native/browser
images and physical action journeys. `cargo xtask verify` passed on source
revision `4e6c6f2593cf10f14e561710f78e335d8238ecba`, including eight zero-tolerance
snapshot fixtures and the production browser/native smokes. The original five
fixtures retain their pixels, semantics and measured observations; only their
honest catalogue coverage count changes. See the
[qualification record](typed-icons-evidence/README.md).

The plan records the verified product state. Independent review, required CI,
eventual squash identity and cleanup are retained on the owning PR. Private
HTTPS gateway access and the existing browser accessibility-adapter limitation
remain explicit; application browser and native acceptance were exercised.
Bokkie integration and the excluded downstream packages remain separate work.
