//! Stopping an edge on the outline it points at, rather than on its box.
//!
//! A diamond's outline sits well inside its bounding box, so an arrowhead that
//! stops at the box floats in the gap beside the shape it is aimed at.

use crate::scene::Point;

use super::layout::PlacedNode;
use super::types::Shape;

/// The outline of a shape, as the polygon it is drawn with.
///
/// `None` for a shape that fills its box, and for a round one — a circle is
/// clipped by its own arithmetic rather than by a polygon.
pub(super) fn outline(node: &PlacedNode) -> Option<Vec<Point>> {
    let (x, y) = (node.at.x, node.at.y);
    let (w, h) = (node.width, node.height);
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    match node.shape {
        Shape::Diamond => Some(vec![
            Point::new(cx, y),
            Point::new(x + w, cy),
            Point::new(cx, y + h),
            Point::new(x, cy),
        ]),
        Shape::Hexagon => {
            let inset = h / 4.0;
            Some(vec![
                Point::new(x + inset, y),
                Point::new(x + w - inset, y),
                Point::new(x + w, cy),
                Point::new(x + w - inset, y + h),
                Point::new(x + inset, y + h),
                Point::new(x, cy),
            ])
        }
        Shape::Asymmetric => Some(vec![
            Point::new(x + 12.0, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x + 12.0, y + h),
            Point::new(x, cy),
        ]),
        Shape::Trapezoid => {
            let inset = w * 0.15;
            Some(vec![
                Point::new(x + inset, y),
                Point::new(x + w - inset, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
            ])
        }
        Shape::TrapezoidAlt => {
            let inset = w * 0.15;
            Some(vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w - inset, y + h),
                Point::new(x + inset, y + h),
            ])
        }
        _ => None,
    }
}

/// Where an axis-aligned ray from `at` crosses the segment `a`–`b`.
pub(super) fn crossing(at: Point, a: Point, b: Point, vertical: bool) -> Option<Point> {
    let (along, from, to) = if vertical {
        (at.x, a.x, b.x)
    } else {
        (at.y, a.y, b.y)
    };
    let span = to - from;
    if span.abs() < 0.001 {
        return None;
    }
    let t = (along - from) / span;
    if !(0.0..=1.0).contains(&t) {
        return None;
    }
    Some(if vertical {
        Point::new(at.x, a.y + t * (b.y - a.y))
    } else {
        Point::new(a.x + t * (b.x - a.x), at.y)
    })
}

/// Move an endpoint onto the outline it points at.
///
/// The ray is axis-aligned rather than aimed at the previous point, because the
/// routing is orthogonal and aiming it anywhere else would put a kink in the
/// last segment. Of the places the ray crosses, the nearest to where the edge
/// came from is the side it should stop on — the far one is where it would end
/// up after passing straight through.
pub(super) fn clip(at: Point, from: Point, node: &PlacedNode) -> Point {
    let vertical = (at.x - from.x).abs() < (at.y - from.y).abs();
    if matches!(node.shape, Shape::Circle | Shape::DoubleCircle) {
        let centre = node.centre();
        let radius = node.width.min(node.height) / 2.0;
        if vertical {
            let dx = at.x - centre.x;
            if dx.abs() > radius {
                return at;
            }
            let dy = (radius * radius - dx * dx).sqrt();
            let above = from.y < centre.y;
            return Point::new(at.x, if above { centre.y - dy } else { centre.y + dy });
        }
        let dy = at.y - centre.y;
        if dy.abs() > radius {
            return at;
        }
        let dx = (radius * radius - dy * dy).sqrt();
        let left = from.x < centre.x;
        return Point::new(if left { centre.x - dx } else { centre.x + dx }, at.y);
    }
    let Some(shape) = outline(node) else {
        return at;
    };
    let mut best: Option<(f64, Point)> = None;
    for pair in 0..shape.len() {
        let (Some(a), Some(b)) = (shape.get(pair), shape.get((pair + 1) % shape.len())) else {
            continue;
        };
        let Some(hit) = crossing(at, *a, *b, vertical) else {
            continue;
        };
        let away = (hit.x - from.x).hypot(hit.y - from.y);
        if best.is_none_or(|(seen, _)| away < seen) {
            best = Some((away, hit));
        }
    }
    best.map_or(at, |(_, hit)| hit)
}

/// Clip both ends of a route to the outlines they touch.
///
/// The neighbours are copied out before anything moves, so each end is clipped
/// against where the run actually came from rather than against an end the
/// other clip has already shifted.
pub(super) fn clip_ends(points: &mut [Point], source: &PlacedNode, target: &PlacedNode) {
    let count = points.len();
    if count < 2 {
        return;
    }
    let second = points.get(1).copied().unwrap_or_default();
    let penultimate = points.get(count - 2).copied().unwrap_or_default();
    if let Some(start) = points.first_mut() {
        *start = clip(*start, second, source);
    }
    if let Some(end) = points.last_mut() {
        *end = clip(*end, penultimate, target);
    }
}
