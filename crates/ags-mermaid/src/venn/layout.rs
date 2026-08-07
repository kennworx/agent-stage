//! Where the circles sit.
//!
//! Every set gets the same radius; only the arrangement changes with how many
//! there are — one centred, two side by side, three on a triangle, and more than
//! three in an overlapping row, which is a fallback rather than a real Venn
//! construction (no arrangement of four equal circles shows all its regions).

use crate::round::count;
use crate::scene::Point;

use super::types::Diagram;

pub const RADIUS: f64 = 120.0;
pub const PADDING: f64 = 28.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
/// How far a set's name sits from its centre, as a fraction of the radius.
pub const SET_LABEL_RADIUS: f64 = 0.62;

/// One set, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedSet {
    pub id: String,
    pub label: String,
    pub at: Point,
    pub r: f64,
    pub color_index: usize,
    pub label_at: Point,
}

/// One overlap region, placed at the centre of the lens its members make.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedUnion {
    pub id: String,
    pub set_ids: Vec<String>,
    pub label: String,
    pub at: Point,
}

/// A laid-out Venn diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub sets: Vec<PlacedSet>,
    pub unions: Vec<PlacedUnion>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// Circle centres about the origin, for `n` sets.
///
/// One entry even for no sets: the canvas is sized from these, and a diagram
/// that declares nothing still has to be a rectangle rather than nothing.
fn centres(n: usize) -> Vec<Point> {
    if n <= 1 {
        return vec![Point::new(0.0, 0.0)];
    }
    if n == 2 {
        let d = RADIUS * 0.92;
        return vec![Point::new(-d / 2.0, 0.0), Point::new(d / 2.0, 0.0)];
    }
    if n == 3 {
        let side = RADIUS * 1.05;
        // The circumradius of the equilateral triangle those sides make.
        let rho = side / 3.0_f64.sqrt();
        return [-90.0_f64, 30.0, 150.0]
            .into_iter()
            .map(|deg| {
                let a = deg.to_radians();
                Point::new(rho * a.cos(), rho * a.sin())
            })
            .collect();
    }
    let d = RADIUS * 0.9;
    (0..n)
        .map(|i| Point::new((count(i) - (count(n) - 1.0) / 2.0) * d, 0.0))
        .collect()
}

/// The direction a set's name is pushed in: away from the middle of the whole
/// arrangement, so it lands on the part of the circle that overlaps nothing.
fn outward(centre: Point) -> Point {
    let magnitude = centre.x.hypot(centre.y);
    if magnitude < 1e-6 {
        // A lone circle has no outward direction, so its name goes on top.
        Point::new(0.0, -1.0)
    } else {
        Point::new(centre.x / magnitude, centre.y / magnitude)
    }
}

/// Lay out a parsed Venn diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    let relative = centres(diagram.sets.len());
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in &relative {
        min_x = min_x.min(c.x - RADIUS);
        max_x = max_x.max(c.x + RADIUS);
        min_y = min_y.min(c.y - RADIUS);
        max_y = max_y.max(c.y + RADIUS);
    }

    let title_band = if diagram.title.is_some() {
        TITLE_HEIGHT
    } else {
        0.0
    };
    let offset = Point::new(PADDING - min_x, PADDING + title_band - min_y);
    let width = max_x - min_x + PADDING * 2.0;
    let height = max_y - min_y + PADDING * 2.0 + title_band;

    let sets: Vec<PlacedSet> = diagram
        .sets
        .iter()
        .zip(&relative)
        .enumerate()
        .map(|(i, (set, rel))| {
            let at = Point::new(rel.x + offset.x, rel.y + offset.y);
            let out = outward(*rel);
            PlacedSet {
                id: set.id.clone(),
                label: set.label.clone(),
                at,
                r: RADIUS,
                color_index: i,
                label_at: Point::new(
                    at.x + out.x * RADIUS * SET_LABEL_RADIUS,
                    at.y + out.y * RADIUS * SET_LABEL_RADIUS,
                ),
            }
        })
        .collect();

    let unions = diagram
        .unions
        .iter()
        .map(|union| {
            let members: Vec<&PlacedSet> = union
                .set_ids
                .iter()
                .filter_map(|id| sets.iter().find(|s| s.id == *id))
                .collect();
            // The centroid of the member centres — which for a pair is the
            // middle of their lens, so each pairwise region sits in its own
            // overlap instead of stacking at the middle of the diagram.
            let at = if members.is_empty() {
                offset
            } else {
                let n = count(members.len());
                Point::new(
                    members.iter().map(|m| m.at.x).sum::<f64>() / n,
                    members.iter().map(|m| m.at.y).sum::<f64>() / n,
                )
            };
            PlacedUnion {
                id: union.id.clone(),
                set_ids: union.set_ids.clone(),
                label: union.label.clone().unwrap_or_default(),
                at,
            }
        })
        .collect();

    Placed {
        width,
        height,
        title: diagram
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        sets,
        unions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::venn::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    #[test]
    fn one_set_is_a_single_circle_named_at_its_top() {
        let out = placed("venn\nset A");
        assert_eq!(out.sets.len(), 1);
        let set = &out.sets[0];
        assert!((set.label_at.x - set.at.x).abs() < 1e-9);
        assert!(set.label_at.y < set.at.y, "a lone name goes above centre");
    }

    #[test]
    fn two_sets_sit_side_by_side_and_overlap() {
        let out = placed("venn\nset A\nset B");
        let (a, b) = (&out.sets[0], &out.sets[1]);
        assert!((a.at.y - b.at.y).abs() < 1e-9, "level with each other");
        let gap = b.at.x - a.at.x;
        assert!(gap < RADIUS * 2.0, "closer than two radii, so they overlap");
    }

    #[test]
    fn three_sets_sit_on_a_triangle() {
        let out = placed("venn\nset A\nset B\nset C");
        let (a, b, c) = (&out.sets[0], &out.sets[1], &out.sets[2]);
        // One above two, and the lower two level with each other.
        assert!(a.at.y < b.at.y);
        assert!((b.at.y - c.at.y).abs() < 1e-9);
        // Equidistant, which is what makes every pair overlap equally.
        let ab = (a.at.x - b.at.x).hypot(a.at.y - b.at.y);
        let bc = (b.at.x - c.at.x).hypot(b.at.y - c.at.y);
        assert!((ab - bc).abs() < 1e-6);
    }

    #[test]
    fn more_than_three_sets_fall_back_to_a_row() {
        let out = placed("venn\nset A\nset B\nset C\nset D");
        assert!(out
            .sets
            .iter()
            .all(|s| (s.at.y - out.sets[0].at.y).abs() < 1e-9));
    }

    #[test]
    fn every_name_is_pushed_away_from_the_middle() {
        let out = placed("venn\nset A\nset B");
        let (a, b) = (&out.sets[0], &out.sets[1]);
        // Left circle names to its left, right circle to its right.
        assert!(a.label_at.x < a.at.x);
        assert!(b.label_at.x > b.at.x);
    }

    #[test]
    fn a_pairwise_region_sits_in_its_own_lens() {
        let out = placed("venn\nset A\nset B\nset C\nunion A, B");
        let union = &out.unions[0];
        let (a, b) = (&out.sets[0], &out.sets[1]);
        assert!((union.at.x - f64::midpoint(a.at.x, b.at.x)).abs() < 1e-9);
        assert!((union.at.y - f64::midpoint(a.at.y, b.at.y)).abs() < 1e-9);
    }

    #[test]
    fn a_region_naming_nothing_that_exists_falls_back_to_the_origin() {
        let out = placed("venn\nset A\nunion Ghost, Phantom");
        assert_eq!(out.unions.len(), 1);
        assert!(out.unions[0].at.x.is_finite());
    }

    #[test]
    fn every_circle_fits_on_the_canvas() {
        for source in [
            "venn\nset A",
            "venn\nset A\nset B",
            "venn\nset A\nset B\nset C",
        ] {
            let out = placed(source);
            for set in &out.sets {
                assert!(set.at.x - set.r >= -1e-9, "{source}");
                assert!(set.at.x + set.r <= out.width + 1e-9, "{source}");
                assert!(set.at.y + set.r <= out.height + 1e-9, "{source}");
            }
        }
    }

    #[test]
    fn a_diagram_declaring_nothing_is_still_a_rectangle() {
        let out = placed("venn");
        assert!(out.sets.is_empty());
        assert!((out.width - (RADIUS * 2.0 + PADDING * 2.0)).abs() < 1e-9);
        assert!((out.height - (RADIUS * 2.0 + PADDING * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_title_pushes_the_circles_down_and_centres_itself() {
        let bare = placed("venn\nset A");
        let titled = placed("venn\ntitle T\nset A");
        assert!((titled.height - bare.height - TITLE_HEIGHT).abs() < 1e-9);
        assert!((titled.sets[0].at.y - bare.sets[0].at.y - TITLE_HEIGHT).abs() < 1e-9);
        let (_, at) = titled.title.clone().expect("a title");
        assert!((at.x - titled.width / 2.0).abs() < 1e-9);
    }
}
