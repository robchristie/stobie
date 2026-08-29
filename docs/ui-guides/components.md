# Components

Use the production recipes in `polyorama-ui-egui`; the gallery must call those
same recipes rather than recreate their appearance. A component is responsible
for its allocation, painted bounds, hit bounds, stable semantic identity and
declared text contract. It is not responsible for document mutation or owning
the application model.

## Required contract

Before adding or changing a reusable component, state and test:

- its text role, horizontal alignment, line limit and overflow policy;
- minimum useful width, narrow behaviour and whether it moves work into an
  explicit overflow control;
- complete semantic name/description, role, state, actions and visible focus;
- visual and minimum hit geometry separately; and
- bounded observations required for `UiSnapshot` and text auditing.

Use `TextSpec`, `TextRole` and `TextOverflow`; production widths come from
egui galley measurement, never character counts. Keep numeric values end
aligned. Retain full semantic text when paint is elided. The five supported
policies are `ellipsis`, `wrap`, `clip`, `scroll` and `expand`; choose one
explicitly rather than relying on incidental clipping.

For dense chrome, preserve a token minimum hit target even when compact visual
geometry is smaller. Do not add decorative cards: spacing, aligned text and
the surface hierarchy should communicate grouping first.

## Existing recipe families

The current component layer covers action buttons, dock tabs and overflow,
splitters, application and pane toolbars, property rows, result rows, status
badges and virtual thumbnail cells. Their deterministic examples are the
gallery catalogue stories, including `tabs/many-long-labels`, `tabs/narrow`,
`splitter/hover-active`, `toolbar/narrow`, `property-row/long-value`,
`status/error-long-message`, and the virtual-grid stories.

When a new state is consequential, add a typed gallery story and a bounded
fixture rather than a runtime description or unbounded data set. See
[UI review](ui-review.md) for the evidence loop.
