//! Placing a label beside the line it names.
//!
//! Two rules, both learned from drawings that broke them.
//!
//! **Beside, not on.** A label centred on its line has that line drawn straight
//! through the middle of the word. Painting a background behind it to hide the
//! line is worse rather than better: the reader then cannot tell whether two
//! lines cross under the label or one of them stops there.
//!
//! **Never two in one place.** Two edges between the same pair of boxes run in
//! adjacent lanes, so their labels land within a few pixels of each other and
//! overlap into an unreadable smear. A label nudged along its own line is still
//! obviously that line's label; two labels on top of each other are neither's.

use crate::scene::Point;

/// A label's footprint: where its centre sits, and how much room it takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

impl Placed {
    pub const fn new(at: Point, width: f64, height: f64) -> Self {
        Self { at, width, height }
    }

    /// Whether two labels share any space at all.
    pub fn overlaps(self, other: Self) -> bool {
        (self.at.x - other.at.x).abs() * 2.0 < self.width + other.width
            && (self.at.y - other.at.y).abs() * 2.0 < self.height + other.height
    }

    /// Whether a line runs through the label.
    pub fn crosses(self, a: Point, b: Point) -> bool {
        let (left, right) = (self.at.x - self.width / 2.0, self.at.x + self.width / 2.0);
        let (top, bottom) = (self.at.y - self.height / 2.0, self.at.y + self.height / 2.0);
        a.x.min(b.x) < right && a.x.max(b.x) > left && a.y.min(b.y) < bottom && a.y.max(b.y) > top
    }
}

/// How far along its line a label will shuffle before it settles for the least
/// bad place it found. Four steps either way is more room than any label needs.
const TRIES: usize = 4;

/// How much of a label is in the way of something else.
///
/// A count rather than a yes-or-no, so that when every place is bad the label
/// can take the least bad one instead of whichever was tried last.
fn cost(here: Placed, taken: &[Placed], lines: &[(Point, Point)]) -> usize {
    taken.iter().filter(|other| here.overlaps(**other)).count()
        + lines.iter().filter(|(a, b)| here.crosses(*a, *b)).count()
}

/// Where a label goes: beside `anchor`, then either side of the line and along
/// it until it is clear of everything.
///
/// `upright` says which way the run under the anchor goes, which decides which
/// way is *across* the line — where the label steps to get off it — and which
/// way is *along* — where it shuffles when something is already there.
///
/// `taken` is everything solid the label must miss: the boxes of the drawing,
/// and the labels already placed. `lines` should hold every run *except* the
/// labelled one's own — a label has to sit next to its own line to be read as
/// its label, and must not sit on anybody else's.
pub fn beside(
    anchor: Point,
    upright: bool,
    size: (f64, f64),
    gap: f64,
    taken: &[Placed],
    lines: &[(Point, Point)],
) -> Placed {
    let (width, height) = size;
    let (across, along) = if upright {
        (width / 2.0 + gap, height + gap)
    } else {
        (height / 2.0 + gap, width + gap)
    };
    // The side tried first: beside an upright line, and above a level one.
    let sides = if upright { [1.0, -1.0] } else { [-1.0, 1.0] };
    let put = |side: f64, step: f64| {
        let at = if upright {
            Point::new(anchor.x + side * across, anchor.y + step * along)
        } else {
            Point::new(anchor.x + step * along, anchor.y + side * across)
        };
        Placed::new(at, width, height)
    };
    let mut best: Option<(usize, Placed)> = None;
    for step in 0..TRIES {
        let steps: &[f64] = if step == 0 { &[0.0] } else { &[1.0, -1.0] };
        for side in sides {
            for direction in steps {
                let here = put(side, direction * crate::layout::as_f64(step));
                let cost = cost(here, taken, lines);
                if cost == 0 {
                    return here;
                }
                if best.is_none_or(|(seen, _)| cost < seen) {
                    best = Some((cost, here));
                }
            }
        }
    }
    best.map_or_else(|| put(1.0, 0.0), |(_, here)| here)
}

/// Every straight run of a route, as the pairs a label has to keep off.
pub fn runs(points: &[Point]) -> Vec<(Point, Point)> {
    points
        .windows(2)
        .filter_map(|pair| Some((*pair.first()?, *pair.get(1)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn two_labels_sharing_any_space_at_all_overlap() {
        let here = Placed::new(at(0.0, 0.0), 40.0, 14.0);
        // Dead on top of each other.
        assert!(here.overlaps(here));
        // Touching along one edge is not sharing.
        assert!(!here.overlaps(Placed::new(at(40.0, 0.0), 40.0, 14.0)));
        assert!(!here.overlaps(Placed::new(at(0.0, 14.0), 40.0, 14.0)));
        // A pixel closer and it is.
        assert!(here.overlaps(Placed::new(at(39.0, 0.0), 40.0, 14.0)));
        assert!(here.overlaps(Placed::new(at(0.0, 13.0), 40.0, 14.0)));
        // Far apart on one axis is enough, whichever axis it is.
        assert!(!here.overlaps(Placed::new(at(0.0, 100.0), 40.0, 14.0)));
        assert!(!here.overlaps(Placed::new(at(100.0, 0.0), 40.0, 14.0)));
    }

    #[test]
    fn a_label_on_an_upright_line_steps_across_to_the_side() {
        let placed = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[], &[]);
        // Clear of the line by its own half-width and the gap.
        assert!((placed.at.x - (100.0 + 20.0 + 6.0)).abs() < 1e-9);
        assert!((placed.at.y - 50.0).abs() < 1e-9);
        // And its near edge really does miss the line.
        assert!(placed.at.x - placed.width / 2.0 > 100.0);
    }

    #[test]
    fn a_label_on_a_level_line_steps_up_off_it() {
        let placed = beside(at(100.0, 50.0), false, (40.0, 14.0), 6.0, &[], &[]);
        assert!((placed.at.x - 100.0).abs() < 1e-9);
        assert!((placed.at.y - (50.0 - 7.0 - 6.0)).abs() < 1e-9);
        assert!(placed.at.y + placed.height / 2.0 < 50.0);
    }

    #[test]
    fn a_second_label_in_the_same_place_crosses_to_the_other_side() {
        // The near side is taken, and the far side of the same line is a
        // shorter move than shuffling along it.
        let first = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[], &[]);
        let second = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[first], &[]);
        assert!(!first.overlaps(second));
        assert!((second.at.y - first.at.y).abs() < 1e-9, "it stayed level");
        assert!(second.at.x < 100.0 && first.at.x > 100.0, "opposite sides");
    }

    #[test]
    fn a_level_line_puts_the_second_label_underneath() {
        let first = beside(at(100.0, 50.0), false, (40.0, 14.0), 6.0, &[], &[]);
        let second = beside(at(100.0, 50.0), false, (40.0, 14.0), 6.0, &[first], &[]);
        assert!(!first.overlaps(second));
        assert!(first.at.y < 50.0, "the first goes above");
        assert!(second.at.y > 50.0, "the second goes below");
    }

    #[test]
    fn a_third_and_fourth_label_shuffle_along_the_line() {
        let mut taken: Vec<Placed> = Vec::new();
        for _ in 0..4 {
            let next = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &taken, &[]);
            for other in &taken {
                assert!(!next.overlaps(*other), "two labels in one place");
            }
            taken.push(next);
        }
        assert_eq!(taken.len(), 4);
        // Both sides are used before anything moves along the line.
        assert!(taken.iter().any(|placed| placed.at.x > 100.0));
        assert!(taken.iter().any(|placed| placed.at.x < 100.0));
    }

    #[test]
    fn a_label_with_nowhere_clear_takes_the_least_bad_place() {
        // Walls down both sides, two deep on the near one and one deep on the
        // far, so no step along the line finds anything clear. The far side
        // wins: an overlapping label is worse than a missing one, but only
        // just, and looking forever is worse than both.
        let near = at(126.0, 50.0);
        let far = at(74.0, 50.0);
        let taken = [
            Placed::new(near, 40.0, 400.0),
            Placed::new(near, 40.0, 400.0),
            Placed::new(far, 40.0, 400.0),
        ];
        let placed = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &taken, &[]);
        assert!(placed.at.x < 100.0, "it took the side with less in the way");
    }

    #[test]
    fn a_line_running_through_a_label_moves_it_off() {
        // Somebody else's line lies exactly where the label wants to sit.
        let through = [(at(126.0, 20.0), at(126.0, 80.0))];
        let alone = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[], &[]);
        let (a, b) = through.first().copied().expect("a line");
        assert!(alone.crosses(a, b));
        let moved = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[], &through);
        assert!(!moved.crosses(a, b));
    }

    #[test]
    fn a_label_will_not_be_pushed_into_a_box() {
        // The whole near side is a box. Stepping along the line would walk
        // further into it; the far side is clear and is where it belongs.
        let box_here = Placed::new(at(160.0, 50.0), 120.0, 200.0);
        let placed = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[box_here], &[]);
        assert!(!placed.overlaps(box_here), "the label sits inside a box");
        assert!(placed.at.x < 100.0);
    }

    #[test]
    fn a_line_is_only_through_a_label_when_it_really_is() {
        let placed = Placed::new(at(100.0, 50.0), 40.0, 14.0);
        // Straight through the middle, either way.
        assert!(placed.crosses(at(0.0, 50.0), at(200.0, 50.0)));
        assert!(placed.crosses(at(100.0, 0.0), at(100.0, 100.0)));
        // Past each edge in turn.
        assert!(!placed.crosses(at(0.0, 50.0), at(79.0, 50.0)));
        assert!(!placed.crosses(at(121.0, 50.0), at(200.0, 50.0)));
        assert!(!placed.crosses(at(0.0, 42.0), at(200.0, 42.0)));
        assert!(!placed.crosses(at(0.0, 58.0), at(200.0, 58.0)));
    }

    #[test]
    fn the_runs_of_a_route_are_its_consecutive_pairs() {
        assert_eq!(
            runs(&[at(0.0, 0.0), at(10.0, 0.0), at(10.0, 20.0)]),
            [
                (at(0.0, 0.0), at(10.0, 0.0)),
                (at(10.0, 0.0), at(10.0, 20.0))
            ]
        );
        assert!(runs(&[at(0.0, 0.0)]).is_empty());
        assert!(runs(&[]).is_empty());
    }

    #[test]
    fn a_label_clear_of_everything_stays_where_it_started() {
        let elsewhere = Placed::new(at(500.0, 500.0), 40.0, 14.0);
        let alone = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[], &[]);
        let crowded = beside(at(100.0, 50.0), true, (40.0, 14.0), 6.0, &[elsewhere], &[]);
        assert_eq!(alone, crowded);
    }
}
