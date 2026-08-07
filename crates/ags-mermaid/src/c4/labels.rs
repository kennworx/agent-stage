//! Where each relationship's step badge sits, and what the legend says.

use super::config as l;
use super::geom::{
    clamp, legs, overlap_area, path_length, point_along, rect_hits_segment, Point, Rect,
};
use super::positioned::PlacedRelationship;

/// Split a relationship label into its step badge and its legend text.
///
/// An author who already numbered the steps — `3a. Spawns workspace-wide`, as a
/// dynamic diagram does — keeps their own numbering. Renumbering it would break
/// the prose around the diagram that refers to step 3a. Everything else is
/// numbered in declaration order.
pub fn step_of(label: &str, techn: Option<&str>, index: usize) -> (String, String) {
    let (step, body) = match marked_step(label) {
        Some((step, rest)) => (step, rest),
        None => ((index + 1).to_string(), label.to_string()),
    };
    let text = match techn {
        Some(t) => format!("{body} [{t}]").trim().to_string(),
        None => body.trim().to_string(),
    };
    (step, text)
}

/// A leading `1.`, `3a)` or similar, and whatever follows it.
///
/// Hand-parsed rather than matched: the reference used a regular expression whose
/// trailing `.*` cannot cross a newline, so a label carrying one is *not* treated
/// as numbered — a detail a naive rewrite loses, and one that decides whether a
/// multi-line label keeps its own step marker.
fn marked_step(label: &str) -> Option<(String, String)> {
    let rest = label.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let after_digits = rest.get(digits.len()..)?;
    let letter: String = after_digits
        .chars()
        .next()
        .filter(char::is_ascii_alphabetic)
        .map(String::from)
        .unwrap_or_default();
    let after_marker = after_digits.get(letter.len()..)?;
    let spaced = after_marker.trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
    let mut chars = spaced.chars();
    if !matches!(chars.next(), Some('.' | ')')) {
        return None;
    }
    let body = chars
        .as_str()
        .trim_start_matches([' ', '\t', '\n', '\r', '\u{b}', '\u{c}']);
    if body.contains('\n') {
        return None;
    }
    Some((format!("{digits}{letter}"), body.to_string()))
}

/// The offsets tried either side of the anchor, in the order the reference walked
/// them.
///
/// Accumulated rather than tabulated because the reference accumulated: `0.04`
/// has no exact binary form, so the ninth step lands a hair above `0.36` and the
/// loop stops one candidate earlier than a table of round numbers would.
fn stops(anchor: f64) -> Vec<f64> {
    let mut out = vec![anchor];
    let mut d = 0.04;
    while d <= 0.36 {
        out.push(anchor - d);
        out.push(anchor + d);
        d += 0.04;
    }
    out.retain(|v| *v >= 0.02 && *v <= 0.98);
    out
}

/// How far off its own line each candidate sits, in badge heights.
const SHIFTS: [f64; 9] = [0.0, 1.0, -1.0, 1.6, -1.6, 2.4, -2.4, 3.2, -3.2];

/// Choose a position for every relationship badge.
///
/// The midpoint of a straight edge is very often *inside another box* — with an
/// opaque badge drawn over it that erases the node's name, and two badges landing
/// together erase each other. So rather than trusting the midpoint, each badge is
/// tried at several points along its own edge and at a perpendicular offset to
/// either side, and scored on how much it covers.
///
/// Covering a node counts far more than brushing another badge: a hidden node name
/// is a lie about the diagram, whereas two badges touching is merely untidy. The
/// first candidate that collides with nothing wins immediately; otherwise the
/// least-bad one is used, so a dense diagram degrades rather than failing.
pub fn place_labels(rels: &mut [PlacedRelationship], boxes: &[Rect]) {
    let wires: Vec<Vec<(Point, Point)>> = rels.iter().map(|rel| legs(&rel.points)).collect();
    let mut placed: Vec<Rect> = Vec::new();

    for ri in 0..rels.len() {
        let Some(rel) = rels.get(ri) else { continue };
        // A badge with no box reserves no space and blocks nothing.
        if rel.badge_width == 0.0 {
            continue;
        }
        // Anchor near the source on a long route, at the middle on a short one,
        // then step outward from there. A badge halfway down a long line says
        // little about which box the line came from, and that is exactly where
        // crossings live.
        let route_len = path_length(&rel.points);
        let anchor = if route_len > 3.0 * l::LABEL_ANCHOR_PX {
            clamp(l::LABEL_ANCHOR_PX / route_len, 0.06, 0.34)
        } else {
            0.5
        };
        // The normal to the chord, to shift a badge clear of its own line's
        // traffic.
        let dx = rel.end.x - rel.start.x;
        let dy = rel.end.y - rel.start.y;
        let len = {
            let h = dx.hypot(dy);
            if h == 0.0 {
                1.0
            } else {
                h
            }
        };
        let (nx, ny) = (-dy / len, dx / len);

        let best = search(rel, &stops(anchor), (nx, ny), boxes, &placed, &wires, ri);
        let Some(rel) = rels.get_mut(ri) else {
            continue;
        };
        if let Some(centre) = best {
            rel.badge_center = centre;
        }
        placed.push(rel.badge_rect());
    }
}

/// The cheapest badge position for one edge, or `None` when there is nowhere to
/// try — which leaves the provisional midpoint in place.
fn search(
    rel: &PlacedRelationship,
    stops: &[f64],
    normal: (f64, f64),
    boxes: &[Rect],
    placed: &[Rect],
    wires: &[Vec<(Point, Point)>],
    ri: usize,
) -> Option<Point> {
    let mut best: Option<Point> = None;
    let mut best_cost = f64::INFINITY;
    for t in stops {
        let on = point_along(&rel.points, *t);
        for shift in SHIFTS {
            let off = shift * (rel.badge_height + 4.0);
            let centre = Point::new(on.x + normal.0 * off, on.y + normal.1 * off);
            let rect = Rect::new(
                centre.x - rel.badge_width / 2.0,
                centre.y - rel.badge_height / 2.0,
                rel.badge_width,
                rel.badge_height,
            );
            let cost = cost_of(&rect, rel, boxes, placed, wires, ri);
            if cost < best_cost {
                best_cost = cost;
                best = Some(centre);
            }
            if cost == 0.0 {
                return best;
            }
        }
    }
    best
}

/// What a candidate badge box covers, weighted by how much the covering matters.
fn cost_of(
    rect: &Rect,
    rel: &PlacedRelationship,
    boxes: &[Rect],
    placed: &[Rect],
    wires: &[Vec<(Point, Point)>],
    ri: usize,
) -> f64 {
    let mut cost = 0.0;
    for b in boxes {
        cost += overlap_area(rect, b) * l::LABEL_BOX_PENALTY;
    }
    for p in placed {
        cost += overlap_area(rect, p);
    }
    for (wi, segs) in wires.iter().enumerate() {
        if wi == ri {
            continue;
        }
        for (a, b) in segs {
            if rect_hits_segment(rect, *a, *b) {
                cost += rel.badge_width * l::LABEL_LINE_PENALTY;
            }
        }
    }
    cost
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(points: Vec<Point>) -> PlacedRelationship {
        let first = points.first().copied().unwrap_or(Point::new(0.0, 0.0));
        let last = points.last().copied().unwrap_or(Point::new(0.0, 0.0));
        PlacedRelationship {
            from: "a".into(),
            to: "b".into(),
            label: "calls".into(),
            techn: None,
            bidirectional: false,
            step: "1".into(),
            start: first,
            end: last,
            points,
            badge_center: Point::new(
                f64::midpoint(first.x, last.x),
                f64::midpoint(first.y, last.y),
            ),
            badge_width: l::BADGE_SIZE,
            badge_height: l::BADGE_SIZE,
            description: "calls".into(),
        }
    }

    #[test]
    fn an_authors_own_numbering_survives() {
        assert_eq!(
            step_of("3a. Spawns workspace-wide", None, 7),
            ("3a".to_string(), "Spawns workspace-wide".to_string())
        );
        assert_eq!(
            step_of("  12)  Reads", None, 0),
            ("12".to_string(), "Reads".to_string())
        );
    }

    #[test]
    fn an_unnumbered_label_takes_its_declaration_order() {
        assert_eq!(
            step_of("Reads the graph", None, 4),
            ("5".to_string(), "Reads the graph".to_string())
        );
        // A number with no separator is part of the prose, not a step marker.
        assert_eq!(
            step_of("3 retries", None, 0),
            ("1".to_string(), "3 retries".to_string())
        );
        assert_eq!(step_of("", None, 0), ("1".to_string(), String::new()));
    }

    #[test]
    fn a_technology_is_appended_for_the_legend() {
        assert_eq!(
            step_of("1. Indexes", Some("CLI"), 0),
            ("1".to_string(), "Indexes [CLI]".to_string())
        );
        // ... and an empty body leaves no stray space in front of it.
        assert_eq!(
            step_of("1.", Some("CLI"), 0),
            ("1".to_string(), "[CLI]".to_string())
        );
    }

    #[test]
    fn a_marker_followed_by_a_second_line_is_not_a_marker() {
        // The reference's `.*` cannot cross a newline, so this keeps its own
        // text and takes the declaration number.
        assert_eq!(
            step_of("1. first\nsecond", None, 0),
            ("1".to_string(), "1. first\nsecond".to_string())
        );
    }

    #[test]
    fn the_stop_list_walks_outward_and_stays_inside_the_route() {
        let out = stops(0.5);
        assert_eq!(out.first(), Some(&0.5));
        assert_eq!(out.get(1), Some(&(0.5 - 0.04)));
        assert_eq!(out.get(2), Some(&(0.5 + 0.04)));
        assert!(out.iter().all(|v| *v >= 0.02 && *v <= 0.98));
        // An anchor near an end drops the candidates that fall off the route.
        assert!(stops(0.06).len() < out.len());
    }

    #[test]
    fn a_badge_that_collides_with_nothing_stays_on_its_line() {
        let mut rels = vec![rel(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)])];
        place_labels(&mut rels, &[]);
        let centre = rels[0].badge_center;
        assert!((centre.y - 0.0).abs() < 1e-9, "{centre:?}");
        assert!((centre.x - 50.0).abs() < 1e-9, "{centre:?}");
    }

    #[test]
    fn a_badge_moves_off_a_box_it_would_have_covered() {
        // The midpoint of this edge sits inside a box.
        let mut rels = vec![rel(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)])];
        let boxes = [Rect::new(30.0, -20.0, 40.0, 40.0)];
        place_labels(&mut rels, &boxes);
        let rect = rels[0].badge_rect();
        assert!(
            overlap_area(&rect, &boxes[0]) == 0.0,
            "{rect:?} still covers the box"
        );
    }

    #[test]
    fn two_badges_on_one_line_do_not_land_on_each_other() {
        let mut rels = vec![
            rel(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)]),
            rel(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)]),
        ];
        place_labels(&mut rels, &[]);
        let a = rels[0].badge_rect();
        let b = rels[1].badge_rect();
        assert!(overlap_area(&a, &b) == 0.0, "{a:?} {b:?}");
    }

    #[test]
    fn a_zero_width_badge_is_left_untouched() {
        let mut rels = vec![rel(vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)])];
        rels[0].badge_width = 0.0;
        let before = rels[0].badge_center;
        place_labels(&mut rels, &[Rect::new(0.0, -50.0, 200.0, 100.0)]);
        assert_eq!(rels[0].badge_center, before);
    }

    #[test]
    fn a_long_route_anchors_near_its_source() {
        let mut rels = vec![rel(vec![Point::new(0.0, 0.0), Point::new(1000.0, 0.0)])];
        place_labels(&mut rels, &[]);
        assert!(rels[0].badge_center.x < 200.0, "{:?}", rels[0].badge_center);
    }

    #[test]
    fn a_degenerate_route_still_places_its_badge() {
        let mut rels = vec![rel(vec![Point::new(5.0, 5.0), Point::new(5.0, 5.0)])];
        place_labels(&mut rels, &[]);
        assert_eq!(rels[0].badge_center, Point::new(5.0, 5.0));
    }
}
