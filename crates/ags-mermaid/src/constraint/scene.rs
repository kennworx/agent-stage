//! Reading a laid-out scene: its shapes, the straight runs an edge is drawn as,
//! and the identity a group lends the pieces it is drawn from.

use crate::layout::as_f64;
use crate::scene::{Content, Node, Point, Role, Shape};

/// Whether a shape encloses area rather than tracing a line.
///
/// A polygon is closed by definition; a path is closed when it says so. Either way
/// the outline returns to where it began, so "does it come back?" is answered by
/// the shape rather than by the drawing.
pub(super) fn closed_shape(content: &Content) -> bool {
    match content {
        Content::Shape(Shape::Polygon(_)) => true,
        Content::Shape(Shape::Path(segs)) => segs.contains(&crate::scene::Seg::Close),
        _ => false,
    }
}

/// How close two parallel runs must be before they read as a single line.
pub(super) const MERGE_TOLERANCE: f64 = 6.0;

/// Shared overlap below which two runs are coincident by accident, not by fault.
pub(super) const MERGE_MIN_LENGTH: f64 = 8.0;

/// An axis-aligned box, the form every check works in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Rect {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

impl Rect {
    /// The smallest rectangle holding both.
    pub(super) fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            w: (self.x + self.w).max(other.x + other.w) - x,
            h: (self.y + self.h).max(other.y + other.h) - y,
        }
    }

    pub(super) fn contains_rect(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.x + other.w <= self.x + self.w
            && other.y + other.h <= self.y + self.h
    }
}

/// Every node in the scene, flattened, with groups replaced by their children.
/// A node, and the identity the group around it lends it.
///
/// A diagram type puts `data-id` and `data-from` on the group and the geometry
/// on its children, so a check that reads them off the same node sees neither.
/// Carrying them down is what lets an edge be told apart from the boxes it
/// legitimately touches.
#[derive(Debug, Clone)]
pub(super) struct Marked<'a> {
    pub(super) node: &'a Node,
    pub(super) id: Option<String>,
    pub(super) from: Option<String>,
    pub(super) to: Option<String>,
    /// What a frame says it is drawn round. Inherited for the same reason the
    /// id is: the datum sits on the group and the geometry on its child.
    pub(super) holds: Option<String>,
}

impl Marked<'_> {
    /// Whether this edge is one of the two boxes it connects.
    pub(super) fn joins(&self, id: Option<&String>) -> bool {
        id.is_some() && (self.from.as_ref() == id || self.to.as_ref() == id)
    }

    /// What to call this in a report.
    ///
    /// A box is named by its id. An edge has none — its identity *is* the pair it
    /// joins — so a report that reads only the id calls every edge "something",
    /// and a finding about two of them names neither. Tracking down which pair
    /// "the edges something and something" meant took a rendered drawing and a
    /// calculator; `Connected → Disconnecting` would have said it outright.
    pub(super) fn name(&self) -> Option<String> {
        self.id.clone().or_else(|| match (&self.from, &self.to) {
            (Some(from), Some(to)) => Some(format!("{from} → {to}")),
            (Some(from), None) => Some(format!("{from} →")),
            (None, Some(to)) => Some(format!("→ {to}")),
            (None, None) => None,
        })
    }

    /// Whether this box belongs to something `edge` connects.
    ///
    /// A box that is drawn on another element records which one in its data —
    /// `data-actor` on a sequence activation. That is a claim of ownership, and a
    /// wire arriving at the owner is not passing through anything.
    pub(super) fn owned_by(&self, edge: &Self) -> bool {
        self.node
            .data
            .iter()
            .any(|(_, owner)| edge.joins(Some(owner)))
    }

    /// Whether these two edges meet at a box in common.
    ///
    /// Lines converging on the same node touch near it by construction, so a
    /// crossing there is the diagram being connected, not a defect.
    pub(super) fn shares_a_box_with(&self, other: &Self) -> bool {
        let mine = [self.from.as_ref(), self.to.as_ref()];
        let theirs = [other.from.as_ref(), other.to.as_ref()];
        mine.iter()
            .flatten()
            .any(|m| theirs.iter().flatten().any(|t| m == t))
    }

    /// Whether this is a route — a line drawn from one place to another — as
    /// opposed to a filled area that happens to carry the edge role.
    ///
    /// A sankey link is an `Edge` with a `from` and a `to`, and it is a ribbon:
    /// a filled band whose outline runs out along one side and back along the
    /// other. Every question these rules ask about a route has a nonsense answer
    /// for it — it "travels away from its target and returns" by construction,
    /// because it is a closed shape. All six backtracking findings over the
    /// reference gallery were one sankey diagram, and they were the shape being
    /// itself.
    ///
    /// Closure is the discriminator, not the shape kind and not the paint. The
    /// band is a `Path`, the same variant an orthogonal route uses, and it takes
    /// its fill from a CSS class rather than from `Paint` — so neither tells the
    /// two apart. What does is that the band's path ends in [`Seg::Close`]: it
    /// returns to where it started, which is what makes it an area and exactly why
    /// every route question about it answers "yes, it comes back".
    pub(super) fn is_route(&self) -> bool {
        self.node.role == Role::Edge && !closed_shape(&self.node.content)
    }

    /// Whether this stroke claims to connect anything at all.
    ///
    /// An edge declares its endpoints as `from`/`to` data. A stroke carrying
    /// neither is a line the diagram draws for another reason — a series, an
    /// axis, a spine — and questions about which boxes it connects have no answer
    /// rather than a bad one.
    pub(super) fn connects(&self) -> bool {
        self.from.is_some() || self.to.is_some()
    }
}

pub(super) fn datum(node: &Node, key: &str) -> Option<String> {
    node.data
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.clone())
}

pub(super) fn mark<'a>(nodes: &'a [Node], inherited: &Marked<'a>, out: &mut Vec<Marked<'a>>) {
    for node in nodes {
        let here = Marked {
            node,
            id: node.id.clone().or_else(|| inherited.id.clone()),
            from: datum(node, "from").or_else(|| inherited.from.clone()),
            to: datum(node, "to").or_else(|| inherited.to.clone()),
            holds: datum(node, "holds").or_else(|| inherited.holds.clone()),
        };
        out.push(here.clone());
        if let Content::Group(children) = &node.content {
            mark(children, &here, out);
        }
    }
}

/// Every node in the scene, each carrying whatever identity it inherited.
pub(super) fn marked(nodes: &[Node]) -> Vec<Marked<'_>> {
    let mut out = Vec::new();
    let root = Marked {
        node: nodes.first().unwrap_or(&PLACEHOLDER),
        id: None,
        from: None,
        to: None,
        holds: None,
    };
    mark(nodes, &root, &mut out);
    out
}

/// Stands in for a parent when there is none. Never read.
pub(super) static PLACEHOLDER: Node = Node {
    role: Role::Decoration,
    layer: crate::scene::Layer::Frame,
    id: None,
    value: None,
    data: Vec::new(),
    class: Vec::new(),
    paint: crate::scene::Paint {
        fill: None,
        stroke: None,
        stroke_width: None,
        dash: None,
        marker_start: None,
        marker_end: None,
    },
    transform: None,
    title: None,
    content: Content::Group(Vec::new()),
};

pub(super) fn flatten<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for node in nodes {
        out.push(node);
        if let Content::Group(children) = &node.content {
            flatten(children, out);
        }
    }
}

/// The bounding box of a shape, or `None` for one that encloses nothing.
pub(super) fn bounds(shape: &Shape) -> Option<Rect> {
    let points: Vec<Point> = match shape {
        Shape::Rect { at, size, .. } => {
            return Some(Rect {
                x: at.x,
                y: at.y,
                w: size.width,
                h: size.height,
            })
        }
        Shape::Circle { c, r } => vec![Point::new(c.x - r, c.y - r), Point::new(c.x + r, c.y + r)],
        Shape::Ellipse { c, rx, ry } => vec![
            Point::new(c.x - rx, c.y - ry),
            Point::new(c.x + rx, c.y + ry),
        ],
        Shape::Line { a, b } => vec![*a, *b],
        Shape::Polyline(points) | Shape::Polygon(points) => points.clone(),
        Shape::Path(segs) => walked(segs),
    };
    from_points(&points)
}

/// The endpoint a segment moves to, for bounding purposes.
/// The points a path is drawn through, with its curves sampled.
///
/// A curve used to be read as the straight line between its ends, which is where
/// it starts and finishes and not where it goes. That is exact for a chord and
/// silent about the arc: a merge curve bulging into a box its chord misses drew
/// through it and nothing said so. Sixteen samples put the error well under the
/// tolerances every rule here works to, and a path of straight segments is
/// unchanged — `MoveTo` and `LineTo` sample to themselves.
fn walked(segs: &[crate::scene::Seg]) -> Vec<Point> {
    use crate::scene::Seg;
    const STEPS: usize = 16;
    let mut out: Vec<Point> = Vec::new();
    let mut at = Point::new(0.0, 0.0);
    for seg in segs {
        match seg {
            Seg::MoveTo(p) | Seg::LineTo(p) => {
                at = *p;
                out.push(at);
            }
            Seg::Quad { ctrl, to } => {
                for step in 1..=STEPS {
                    let t = as_f64(step) / as_f64(STEPS);
                    let u = 1.0 - t;
                    out.push(Point::new(
                        u * u * at.x + 2.0 * u * t * ctrl.x + t * t * to.x,
                        u * u * at.y + 2.0 * u * t * ctrl.y + t * t * to.y,
                    ));
                }
                at = *to;
            }
            Seg::Cubic { c1, c2, to } => {
                for step in 1..=STEPS {
                    let along = as_f64(step) / as_f64(STEPS);
                    let (t, u) = (along, 1.0 - along);
                    let (start, first, second, finish) =
                        (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push(Point::new(
                        start * at.x + first * c1.x + second * c2.x + finish * to.x,
                        start * at.y + first * c1.y + second * c2.y + finish * to.y,
                    ));
                }
                at = *to;
            }
            // An arc's own sweep is not reconstructed here: no diagram draws one
            // as a route, so its ends are all any rule has ever asked about.
            Seg::Arc { to, .. } => {
                at = *to;
                out.push(at);
            }
            Seg::Close => {}
        }
    }
    out
}

pub(super) fn from_points(points: &[Point]) -> Option<Rect> {
    let first = points.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Some(Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// The straight runs an edge is drawn as.
pub(super) fn runs(node: &Node) -> Vec<(Point, Point)> {
    let points = match &node.content {
        Content::Shape(Shape::Polyline(points) | Shape::Polygon(points)) => points.clone(),
        Content::Shape(Shape::Line { a, b }) => vec![*a, *b],
        Content::Shape(Shape::Path(segs)) => walked(segs),
        _ => Vec::new(),
    };
    points
        .windows(2)
        .filter_map(|w| match w {
            [a, b] if (a.x - b.x).abs() > 0.5 || (a.y - b.y).abs() > 0.5 => Some((*a, *b)),
            _ => None,
        })
        .collect()
}

/// Length over which two axis-aligned runs are drawn on top of each other.
pub(super) fn shared_length(a: (Point, Point), b: (Point, Point)) -> f64 {
    let horizontal = |r: (Point, Point)| (r.0.y - r.1.y).abs() < 0.5;
    if horizontal(a) != horizontal(b) {
        return 0.0;
    }
    let (across_a, across_b) = if horizontal(a) {
        (a.0.y, b.0.y)
    } else {
        (a.0.x, b.0.x)
    };
    if (across_a - across_b).abs() > MERGE_TOLERANCE {
        return 0.0;
    }
    let span = |r: (Point, Point)| {
        if horizontal(r) {
            (r.0.x.min(r.1.x), r.0.x.max(r.1.x))
        } else {
            (r.0.y.min(r.1.y), r.0.y.max(r.1.y))
        }
    };
    let (a0, a1) = span(a);
    let (b0, b1) = span(b);
    (a1.min(b1) - a0.max(b0)).max(0.0)
}

/// Whether a run passes through a box's interior.
///
/// This was a comparison of the two bounding boxes, which is exact while every
/// run is axis-aligned and wrong the moment one is not: a diagonal fills only a
/// sliver of the box it spans, and asking whether the *boxes* overlap reports it
/// as hitting everything in the corner it reaches across. Two of the gallery's
/// last six findings were that — a requirement diagram, which joins its boxes
/// with straight diagonals, and a git graph, whose merge curve is a cubic.
///
/// So the run is clipped against the box instead, which is the same answer for
/// an axis-aligned run and the right one for the rest.
pub(super) fn run_hits(run: (Point, Point), box_: Rect) -> bool {
    // Inset, so a wire running along a border is not counted as passing through.
    const PAD: f64 = 2.0;
    let (low, high) = (
        Point::new(box_.x + PAD, box_.y + PAD),
        Point::new(box_.x + box_.w - PAD, box_.y + box_.h - PAD),
    );
    // A box no wider than the inset has no interior left to pass through.
    if low.x >= high.x || low.y >= high.y {
        return false;
    }
    let (mut enters, mut leaves) = (0.0_f64, 1.0_f64);
    for (step, start, lower, upper) in [
        (run.1.x - run.0.x, run.0.x, low.x, high.x),
        (run.1.y - run.0.y, run.0.y, low.y, high.y),
    ] {
        // Parallel to this pair of sides: it either runs between them for its
        // whole length or misses the box entirely.
        if step.abs() < 1e-9 {
            if start <= lower || start >= upper {
                return false;
            }
            continue;
        }
        let (a, b) = ((lower - start) / step, (upper - start) / step);
        enters = enters.max(a.min(b));
        leaves = leaves.min(a.max(b));
    }
    enters < leaves
}

/// How far a route may travel away from its target before it reads as a detour
/// rather than a bend. A lattice router turns corners constantly; only a real
/// excursion is worth saying anything about.
pub(super) const BACKTRACK_TOLERANCE: f64 = 24.0;

/// The centre of a box.
pub(super) fn centre(r: Rect) -> Point {
    Point::new(r.x.midpoint(r.x + r.w), r.y.midpoint(r.y + r.h))
}

/// The outward normal of the box face `p` sits on, or `None` when `p` is not on
/// one — which is the usual case for a route that stops short of the border.
pub(super) fn exit_normal(p: Point, r: Rect) -> Option<Point> {
    const PAD: f64 = 2.0;
    let near = |a: f64, b: f64| (a - b).abs() <= PAD;
    let within_x = p.x >= r.x - PAD && p.x <= r.x + r.w + PAD;
    let within_y = p.y >= r.y - PAD && p.y <= r.y + r.h + PAD;
    if within_y && near(p.x, r.x) {
        return Some(Point::new(-1.0, 0.0));
    }
    if within_y && near(p.x, r.x + r.w) {
        return Some(Point::new(1.0, 0.0));
    }
    if within_x && near(p.y, r.y) {
        return Some(Point::new(0.0, -1.0));
    }
    if within_x && near(p.y, r.y + r.h) {
        return Some(Point::new(0.0, 1.0));
    }
    None
}

/// Whether leaving `from` at `p` heads away from `to`.
///
/// "Away" is not merely "not towards": an edge leaving the bottom face to reach
/// something to the right is fine, and flagging it would flag most of every
/// lattice route. The face has to point *behind* the target — past the far side of
/// the box it is leaving — before the drawing is actually misleading.
pub(super) fn faces_away(p: Point, from: Rect, to: Rect) -> bool {
    let Some(n) = exit_normal(p, from) else {
        return false;
    };
    let (a, b) = (centre(from), centre(to));
    let along = n.x * (b.x - a.x) + n.y * (b.y - a.y);
    // The box's half-extent along the face normal — only one term is ever non-zero,
    // since a normal is axis-aligned, so this is that side's half-width.
    let half = f64::midpoint(n.x.abs() * from.w, n.y.abs() * from.h);
    along < -half
}

/// Whether two axis-aligned runs cross at a point interior to both.
///
/// Touching at a shared end is not a crossing — that is two segments of one route,
/// or two edges meeting at the box they share.
pub(super) fn runs_cross(a: (Point, Point), b: (Point, Point)) -> bool {
    const PAD: f64 = 1.0;
    let horizontal = |r: (Point, Point)| (r.0.y - r.1.y).abs() < 0.5;
    if horizontal(a) == horizontal(b) {
        return false;
    }
    let (h, v) = if horizontal(a) { (a, b) } else { (b, a) };
    let (x0, x1) = (h.0.x.min(h.1.x), h.0.x.max(h.1.x));
    let (y0, y1) = (v.0.y.min(v.1.y), v.0.y.max(v.1.y));
    v.0.x > x0 + PAD && v.0.x < x1 - PAD && h.0.y > y0 + PAD && h.0.y < y1 - PAD
}

/// Each named box once, as everything drawn under that name put together.
///
/// A shape apiece would report the same box several times — a subroutine is a
/// rectangle and two rules — and would ask whether a part of it is enclosed
/// rather than whether the box is.
pub(super) fn boxes(nodes: &[Marked<'_>]) -> Vec<(String, Rect)> {
    let mut out: Vec<(String, Rect)> = Vec::new();
    for held in nodes.iter().filter(|held| held.node.role == Role::Node) {
        let (Some(id), Content::Shape(shape)) = (held.id.clone(), &held.node.content) else {
            continue;
        };
        let Some(rect) = bounds(shape) else { continue };
        match out.iter_mut().find(|(seen, _)| *seen == id) {
            Some((_, area)) => *area = area.union(rect),
            None => out.push((id, rect)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_drawn_line_has_runs_to_ask_about() {
        // A line is one run; a group has no geometry of its own, so there is
        // nothing to ask about where it goes.
        let line = Node::new(
            Role::Edge,
            Content::Shape(Shape::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(10.0, 0.0),
            }),
        );
        assert_eq!(runs(&line).len(), 1);
        assert!(runs(&Node::new(Role::Edge, Content::Group(Vec::new()))).is_empty());
        // And a run going nowhere is not a run.
        let dot = Node::new(
            Role::Edge,
            Content::Shape(Shape::Polyline(vec![
                Point::new(5.0, 5.0),
                Point::new(5.0, 5.0),
            ])),
        );
        assert!(runs(&dot).is_empty());
    }

    #[test]
    fn two_rectangles_join_into_the_one_that_holds_both() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        };
        let b = Rect {
            x: 20.0,
            y: 5.0,
            w: 10.0,
            h: 10.0,
        };
        let joined = a.union(b);
        assert_eq!(
            joined,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 30.0,
                h: 15.0
            }
        );
        assert!(joined.contains_rect(a) && joined.contains_rect(b));
    }
}
