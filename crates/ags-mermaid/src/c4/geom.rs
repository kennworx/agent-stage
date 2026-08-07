//! Points, rectangles and the measurements taken over them.
//!
//! Everything here is pure geometry with no notion of a diagram, which is what
//! lets the routing, separation and label passes share one vocabulary instead of
//! each carrying its own overlap test.

use crate::round::round_half_up;

/// A point in diagram space.
///
/// The scene's own point type, rather than a private twin: layout and drawing
/// speak about the same coordinates, and a conversion at the boundary between
/// them would be pure noise with a transposition bug waiting inside it.
pub use crate::scene::Point;

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center_x(&self) -> f64 {
        self.x + self.width / 2.0
    }

    pub fn center_y(&self) -> f64 {
        self.y + self.height / 2.0
    }
}

/// Which face of a box an edge attaches to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    /// Whether the face runs horizontally, so ports along it slide in x.
    pub fn is_horizontal_face(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// The faces an edge leaves and arrives on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidePair {
    pub start: Side,
    pub end: Side,
}

impl SidePair {
    /// Whether the two ends look straight at each other across a gutter.
    pub fn facing(self) -> bool {
        matches!(
            (self.start, self.end),
            (Side::Bottom, Side::Top)
                | (Side::Top, Side::Bottom)
                | (Side::Right, Side::Left)
                | (Side::Left, Side::Right)
        )
    }
}

/// Which axis a run travels along.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    H,
    V,
}

impl Axis {
    pub fn other(self) -> Self {
        match self {
            Axis::H => Axis::V,
            Axis::V => Axis::H,
        }
    }
}

/// Below this, two coordinates are the same coordinate.
pub const EPS: f64 = 0.5;

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    lo.max(hi.min(v))
}

/// A coordinate quantised to a tenth, as the identity of a lane or a point.
///
/// JavaScript's `toFixed(1)` breaks a tie toward positive infinity, which is
/// what [`round_half_up`] does; the reference keyed its congestion and occupancy
/// maps on that string, and an identity that split differently would send two
/// edges down a lane the other implementation considered one.
///
/// Returned as tenths rather than text because every coordinate reaching here is
/// positive — the lattice is built from placed boxes, which start at the canvas
/// padding — so the one case where the two differ, `-0.0` against `0.0`, cannot
/// arise.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a diagram coordinate in tenths is far inside i64; the input is a placed pixel position, not arbitrary"
)]
pub fn fixed1(v: f64) -> i64 {
    round_half_up(v * 10.0) as i64
}

/// A count as a coordinate.
///
/// Every count that reaches arithmetic here is a number of boxes, lanes, rows or
/// edges in one diagram — orders of magnitude below the point where an `f64`
/// starts skipping integers.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts of elements in a diagram, never near 2^53"
)]
pub fn count(n: usize) -> f64 {
    n as f64
}

/// A non-negative measure truncated to an index, as `Math.floor` does.
///
/// The reference floors; for the values here — a scaled random fraction, a column
/// count — that is the same as truncating toward zero.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "callers pass a non-negative value bounded by a collection length"
)]
pub fn whole(v: f64) -> usize {
    v as usize
}

/// Centre of one face of a box — the neutral port used while trialling sides.
pub fn face_point(el: &Rect, side: Side) -> Point {
    match side {
        Side::Top => Point::new(el.center_x(), el.y),
        Side::Bottom => Point::new(el.center_x(), el.bottom()),
        Side::Left => Point::new(el.x, el.center_y()),
        Side::Right => Point::new(el.right(), el.center_y()),
    }
}

/// Gap between two boxes, measured edge to edge rather than centre to centre.
///
/// Two boxes sitting side by side score ~0 however wide they are, so "these are
/// neighbours, the line between them should be straight" ranks ahead of a hop
/// across the diagram — which centre-to-centre distance would confuse with it
/// whenever the boxes are large.
pub fn separation(a: &Rect, b: &Rect) -> f64 {
    let dx = 0.0_f64.max((a.x - b.right()).max(b.x - a.right()));
    let dy = 0.0_f64.max((a.y - b.bottom()).max(b.y - a.bottom()));
    dx + dy
}

/// Drop consecutive duplicate points, which would render as zero-length segments.
pub fn dedupe(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        let same = out
            .last()
            .is_some_and(|q| (p.x - q.x).abs() <= EPS && (p.y - q.y).abs() <= EPS);
        if !same {
            out.push(*p);
        }
    }
    out
}

/// Drop waypoints that lie on a straight run.
///
/// A lattice path threads through every lane intersection it passes, so a single
/// straight leg arrives as a string of collinear points. Left in, the renderer
/// treats each one as a corner and rounds it, which puts a visible kink in what
/// should be one clean line — and makes the route look like it has ten bends when
/// it has two.
pub fn simplify(points: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    for p in points {
        let collinear = match (out.len().checked_sub(2), out.len().checked_sub(1)) {
            (Some(i), Some(j)) => match (out.get(i), out.get(j)) {
                (Some(a), Some(b)) => {
                    let straight_x = (a.x - b.x).abs() < EPS && (b.x - p.x).abs() < EPS;
                    let straight_y = (a.y - b.y).abs() < EPS && (b.y - p.y).abs() < EPS;
                    straight_x || straight_y
                }
                _ => false,
            },
            _ => false,
        };
        if collinear {
            if let Some(last) = out.last_mut() {
                *last = *p;
            }
        } else {
            out.push(*p);
        }
    }
    out
}

/// Manhattan length of a polyline.
pub fn path_length(points: &[Point]) -> f64 {
    legs(points)
        .into_iter()
        .map(|(a, b)| (b.x - a.x).abs() + (b.y - a.y).abs())
        .sum()
}

/// Corners in a polyline — a direction change, not merely a waypoint.
pub fn bend_count(points: &[Point]) -> usize {
    points
        .windows(3)
        .filter(|w| match (w.first(), w.get(1), w.get(2)) {
            (Some(a), Some(b), Some(c)) => {
                let straight = ((a.x - b.x).abs() < EPS && (b.x - c.x).abs() < EPS)
                    || ((a.y - b.y).abs() < EPS && (b.y - c.y).abs() < EPS);
                !straight
            }
            _ => false,
        })
        .count()
}

/// The straight legs of a polyline, as point pairs.
///
/// Paired by zipping the sequence with its own tail rather than by index, so
/// there is no "and if there were no second point" arm that can never be taken.
pub fn legs(points: &[Point]) -> Vec<(Point, Point)> {
    points
        .iter()
        .zip(points.iter().skip(1))
        .map(|(a, b)| (*a, *b))
        .collect()
}

/// How many already-placed segments a candidate route would cross.
pub fn crosses_existing(points: &[Point], existing: &[(Point, Point)]) -> usize {
    let mut n = 0;
    for (p, q) in legs(points) {
        let horizontal = (p.y - q.y).abs() < EPS;
        for (a, b) in existing {
            let seg_h = (a.y - b.y).abs() < EPS;
            if seg_h == horizontal {
                continue;
            }
            let (h, v) = if horizontal {
                ((p, q), (*a, *b))
            } else {
                ((*a, *b), (p, q))
            };
            if h.0.x.min(h.1.x) < v.0.x
                && h.0.x.max(h.1.x) > v.0.x
                && v.0.y.min(v.1.y) < h.0.y
                && v.0.y.max(v.1.y) > h.0.y
            {
                n += 1;
            }
        }
    }
    n
}

/// Whether an axis-aligned segment passes through a box's interior.
///
/// Inset a little so a route running along a border does not count as a hit.
pub fn segment_hits_box(p: Point, q: Point, boxed: &Rect) -> bool {
    let pad = 2.0;
    p.x.min(q.x) < boxed.right() - pad
        && p.x.max(q.x) > boxed.x + pad
        && p.y.min(q.y) < boxed.bottom() - pad
        && p.y.max(q.y) > boxed.y + pad
}

/// How many obstacles a route's segments pass through.
pub fn crossings(points: &[Point], obstacles: &[Rect]) -> usize {
    obstacles
        .iter()
        .filter(|boxed| {
            legs(points)
                .into_iter()
                .any(|(p, q)| segment_hits_box(p, q, boxed))
        })
        .count()
}

/// Area of the intersection of two rectangles (0 when they don't overlap).
pub fn overlap_area(a: &Rect, b: &Rect) -> f64 {
    let dx = a.right().min(b.right()) - a.x.max(b.x);
    let dy = a.bottom().min(b.bottom()) - a.y.max(b.y);
    if dx > 0.0 && dy > 0.0 {
        dx * dy
    } else {
        0.0
    }
}

/// Whether a badge box touches an axis-aligned wire segment.
pub fn rect_hits_segment(r: &Rect, a: Point, b: Point) -> bool {
    a.x.min(b.x) <= r.right()
        && a.x.max(b.x) >= r.x
        && a.y.min(b.y) <= r.bottom()
        && a.y.max(b.y) >= r.y
}

/// Sample a point a fraction `t` along a polyline, by arc length.
pub fn point_along(points: &[Point], t: f64) -> Point {
    let Some(first) = points.first() else {
        return Point::new(0.0, 0.0);
    };
    if points.len() < 2 {
        return *first;
    }
    let segs: Vec<f64> = legs(points)
        .into_iter()
        .map(|(a, b)| (b.x - a.x).hypot(b.y - a.y))
        .collect();
    let total: f64 = segs.iter().sum();
    if total == 0.0 {
        return *first;
    }
    let mut want = total * t;
    for (i, len) in segs.iter().enumerate() {
        if want <= *len {
            let f = if *len == 0.0 { 0.0 } else { want / *len };
            return match (points.get(i), points.get(i + 1)) {
                (Some(p), Some(q)) => Point::new(p.x + (q.x - p.x) * f, p.y + (q.y - p.y) * f),
                _ => *first,
            };
        }
        want -= *len;
    }
    points.last().copied().unwrap_or(*first)
}

/// The bounding box of a set of rectangles, or `None` when there are none.
pub fn bounds(rects: &[Rect]) -> Option<Rect> {
    let first = rects.first()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.right();
    let mut max_y = first.bottom();
    for r in rects {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.right());
        max_y = max_y.max(r.bottom());
    }
    Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_tenth_is_the_identity_of_a_lane() {
        assert_eq!(fixed1(123.44), 1234);
        assert_eq!(fixed1(123.46), 1235);
        // The tie case: an odd quarter is exact in binary, and the reference
        // rounds it up rather than to even.
        assert_eq!(fixed1(0.25), 3);
        assert_eq!(fixed1(0.75), 8);
    }

    #[test]
    fn separation_is_zero_for_neighbours_however_wide() {
        let a = Rect::new(0.0, 0.0, 300.0, 100.0);
        let b = Rect::new(300.0, 0.0, 300.0, 100.0);
        assert!((separation(&a, &b) - 0.0).abs() < 1e-9);
        // ... and grows only with the actual gap.
        let far = Rect::new(400.0, 0.0, 10.0, 100.0);
        assert!((separation(&a, &far) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn separation_counts_both_axes() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(30.0, 40.0, 10.0, 10.0);
        assert!((separation(&a, &b) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn face_points_sit_on_the_middle_of_each_face() {
        let r = Rect::new(10.0, 20.0, 100.0, 40.0);
        assert_eq!(face_point(&r, Side::Top), p(60.0, 20.0));
        assert_eq!(face_point(&r, Side::Bottom), p(60.0, 60.0));
        assert_eq!(face_point(&r, Side::Left), p(10.0, 40.0));
        assert_eq!(face_point(&r, Side::Right), p(110.0, 40.0));
    }

    #[test]
    fn dedupe_drops_repeats_and_keeps_the_rest() {
        let out = dedupe(&[p(0.0, 0.0), p(0.2, 0.1), p(10.0, 0.0), p(10.0, 10.0)]);
        assert_eq!(out, vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        assert!(dedupe(&[]).is_empty());
    }

    #[test]
    fn simplify_collapses_a_straight_run_to_its_ends() {
        let out = simplify(&[
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(20.0, 0.0),
            p(20.0, 10.0),
            p(20.0, 20.0),
        ]);
        assert_eq!(out, vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 20.0)]);
    }

    #[test]
    fn simplify_keeps_a_corner() {
        let out = simplify(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        assert_eq!(out.len(), 3);
        assert_eq!(simplify(&[p(1.0, 1.0)]), vec![p(1.0, 1.0)]);
    }

    #[test]
    fn length_and_bends_read_a_polyline() {
        let route = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(30.0, 10.0)];
        assert!((path_length(&route) - 40.0).abs() < 1e-9);
        assert_eq!(bend_count(&route), 2);
        assert_eq!(bend_count(&[p(0.0, 0.0), p(5.0, 0.0)]), 0);
        assert!((path_length(&[]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_proper_crossing_counts_and_a_shared_corner_does_not() {
        let vertical = [(p(5.0, -5.0), p(5.0, 5.0))];
        assert_eq!(crosses_existing(&[p(0.0, 0.0), p(10.0, 0.0)], &vertical), 1);
        // Meeting end to end at a corner is not a crossing.
        assert_eq!(crosses_existing(&[p(5.0, 0.0), p(10.0, 0.0)], &vertical), 0);
        // Two parallel runs never cross.
        let horizontal = [(p(0.0, 0.0), p(10.0, 0.0))];
        assert_eq!(
            crosses_existing(&[p(0.0, 2.0), p(10.0, 2.0)], &horizontal),
            0
        );
    }

    #[test]
    fn a_segment_through_a_box_is_a_hit_and_one_along_its_border_is_not() {
        let boxed = Rect::new(0.0, 0.0, 100.0, 50.0);
        assert!(segment_hits_box(p(-10.0, 25.0), p(110.0, 25.0), &boxed));
        assert!(!segment_hits_box(p(-10.0, 0.0), p(110.0, 0.0), &boxed));
        assert_eq!(crossings(&[p(-10.0, 25.0), p(110.0, 25.0)], &[boxed]), 1);
        assert_eq!(crossings(&[p(-10.0, 80.0), p(110.0, 80.0)], &[boxed]), 0);
    }

    #[test]
    fn overlap_is_an_area_and_is_zero_when_apart() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!((overlap_area(&a, &Rect::new(5.0, 5.0, 10.0, 10.0)) - 25.0).abs() < 1e-9);
        assert!((overlap_area(&a, &Rect::new(20.0, 0.0, 10.0, 10.0)) - 0.0).abs() < 1e-9);
        assert!((overlap_area(&a, &Rect::new(0.0, 20.0, 10.0, 10.0)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_badge_touching_a_wire_is_detected() {
        let badge = Rect::new(0.0, 0.0, 20.0, 20.0);
        assert!(rect_hits_segment(&badge, p(10.0, -5.0), p(10.0, 25.0)));
        assert!(!rect_hits_segment(&badge, p(30.0, -5.0), p(30.0, 25.0)));
        assert!(!rect_hits_segment(&badge, p(0.0, 30.0), p(20.0, 30.0)));
    }

    #[test]
    fn sampling_walks_the_route_by_arc_length() {
        let route = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)];
        assert_eq!(point_along(&route, 0.0), p(0.0, 0.0));
        assert_eq!(point_along(&route, 0.5), p(10.0, 0.0));
        assert_eq!(point_along(&route, 1.0), p(10.0, 10.0));
        // Degenerate inputs still answer with a point.
        assert_eq!(point_along(&[p(3.0, 4.0)], 0.5), p(3.0, 4.0));
        assert_eq!(point_along(&[], 0.5), p(0.0, 0.0));
        assert_eq!(point_along(&[p(1.0, 1.0), p(1.0, 1.0)], 0.5), p(1.0, 1.0));
    }

    #[test]
    fn bounds_wrap_everything_or_nothing() {
        let got = bounds(&[
            Rect::new(10.0, 10.0, 5.0, 5.0),
            Rect::new(0.0, 20.0, 5.0, 5.0),
        ]);
        assert_eq!(got, Some(Rect::new(0.0, 10.0, 15.0, 15.0)));
        assert_eq!(bounds(&[]), None);
    }

    #[test]
    fn facing_pairs_are_the_ones_that_look_across_a_gutter() {
        assert!(SidePair {
            start: Side::Bottom,
            end: Side::Top
        }
        .facing());
        assert!(SidePair {
            start: Side::Left,
            end: Side::Right
        }
        .facing());
        assert!(!SidePair {
            start: Side::Top,
            end: Side::Left
        }
        .facing());
        assert!(Side::Top.is_horizontal_face());
        assert!(!Side::Left.is_horizontal_face());
        assert_eq!(Axis::H.other(), Axis::V);
        assert_eq!(Axis::V.other(), Axis::H);
    }

    #[test]
    fn clamp_holds_a_value_between_its_bounds() {
        assert!((clamp(5.0, 0.0, 10.0) - 5.0).abs() < 1e-9);
        assert!((clamp(-5.0, 0.0, 10.0) - 0.0).abs() < 1e-9);
        assert!((clamp(50.0, 0.0, 10.0) - 10.0).abs() < 1e-9);
    }
}
