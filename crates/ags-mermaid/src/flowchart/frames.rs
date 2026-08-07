//! The box drawn round a subgraph, and the room the drawing needs for it.
//!
//! A frame is drawn round its members' own frames rather than round the raw
//! nodes, which is what makes a nested box sit inside the one round it. Since
//! `nest` lays each group out as a unit, the members are contiguous by
//! construction and a frame can no longer reach round a stranger.

use crate::label::Placed as PlacedLabel;
use crate::scene::Point;

use super::config::Config;
use super::layout::{Placed, PlacedGroup, PlacedNode};
use super::types::Graph;

/// The group that holds `at`, when one does.
pub(super) fn parent_of(graph: &Graph, at: usize) -> Option<usize> {
    graph
        .groups
        .iter()
        .position(|group| group.groups.contains(&at))
}

/// Every node a group holds, however deeply.
///
/// A frame is drawn round its children's frames too, so what it holds has to
/// include theirs or every nested node reads as a stranger.
pub(super) fn holds(graph: &Graph, at: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![at];
    // Bounded because a group cannot hold itself; the count is there because
    // the graph is built from a source and a source can say anything.
    for _ in 0..=graph.groups.len() {
        let Some(here) = pending.pop() else { break };
        let Some(group) = graph.groups.get(here) else {
            continue;
        };
        out.extend(group.nodes.iter().cloned());
        pending.extend(group.groups.iter().copied());
    }
    out
}

/// How deeply a group is nested. Outermost is nought.
pub(super) fn depth_of(graph: &Graph, at: usize) -> usize {
    let mut depth = 0;
    let mut found = at;
    // A group cannot hold itself, so walking up terminates; the bound is there
    // because the graph is built from a source and a source can say anything.
    while let Some(parent) = parent_of(graph, found) {
        depth += 1;
        found = parent;
        if depth > graph.groups.len() {
            break;
        }
    }
    depth
}

/// A rectangle grown to hold whatever it is shown.
#[derive(Clone, Copy)]
pub(super) struct Extent {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

impl Extent {
    const fn empty() -> Self {
        Self {
            left: f64::INFINITY,
            top: f64::INFINITY,
            right: f64::NEG_INFINITY,
            bottom: f64::NEG_INFINITY,
        }
    }

    fn hold(&mut self, x: f64, y: f64, width: f64, height: f64) {
        self.left = self.left.min(x);
        self.top = self.top.min(y);
        self.right = self.right.max(x + width);
        self.bottom = self.bottom.max(y + height);
    }
}

/// The box round each group.
///
/// Computed innermost first, each parent round its children's finished boxes
/// rather than round the raw nodes — which is what makes a nested box actually
/// sit inside the one round it when the two hold the same things.
///
/// A group is drawn round wherever its members ended up. Nothing yet keeps
/// those members together, so a box can enclose a node that does not belong to
/// it; the ordering pass has to learn about groups before that stops being
/// possible.
pub(super) fn group_boxes(graph: &Graph, nodes: &[PlacedNode], cfg: &Config) -> Vec<PlacedGroup> {
    // The parser closes a group when it ends, so the list is innermost first
    // and a child is always finished before the parent that needs it.
    let mut boxes: Vec<Option<PlacedGroup>> = vec![None; graph.groups.len()];
    for (at, group) in graph.groups.iter().enumerate() {
        let mut extent = Extent::empty();
        for id in &group.nodes {
            if let Some(node) = nodes.iter().find(|node| &node.id == id) {
                extent.hold(node.at.x, node.at.y, node.width, node.height);
            }
        }
        for child in &group.groups {
            if let Some(Some(held)) = boxes.get(*child) {
                extent.hold(held.at.x, held.at.y, held.width, held.height);
            }
        }
        if !extent.left.is_finite() {
            continue;
        }
        let at_x = extent.left - cfg.group_pad;
        let at_y = extent.top - cfg.group_pad - cfg.group_header;
        if let Some(slot) = boxes.get_mut(at) {
            *slot = Some(PlacedGroup {
                id: group.id.clone(),
                label: group.label.clone(),
                at: Point::new(at_x, at_y),
                width: extent.right + cfg.group_pad - at_x,
                height: extent.bottom + cfg.group_pad - at_y,
                depth: depth_of(graph, at),
                holds: holds(graph, at),
            });
        }
    }
    let mut out: Vec<PlacedGroup> = boxes.into_iter().flatten().collect();
    // Outermost first, so a nested box paints over the one round it.
    out.sort_by_key(|group| group.depth);
    out
}

/// Move everything so nothing sits at a negative coordinate.
///
/// A group's box reaches above and to the left of what it holds, which is off
/// the canvas until the drawing makes room for it. So does an edge label, which
/// now stands beside its wire rather than on it.
pub(super) fn make_room(placed: &mut Placed) {
    let labels: Vec<PlacedLabel> = placed
        .edges
        .iter()
        .filter_map(|edge| edge.label_at)
        .collect();
    let left = placed
        .groups
        .iter()
        .map(|group| group.at.x)
        .chain(labels.iter().map(|l| l.at.x - l.width / 2.0))
        .fold(0.0_f64, f64::min);
    let top = placed
        .groups
        .iter()
        .map(|group| group.at.y)
        .chain(labels.iter().map(|l| l.at.y - l.height / 2.0))
        .fold(0.0_f64, f64::min);
    let (dx, dy) = (-left, -top);
    if dx > 0.0 || dy > 0.0 {
        for node in &mut placed.nodes {
            node.at = Point::new(node.at.x + dx, node.at.y + dy);
        }
        for edge in &mut placed.edges {
            for point in &mut edge.points {
                *point = Point::new(point.x + dx, point.y + dy);
            }
            if let Some(label) = edge.label_at {
                edge.label_at = Some(PlacedLabel::new(
                    Point::new(label.at.x + dx, label.at.y + dy),
                    label.width,
                    label.height,
                ));
            }
        }
        for group in &mut placed.groups {
            group.at = Point::new(group.at.x + dx, group.at.y + dy);
        }
        placed.width += dx;
        placed.height += dy;
    }
    // And grow to fit anything reaching past the far edges. The labels are
    // measured again rather than shifted, so this reads the boxes where they
    // finally landed.
    for group in &placed.groups {
        placed.width = placed.width.max(group.at.x + group.width);
        placed.height = placed.height.max(group.at.y + group.height);
    }
    for edge in &placed.edges {
        if let Some(label) = edge.label_at {
            placed.width = placed.width.max(label.at.x + label.width / 2.0);
            placed.height = placed.height.max(label.at.y + label.height / 2.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout::PlacedEdge;
    use super::*;
    use crate::flowchart::types::{EdgeStyle, Shape};

    #[test]
    fn a_frame_reaching_off_the_canvas_moves_the_whole_drawing_clear() {
        // A group's box reaches above and to the left of what it holds. Now that
        // `nest` places each group as a unit, a real diagram seldom takes this
        // arm — which is exactly why it is worth asking directly.
        let mut placed = Placed {
            width: 100.0,
            height: 100.0,
            nodes: vec![PlacedNode {
                id: "A".into(),
                label: "A".into(),
                shape: Shape::Rectangle,
                classes: Vec::new(),
                at: Point::new(0.0, 0.0),
                width: 40.0,
                height: 20.0,
            }],
            edges: Vec::new(),
            groups: vec![PlacedGroup {
                id: "g".into(),
                label: "g".into(),
                at: Point::new(-10.0, -20.0),
                width: 60.0,
                height: 60.0,
                depth: 0,
                holds: vec!["A".into()],
            }],
        };
        make_room(&mut placed);
        let group = placed.groups.first().expect("kept");
        assert!(group.at.x >= 0.0 && group.at.y >= 0.0, "{group:?}");
        let node = placed.nodes.first().expect("kept");
        assert!((node.at.x - 10.0).abs() < 1e-9, "{node:?}");
        assert!((node.at.y - 20.0).abs() < 1e-9, "{node:?}");
        // The canvas grows by what it moved, so nothing falls off the far side.
        assert!((placed.width - 110.0).abs() < 1e-9);
        assert!((placed.height - 120.0).abs() < 1e-9);
    }

    #[test]
    fn a_label_beside_its_wire_is_carried_along_and_then_fitted() {
        // A label now stands beside its run rather than on it, so it too can
        // reach off the near corner — and past the far one.
        let wire = |at: Point, label: Point| PlacedEdge {
            source: "A".into(),
            target: "B".into(),
            label: "x".into(),
            style: EdgeStyle::Solid,
            head_start: false,
            head_end: true,
            points: vec![at],
            label_at: Some(PlacedLabel::new(label, 20.0, 10.0)),
        };
        let mut placed = Placed {
            width: 100.0,
            height: 100.0,
            edges: vec![
                wire(Point::new(0.0, 0.0), Point::new(-5.0, 50.0)),
                wire(Point::new(50.0, 50.0), Point::new(95.0, 50.0)),
            ],
            ..Placed::default()
        };
        make_room(&mut placed);
        let first = placed.edges.first().expect("kept");
        let moved = first.label_at.expect("kept its label");
        assert!(moved.at.x - moved.width / 2.0 >= -1e-9, "{moved:?}");
        assert!(
            (first.points[0].x - 15.0).abs() < 1e-9,
            "the wire moved too"
        );
        // And the canvas grew for the one reaching past the right-hand edge.
        assert!(
            placed.width >= 95.0 + 15.0 + 10.0 - 1e-9,
            "{}",
            placed.width
        );
    }

    #[test]
    fn a_drawing_already_clear_of_the_corner_is_left_where_it_is() {
        let mut placed = Placed {
            width: 80.0,
            height: 60.0,
            ..Placed::default()
        };
        make_room(&mut placed);
        assert!((placed.width - 80.0).abs() < 1e-9);
        assert!((placed.height - 60.0).abs() < 1e-9);
    }
}
