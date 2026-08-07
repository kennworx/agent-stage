//! Packing boxes into rows.
//!
//! The one shape decision the whole layout rests on: rows are **left-aligned**,
//! not centred. Centring a short final row offsets it by half a pitch, which
//! puts its boxes opposite the gaps of every other row and destroys the vertical
//! gutters the edge router runs in. A ragged last row is a small price for a grid
//! edges can actually be routed on.

use super::geom::{count, Point};

/// A box's size, before it has a place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// Where each box landed, and how big the block came out.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Packing {
    pub width: f64,
    pub height: f64,
    /// One offset per input, in input order.
    pub positions: Vec<Point>,
}

/// Pack boxes into rows of at most `cols` items.
///
/// Each item is centred vertically within its row, so mixed-height items —
/// a nested boundary frame beside a plain box — line up on their centres.
pub fn pack_rows(sizes: &[Size], cols: usize, gap_x: f64, gap_y: f64) -> Packing {
    if sizes.is_empty() {
        return Packing::default();
    }
    let per_row = cols.clamp(1, sizes.len());

    let mut positions = vec![Point::new(0.0, 0.0); sizes.len()];
    let mut width: f64 = 0.0;
    let mut y = 0.0;
    for (r, row) in sizes.chunks(per_row).enumerate() {
        let row_w =
            row.iter().map(|s| s.width).sum::<f64>() + gap_x * count(row.len().saturating_sub(1));
        let row_h = row
            .iter()
            .map(|s| s.height)
            .fold(f64::NEG_INFINITY, f64::max);
        width = width.max(row_w);
        let mut x = 0.0;
        for (c, size) in row.iter().enumerate() {
            if let Some(slot) = positions.get_mut(r * per_row + c) {
                *slot = Point::new(x, y + (row_h - size.height) / 2.0);
            }
            x += size.width + gap_x;
        }
        y += row_h + gap_y;
    }

    Packing {
        width,
        height: y - gap_y,
        positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(w: f64, h: f64) -> Size {
        Size::new(w, h)
    }

    #[test]
    fn nothing_packs_to_nothing() {
        assert_eq!(pack_rows(&[], 3, 10.0, 10.0), Packing::default());
    }

    #[test]
    fn a_single_row_runs_left_to_right() {
        let got = pack_rows(&[s(100.0, 50.0), s(100.0, 50.0)], 4, 20.0, 30.0);
        assert_eq!(
            got.positions,
            vec![Point::new(0.0, 0.0), Point::new(120.0, 0.0)]
        );
        assert!((got.width - 220.0).abs() < 1e-9);
        assert!((got.height - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_short_final_row_stays_left_aligned() {
        // The gutters the router needs only exist because this row does not
        // centre itself under the block above.
        let got = pack_rows(&[s(100.0, 50.0); 3], 2, 20.0, 30.0);
        assert_eq!(got.positions.get(2), Some(&Point::new(0.0, 80.0)));
        assert!((got.width - 220.0).abs() < 1e-9);
        assert!((got.height - 130.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_heights_centre_within_their_row() {
        let got = pack_rows(&[s(100.0, 100.0), s(100.0, 40.0)], 2, 0.0, 0.0);
        assert_eq!(got.positions.get(1), Some(&Point::new(100.0, 30.0)));
    }

    #[test]
    fn a_column_count_below_one_still_packs() {
        let got = pack_rows(&[s(10.0, 10.0), s(10.0, 10.0)], 0, 5.0, 5.0);
        assert_eq!(
            got.positions,
            vec![Point::new(0.0, 0.0), Point::new(0.0, 15.0)]
        );
    }

    #[test]
    fn more_columns_than_items_is_one_row() {
        let got = pack_rows(&[s(10.0, 10.0)], 9, 5.0, 5.0);
        assert!((got.width - 10.0).abs() < 1e-9);
        assert!((got.height - 10.0).abs() < 1e-9);
    }
}
