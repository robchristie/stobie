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
    pub visible_rows: Range<usize>,
    pub materialised_items: Range<usize>,
}

pub fn virtual_grid(
    scroll_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    cell: (f32, f32),
    total: usize,
    overscan_rows: usize,
) -> VirtualGrid {
    let columns = (viewport_width / cell.0).floor().max(1.0) as usize;
    let row_count = total.div_ceil(columns);
    let rows = virtual_rows(scroll_y, viewport_height, cell.1, row_count, overscan_rows);
    let materialised_items = (rows.materialised.start * columns).min(total)
        ..(rows.materialised.end * columns).min(total);
    VirtualGrid {
        columns,
        visible_rows: rows.visible,
        materialised_items,
    }
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
}
