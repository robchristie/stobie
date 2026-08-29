# UI review

Review UI changes as observable behaviour, not as a screenshot-only opinion.
Start with the smallest affected Rust test, then render the relevant fixed
gallery story and inspect its semantic and text observations. Use the current
`cargo xtask ui` surface for list, render, inspect and text audit operations;
write output to an explicit disposable directory and never update a baseline
as part of review.

The exact command forms and baseline policy are documented in the
[deterministic snapshot guide](../ui-snapshots/README.md). The canonical
verification command is `cargo xtask ui verify --output-dir <path>`; it has no
baseline-update option and emits a complete failure bundle on drift.

## Required review questions

- Does the component use typed tokens and egui measurement, with no unmanaged
  visual literals or character-count sizing?
- Are overflow, alignment, line limits, full semantic text and narrow behaviour
  declared and visible in the appropriate long-text or narrow story?
- Does `UiSnapshot` contain stable current geometry, correct role/name/state,
  action and pane/domain references without enumerating virtualised data?
- Do AccessKit parity and text-layout audits have no unexplained findings?
- Do pointer, keyboard and—where relevant—physical native/browser paths use
  the same current semantic target?
- Does the warmed UI remain event-driven, with no unconditional repaint?

Use the catalogue's fixed story IDs and recommended viewports. Review at the
changed story's documented theme/density/font-scale state, and add a focused
story when the defect cannot be represented by an existing frozen fixture.
For a material UI change, finish with the relevant native and Wasm build and
the canonical `cargo xtask verify` before delivery. Score frozen tasks with
[`../ui-evaluation-seed.md`](../ui-evaluation-seed.md); retain command output,
snapshots and captures with the evidence owner.
