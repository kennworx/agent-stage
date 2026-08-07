//! How hard a drawing is to follow, and the search for an element order that
//! makes it easier.
//!
//! Element order decides row and column, and therefore how far edges have to
//! travel and how often they collide — on a real diagram the author's declaration
//! order scored 29 crossings where a better order scored 5. So the order is
//! searched rather than taken as given: a fast grid proxy shortlists candidates,
//! then each is laid out for real and scored on the drawing that results.
//!
//! Scoring the shortlist for real, rather than trusting the proxy, is what keeps
//! this from regressing diagrams that were already clean — the proxy alone made
//! two of them worse.

use std::collections::HashMap;

use super::config as l;
use super::geom::{count, legs, whole, Point, EPS};
use super::types::{Diagram, Element, Relationship};

/// One straight run of a drawn edge, for quality scoring.
#[derive(Debug, Clone, Copy)]
struct Run {
    a: Point,
    b: Point,
    horiz: bool,
    edge: usize,
}

/// The straight runs of every edge, tagged with the edge they belong to.
fn edge_runs(routes: &[Vec<Point>]) -> Vec<Run> {
    let mut out = Vec::new();
    for (edge, points) in routes.iter().enumerate() {
        for (a, b) in legs(points) {
            if (a.x - b.x).abs() < EPS && (a.y - b.y).abs() < EPS {
                continue;
            }
            out.push(Run {
                a,
                b,
                horiz: (a.y - b.y).abs() < EPS,
                edge,
            });
        }
    }
    out
}

/// Length over which two parallel runs are drawn on top of each other.
fn shared_run(s: &Run, t: &Run) -> f64 {
    if s.horiz != t.horiz {
        return 0.0;
    }
    let fixed = |r: &Run| if r.horiz { r.a.y } else { r.a.x };
    if (fixed(s) - fixed(t)).abs() > l::NUDGE_EPS {
        return 0.0;
    }
    let span = |r: &Run| {
        if r.horiz {
            (r.a.x, r.b.x)
        } else {
            (r.a.y, r.b.y)
        }
    };
    let (sa, sb) = span(s);
    let (ta, tb) = span(t);
    let lo = sa.min(sb).max(ta.min(tb));
    let hi = sa.max(sb).min(ta.max(tb));
    (hi - lo).max(0.0)
}

/// Whether two perpendicular runs properly cross.
///
/// Touching at a shared corner does not count: that is two legs of one drawing
/// meeting, not two lines a reader has to tell apart.
fn runs_cross(s: &Run, t: &Run) -> bool {
    if s.horiz == t.horiz {
        return false;
    }
    let (h, v) = if s.horiz { (s, t) } else { (t, s) };
    v.a.x > h.a.x.min(h.b.x) + 1.0
        && v.a.x < h.a.x.max(h.b.x) - 1.0
        && h.a.y > v.a.y.min(v.b.y) + 1.0
        && h.a.y < v.a.y.max(v.b.y) - 1.0
}

/// How hard the drawn edges are to follow. Lower is better.
///
/// Merged runs are weighted per pixel and above crossings, because they are the
/// worse failure: a crossing leaves both lines identifiable, whereas two edges
/// sharing a line for 100px are one line to the reader for that whole length.
pub fn draw_quality(routes: &[Vec<Point>]) -> f64 {
    let runs = edge_runs(routes);
    let mut crossings = 0.0;
    let mut merged = 0.0;
    for (i, a) in runs.iter().enumerate() {
        for b in runs.iter().skip(i + 1) {
            if a.edge == b.edge {
                continue;
            }
            merged += shared_run(a, b);
            if runs_cross(a, b) {
                crossings += 1.0;
            }
        }
    }
    let bends: f64 = routes.iter().map(|r| (count(r.len()) - 2.0).max(0.0)).sum();
    merged * 40.0 + crossings * 400.0 + bends * 12.0
}

/// Cheap stand-in for [`draw_quality`], scored on grid cells rather than pixels.
///
/// An element's row and column follow directly from its position in the order, so
/// this needs no placement and no routing — which is what makes a few thousand
/// candidate orders affordable. A full layout costs milliseconds; this costs
/// microseconds. It only ranks candidates: the survivors are re-scored for real.
fn grid_score(order: &[usize], elements: &[Element], rels: &[Relationship], per_row: usize) -> f64 {
    let mut at: HashMap<&str, Point> = HashMap::new();
    for (i, &index) in order.iter().enumerate() {
        if let Some(el) = elements.get(index) {
            at.insert(
                el.alias.as_str(),
                Point::new(count(i % per_row), count(i / per_row)),
            );
        }
    }
    let chords: Vec<(Point, Point)> = rels
        .iter()
        .filter_map(|rel| Some((*at.get(rel.from.as_str())?, *at.get(rel.to.as_str())?)))
        .collect();
    let ccw = |p: Point, q: Point, r: Point| (r.y - p.y) * (q.x - p.x) > (q.y - p.y) * (r.x - p.x);
    let mut score = 0.0;
    for (i, &(a, b)) in chords.iter().enumerate() {
        score += (a.x - b.x).abs() + (a.y - b.y).abs();
        for &(c, d) in chords.iter().skip(i + 1) {
            if ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d) {
                score += 10.0;
            }
        }
    }
    score
}

/// Hill-climb one boundary group's element order against [`grid_score`].
///
/// Seeded and fixed-length, so the same diagram always yields the same drawing —
/// a layout that changed between renders would be worse than a mediocre one.
fn climb_group(
    group: &[usize],
    elements: &[Element],
    rels: &[Relationship],
    per_row: usize,
    seed: u32,
) -> Vec<usize> {
    let mut state = seed;
    let mut rnd = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(state) / 2.0_f64.powi(32)
    };
    let mut best = group.to_vec();
    let mut best_score = grid_score(&best, elements, rels, per_row);
    for _ in 0..4000 {
        let i = whole(rnd() * count(best.len()));
        let j = whole(rnd() * count(best.len()));
        if i == j {
            continue;
        }
        let mut next = best.clone();
        if next.get(i).is_none() || next.get(j).is_none() {
            continue;
        }
        next.swap(i, j);
        let score = grid_score(&next, elements, rels, per_row);
        if score <= best_score {
            best = next;
            best_score = score;
        }
    }
    best
}

/// Candidate element orders: the author's, plus the best few the climb finds.
///
/// Each order is a permutation of indices into the diagram's element list.
pub fn candidate_orders(diagram: &Diagram) -> Vec<Vec<usize>> {
    let mut groups: Vec<(Option<&str>, Vec<usize>)> = Vec::new();
    for (i, el) in diagram.elements.iter().enumerate() {
        let key = el.boundary.as_deref();
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(i),
            None => groups.push((key, vec![i])),
        }
    }
    let per_row = diagram.config.shape_in_row.max(1);
    let climbed: Vec<Vec<usize>> = [1, 7, 13, 29]
        .into_iter()
        .map(|seed| {
            groups
                .iter()
                .flat_map(|(_, group)| {
                    climb_group(
                        group,
                        &diagram.elements,
                        &diagram.relationships,
                        per_row,
                        seed,
                    )
                })
                .collect()
        })
        .collect();

    let mut scored: Vec<(Vec<usize>, f64)> = climbed
        .into_iter()
        .map(|order| {
            let score = grid_score(&order, &diagram.elements, &diagram.relationships, per_row);
            (order, score)
        })
        .collect();
    scored.sort_by(|p, q| p.1.total_cmp(&q.1));

    let mut seen: Vec<Vec<usize>> = Vec::new();
    let mut distinct: Vec<Vec<usize>> = Vec::new();
    for (order, _) in scored {
        if seen.contains(&order) {
            continue;
        }
        seen.push(order.clone());
        distinct.push(order);
        if distinct.len() == 2 {
            break;
        }
    }

    // The author's order comes first so it wins ties: when reordering buys
    // nothing measurable, the diagram should read in the order it was written.
    let mut out = vec![(0..diagram.elements.len()).collect::<Vec<usize>>()];
    out.extend(distinct);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c4::parse;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn diagram(source: &str) -> Diagram {
        parse(source)
    }

    #[test]
    fn a_clean_pair_of_routes_scores_nothing() {
        let routes = vec![
            vec![p(0.0, 0.0), p(100.0, 0.0)],
            vec![p(0.0, 50.0), p(100.0, 50.0)],
        ];
        assert!((draw_quality(&routes) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_crossing_costs_and_a_merged_run_costs_more() {
        let crossing = vec![
            vec![p(0.0, 0.0), p(100.0, 0.0)],
            vec![p(50.0, -50.0), p(50.0, 50.0)],
        ];
        let merged = vec![
            vec![p(0.0, 0.0), p(100.0, 0.0)],
            vec![p(0.0, 1.0), p(100.0, 1.0)],
        ];
        assert!((draw_quality(&crossing) - 400.0).abs() < 1e-9);
        assert!(draw_quality(&merged) > draw_quality(&crossing));
    }

    #[test]
    fn two_legs_of_one_edge_are_not_counted_against_it() {
        // An L: the two runs share a corner and belong to one edge.
        let routes = vec![vec![p(0.0, 0.0), p(100.0, 0.0), p(100.0, 100.0)]];
        // One bend, nothing else.
        assert!((draw_quality(&routes) - 12.0).abs() < 1e-9);
    }

    #[test]
    fn runs_meeting_at_a_corner_do_not_count_as_crossing() {
        let a = Run {
            a: p(0.0, 0.0),
            b: p(100.0, 0.0),
            horiz: true,
            edge: 0,
        };
        let b = Run {
            a: p(100.0, 0.0),
            b: p(100.0, 100.0),
            horiz: false,
            edge: 1,
        };
        assert!(!runs_cross(&a, &b));
        assert!(!runs_cross(&a, &a));
    }

    #[test]
    fn parallel_runs_only_merge_where_they_overlap() {
        let a = Run {
            a: p(0.0, 0.0),
            b: p(100.0, 0.0),
            horiz: true,
            edge: 0,
        };
        let b = Run {
            a: p(60.0, 1.0),
            b: p(200.0, 1.0),
            horiz: true,
            edge: 1,
        };
        assert!((shared_run(&a, &b) - 40.0).abs() < 1e-9);
        // Far enough apart to read as two lines.
        let far = Run {
            a: p(0.0, 30.0),
            b: p(100.0, 30.0),
            horiz: true,
            edge: 1,
        };
        assert!((shared_run(&a, &far) - 0.0).abs() < 1e-9);
        // Perpendicular runs never merge.
        let across = Run {
            a: p(0.0, 0.0),
            b: p(0.0, 100.0),
            horiz: false,
            edge: 1,
        };
        assert!((shared_run(&a, &across) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_zero_length_leg_is_not_a_run() {
        assert!(edge_runs(&[vec![p(1.0, 1.0), p(1.0, 1.0)]]).is_empty());
    }

    #[test]
    fn the_grid_proxy_prefers_short_uncrossed_chords() {
        // Two chords that cross on the diagonal, against the order that puts
        // each pair in its own column.
        let d = diagram(
            "C4Context\nSystem(a,\"A\")\nSystem(b,\"B\")\nSystem(c,\"C\")\nSystem(d,\"D\")\nRel(a,d,\"x\")\nRel(b,c,\"y\")",
        );
        let crossed = grid_score(&[0, 1, 2, 3], &d.elements, &d.relationships, 2);
        let stacked = grid_score(&[0, 1, 3, 2], &d.elements, &d.relationships, 2);
        assert!(stacked < crossed, "{stacked} !< {crossed}");
    }

    #[test]
    fn a_relationship_naming_a_missing_element_is_skipped() {
        let d = diagram("C4Context\nSystem(a,\"A\")\nRel(a,ghost,\"x\")");
        assert!((grid_score(&[0], &d.elements, &d.relationships, 2) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn the_climb_is_seeded_so_a_diagram_always_draws_the_same() {
        let d = diagram(
            "C4Context\nSystem(a,\"A\")\nSystem(b,\"B\")\nSystem(c,\"C\")\nSystem(d,\"D\")\nRel(a,d,\"x\")\nRel(b,c,\"y\")",
        );
        let group: Vec<usize> = (0..4).collect();
        let once = climb_group(&group, &d.elements, &d.relationships, 2, 7);
        let twice = climb_group(&group, &d.elements, &d.relationships, 2, 7);
        assert_eq!(once, twice);
        // ... and it is a permutation, not a rewrite.
        let mut sorted = once.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, group);
    }

    #[test]
    fn a_single_element_group_climbs_to_itself() {
        let d = diagram("C4Context\nSystem(a,\"A\")");
        assert_eq!(climb_group(&[0], &d.elements, &d.relationships, 2, 1), [0]);
    }

    #[test]
    fn the_authors_order_is_always_the_first_candidate() {
        let d =
            diagram("C4Context\nSystem(a,\"A\")\nSystem(b,\"B\")\nSystem(c,\"C\")\nRel(a,c,\"x\")");
        let orders = candidate_orders(&d);
        assert_eq!(orders.first(), Some(&vec![0, 1, 2]));
        assert!(orders.len() <= 3);
        // Every candidate is a permutation of the elements.
        for order in &orders {
            let mut sorted = order.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, vec![0, 1, 2]);
        }
    }

    #[test]
    fn elements_never_leave_their_own_boundary_group() {
        let d = diagram(
            "C4Context\nSystem(free,\"F\")\nSystem_Boundary(g,\"G\"){\nSystem(a,\"A\")\nSystem(b,\"B\")\n}",
        );
        for order in candidate_orders(&d) {
            // The ungrouped element is declared first and its group is emitted
            // first, so it stays at the front however the members shuffle.
            assert_eq!(order.first(), Some(&0), "{order:?}");
        }
    }
}
