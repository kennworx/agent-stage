//! Which face of which box every edge attaches to, and where along it.
//!
//! Two passes, in this order: pick faces, then spread the edges sharing a face.
//! Assigning ports before routing is what stops three arrows leaving one box from
//! emerging on top of one another and only separating further out — which reads
//! as a single arrow.

use super::config as l;
use super::geom::{clamp, count, separation, Point, Rect, Side, SidePair};

/// An edge with both endpoints resolved, ready to be given ports and routed.
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    pub from_alias: String,
    pub to_alias: String,
    pub from: Rect,
    pub to: Rect,
    /// Reassigned by trial routing, so it is not fixed at construction.
    pub sides: SidePair,
}

/// Which end of an edge a port belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum End {
    Start,
    Finish,
}

/// Where one edge attaches at each end, and how many edges share those faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ports {
    pub start: Point,
    pub end: Point,
    pub start_shared: usize,
    pub end_shared: usize,
}

impl Ports {
    fn at(&self, which: End) -> Point {
        match which {
            End::Start => self.start,
            End::Finish => self.end,
        }
    }

    fn set(&mut self, which: End, p: Point) {
        match which {
            End::Start => self.start = p,
            End::Finish => self.end = p,
        }
    }
}

/// A port's identity: the edge it belongs to and which end of it.
type PortId = (usize, End);

/// Read one port's coordinate along the axis it may slide on.
fn axis_of(ports: &[Ports], id: PortId, vertical: bool) -> f64 {
    let p = ports.get(id.0).map_or(Point::new(0.0, 0.0), |q| q.at(id.1));
    if vertical {
        p.x
    } else {
        p.y
    }
}

/// Move one port along the axis it may slide on.
fn slide(ports: &mut [Ports], id: PortId, vertical: bool, to: f64) {
    let Some(port) = ports.get_mut(id.0) else {
        return;
    };
    let mut p = port.at(id.1);
    if vertical {
        p.x = to;
    } else {
        p.y = to;
    }
    port.set(id.1, p);
}

/// The face pairs an edge could plausibly use, best guess first.
///
/// Both axes are offered because the centre-to-centre direction is only a guess
/// at how the edge will actually run: once a route has to detour around a box it
/// can arrive from a completely different quarter, and attaching to the face the
/// guess picked makes the line double back to get in. The caller trial-routes
/// these and keeps whichever the geometry actually prefers.
pub fn candidate_side_pairs(from: &Rect, to: &Rect) -> [SidePair; 2] {
    let dx = to.center_x() - from.center_x();
    let dy = to.center_y() - from.center_y();
    let horizontal = if dx >= 0.0 {
        SidePair {
            start: Side::Right,
            end: Side::Left,
        }
    } else {
        SidePair {
            start: Side::Left,
            end: Side::Right,
        }
    };
    let vertical = if dy >= 0.0 {
        SidePair {
            start: Side::Bottom,
            end: Side::Top,
        }
    } else {
        SidePair {
            start: Side::Top,
            end: Side::Bottom,
        }
    };
    if dx.abs() > dy.abs() {
        [horizontal, vertical]
    } else {
        [vertical, horizontal]
    }
}

/// The first guess at an edge's faces, before any trial routing.
pub fn choose_sides(from: &Rect, to: &Rect) -> SidePair {
    candidate_side_pairs(from, to)[0]
}

/// How far a pair of attachment faces disagrees with where the edge is going.
///
/// Two ways to get it wrong, and both look like errors on the page:
///
/// - **Wrong axis.** A line whose journey is mostly upward should leave by the
///   top, not squeeze out of a side face and climb alongside the box.
/// - **Wrong direction.** Leaving by the right to reach something on the left
///   means doubling back across the box just left.
///
/// Scored rather than enforced: a face that disagrees is sometimes the only way
/// past an obstacle, and one wrong-looking edge beats one drawn through a box.
pub fn face_mismatch(sides: SidePair, from: &Rect, to: &Rect) -> usize {
    let dx = to.center_x() - from.center_x();
    let dy = to.center_y() - from.center_y();
    let want_vertical = dy.abs() > dx.abs();
    let is_vertical = sides.start.is_horizontal_face();
    let mut wrong = 0;
    if want_vertical != is_vertical {
        wrong += 1;
    }
    if is_vertical {
        if dy < 0.0 && sides.start != Side::Top {
            wrong += 1;
        }
        if dy > 0.0 && sides.start != Side::Bottom {
            wrong += 1;
        }
    } else {
        if dx < 0.0 && sides.start != Side::Left {
            wrong += 1;
        }
        if dx > 0.0 && sides.start != Side::Right {
            wrong += 1;
        }
    }
    wrong
}

/// One edge's claim on a box face.
struct Member {
    edge: usize,
    which: End,
    el: Rect,
    side: Side,
    /// Where along the face this edge would ideally attach: level with the box at
    /// its far end.
    want: f64,
}

/// Group every edge end by the face it touches, in declaration order.
fn members_by_face(routable: &[Attachment]) -> Vec<((String, Side), Vec<Member>)> {
    let aim = |side: Side, other: &Rect| {
        if side.is_horizontal_face() {
            other.center_x()
        } else {
            other.center_y()
        }
    };
    let mut faces: Vec<((String, Side), Vec<Member>)> = Vec::new();
    let mut push =
        |key: (String, Side), member: Member| match faces.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(member),
            None => faces.push((key, vec![member])),
        };
    for (edge, r) in routable.iter().enumerate() {
        push(
            (r.from_alias.clone(), r.sides.start),
            Member {
                edge,
                which: End::Start,
                el: r.from,
                side: r.sides.start,
                want: aim(r.sides.start, &r.to),
            },
        );
        push(
            (r.to_alias.clone(), r.sides.end),
            Member {
                edge,
                which: End::Finish,
                el: r.to,
                side: r.sides.end,
                want: aim(r.sides.end, &r.from),
            },
        );
    }
    faces
}

/// Spread the edges that share a box face along that face.
pub fn assign_ports(routable: &[Attachment]) -> Vec<Ports> {
    let mut ports = vec![
        Ports {
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 0.0),
            start_shared: 1,
            end_shared: 1,
        };
        routable.len()
    ];

    for (_, mut members) in members_by_face(routable) {
        // Hand out slots level with each target, not in declaration order. Two
        // edges leaving one face for targets on opposite sides otherwise take
        // each other's slot and cross before they have left the box.
        members.sort_by(|p, q| p.want.total_cmp(&q.want).then_with(|| p.edge.cmp(&q.edge)));
        let n = members.len();
        for (slot, m) in members.iter().enumerate() {
            let horizontal = m.side.is_horizontal_face();
            let extent = if horizontal { m.el.width } else { m.el.height };
            // Keep ports inside the face with a margin, and never further apart
            // than the face can hold.
            let spacing = l::PORT_SPACING.min((extent - 2.0 * l::PORT_MARGIN) / count(n.max(1)));
            let offset = (count(slot) - (count(n) - 1.0) / 2.0) * spacing;
            let point = if horizontal {
                Point::new(
                    m.el.center_x() + offset,
                    if m.side == Side::Top {
                        m.el.y
                    } else {
                        m.el.bottom()
                    },
                )
            } else {
                Point::new(
                    if m.side == Side::Left {
                        m.el.x
                    } else {
                        m.el.right()
                    },
                    m.el.center_y() + offset,
                )
            };
            if let Some(port) = ports.get_mut(m.edge) {
                port.set(m.which, point);
                match m.which {
                    End::Start => port.start_shared = n,
                    End::Finish => port.end_shared = n,
                }
            }
        }
    }

    align_facing_ports(routable, &mut ports);
    separate_across_gutter(routable, &mut ports);
    ports
}

/// Every port sitting on each face, as identities rather than coordinates, so a
/// pass that moves one sees the new position through `ports`.
fn ports_by_face(routable: &[Attachment]) -> Vec<((String, Side), Vec<PortId>)> {
    let mut faces: Vec<((String, Side), Vec<PortId>)> = Vec::new();
    let mut push = |key: (String, Side), id: PortId| match faces.iter_mut().find(|(k, _)| *k == key)
    {
        Some((_, list)) => list.push(id),
        None => faces.push((key, vec![id])),
    };
    for (i, r) in routable.iter().enumerate() {
        push((r.from_alias.clone(), r.sides.start), (i, End::Start));
        push((r.to_alias.clone(), r.sides.end), (i, End::Finish));
    }
    faces
}

/// Edges ordered by how far their two boxes sit apart, nearest first.
///
/// The line between two boxes that sit side by side is the one a reader most
/// expects to be straight, so it gets first claim; edges that travel further bend
/// around whatever is left.
fn by_hop(routable: &[Attachment]) -> Vec<usize> {
    let mut order: Vec<(usize, f64)> = routable
        .iter()
        .enumerate()
        .map(|(i, r)| (i, separation(&r.from, &r.to)))
        .collect();
    order.sort_by(|p, q| p.1.total_cmp(&q.1).then_with(|| p.0.cmp(&q.0)));
    order.into_iter().map(|(i, _)| i).collect()
}

/// Straighten an edge whose two ends face each other by sliding the freer port to
/// meet the other.
///
/// Spreading ports keeps sibling edges from leaving a box on top of one another,
/// but it also knocks a lone edge off-axis: if one face carries two edges and the
/// opposite face carries one, the pair is offset by half the spacing and the route
/// acquires a dog-leg for no reason. When one side is uncontested it can simply
/// move to line up, turning that route into a single straight segment.
fn align_facing_ports(routable: &[Attachment], ports: &mut [Ports]) {
    let faces = ports_by_face(routable);
    let on_face = |alias: &str, side: Side| -> Vec<PortId> {
        faces
            .iter()
            .find(|((a, s), _)| a == alias && *s == side)
            .map(|(_, list)| list.clone())
            .unwrap_or_default()
    };

    for i in by_hop(routable) {
        let Some(r) = routable.get(i) else { continue };
        if !r.sides.facing() {
            continue;
        }
        // `vertical` = the edge travels vertically, so its ports slide along x.
        let vertical = r.sides.start.is_horizontal_face();
        let low_of = |el: &Rect| (if vertical { el.x } else { el.y }) + l::PORT_MARGIN;
        let high_of =
            |el: &Rect| (if vertical { el.right() } else { el.bottom() }) - l::PORT_MARGIN;
        // Only the overlap of the two faces can carry a straight line at all.
        let low = low_of(&r.from).max(low_of(&r.to));
        let high = high_of(&r.from).min(high_of(&r.to));
        if low > high {
            continue;
        }

        let mut others: Vec<PortId> = on_face(&r.from_alias, r.sides.start)
            .into_iter()
            .filter(|id| *id != (i, End::Start))
            .collect();
        others.extend(
            on_face(&r.to_alias, r.sides.end)
                .into_iter()
                .filter(|id| *id != (i, End::Finish)),
        );

        let start_at = axis_of(ports, (i, End::Start), vertical);
        let end_at = axis_of(ports, (i, End::Finish), vertical);
        let free = |v: f64, ports: &[Ports]| {
            others
                .iter()
                .all(|id| (axis_of(ports, *id, vertical) - v).abs() >= l::PORT_SPACING * 0.6)
        };
        // Meet in the middle if that slot is free; otherwise let one end come to
        // the other, which is still one straight segment.
        let wanted = [
            clamp(f64::midpoint(start_at, end_at), low, high),
            clamp(end_at, low, high),
            clamp(start_at, low, high),
        ]
        .into_iter()
        .find(|v| free(*v, ports));
        let Some(wanted) = wanted else { continue };

        slide(ports, (i, End::Start), vertical, wanted);
        slide(ports, (i, End::Finish), vertical, wanted);
    }
}

/// Boxes sitting directly across a gutter from one another, with the two faces
/// that look across it.
fn facing_boxes(routable: &[Attachment]) -> Vec<((String, Side), (String, Side))> {
    let mut els: Vec<(String, Rect)> = Vec::new();
    let mut note = |alias: &str, rect: Rect| {
        if !els.iter().any(|(a, _)| a == alias) {
            els.push((alias.to_string(), rect));
        }
    };
    for r in routable {
        note(&r.from_alias, r.from);
        note(&r.to_alias, r.to);
    }

    let mut out = Vec::new();
    for (i, (a_alias, a)) in els.iter().enumerate() {
        for (b_alias, b) in els.iter().skip(i + 1) {
            let share_y = a.bottom().min(b.bottom()) - a.y.max(b.y);
            let share_x = a.right().min(b.right()) - a.x.max(b.x);
            let gap_x = (a.x - b.right()).max(b.x - a.right());
            let gap_y = (a.y - b.bottom()).max(b.y - a.bottom());
            if share_y > 20.0 && gap_x > 0.0 {
                out.push(if a.x < b.x {
                    (
                        (a_alias.clone(), Side::Right),
                        (b_alias.clone(), Side::Left),
                    )
                } else {
                    (
                        (a_alias.clone(), Side::Left),
                        (b_alias.clone(), Side::Right),
                    )
                });
            } else if share_x > 20.0 && gap_y > 0.0 {
                out.push(if a.y < b.y {
                    (
                        (a_alias.clone(), Side::Bottom),
                        (b_alias.clone(), Side::Top),
                    )
                } else {
                    (
                        (a_alias.clone(), Side::Top),
                        (b_alias.clone(), Side::Bottom),
                    )
                });
            }
        }
    }
    out
}

/// Pull apart ports that face each other across the same gutter.
///
/// Faces are slotted one box at a time, so a lone edge landing on the right of one
/// box and a lone edge leaving the left of the box beside it both take the centre
/// of their own face — the same height, head to head across the gutter, with the
/// whole rest of the face free. Two arrowheads meeting like that read as one
/// connection.
///
/// Ports belonging to a route that already runs dead straight between the pair are
/// pinned: that alignment is the thing worth keeping, and separating it would put
/// a bend back into a line that has no reason to have one.
fn separate_across_gutter(routable: &[Attachment], ports: &mut [Ports]) {
    let faces = ports_by_face(routable);
    let pinned: Vec<PortId> = ports
        .iter()
        .enumerate()
        .filter(|(_, p)| (p.start.x - p.end.x).abs() < 0.5 || (p.start.y - p.end.y).abs() < 0.5)
        .flat_map(|(i, _)| [(i, End::Start), (i, End::Finish)])
        .collect();
    let rect_of = |id: PortId| -> Rect {
        routable
            .get(id.0)
            .map_or(Rect::new(0.0, 0.0, 0.0, 0.0), |r| match id.1 {
                End::Start => r.from,
                End::Finish => r.to,
            })
    };

    for ((a_alias, a_side), (b_alias, b_side)) in facing_boxes(routable) {
        // `vertical` = the gutter is horizontal, so ports along these faces slide
        // in x.
        let vertical = a_side.is_horizontal_face();
        let pick = |alias: &String, side: Side| -> Vec<PortId> {
            faces
                .iter()
                .find(|((a, s), _)| a == alias && *s == side)
                .map(|(_, list)| list.clone())
                .unwrap_or_default()
        };
        let mut members = pick(&a_alias, a_side);
        members.extend(pick(&b_alias, b_side));
        if members.len() < 2 {
            continue;
        }
        members
            .sort_by(|m, n| axis_of(ports, *m, vertical).total_cmp(&axis_of(ports, *n, vertical)));

        let need = l::PORT_SPACING * 0.7;
        for k in 1..members.len() {
            let (Some(&prev), Some(&cur)) = (members.get(k - 1), members.get(k)) else {
                continue;
            };
            // The two ends of one edge are allowed to line up — that is a
            // straight route.
            if prev.0 == cur.0 {
                continue;
            }
            let prev_at = axis_of(ports, prev, vertical);
            let cur_at = axis_of(ports, cur, vertical);
            if cur_at - prev_at >= need {
                continue;
            }
            let mover = if pinned.contains(&cur) {
                if pinned.contains(&prev) {
                    continue;
                }
                prev
            } else {
                cur
            };
            let el = rect_of(mover);
            let lo = (if vertical { el.x } else { el.y }) + l::PORT_MARGIN;
            let hi = (if vertical { el.right() } else { el.bottom() }) - l::PORT_MARGIN;
            let target = if mover == cur {
                clamp(prev_at + need, lo, hi)
            } else {
                clamp(cur_at - need, lo, hi)
            };
            slide(ports, mover, vertical, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from: Rect, to: Rect) -> Attachment {
        Attachment {
            from_alias: "a".into(),
            to_alias: "b".into(),
            from,
            to,
            sides: choose_sides(&from, &to),
        }
    }

    fn named(from_alias: &str, from: Rect, to_alias: &str, to: Rect) -> Attachment {
        Attachment {
            from_alias: from_alias.into(),
            to_alias: to_alias.into(),
            from,
            to,
            sides: choose_sides(&from, &to),
        }
    }

    /// Two boxes side by side, with a gutter between them.
    fn side_by_side() -> (Rect, Rect) {
        (
            Rect::new(0.0, 0.0, 180.0, 120.0),
            Rect::new(280.0, 0.0, 180.0, 120.0),
        )
    }

    #[test]
    fn the_wider_separation_picks_the_axis() {
        let (a, b) = side_by_side();
        assert_eq!(
            choose_sides(&a, &b),
            SidePair {
                start: Side::Right,
                end: Side::Left
            }
        );
        assert_eq!(
            choose_sides(&b, &a),
            SidePair {
                start: Side::Left,
                end: Side::Right
            }
        );
        let below = Rect::new(0.0, 400.0, 180.0, 120.0);
        assert_eq!(
            choose_sides(&a, &below),
            SidePair {
                start: Side::Bottom,
                end: Side::Top
            }
        );
        assert_eq!(
            choose_sides(&below, &a),
            SidePair {
                start: Side::Top,
                end: Side::Bottom
            }
        );
    }

    #[test]
    fn both_axes_are_offered_so_a_detour_can_change_its_mind() {
        let (a, b) = side_by_side();
        let pairs = candidate_side_pairs(&a, &b);
        assert_ne!(
            pairs[0].start.is_horizontal_face(),
            pairs[1].start.is_horizontal_face()
        );
    }

    #[test]
    fn a_face_agreeing_with_the_journey_scores_nothing() {
        let (a, b) = side_by_side();
        assert_eq!(face_mismatch(choose_sides(&a, &b), &a, &b), 0);
    }

    #[test]
    fn leaving_by_the_wrong_face_is_scored_on_both_counts() {
        let (a, b) = side_by_side();
        // Wrong direction: out of the left to reach something on the right.
        let backwards = SidePair {
            start: Side::Left,
            end: Side::Right,
        };
        assert_eq!(face_mismatch(backwards, &a, &b), 1);
        // Wrong axis only: leaving downward for something beside and below.
        let lower = Rect::new(280.0, 100.0, 180.0, 120.0);
        let sideways = SidePair {
            start: Side::Bottom,
            end: Side::Top,
        };
        assert_eq!(face_mismatch(sideways, &a, &lower), 1);
        // Both at once: leaving *upward* for something beside and below.
        let inverted = SidePair {
            start: Side::Top,
            end: Side::Bottom,
        };
        assert_eq!(face_mismatch(inverted, &a, &lower), 2);
    }

    #[test]
    fn a_vertical_journey_is_scored_on_the_vertical_faces() {
        let a = Rect::new(0.0, 0.0, 180.0, 120.0);
        let below = Rect::new(0.0, 400.0, 180.0, 120.0);
        assert_eq!(
            face_mismatch(
                SidePair {
                    start: Side::Top,
                    end: Side::Bottom
                },
                &a,
                &below
            ),
            1
        );
        assert_eq!(
            face_mismatch(
                SidePair {
                    start: Side::Bottom,
                    end: Side::Top
                },
                &below,
                &a
            ),
            1
        );
    }

    #[test]
    fn a_lone_edge_attaches_at_the_middle_of_its_face() {
        let (a, b) = side_by_side();
        let ports = assign_ports(&[edge(a, b)]);
        assert_eq!(ports.len(), 1);
        assert!((ports[0].start.x - 180.0).abs() < 1e-9);
        assert!((ports[0].start.y - 60.0).abs() < 1e-9);
        assert!((ports[0].end.x - 280.0).abs() < 1e-9);
        // Facing, uncontested and level: the route is one straight segment.
        assert!((ports[0].start.y - ports[0].end.y).abs() < 1e-9);
    }

    #[test]
    fn siblings_sharing_a_face_are_spread_along_it() {
        // Both targets are far enough to the right that the edges leave by the
        // same face, and far enough apart vertically to want different slots.
        let a = Rect::new(0.0, 0.0, 180.0, 220.0);
        let up = Rect::new(400.0, -100.0, 180.0, 120.0);
        let down = Rect::new(400.0, 300.0, 180.0, 120.0);
        let ports = assign_ports(&[named("a", a, "up", up), named("a", a, "down", down)]);
        assert!(
            (ports[0].start.y - ports[1].start.y).abs() > 1.0,
            "{ports:?}"
        );
        assert_eq!(ports[0].start_shared, 2);
        // Slots are handed out level with the target, so the one going up sits
        // above the one going down.
        assert!(ports[0].start.y < ports[1].start.y, "{ports:?}");
    }

    #[test]
    fn a_lone_edge_slides_to_line_up_with_a_contested_face() {
        // Two edges share the left face of `b`; one of them also has the whole
        // right face of `a` to itself, so it can move to meet its partner.
        let a = Rect::new(0.0, 0.0, 180.0, 120.0);
        let b = Rect::new(280.0, 0.0, 180.0, 120.0);
        let c = Rect::new(0.0, 300.0, 180.0, 120.0);
        let ports = assign_ports(&[named("a", a, "b", b), named("c", c, "b", b)]);
        // The straight hop stayed straight.
        assert!(
            (ports[0].start.y - ports[0].end.y).abs() < 1e-9,
            "{ports:?}"
        );
    }

    #[test]
    fn two_heads_meeting_across_a_gutter_are_pulled_apart() {
        // One edge out of a's right face, one into a's right face from the box
        // opposite: both would take the centre of their own face and meet.
        let a = Rect::new(0.0, 0.0, 180.0, 220.0);
        let b = Rect::new(280.0, 0.0, 180.0, 220.0);
        let ports = assign_ports(&[named("a", a, "b", b), named("b", b, "a", a)]);
        let first = ports[0].start.y;
        let second = ports[1].end.y;
        assert!(
            (first - second).abs() >= l::PORT_SPACING * 0.7 - 1e-9,
            "{ports:?}"
        );
    }

    #[test]
    fn ports_across_a_horizontal_gutter_are_pulled_apart_too() {
        // Stacked boxes: the faces look at each other across a horizontal
        // gutter, so the ports slide in x rather than y.
        let top = Rect::new(0.0, 0.0, 220.0, 120.0);
        let bottom = Rect::new(0.0, 300.0, 220.0, 120.0);
        let ports = assign_ports(&[named("t", top, "b", bottom), named("b", bottom, "t", top)]);
        assert!(
            (ports[0].start.x - ports[1].end.x).abs() >= l::PORT_SPACING * 0.7 - 1e-9,
            "{ports:?}"
        );
        // Both ends of one edge still line up: that is a straight route, not a
        // collision.
        assert!(
            (ports[0].start.x - ports[0].end.x).abs() < 1e-9,
            "{ports:?}"
        );
    }

    #[test]
    fn a_straight_route_keeps_its_line_when_a_neighbour_crowds_it() {
        // Three boxes down one column against one on the right: the straight hop
        // is pinned, so separation moves whatever else lands beside it.
        let a = Rect::new(0.0, 0.0, 180.0, 220.0);
        let b = Rect::new(280.0, 0.0, 180.0, 220.0);
        let c = Rect::new(0.0, 300.0, 180.0, 220.0);
        let ports = assign_ports(&[
            named("a", a, "b", b),
            named("c", c, "b", b),
            named("b", b, "a", a),
        ]);
        // Whatever moved, no two ports on the shared gutter still coincide.
        let on_gutter = [ports[0].start.y, ports[2].end.y];
        assert!((on_gutter[0] - on_gutter[1]).abs() > 1.0, "{ports:?}");
    }

    #[test]
    fn two_lone_edges_head_to_head_across_a_gutter_are_pulled_apart() {
        // The case this pass exists for: neither face is contested, so slotting
        // puts both ports at the centre of their own face — the same height,
        // pointing at each other across the gutter.
        let a = Rect::new(0.0, 0.0, 180.0, 220.0);
        let b = Rect::new(280.0, 0.0, 180.0, 220.0);
        // Offset vertically, so the edge out of `a` is not already straight and
        // is therefore free to move.
        let far_right = Rect::new(600.0, 300.0, 180.0, 220.0);
        let far_left = Rect::new(-320.0, 0.0, 180.0, 220.0);
        let ports = assign_ports(&[
            named("a", a, "far_right", far_right),
            named("b", b, "far_left", far_left),
        ]);
        assert!(
            (ports[0].start.y - ports[1].start.y).abs() >= l::PORT_SPACING * 0.7 - 1e-9,
            "{ports:?}"
        );
        // The straight route was pinned, so it is the other one that moved.
        assert!((ports[1].start.y - 110.0).abs() < 1e-9, "{ports:?}");
    }

    #[test]
    fn the_pinned_port_holds_its_line_and_the_other_gives_way() {
        // Mirror of the above: here it is the *first* port along the face that
        // carries a straight route, so the second one is the one that moves.
        let a = Rect::new(0.0, 0.0, 180.0, 220.0);
        let b = Rect::new(280.0, 0.0, 180.0, 220.0);
        let far_right = Rect::new(600.0, 0.0, 180.0, 220.0);
        let far_left = Rect::new(-320.0, 300.0, 180.0, 220.0);
        let ports = assign_ports(&[
            named("a", a, "far_right", far_right),
            named("b", b, "far_left", far_left),
        ]);
        assert!((ports[0].start.y - 110.0).abs() < 1e-9, "{ports:?}");
        assert!(
            (ports[1].start.y - 110.0).abs() >= l::PORT_SPACING * 0.7 - 1e-9,
            "{ports:?}"
        );
    }

    #[test]
    fn a_facing_pair_with_one_port_between_them_needs_no_separating() {
        // `b` and `c` sit across a gutter from each other, but neither edge
        // attaches to the faces that look across it — so that gutter holds a
        // single port and there is nothing to pull apart.
        let a = Rect::new(0.0, 0.0, 180.0, 220.0);
        let b = Rect::new(280.0, 0.0, 180.0, 220.0);
        let c = Rect::new(280.0, 500.0, 180.0, 220.0);
        let ports = assign_ports(&[named("a", a, "b", b), named("a", a, "c", c)]);
        assert_eq!(ports.len(), 2);
        // The edge into `c` still arrives on its top face, untouched.
        assert!((ports[1].end.y - 500.0).abs() < 1e-9, "{ports:?}");
    }

    #[test]
    fn an_edge_with_no_facing_partner_is_left_alone() {
        // Diagonal boxes: the faces do not look at each other, so neither the
        // alignment nor the gutter pass applies.
        let a = Rect::new(0.0, 0.0, 180.0, 120.0);
        let b = Rect::new(280.0, 300.0, 180.0, 120.0);
        let ports = assign_ports(&[edge(a, b)]);
        assert_eq!(ports.len(), 1);
        assert!(ports[0].start.x > 0.0);
    }

    #[test]
    fn faces_that_do_not_overlap_cannot_carry_a_straight_line() {
        // `b` sits far below `a`'s face, so no shared band exists to align on.
        let a = Rect::new(0.0, 0.0, 180.0, 60.0);
        let b = Rect::new(280.0, 400.0, 180.0, 60.0);
        let ports = assign_ports(&[edge(a, b)]);
        assert!((ports[0].start.y - ports[0].end.y).abs() > 1.0, "{ports:?}");
    }

    #[test]
    fn nothing_to_route_yields_no_ports() {
        assert!(assign_ports(&[]).is_empty());
    }
}
