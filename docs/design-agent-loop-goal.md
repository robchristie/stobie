# Polyorama design system and agent UI loop

Status: active normative goal

Baseline revision: `b8c66317aaa9284c45e712278010bc9cd285c01b`

## Outcome

Build the first design and agent-control plane for Polyorama. The result must
make dense analytical UI calm, precise, legible, keyboard-operable,
semantically observable and deterministically reviewable while remaining a
layer above egui and wgpu.

The goal includes:

- one documented visual language and one machine-readable design-token source;
- validated deterministic typed Rust generated from that source;
- persisted appearance, contrast, density, font-scale and motion preferences;
- measured text roles and explicit wrapping, truncation and overflow policy;
- reusable production shell components and a native/browser component gallery;
- AccessKit-aligned semantics, stable Polyorama IDs and current geometry;
- one action registry shared by controls, shortcuts, tests and inspection;
- deterministic story rendering, semantic inspection and text-layout audits;
- CI evidence artefacts, focused agent guides and a seed UI evaluation suite;
- complete migration of Analytical Workspace Lab; and
- native, browser, performance, idle-repaint and physical-interaction proof.

The detailed acceptance checklist in this file is the durable repository
summary of the user-provided specification. The active campaign state belongs
in [design-agent-loop-plan.md](design-agent-loop-plan.md); detailed final proof
belongs in `docs/design-agent-loop-report.md`.

## Visual thesis

Polyorama should look like a precise analytical instrument: dense but not
cramped, technically capable without looking unfinished, and visually quiet
enough that imagery, data and current selection remain dominant.

## Task and interaction thesis

The operator's primary goal is to inspect linked scientific views, results and
annotations without losing analytical context. The current model, selection,
camera, tool and worker provenance must remain legible. High-value actions stay
available in the application bar or relevant pane toolbar. Each pane owns its
scrolling; the dock owns workspace layout. Feedback should be immediate and
restrained, with motion used only for continuity and reduced-motion honoured.

## Architectural constraints

- `polyorama-core` remains independent of egui, eframe, wgpu and browser APIs.
- `polyorama-runtime` remains independent of egui and wgpu.
- `polyorama-render-wgpu` owns persistent GPU resources and typed render work.
- `polyorama-ui-egui` is the sole framework crate that understands egui.
- The serialisable `Workspace` remains the only authoritative dock tree.
- Panes continue to use narrow read models and feature output sinks.
- Durable changes continue through intents and validated commands.
- Repaint remains reason-driven; no unconditional loop is permitted.
- Tokens constrain styling; they do not form a runtime stylesheet or UI DSL.
- Egui font layout is the only authority for production text measurement.
- Egui/AccessKit role, name and state are the semantic source where available;
  Polyorama augments them with stable IDs, geometry, actions and domain data.
- Complete result and thumbnail collections must never be materialised.

## Required visual grammar

The system defines semantic surface, text, border, accent, selection, focus and
status roles; one spacing scale; compact and comfortable control geometry;
distinct visual and hit geometry; semantic text roles; a bounded typed icon
vocabulary; and narrow/regular/wide plus shallow/regular/tall pane behaviour.

Every reusable component declares its text alignment, line count, overflow,
minimum useful width, tooltip/semantic completion and responsive strategy.
The required overflow strategies are scroll, wrap, truncate, collapse, move to
overflow controls, or a deliberate minimum-state presentation.

## Mandatory acceptance checklist

### Tokens and preferences

- [ ] One documented bounded DTCG-style JSON subset authors all design tokens.
- [ ] Types, aliases, missing references, cycles and finite values are checked.
- [ ] Light, dark and high-contrast modes form one coherent visual system.
- [ ] Compact and comfortable densities share one component vocabulary.
- [ ] Generated typed Rust is deterministic, checked in and checked for drift.
- [ ] Runtime code consumes typed values rather than arbitrary token strings.
- [ ] Appearance, contrast, density, font scale and motion are orthogonal,
      versioned, validated and persisted independently of document content.
- [ ] Font scale is safely bounded; obsolete preferences fall back predictably.
- [ ] Production style checks reject unmanaged colour, spacing, radius, font
      sizing, icon glyph and character-count text-width shortcuts.

### Text and components

- [ ] Production component text is measured by egui font layout.
- [ ] Character-count-derived text sizing is absent from production UI.
- [ ] Reusable components declare overflow behaviour and semantic full text.
- [ ] Tabs, toolbars, inspector rows, results and diagnostics behave correctly
      with long text, narrow panes and 100%, 125% and 150% font scale.
- [ ] Numeric result columns remain deterministically right aligned.
- [ ] Text observations expose allocation, paint, clip, lines and truncation.
- [ ] Audits reject undeclared overflow, sibling overlap, invalid geometry and
      unexplained alignment deviation.
- [ ] No mandatory story contains accidental clipping or overlap.
- [ ] Production and gallery use the same typed component implementations.

### Gallery, semantics and actions

- [ ] `polyorama-gallery` runs natively and in a browser.
- [ ] Stable typed stories cover required states, themes, densities, font
      scales, widths and composed application scenes without a runtime UI DSL.
- [ ] Every custom interactive control exposes role, name, state, actions,
      focusability, visible focus and a usable hit target.
- [ ] Buttons, tabs and splitters are keyboard operable; splitter adjustment
      and representative shortcuts are tested.
- [ ] One application-owned `ActionKey` drives control presentation, shortcut routing,
      accessibility metadata, semantic tests and physical targeting.
- [ ] Availability is context-sensitive and disabled reasons are observable.
- [ ] A reusable `UiSnapshot` exposes stable semantic IDs, current bounded
      geometry, actions, text observations and pane/domain references.
- [ ] AccessKit and augmented snapshot semantics are checked for disagreement.
- [ ] Migrated physical automation targets current semantic geometry.
- [ ] A compatible egui/AccessKit semantic test path is established without
      downgrading the current egui/wgpu stack.

### Tooling, CI and agent guidance

- [ ] `cargo xtask ui list`, `render`, `inspect`, `audit-text` and `verify`
      provide stable machine-readable outputs and explicit output directories.
- [ ] Selected deterministic visual snapshots use pinned dimensions, data,
      theme, density, fonts and renderer metadata.
- [ ] Snapshot failure emits expected, actual, diff, semantic, text and log
      evidence; CI never updates baselines automatically.
- [ ] CI runs token, semantic, text, gallery, build and architecture gates and
      uploads useful evidence on failure.
- [ ] Focused guides cover components, panes, interactions, tokens,
      accessibility and UI review while root `AGENTS.md` stays concise.
- [ ] At least five frozen UI tasks and a measurable scoring rubric exist.

### Application and integration

- [ ] Analytical Workspace Lab uses the tokens, recipes, actions and semantics
      for its bar, tabs, splitters, pane chrome, toolbars, results, thumbnails,
      inspector, diagnostics, status/error text and appearance controls.
- [ ] Appearance, contrast, density and font scale are usable and persisted.
- [ ] Required dark, light, high-contrast, narrow, long-text, 150%-scale,
      keyboard-focus, loading/error and gallery captures are inspected.
- [ ] GPU sharing/rendering, workers, demands, caches, render plan, docking,
      persistence, linked cameras, annotation editing, exact undo/redo,
      virtualisation and physical native/browser interactions remain green.
- [ ] Warmed idle remains event-driven; tokens/JSON are not parsed per frame.
- [ ] Before/after release observations cover idle, four views, result and
      thumbnail scrolling, gallery, theme switching and font scaling.
- [ ] `cargo xtask verify` passes and the final report maps every criterion to
      directly verified, approximate, blocked or unavailable evidence.

## Required evidence

The campaign must retain exact commands, revisions, environment versions,
native and browser backends, screenshots, semantic snapshots, text
observations, visual diffs, performance observations and idle diagnostics.
Visual opinion alone cannot satisfy a criterion.

## Non-goals

Do not add Figma integration, DOM rendering, CSS or a CSS-like cascade, a
runtime UI description language, runtime LLM-generated core UI, A2UI, MCP UI
control, a replacement for egui or its text renderer, a backend-neutral GUI
abstraction, a comprehensive widget library, complete localisation, a complete
overlay manager, automatic baseline approval, hosted design services, or new
Geometis-specific capability.

## Stop condition

Do not claim completion with an unverified mandatory criterion. A blocker is a
valid terminal state only after retaining a minimal reproduction, exact
versions and commands, attempted routes, affected criteria, completed versus
approximate versus unverified claims, and the smallest input needed to resume.
