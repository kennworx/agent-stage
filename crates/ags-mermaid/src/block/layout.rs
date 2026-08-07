//! Where each block sits on the grid.
//!
//! Every cell is the same size, so a `(col, row)` maps straight to a pixel
//! position — no graph layout is involved. Wires are straight lines clipped to
//! the two blocks' borders, so an arrowhead lands on an edge rather than
//! disappearing under the box it points at.

use crate::round::count;
use crate::scene::Point;

use super::types::Diagram;

pub const PADDING: f64 = 40.0;
pub const GAP_X: f64 = 36.0;
pub const GAP_Y: f64 = 36.0;
pub const BLOCK_HEIGHT: f64 = 48.0;
pub const MIN_BLOCK_WIDTH: f64 = 80.0;
pub const LABEL_PADDING: f64 = 20.0;
pub const LABEL_FONT: f64 = 13.0;
pub const LABEL_WEIGHT: u32 = 500;

/// One block, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedBlock {
    pub id: String,
    pub label: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

impl PlacedBlock {
    fn centre(&self) -> Point {
        Point::new(self.at.x + self.width / 2.0, self.at.y + self.height / 2.0)
    }
}

/// One wire, placed: the two points where it meets its blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEdge {
    pub source: String,
    pub target: String,
    pub from: Point,
    pub to: Point,
    /// Bends between the ends, when a straight line would cross a block that the
    /// edge does not name. Usually empty — the grid puts most related blocks
    /// next to each other.
    pub via: Vec<Point>,
}

impl PlacedEdge {
    /// The whole run, ends included.
    pub fn points(&self) -> Vec<Point> {
        let mut out = vec![self.from];
        out.extend(self.via.iter().copied());
        out.push(self.to);
        out
    }
}

/// A laid-out block diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub blocks: Vec<PlacedBlock>,
    pub edges: Vec<PlacedEdge>,
}

/// Whether the segment `from`–`to` passes through `block`.
///
/// Sampled rather than solved: the run is a straight line between two cells of a
/// grid, so a handful of points along it settles the question, and the arithmetic
/// stays something a reader can check.
fn crosses(from: Point, to: Point, block: &PlacedBlock) -> bool {
    const STEPS: usize = 24;
    (1..STEPS).any(|step| {
        let along = count(step) / count(STEPS);
        let x = (to.x - from.x).mul_add(along, from.x);
        let y = (to.y - from.y).mul_add(along, from.y);
        x > block.at.x
            && x < block.at.x + block.width
            && y > block.at.y
            && y < block.at.y + block.height
    })
}

/// One edge, routed round any block that is in the way.
///
/// A straight line between two cells on the same row runs through whatever sits
/// between them, and the drawing then says something the source did not: with
/// `API --> Cache` drawn through `Auth`, a reader sees Auth connected to Cache.
/// So the run drops into the gap below its row and comes back up, which reads as
/// one edge going past rather than two edges meeting.
fn detoured(
    a: &PlacedBlock,
    b: &PlacedBlock,
    source: String,
    target: String,
    blocks: &[PlacedBlock],
) -> PlacedEdge {
    let straight = PlacedEdge {
        source,
        target,
        from: border_point(a, b.centre()),
        to: border_point(b, a.centre()),
        via: Vec::new(),
    };
    let blocked = blocks
        .iter()
        .filter(|other| other.id != a.id && other.id != b.id)
        .any(|other| crosses(straight.from, straight.to, other));
    if !blocked {
        return straight;
    }
    // Under the lower of the two, far enough down to clear whatever is between.
    let below = (a.at.y + a.height).max(b.at.y + b.height) + GAP_Y / 2.0;
    // Off-centre, leaning the way it is going. Down the middle it would share a
    // line with an edge that genuinely drops straight down — `API --> Database`
    // and the detour to `Cache` left by the same point and read as one wire for
    // the first stretch.
    let lean = |block: &PlacedBlock, toward: f64| {
        let side = if toward > block.centre().x {
            0.75
        } else {
            0.25
        };
        block.at.x + block.width * side
    };
    let (out_x, in_x) = (lean(a, b.centre().x), lean(b, a.centre().x));
    PlacedEdge {
        from: Point::new(out_x, a.at.y + a.height),
        to: Point::new(in_x, b.at.y + b.height),
        via: vec![Point::new(out_x, below), Point::new(in_x, below)],
        ..straight
    }
}

/// Where the ray from the centre of `rect` toward `towards` leaves the border.
///
/// Scaling by whichever axis runs out first is what picks the side: the smaller
/// factor is the border the ray reaches without leaving the rectangle.
fn border_point(rect: &PlacedBlock, towards: Point) -> Point {
    let centre = rect.centre();
    let dx = towards.x - centre.x;
    let dy = towards.y - centre.y;
    // Two blocks sharing a centre give no direction to leave along.
    if dx == 0.0 && dy == 0.0 {
        return centre;
    }
    let scale_x = if dx == 0.0 {
        f64::INFINITY
    } else {
        rect.width / 2.0 / dx.abs()
    };
    let scale_y = if dy == 0.0 {
        f64::INFINITY
    } else {
        rect.height / 2.0 / dy.abs()
    };
    let scale = scale_x.min(scale_y);
    Point::new(centre.x + dx * scale, centre.y + dy * scale)
}

/// The width every cell takes: enough for the longest label anywhere.
///
/// Uniform rather than per-column, because a grid whose columns each shrink to
/// their own content stops reading as a grid.
fn cell_width(diagram: &Diagram) -> f64 {
    let widest = diagram
        .blocks
        .iter()
        .map(|b| crate::metrics::text_width(&b.label, LABEL_FONT, LABEL_WEIGHT))
        .fold(0.0_f64, f64::max);
    MIN_BLOCK_WIDTH.max(widest.ceil() + LABEL_PADDING * 2.0)
}

/// The point each block's wires are aimed at, by id.
///
/// A repeated id keeps the last block that claimed it, which is what a lookup
/// built by assignment does — and an ambiguous edge has to pick something.
fn by_id(blocks: &[PlacedBlock]) -> Vec<(&str, &PlacedBlock)> {
    let mut out: Vec<(&str, &PlacedBlock)> = Vec::new();
    for block in blocks {
        if let Some(slot) = out.iter_mut().find(|(id, _)| *id == block.id) {
            slot.1 = block;
        } else {
            out.push((block.id.as_str(), block));
        }
    }
    out
}

/// Lay out a parsed block diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    let block_width = cell_width(diagram);
    let blocks: Vec<PlacedBlock> = diagram
        .blocks
        .iter()
        .map(|b| PlacedBlock {
            id: b.id.clone(),
            label: b.label.clone(),
            at: Point::new(
                PADDING + count(b.col) * (block_width + GAP_X),
                PADDING + count(b.row) * (BLOCK_HEIGHT + GAP_Y),
            ),
            width: block_width,
            height: BLOCK_HEIGHT,
        })
        .collect();

    let index = by_id(&blocks);
    let find = |id: &str| index.iter().find(|(k, _)| *k == id).map(|(_, b)| *b);
    let edges: Vec<PlacedEdge> = diagram
        .edges
        .iter()
        // An edge naming a block that was never written is dropped rather than
        // drawn to nowhere.
        .filter_map(|e| {
            let a = find(&e.source)?;
            let b = find(&e.target)?;
            Some(detoured(a, b, e.source.clone(), e.target.clone(), &blocks))
        })
        .collect();

    let rows = diagram.blocks.iter().map(|b| b.row + 1).max().unwrap_or(0);
    let cols = diagram.columns;
    Placed {
        width: PADDING * 2.0 + count(cols) * block_width + (count(cols) - 1.0) * GAP_X,
        height: PADDING * 2.0 + count(rows) * BLOCK_HEIGHT + (count(rows.max(1)) - 1.0) * GAP_Y,
        blocks,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    #[test]
    fn a_wire_past_a_block_goes_round_it_rather_than_through_it() {
        // Straight from A to C runs through B, and the drawing then says B is
        // connected to C when the source never said so.
        let out = placed("block-beta\ncolumns 3\nA B C\nA --> C");
        let edge = &out.edges[0];
        assert!(!edge.via.is_empty(), "it has to bend: {edge:?}");
        let b = &out.blocks[1];
        for pair in edge.points().windows(2) {
            assert!(
                !crosses(pair[0], pair[1], b),
                "no part of it may cross B: {:?}",
                edge.points()
            );
        }
    }

    #[test]
    fn a_wire_with_nothing_in_the_way_stays_straight() {
        let out = placed("block-beta\ncolumns 2\nA B\nA --> B");
        assert!(out.edges[0].via.is_empty(), "{:?}", out.edges[0]);
        assert_eq!(out.edges[0].points().len(), 2);
    }

    #[test]
    fn a_detour_leans_the_way_it_is_going() {
        // Down the middle it would share its first stretch with an edge that
        // genuinely drops straight down, and the two would read as one wire.
        let out = placed("block-beta\ncolumns 3\nA B C\nD space space\nA --> C\nA --> D");
        let round = &out.edges[0];
        let down = &out.edges[1];
        assert!(
            round.from.x > down.from.x,
            "the detour leaves right of the drop: {round:?} {down:?}"
        );
    }

    #[test]
    fn a_cell_position_follows_its_grid_coordinates() {
        let out = placed("block-beta\ncolumns 2\nA B\nC D");
        let pitch_x = out.blocks[0].width + GAP_X;
        assert!((out.blocks[1].at.x - (PADDING + pitch_x)).abs() < 1e-9);
        assert!((out.blocks[2].at.y - (PADDING + BLOCK_HEIGHT + GAP_Y)).abs() < 1e-9);
    }

    #[test]
    fn every_cell_is_the_same_width_and_it_fits_the_longest_label() {
        let out = placed("block-beta\ncolumns 2\nA[\"x\"] B[\"a much longer label\"]");
        assert!((out.blocks[0].width - out.blocks[1].width).abs() < 1e-9);
        let needed =
            crate::metrics::text_width("a much longer label", LABEL_FONT, LABEL_WEIGHT).ceil();
        assert!(out.blocks[0].width >= needed + LABEL_PADDING * 2.0);
    }

    #[test]
    fn a_short_label_still_gets_a_minimum_width() {
        assert!((placed("block-beta\nA[\"x\"]").blocks[0].width - MIN_BLOCK_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn a_wire_stops_at_the_border_rather_than_the_centre() {
        let out = placed("block-beta\ncolumns 2\nA B\nA --> B");
        let edge = &out.edges[0];
        let a = &out.blocks[0];
        // Leaving rightward, so it lands on the right-hand edge at mid-height.
        assert!((edge.from.x - (a.at.x + a.width)).abs() < 1e-9);
        assert!((edge.from.y - (a.at.y + a.height / 2.0)).abs() < 1e-9);
        assert!(edge.to.x < out.blocks[1].at.x + 1e-9);
    }

    #[test]
    fn a_vertical_wire_leaves_through_the_horizontal_border() {
        let out = placed("block-beta\ncolumns 1\nA\nB\nA --> B");
        let a = &out.blocks[0];
        assert!((out.edges[0].from.y - (a.at.y + a.height)).abs() < 1e-9);
        assert!((out.edges[0].from.x - (a.at.x + a.width / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn two_blocks_sharing_a_centre_give_a_wire_no_direction_to_leave_along() {
        let block = PlacedBlock {
            id: "a".into(),
            label: "a".into(),
            at: Point::new(0.0, 0.0),
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(border_point(&block, block.centre()), block.centre());
    }

    #[test]
    fn a_wire_to_a_block_that_was_never_written_is_dropped() {
        assert!(placed("block-beta\nA\nA --> Ghost").edges.is_empty());
        assert!(placed("block-beta\nA\nGhost --> A").edges.is_empty());
    }

    #[test]
    fn a_repeated_id_resolves_to_the_last_block_that_claimed_it() {
        let out = placed("block-beta\ncolumns 1\nA\nA\nB\nB --> A");
        // The second A is one row lower, so the wire aims down-to-up at it.
        assert!(out.edges[0].to.y > out.blocks[0].at.y + BLOCK_HEIGHT);
    }

    #[test]
    fn the_canvas_follows_the_declared_width_and_the_rows_used() {
        let one = placed("block-beta\ncolumns 2\nA B");
        let two = placed("block-beta\ncolumns 2\nA B\nC D");
        assert!((one.width - two.width).abs() < 1e-9);
        assert!((two.height - one.height - (BLOCK_HEIGHT + GAP_Y)).abs() < 1e-9);
    }

    #[test]
    fn an_empty_diagram_is_padding_alone() {
        let out = placed("block-beta");
        assert!((out.height - PADDING * 2.0).abs() < 1e-9);
        assert!((out.width - (PADDING * 2.0 + MIN_BLOCK_WIDTH)).abs() < 1e-9);
    }
}
