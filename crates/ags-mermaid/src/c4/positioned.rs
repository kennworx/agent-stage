//! A C4 diagram with pixel geometry: what layout produces and the renderer draws.

use super::geom::{Point, Rect};
use super::types::{ElementKind, Variant};

/// One element box, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedElement {
    pub alias: String,
    pub kind: ElementKind,
    pub variant: Option<Variant>,
    /// Small kind tag rendered above the label, e.g. «Person».
    pub tag: String,
    pub label: String,
    pub techn: Option<String>,
    /// Description wrapped into display lines.
    pub descr: Vec<String>,
    pub external: bool,
    pub rect: Rect,
}

/// One relationship arrow, routed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedRelationship {
    pub from: String,
    pub to: String,
    pub label: String,
    pub techn: Option<String>,
    pub bidirectional: bool,
    /// Step marker shown in the badge on the edge, e.g. `1` or `3a`.
    ///
    /// Taken from a leading `1.` or `3a.` in the source label when the author
    /// numbered the steps themselves; otherwise assigned in declaration order.
    pub step: String,
    /// Arrow start, on the border of the source box.
    pub start: Point,
    /// Arrow end, on the border of the target box.
    pub end: Point,
    /// The full orthogonal route, start to end inclusive.
    ///
    /// Every segment is axis-aligned, so edges read as wiring rather than as
    /// loose diagonals, and parallel runs line up instead of fanning out.
    pub points: Vec<Point>,
    /// Centre of the step badge, chosen to avoid covering boxes and other badges.
    pub badge_center: Point,
    /// Badge box size, so the renderer draws the circle the placement pass
    /// reserved rather than one of its own.
    pub badge_width: f64,
    pub badge_height: f64,
    /// What the relationship says, with the step marker stripped and the
    /// technology appended in brackets. Shown in the description bubble that
    /// hovering the badge or either arrowhead raises.
    pub description: String,
}

impl PlacedRelationship {
    /// The badge as a rectangle, which is what the overlap tests take.
    pub fn badge_rect(&self) -> Rect {
        Rect::new(
            self.badge_center.x - self.badge_width / 2.0,
            self.badge_center.y - self.badge_height / 2.0,
            self.badge_width,
            self.badge_height,
        )
    }
}

/// One boundary frame, fitted to its content.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedBoundary {
    pub alias: String,
    pub label: String,
    pub kind: super::types::BoundaryKind,
    /// Nesting depth (0 = outermost), so the renderer can inset nested frames.
    pub depth: usize,
    pub rect: Rect,
}

/// A laid-out C4 diagram, ready for the renderer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<String>,
    pub elements: Vec<PlacedElement>,
    pub relationships: Vec<PlacedRelationship>,
    pub boundaries: Vec<PlacedBoundary>,
}

impl Placed {
    /// The element boxes as bare rectangles, which is all the routing and
    /// separation passes need of them.
    pub fn element_rects(&self) -> Vec<Rect> {
        self.elements.iter().map(|e| e.rect).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c4::types::BoundaryKind;

    fn relationship() -> PlacedRelationship {
        PlacedRelationship {
            from: "a".into(),
            to: "b".into(),
            label: "calls".into(),
            techn: None,
            bidirectional: false,
            step: "1".into(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            badge_center: Point::new(11.0, 11.0),
            badge_width: 22.0,
            badge_height: 22.0,
            description: "calls".into(),
        }
    }

    #[test]
    fn a_badge_reports_the_box_it_reserved() {
        assert_eq!(relationship().badge_rect(), Rect::new(0.0, 0.0, 22.0, 22.0));
    }

    #[test]
    fn a_placed_diagram_yields_its_boxes() {
        let placed = Placed {
            elements: vec![PlacedElement {
                alias: "a".into(),
                kind: ElementKind::System,
                variant: None,
                tag: "«System»".into(),
                label: "A".into(),
                techn: None,
                descr: vec![],
                external: false,
                rect: Rect::new(1.0, 2.0, 3.0, 4.0),
            }],
            boundaries: vec![PlacedBoundary {
                alias: "b".into(),
                label: "B".into(),
                kind: BoundaryKind::System,
                depth: 0,
                rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            }],
            relationships: vec![relationship()],
            ..Placed::default()
        };
        assert_eq!(placed.element_rects(), vec![Rect::new(1.0, 2.0, 3.0, 4.0)]);
        assert_eq!(placed.boundaries.len(), 1);
        assert_eq!(placed.relationships.len(), 1);
    }
}
