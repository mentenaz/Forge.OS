//! How a grid of character cells lies on a pixel framebuffer.
//!
//! A pixel backend needs two answers: how many cells fit on the screen, and
//! where a given cell starts. Both are arithmetic, so they live here where the
//! host can test them, and the backend keeps only the pixel writes.

use crate::Size;

/// A cell grid laid out with the same margin on all four sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellGeometry {
    margin: usize,
    cell_width: usize,
    cell_height: usize,
}

impl CellGeometry {
    /// A layout with `margin` pixels around the grid and cells of
    /// `cell_width` by `cell_height` pixels.
    pub const fn new(margin: usize, cell_width: usize, cell_height: usize) -> Self {
        Self {
            margin,
            cell_width,
            cell_height,
        }
    }

    /// The largest grid that fits a `width` by `height` pixel framebuffer
    /// with the margin kept on every side.
    ///
    /// A framebuffer too small for one cell, or a cell with no size, gives an
    /// empty grid. Each side is capped at `u16::MAX` cells.
    pub const fn fit(&self, width: usize, height: usize) -> Size {
        Size::new(
            fit_axis(width, self.margin, self.cell_width),
            fit_axis(height, self.margin, self.cell_height),
        )
    }

    /// The pixel position of the top left corner of a cell, or `None` if the
    /// position does not fit in a `usize`.
    pub fn origin(&self, col: usize, row: usize) -> Option<(usize, usize)> {
        let x = self.cell_width.checked_mul(col)?.checked_add(self.margin)?;
        let y = self
            .cell_height
            .checked_mul(row)?
            .checked_add(self.margin)?;
        Some((x, y))
    }
}

const fn fit_axis(pixels: usize, margin: usize, cell: usize) -> u16 {
    if cell == 0 {
        return 0;
    }
    let cells = pixels.saturating_sub(margin.saturating_mul(2)) / cell;
    if cells > u16::MAX as usize {
        u16::MAX
    } else {
        cells as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typical_framebuffer_fits_whole_cells_inside_its_margins() {
        let geometry = CellGeometry::new(8, 9, 18);
        assert_eq!(geometry.fit(1024, 768), Size::new(112, 41));
    }

    #[test]
    fn the_last_cell_fits_exactly_and_one_pixel_less_loses_it() {
        let geometry = CellGeometry::new(8, 9, 18);
        // 2 * 8 + 112 * 9 = 1024 and 2 * 8 + 41 * 18 = 754.
        assert_eq!(geometry.fit(1024, 754).cols, 112);
        assert_eq!(geometry.fit(1023, 754).cols, 111);
        assert_eq!(geometry.fit(1024, 754).rows, 41);
        assert_eq!(geometry.fit(1024, 753).rows, 40);
    }

    #[test]
    fn a_framebuffer_too_small_for_a_cell_gives_an_empty_grid() {
        let geometry = CellGeometry::new(8, 9, 18);
        assert_eq!(geometry.fit(0, 0), Size::new(0, 0));
        assert_eq!(geometry.fit(10, 10), Size::new(0, 0));
        assert_eq!(geometry.fit(16, 16), Size::new(0, 0));
        assert_eq!(geometry.fit(24, 33), Size::new(0, 0));
        assert_eq!(geometry.fit(25, 34), Size::new(1, 1));
    }

    #[test]
    fn a_margin_larger_than_the_framebuffer_is_not_an_underflow() {
        let geometry = CellGeometry::new(usize::MAX, 9, 18);
        assert_eq!(geometry.fit(1024, 768), Size::new(0, 0));
    }

    #[test]
    fn a_cell_with_no_size_gives_an_empty_grid() {
        assert_eq!(CellGeometry::new(8, 0, 18).fit(1024, 768).cols, 0);
        assert_eq!(CellGeometry::new(8, 9, 0).fit(1024, 768).rows, 0);
    }

    #[test]
    fn each_side_is_capped_at_the_largest_size() {
        let geometry = CellGeometry::new(0, 1, 1);
        assert_eq!(
            geometry.fit(1_000_000, 1_000_000),
            Size::new(u16::MAX, u16::MAX)
        );
    }

    #[test]
    fn width_and_height_are_not_mixed_up() {
        let geometry = CellGeometry::new(0, 10, 100);
        assert_eq!(geometry.fit(1000, 1000), Size::new(100, 10));
    }

    #[test]
    fn cells_start_after_the_margin_and_step_by_the_cell_size() {
        let geometry = CellGeometry::new(8, 9, 18);
        assert_eq!(geometry.origin(0, 0), Some((8, 8)));
        assert_eq!(geometry.origin(2, 3), Some((26, 62)));
        assert_eq!(geometry.origin(1, 0), Some((17, 8)));
        assert_eq!(geometry.origin(0, 1), Some((8, 26)));
    }

    #[test]
    fn a_position_beyond_usize_is_refused_on_each_axis() {
        assert_eq!(CellGeometry::new(0, usize::MAX, 1).origin(2, 0), None);
        assert_eq!(CellGeometry::new(0, 1, usize::MAX).origin(0, 2), None);
        assert_eq!(CellGeometry::new(usize::MAX, 1, 1).origin(1, 0), None);
        assert_eq!(CellGeometry::new(usize::MAX, 1, 1).origin(0, 1), None);
    }

    #[test]
    fn every_cell_of_the_fitted_grid_is_on_screen_and_one_more_is_not() {
        let geometry = CellGeometry::new(3, 5, 7);
        for width in 0..120 {
            for height in 0..120 {
                let size = geometry.fit(width, height);
                let (cols, rows) = (usize::from(size.cols), usize::from(size.rows));
                if cols > 0 && rows > 0 {
                    let (x, y) = geometry.origin(cols - 1, rows - 1).unwrap();
                    assert!(x + 5 + 3 <= width, "{width}x{height}");
                    assert!(y + 7 + 3 <= height, "{width}x{height}");
                }
                let (x, _) = geometry.origin(cols, 0).unwrap();
                let (_, y) = geometry.origin(0, rows).unwrap();
                assert!(x + 5 + 3 > width, "{width}x{height}");
                assert!(y + 7 + 3 > height, "{width}x{height}");
            }
        }
    }
}
