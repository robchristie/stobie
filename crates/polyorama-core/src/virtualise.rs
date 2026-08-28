use std::ops::Range;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualRange {
    pub visible: Range<usize>,
    pub materialised: Range<usize>,
    pub overscan: usize,
}

pub fn virtual_rows(
    scroll_y: f32,
    viewport_height: f32,
    row_height: f32,
    total: usize,
    overscan: usize,
) -> VirtualRange {
    let start = (scroll_y.max(0.0) / row_height).floor() as usize;
    let end = ((scroll_y + viewport_height).max(0.0) / row_height).ceil() as usize;
    let visible = start.min(total)..end.min(total);
    let materialised =
        visible.start.saturating_sub(overscan)..visible.end.saturating_add(overscan).min(total);
    VirtualRange {
        visible,
        materialised,
        overscan,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGrid {
    pub columns: usize,
    pub total_rows: usize,
    pub visible_rows: Range<usize>,
    pub materialised_rows: Range<usize>,
    pub visible_items: Range<usize>,
    pub materialised_items: Range<usize>,
}

/// Calculate one canonical virtual-grid layout from the rows selected by a
/// presentation integration.
///
/// Egui deliberately expands its visible row range at the viewport boundary,
/// so the presenter supplies that exact range rather than independently
/// reconstructing it from a scroll offset. All row clamping, overscan and item
/// mapping remain renderer- and UI-independent here.
pub fn layout_virtual_grid(
    total_items: usize,
    columns: usize,
    visible_rows: Range<usize>,
    overscan_rows: usize,
) -> VirtualGrid {
    let columns = columns.max(1);
    let total_rows = total_items.div_ceil(columns);
    let visible_start = visible_rows.start.min(total_rows);
    let visible_end = visible_rows.end.clamp(visible_start, total_rows);
    let visible_rows = visible_start..visible_end;
    let materialised_rows = visible_rows.start.saturating_sub(overscan_rows)
        ..visible_rows
            .end
            .saturating_add(overscan_rows)
            .min(total_rows);
    let visible_items = (visible_rows.start * columns).min(total_items)
        ..(visible_rows.end * columns).min(total_items);
    let materialised_items = (materialised_rows.start * columns).min(total_items)
        ..(materialised_rows.end * columns).min(total_items);
    VirtualGrid {
        columns,
        total_rows,
        visible_rows,
        materialised_rows,
        visible_items,
        materialised_items,
    }
}

pub fn virtual_grid(
    scroll_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    cell: (f32, f32),
    total: usize,
    overscan_rows: usize,
) -> VirtualGrid {
    let cell_width = cell.0.max(1.0);
    let cell_height = cell.1.max(1.0);
    let columns = (viewport_width.max(0.0) / cell_width).floor().max(1.0) as usize;
    let row_count = total.div_ceil(columns);
    let rows = virtual_rows(
        scroll_y,
        viewport_height,
        cell_height,
        row_count,
        overscan_rows,
    );
    layout_virtual_grid(total, columns, rows.visible, overscan_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn million_row_range_materialises_a_small_bound() {
        let range = virtual_rows(500_000.0, 720.0, 24.0, 1_000_000, 8);
        assert!(range.materialised.len() < 500);
        assert!(range.visible.start > 0);
    }

    #[test]
    fn thumbnail_grid_materialises_only_visible_region() {
        let grid = virtual_grid(10_000.0, 800.0, 600.0, (112.0, 104.0), 100_000, 2);
        assert!(grid.materialised_items.len() < 200);
        assert!(grid.materialised_items.end < 100_000);
    }

    #[test]
    fn grid_layout_is_canonical_for_partial_rows_and_clipped_overscan() {
        let first = layout_virtual_grid(10, 3, 0..2, 2);
        assert_eq!(first.total_rows, 4);
        assert_eq!(first.visible_rows, 0..2);
        assert_eq!(first.materialised_rows, 0..4);
        assert_eq!(first.visible_items, 0..6);
        assert_eq!(first.materialised_items, 0..10);

        let last = layout_virtual_grid(10, 3, 3..5, 2);
        assert_eq!(last.visible_rows, 3..4);
        assert_eq!(last.materialised_rows, 1..4);
        assert_eq!(last.visible_items, 9..10);
        assert_eq!(last.materialised_items, 3..10);

        let empty = layout_virtual_grid(0, 0, 0..1, 2);
        assert_eq!(empty.columns, 1);
        assert_eq!(empty.total_rows, 0);
        assert!(empty.visible_items.is_empty());
        assert!(empty.materialised_items.is_empty());
    }

    #[test]
    fn scroll_geometry_delegates_to_the_canonical_grid_layout() {
        let from_scroll = virtual_grid(10_000.0, 800.0, 600.0, (112.0, 104.0), 100_000, 2);
        let delegated = layout_virtual_grid(
            100_000,
            from_scroll.columns,
            from_scroll.visible_rows.clone(),
            2,
        );
        assert_eq!(from_scroll, delegated);
    }
}
