//! Turning a parsed C4 diagram into pixel geometry.

use std::collections::HashMap;

use crate::round::round_half_up;

use super::config as l;
use super::geom::{
    bounds, count, crosses_existing, face_point, legs, path_length, separation, simplify, Point,
    Rect,
};
use super::labels::{place_labels, step_of};
use super::lattice::{
    build_lattice, charge_lanes, mark_occupied, route_on_lattice, LaneLoad, Lattice, Occupancy,
};
use super::place::{place, size_boxes, Placement};
use super::ports::{assign_ports, candidate_side_pairs, choose_sides, face_mismatch, Attachment};
use super::positioned::{Placed, PlacedRelationship};
use super::quality::{candidate_orders, draw_quality};
use super::types::{Diagram, Relationship};

/// Lay out a parsed C4 diagram into pixel geometry.
///
/// The element order is searched, not taken as given: see [`candidate_orders`].
/// Above a few dozen elements the search is skipped — the proxy is quadratic in
/// relationships and the win shrinks as the grid grows.
pub fn layout(diagram: &Diagram) -> Placed {
    let searchable = diagram.elements.len() > 2
        && diagram.elements.len() <= 48
        && diagram.relationships.len() <= 48;
    if !searchable {
        return layout_once(diagram);
    }

    let mut best: Option<(Placed, f64)> = None;
    for order in candidate_orders(diagram) {
        let permuted = Diagram {
            elements: order
                .iter()
                .filter_map(|&i| diagram.elements.get(i).cloned())
                .collect(),
            ..diagram.clone()
        };
        let placed = layout_once(&permuted);
        let routes: Vec<Vec<Point>> = placed
            .relationships
            .iter()
            .map(|r| r.points.clone())
            .collect();
        let score = draw_quality(&routes);
        if best.as_ref().is_none_or(|(_, b)| score < *b) {
            best = Some((placed, score));
        }
    }
    best.map_or_else(|| layout_once(diagram), |(placed, _)| placed)
}

/// Lay out a parsed C4 diagram, for one fixed element order.
fn layout_once(diagram: &Diagram) -> Placed {
    let (box_size, shells) = size_boxes(&diagram.elements);
    let title_band = if diagram.title.is_some() {
        l::TITLE_H + l::TITLE_GAP
    } else {
        0.0
    };
    let top = l::PADDING + title_band;
    let placement = place(diagram, &shells, box_size, l::PADDING, top);

    let (attachments, sources) = attachments_of(diagram, &placement);
    let relationships = route_all(diagram, &placement, attachments, &sources);

    let mut placed = Placed {
        width: 0.0,
        height: 0.0,
        title: diagram.title.clone(),
        elements: placement.elements,
        relationships,
        boundaries: placement.boundaries,
    };
    finish(&mut placed, top);
    placed
}

/// Every relationship whose two ends were actually placed, with the faces it will
/// start from.
fn attachments_of(diagram: &Diagram, placement: &Placement) -> (Vec<Attachment>, Vec<usize>) {
    let mut by_alias: HashMap<&str, Rect> = HashMap::new();
    for e in &placement.elements {
        by_alias.insert(e.alias.as_str(), e.rect);
    }
    let mut attachments = Vec::new();
    let mut sources = Vec::new();
    for (i, rel) in diagram.relationships.iter().enumerate() {
        let (Some(&from), Some(&to)) = (
            by_alias.get(rel.from.as_str()),
            by_alias.get(rel.to.as_str()),
        ) else {
            continue;
        };
        attachments.push(Attachment {
            from_alias: rel.from.clone(),
            to_alias: rel.to.clone(),
            from,
            to,
            sides: choose_sides(&from, &to),
        });
        sources.push(i);
    }
    (attachments, sources)
}

/// Route the short hops first.
///
/// Whichever edge routes first gets the clear lane, and routing in declaration
/// order hands that to whoever the author happened to write down first. A long
/// detour taking the gutter between two side-by-side boxes forces the hop between
/// *those two boxes* — a line that should obviously be straight — to weave through
/// it. Shortest first, so the trivially straight edges claim their straight line
/// and the long-haul routes bend around them instead.
fn shortest_first(attachments: &[Attachment]) -> Vec<usize> {
    let mut order: Vec<(usize, f64)> = attachments
        .iter()
        .enumerate()
        .map(|(i, r)| (i, separation(&r.from, &r.to)))
        .collect();
    order.sort_by(|p, q| p.1.total_cmp(&q.1).then_with(|| p.0.cmp(&q.0)));
    order.into_iter().map(|(i, _)| i).collect()
}

/// Every box except the two an edge connects — what that edge must route around.
fn obstacles_for(placement: &Placement, r: &Attachment) -> Vec<Rect> {
    placement
        .elements
        .iter()
        .filter(|el| el.alias != r.from_alias && el.alias != r.to_alias)
        .map(|el| el.rect)
        .collect()
}

/// Choose each edge's faces by actually routing both options from the face
/// centres and keeping the tidier result.
///
/// Deciding from the centre-to-centre angle alone is what made an edge that
/// travels upward attach to a side face and then hook back in.
///
/// Trials accumulate: each edge's chosen route is recorded before the next edge is
/// trialled, so a later edge can see what is already in the way. Judging every
/// edge against an empty diagram picks tidy routes individually and a tangle
/// collectively.
fn choose_faces(
    attachments: &mut [Attachment],
    order: &[usize],
    placement: &Placement,
    lattice: &Lattice,
) {
    let mut load = LaneLoad::new();
    let mut occupied = Occupancy::new();
    let mut committed: Vec<(Point, Point)> = Vec::new();
    for &si in order {
        let Some(r) = attachments.get(si) else {
            continue;
        };
        let obstacles = obstacles_for(placement, r);
        let mut best = r.sides;
        let mut best_score = f64::INFINITY;
        let mut best_raw: Vec<Point> = Vec::new();
        for sides in candidate_side_pairs(&r.from, &r.to) {
            let raw = route_on_lattice(
                face_point(&r.from, sides.start),
                face_point(&r.to, sides.end),
                sides,
                lattice,
                &load,
                &obstacles,
                &occupied,
            );
            let trial = simplify(&raw);
            // Crossing another edge costs more than an extra corner: a corner is
            // read at a glance, an intersection makes two lines momentarily
            // ambiguous.
            let score = count(crosses_existing(&trial, &committed)) * 3000.0
                + count(super::geom::bend_count(&trial)) * 1000.0
                + count(face_mismatch(sides, &r.from, &r.to)) * l::FACE_PENALTY
                + path_length(&trial);
            if score < best_score {
                best_score = score;
                best = sides;
                best_raw = raw;
            }
        }
        if let Some(r) = attachments.get_mut(si) {
            r.sides = best;
        }
        charge_lanes(&best_raw, &mut load);
        mark_occupied(&best_raw, &mut occupied);
        committed.extend(legs(&simplify(&best_raw)));
    }
}

/// Assign ports, route every edge, and separate what ended up drawn together.
///
/// Routed shortest-first, but stored — and numbered — in declaration order, so
/// the badges still read 1, 2, 3 down the author's sequence.
fn route_all(
    diagram: &Diagram,
    placement: &Placement,
    mut attachments: Vec<Attachment>,
    sources: &[usize],
) -> Vec<PlacedRelationship> {
    let boxes: Vec<Rect> = placement.elements.iter().map(|e| e.rect).collect();
    let (gap_x, gap_y) = super::place::gutters(&diagram.boundaries);
    let lattice = build_lattice(&boxes, gap_x, gap_y);
    let order = shortest_first(&attachments);
    choose_faces(&mut attachments, &order, placement, &lattice);

    let ports = assign_ports(&attachments);
    let mut load = LaneLoad::new();
    let mut occupied = Occupancy::new();
    let mut routes: Vec<Vec<Point>> = vec![Vec::new(); attachments.len()];
    for &i in &order {
        let (Some(r), Some(port)) = (attachments.get(i), ports.get(i)) else {
            continue;
        };
        let obstacles = obstacles_for(placement, r);
        // Occupancy is recorded on the *raw* lattice path, before collinear
        // points are dropped. Simplifying first would leave a long straight run
        // marked only at its two corners, so later edges would cross its middle
        // for free — which is exactly where crossings happen.
        let raw = route_on_lattice(
            port.start, port.end, r.sides, &lattice, &load, &obstacles, &occupied,
        );
        charge_lanes(&raw, &mut load);
        mark_occupied(&raw, &mut occupied);
        if let Some(slot) = routes.get_mut(i) {
            *slot = simplify(&raw);
        }
    }

    let mut relationships: Vec<PlacedRelationship> = routes
        .iter()
        .enumerate()
        .filter_map(|(i, points)| {
            let rel = sources.get(i).and_then(|&s| diagram.relationships.get(s))?;
            Some(assemble(rel, points, i))
        })
        .collect();

    // Separate coincident runs before the badges are placed, so a badge lands on
    // the line it actually labels rather than on the pair's shared centre.
    let mut routes: Vec<Vec<Point>> = relationships.iter().map(|r| r.points.clone()).collect();
    super::nudge::nudge_overlaps(&mut routes, &boxes);
    for (rel, points) in relationships.iter_mut().zip(routes) {
        rel.points = points;
    }
    place_labels(&mut relationships, &boxes);
    relationships
}

/// One routed edge, with its badge provisionally at the middle of its ends.
///
/// The edge carries a numbered badge, not its prose. A badge is small enough that
/// it never has to fight a box or a sibling for space, and the reader gets the
/// wording by hovering the badge or either arrowhead.
fn assemble(rel: &Relationship, points: &[Point], index: usize) -> PlacedRelationship {
    let first = points.first().copied().unwrap_or(Point::new(0.0, 0.0));
    let last = points.last().copied().unwrap_or(Point::new(0.0, 0.0));
    let (step, text) = step_of(&rel.label, rel.techn.as_deref(), index);
    PlacedRelationship {
        from: rel.from.clone(),
        to: rel.to.clone(),
        label: rel.label.clone(),
        techn: rel.techn.clone(),
        bidirectional: rel.bidirectional,
        step,
        start: first,
        end: last,
        points: points.to_vec(),
        // Provisional: `place_labels` moves it onto a clear stretch of the route.
        badge_center: Point::new(
            f64::midpoint(first.x, last.x),
            f64::midpoint(first.y, last.y),
        ),
        badge_width: l::BADGE_SIZE,
        badge_height: l::BADGE_SIZE,
        description: text,
    }
}

/// Everything actually drawn, as rectangles — what the canvas has to hold.
///
/// Measured over the real geometry, boundary frames and badges included. Sizing
/// from the grid alone silently cropped whatever the label pass had nudged
/// outward, and a badge pushed past the edge is invisible, which is worse than
/// the collision it was avoiding.
fn drawn_extent(placed: &Placed) -> Vec<Rect> {
    let mut drawn: Vec<Rect> = placed.elements.iter().map(|e| e.rect).collect();
    drawn.extend(placed.boundaries.iter().map(|b| b.rect));
    for rel in &placed.relationships {
        drawn.extend(rel.points.iter().map(|p| Rect::new(p.x, p.y, 0.0, 0.0)));
    }
    drawn.extend(
        placed
            .relationships
            .iter()
            .filter(|rel| rel.badge_width > 0.0)
            .map(PlacedRelationship::badge_rect),
    );
    drawn
}

/// Slide everything so the top-left of what is drawn sits on the padding.
fn shift_by(placed: &mut Placed, dx: f64, dy: f64) {
    for e in &mut placed.elements {
        e.rect.x += dx;
        e.rect.y += dy;
    }
    for b in &mut placed.boundaries {
        b.rect.x += dx;
        b.rect.y += dy;
    }
    for rel in &mut placed.relationships {
        rel.start.x += dx;
        rel.start.y += dy;
        rel.end.x += dx;
        rel.end.y += dy;
        rel.badge_center.x += dx;
        rel.badge_center.y += dy;
        for p in &mut rel.points {
            p.x += dx;
            p.y += dy;
        }
    }
}

/// Fit the canvas to the drawing.
///
/// There used to be a numbered key beneath it, and the canvas was sized to hold
/// whichever was wider. Hovering a badge or an arrowhead already raises a bubble
/// carrying the same sentence, so the key restated on every diagram what the
/// drawing says on demand — and it charged for that in height on every diagram,
/// read or not.
fn finish(placed: &mut Placed, top: f64) {
    let Some(extent) = bounds(&drawn_extent(placed)) else {
        // Nothing to draw — an empty diagram still needs a valid canvas.
        placed.width = l::PADDING * 2.0;
        placed.height = top + l::PADDING;
        return;
    };
    let shift_x = l::PADDING - extent.x;
    let shift_y = top - extent.y;
    if shift_x != 0.0 || shift_y != 0.0 {
        shift_by(placed, shift_x, shift_y);
    }

    placed.width = round_half_up(extent.right() + shift_x + l::PADDING);
    placed.height = round_half_up(extent.bottom() + shift_y + l::PADDING);
}
