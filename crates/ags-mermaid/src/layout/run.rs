//! The five passes, in order.
//!
//! Everything is laid out downwards. A direction that runs across the page
//! turns each box on its side first and the whole drawing back at the end, and
//! a direction that runs backwards is flipped about the drawing's own extent —
//! so there is one implementation of the layout rather than four.

use super::cycles::{break_cycles, Acyclic};
use super::layers::{assign_layers, Layering};
use super::order::order_layers;
use super::place::{depth, extent, layer_tops, place};
use super::route::{gap_lanes, route, route_loops, Placement, PORT_SPACING};
use super::table::{as_f64, Table};
use super::types::{Direction, Graph, Node, Placed, PlacedEdge, PlacedNode, Point};

/// A box as the engine sees it: turned on its side when the layout runs across
/// the page.
const fn to_layout(direction: Direction, node: Node) -> Node {
    if direction.across() {
        Node::new(node.height, node.width)
    } else {
        node
    }
}

/// A laid-out point, back in the caller's coordinates.
///
/// `depth` is how far the drawing runs along the axis a backward direction
/// flips about.
fn from_layout(direction: Direction, at: Point, depth: f64) -> Point {
    let flipped = if direction.reversed() {
        Point::new(at.x, depth - at.y)
    } else {
        at
    };
    if direction.across() {
        Point::new(flipped.y, flipped.x)
    } else {
        flipped
    }
}

/// Widen each box until its own side has room for the edges meeting it.
///
/// `route::spread` shares a node's side between the edges attaching to it, and a
/// side too short for them puts two wires closer than `spacing.edge` — the very
/// clearance the engine keeps everywhere else. It cannot solve that by itself:
/// a port has to stay on the box, so the box is what has to give.
///
/// The width is in layout space, so a drawing that runs across the page grows
/// the side that faces its neighbours rather than always the horizontal one.
/// The box comes back at the new size, because a box drawn narrower than it was
/// laid out puts its own ports outside itself.
///
/// Counted on the arcs the cycle break produced, never on the edges the author
/// wrote. A back edge is turned round before anything is routed, so a node the
/// author drew two edges *into* can have four leaving it — which is what
/// `develop` had in a branching diagram, on a side sized for three. The fourth
/// port fell off the box, the pass that pulls a port back on had only the room
/// between two columns to put it in, and it landed the far side of its own
/// neighbour: the two wires swapped over and crossed.
fn fit_ports(sizes: &mut [Node], acyclic: &Acyclic) {
    let mut leaving = vec![0usize; sizes.len()];
    let mut arriving = vec![0usize; sizes.len()];
    // Self-loops never reach the arcs — they are routed round the outside and
    // take no port — so there is nothing here to skip.
    for arc in &acyclic.arcs {
        if let Some(slot) = leaving.get_mut(arc.from) {
            *slot += 1;
        }
        if let Some(slot) = arriving.get_mut(arc.to) {
            *slot += 1;
        }
    }
    for (at, size) in sizes.iter_mut().enumerate() {
        let most = leaving
            .get(at)
            .copied()
            .unwrap_or(0)
            .max(arriving.get(at).copied().unwrap_or(0));
        // One port sits at the middle of the side and needs no room made for
        // it; nor does a box nothing attaches to.
        if most < 2 {
            continue;
        }
        // `spread` divides the side into one more part than it has ports, so
        // that many parts is what it takes to keep them apart — and the part it
        // uses is `PORT_SPACING`, never `spacing.edge`. Sizing by the smaller of
        // the two left every crowded face 2px per gap short of what the pass
        // that fills it was going to ask for. The last port then landed outside
        // the box, and the pass that brings a port back inside had only the room
        // between two columns to put it in.
        let Some(parts) = most.checked_add(1) else {
            continue;
        };
        size.width = size.width.max(as_f64(parts) * PORT_SPACING);
    }
}

/// Each layer's band, with every gap sized for the lanes that will cross it.
///
/// Computed twice on purpose: once at the bare layer spacing to ask how many
/// lanes each gap carries, and once at the answer. The question is safe to ask of
/// provisional heights because a port is chosen from the node's column and its
/// width and never from a layer's height, so the first pass reads exactly what
/// the second will route — see [`gap_lanes`].
fn heights(
    layering: &Layering,
    layers: &[Vec<usize>],
    centres: &Table<f64>,
    turned: &[bool],
    graph: &Graph,
) -> Vec<(f64, f64)> {
    let bare = layer_tops(layering, layers, &graph.spacing, &[]);
    let lanes = gap_lanes(
        &Placement {
            layering,
            centres,
            tops: &bare,
            spacing: &graph.spacing,
        },
        graph.edges.len(),
        &graph.ports,
        turned,
    );
    layer_tops(layering, layers, &graph.spacing, &lanes)
}

/// Lay out a graph.
///
/// Nodes and edges come back in the caller's order. An edge naming a box that
/// does not exist comes back with no points rather than stopping the drawing.
pub fn layout(graph: &Graph) -> Placed {
    if graph.nodes.is_empty() {
        return Placed::default();
    }
    let mut sizes: Vec<Node> = graph
        .nodes
        .iter()
        .map(|node| to_layout(graph.direction, *node))
        .collect();
    // Sized after the break, so a box is widened for the wires that will
    // actually meet it rather than the ones the author wrote.
    let acyclic = break_cycles(sizes.len(), &graph.edges);
    fit_ports(&mut sizes, &acyclic);

    let layering = assign_layers(&sizes, &acyclic.arcs, graph.edges.len());
    let layers = order_layers(&layering);
    let centres = place(&layering, &layers, &graph.spacing);

    let mut turned = vec![false; graph.edges.len()];
    for arc in &acyclic.arcs {
        if let Some(slot) = turned.get_mut(arc.source) {
            *slot = arc.reversed;
        }
    }

    let tops = heights(&layering, &layers, &centres, &turned, graph);

    let placement = Placement {
        layering: &layering,
        centres: &centres,
        tops: &tops,
        spacing: &graph.spacing,
    };
    let mut routes = route(
        &placement,
        &acyclic.loops,
        &turned,
        graph.edges.len(),
        &graph.ports,
    );
    let pinned: Vec<(usize, usize)> = acyclic
        .loops
        .iter()
        .filter_map(|edge| graph.edges.get(*edge).map(|found| (*edge, found.from)))
        .collect();
    route_loops(&placement, &pinned, &mut routes);

    let across = extent(&layering, &centres);
    let down = depth(&tops);
    let pad = graph.spacing.padding;

    let nodes = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(at, _)| {
            let size = sizes.get(at).copied().unwrap_or(Node::new(0.0, 0.0));
            let half = Point::new(
                centres.get(at) - size.width / 2.0,
                top_of(&layering, &tops, at),
            );
            // A backward direction flips about the drawing, so a box's corner
            // becomes its far corner; the size is what puts it back.
            let corner = from_layout(
                graph.direction,
                Point::new(
                    half.x,
                    if graph.direction.reversed() {
                        half.y + size.height
                    } else {
                        half.y
                    },
                ),
                down,
            );
            // Back out of layout space, so a widened side is the one the
            // caller sees widened.
            let drawn = to_layout(graph.direction, size);
            PlacedNode {
                at: Point::new(corner.x + pad, corner.y + pad),
                width: drawn.width,
                height: drawn.height,
            }
        })
        .collect();

    let edges = routes
        .into_iter()
        .map(|points| PlacedEdge {
            points: points
                .into_iter()
                .map(|at| {
                    let moved = from_layout(graph.direction, at, down);
                    Point::new(moved.x + pad, moved.y + pad)
                })
                .collect(),
        })
        .collect();

    let (width, height) = if graph.direction.across() {
        (down, across)
    } else {
        (across, down)
    };
    Placed {
        width: width + pad * 2.0,
        height: height + pad * 2.0,
        nodes,
        edges,
    }
}

/// Where a node's own box starts within its layer's band.
fn top_of(layering: &super::layers::Layering, tops: &[(f64, f64)], node: usize) -> f64 {
    let layer = layering.nodes.get(node).map_or(0, |found| found.layer);
    let (top, band) = tops.get(layer).copied().unwrap_or((0.0, 0.0));
    let own = layering
        .nodes
        .get(node)
        .map_or(0.0, |found| found.size.height);
    top + (band - own) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_grows_until_its_side_holds_the_edges_meeting_it() {
        // `route::spread` shares a side between the edges on it, so a side too
        // short puts two wires closer than `spacing.edge`.
        let mut input = graph(4, &[(0, 1), (0, 2), (0, 3)], Direction::Down);
        input.nodes = vec![Node::new(10.0, 20.0); 4];
        let out = layout(&input);
        let spacing = Spacing::default();
        // Three edges leave node 0, so its side is divided into four parts —
        // of the spacing `spread` actually uses, which is the wider of the two.
        assert!(
            out.nodes[0].width >= 4.0 * PORT_SPACING - 1e-9,
            "{:?}",
            out.nodes[0]
        );
        assert!(PORT_SPACING > spacing.edge, "the two have converged");
        // A box with one edge is left at the size the caller asked for: a lone
        // port sits at the middle of the side and shares it with nothing.
        assert!((out.nodes[1].width - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_box_is_sized_for_the_wires_that_meet_it_after_the_cycle_break() {
        // Two edges leave node 0 and one comes back to it. The one coming back
        // is a back edge, so the cycle break turns it round and node 0 has three
        // wires *leaving* it, not two — and a side sized for two puts the third
        // port off the box. Counting what the author wrote is one short.
        let mut input = graph(3, &[(0, 1), (0, 2), (2, 0)], Direction::Down);
        input.nodes = vec![Node::new(10.0, 20.0); 3];
        let out = layout(&input);
        assert!(
            out.nodes[0].width >= 4.0 * PORT_SPACING - 1e-9,
            "sized for two, not three: {:?}",
            out.nodes[0]
        );
    }

    #[test]
    fn a_self_loop_takes_no_room_on_the_side() {
        // It is routed round the outside, so it is not a port to make space for.
        let mut input = graph(1, &[(0, 0)], Direction::Down);
        input.nodes = vec![Node::new(10.0, 20.0)];
        let out = layout(&input);
        assert!((out.nodes[0].width - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_drawing_that_runs_across_the_page_grows_the_side_that_faces_its_neighbours() {
        let mut input = graph(4, &[(0, 1), (0, 2), (0, 3)], Direction::Right);
        input.nodes = vec![Node::new(10.0, 20.0); 4];
        let out = layout(&input);
        // Laid out across, so the side shared between edges is the height.
        assert!(out.nodes[0].height >= 4.0 * Spacing::default().edge - 1e-9);
    }
    use crate::layout::types::{Edge, Spacing};

    fn graph(n: usize, pairs: &[(usize, usize)], direction: Direction) -> Graph {
        Graph {
            ports: Vec::new(),
            nodes: vec![Node::new(100.0, 40.0); n],
            edges: pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect(),
            direction,
            spacing: Spacing::default(),
        }
    }

    fn down(n: usize, pairs: &[(usize, usize)]) -> Placed {
        layout(&graph(n, pairs, Direction::Down))
    }

    #[test]
    fn a_chain_runs_down_the_page_in_order() {
        let out = down(3, &[(0, 1), (1, 2)]);
        assert_eq!(out.nodes.len(), 3);
        assert!(out.nodes[1].at.y > out.nodes[0].at.y);
        assert!(out.nodes[2].at.y > out.nodes[1].at.y);
        // One column, so every box shares an x.
        assert!((out.nodes[1].at.x - out.nodes[0].at.x).abs() < 1e-9);
    }

    #[test]
    fn every_box_keeps_the_size_it_was_given() {
        let mut input = graph(2, &[(0, 1)], Direction::Down);
        input.nodes = vec![Node::new(120.0, 30.0), Node::new(60.0, 80.0)];
        let out = layout(&input);
        assert!((out.nodes[0].width - 120.0).abs() < 1e-9);
        assert!((out.nodes[0].height - 30.0).abs() < 1e-9);
        assert!((out.nodes[1].width - 60.0).abs() < 1e-9);
        assert!((out.nodes[1].height - 80.0).abs() < 1e-9);
    }

    #[test]
    fn the_drawing_is_padded_on_every_side() {
        let out = down(1, &[]);
        let pad = Spacing::default().padding;
        assert!((out.nodes[0].at.x - pad).abs() < 1e-9);
        assert!((out.nodes[0].at.y - pad).abs() < 1e-9);
        assert!((out.width - (100.0 + pad * 2.0)).abs() < 1e-9);
        assert!((out.height - (40.0 + pad * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn no_two_boxes_overlap() {
        let out = down(8, &[(0, 1), (0, 2), (0, 3), (1, 4), (2, 5), (3, 6), (4, 7)]);
        for (at, a) in out.nodes.iter().enumerate() {
            for b in out.nodes.iter().skip(at + 1) {
                let apart = a.at.x + a.width <= b.at.x + 1e-6
                    || b.at.x + b.width <= a.at.x + 1e-6
                    || a.at.y + a.height <= b.at.y + 1e-6
                    || b.at.y + b.height <= a.at.y + 1e-6;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn every_edge_runs_the_way_the_layout_does() {
        let out = down(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        for (at, edge) in out.edges.iter().enumerate() {
            let (Some(first), Some(last)) = (edge.points.first(), edge.points.last()) else {
                continue;
            };
            assert!(last.y >= first.y - 1e-6, "edge {at} runs back up");
        }
    }

    #[test]
    fn everything_drawn_is_inside_the_drawing() {
        // The engine has no nesting, so the box every node has to be inside is
        // the canvas — and a node reaching past it is cut rather than shrunk,
        // which is the one failure a reader cannot work around.
        for out in [
            down(8, &[(0, 1), (0, 2), (0, 3), (1, 4), (2, 5), (3, 6), (4, 7)]),
            down(4, &[(0, 1), (1, 2), (2, 0), (0, 3)]),
            layout(&graph(
                5,
                &[(0, 1), (1, 2), (0, 3), (3, 4)],
                Direction::Right,
            )),
            layout(&graph(5, &[(0, 1), (1, 2), (0, 3), (3, 4)], Direction::Up)),
            layout(&graph(
                5,
                &[(0, 1), (1, 2), (0, 3), (3, 4)],
                Direction::Left,
            )),
        ] {
            for node in &out.nodes {
                assert!(node.at.x >= -1e-6, "{node:?} reaches off the left");
                assert!(node.at.y >= -1e-6, "{node:?} reaches off the top");
                assert!(
                    node.at.x + node.width <= out.width + 1e-6,
                    "{node:?} reaches past a canvas {} wide",
                    out.width
                );
                assert!(
                    node.at.y + node.height <= out.height + 1e-6,
                    "{node:?} reaches past a canvas {} tall",
                    out.height
                );
            }
            for edge in &out.edges {
                for point in &edge.points {
                    assert!(point.x >= -1e-6 && point.x <= out.width + 1e-6, "{point:?}");
                    assert!(
                        point.y >= -1e-6 && point.y <= out.height + 1e-6,
                        "{point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_edge_runs_the_way_the_layout_does_once_the_cycles_are_back() {
        // A cycle of three with a tail. Breaking the cycle turned one edge to
        // get the layers assigned; restoring it has to give the caller back the
        // edge they wrote, so exactly that one runs against the layout and
        // every other one runs with it.
        let out = down(4, &[(0, 1), (1, 2), (2, 0), (1, 3)]);
        let ends = |at: usize| {
            let edge = out.edges.get(at).cloned().unwrap_or_default();
            let first = edge.points.first().copied().unwrap_or_default();
            let last = edge.points.last().copied().unwrap_or_default();
            (first, last)
        };
        for forward in [0, 1, 3] {
            let (first, last) = ends(forward);
            assert!(last.y >= first.y - 1e-6, "edge {forward} runs back up");
        }
        // And the one that closes the cycle runs the other way, because that is
        // what the caller wrote and what a cycle means.
        let (first, last) = ends(2);
        assert!(last.y < first.y, "the edge closing the cycle runs forward");
    }

    #[test]
    fn a_turned_edge_still_starts_where_the_caller_said() {
        let out = down(3, &[(0, 1), (1, 2), (2, 0)]);
        let back = out.edges.get(2).cloned().unwrap_or_default();
        let start = back.points.first().copied().unwrap_or_default();
        let two = out.nodes.get(2).copied().unwrap_or_default();
        assert!(
            start.y >= two.at.y - 1e-6 && start.y <= two.at.y + two.height + 1e-6,
            "starts at {start:?}, box 2 is {two:?}"
        );
    }

    #[test]
    fn a_backward_direction_is_the_same_drawing_flipped() {
        let pairs = [(0usize, 1usize), (0, 2), (1, 3), (2, 3)];
        let downward = layout(&graph(4, &pairs, Direction::Down));
        let upward = layout(&graph(4, &pairs, Direction::Up));
        let rightward = layout(&graph(4, &pairs, Direction::Right));
        let leftward = layout(&graph(4, &pairs, Direction::Left));
        assert!((downward.width - upward.width).abs() < 1e-9);
        assert!((downward.height - upward.height).abs() < 1e-9);
        assert!((rightward.width - leftward.width).abs() < 1e-9);
        assert!((rightward.height - leftward.height).abs() < 1e-9);
    }

    #[test]
    fn a_layout_across_the_page_is_the_downward_one_transposed() {
        // Only with square boxes. Turning a wide box on its side changes how
        // much room a layer needs, so the two drawings genuinely differ — the
        // transposition is of the layout, not of the finished picture.
        let square = |direction| Graph {
            ports: Vec::new(),
            nodes: vec![Node::new(50.0, 50.0); 4],
            edges: [(0usize, 1usize), (0, 2), (1, 3), (2, 3)]
                .iter()
                .map(|(a, b)| Edge::new(*a, *b))
                .collect(),
            direction,
            spacing: Spacing::default(),
        };
        let downward = layout(&square(Direction::Down));
        let rightward = layout(&square(Direction::Right));
        assert!((downward.width - rightward.height).abs() < 1e-9);
        assert!((downward.height - rightward.width).abs() < 1e-9);
        for (a, b) in downward.nodes.iter().zip(&rightward.nodes) {
            assert!((a.at.x - b.at.y).abs() < 1e-9, "{a:?} against {b:?}");
            assert!((a.at.y - b.at.x).abs() < 1e-9, "{a:?} against {b:?}");
        }
    }

    #[test]
    fn a_downward_layout_reversed_puts_the_first_box_at_the_bottom() {
        let downward = down(2, &[(0, 1)]);
        let upward = layout(&graph(2, &[(0, 1)], Direction::Up));
        assert!(downward.nodes[0].at.y < downward.nodes[1].at.y);
        assert!(upward.nodes[0].at.y > upward.nodes[1].at.y);
    }

    #[test]
    fn a_rightward_layout_runs_across_the_page() {
        let out = layout(&graph(3, &[(0, 1), (1, 2)], Direction::Right));
        assert!(out.nodes[1].at.x > out.nodes[0].at.x);
        assert!(out.nodes[2].at.x > out.nodes[1].at.x);
        assert!((out.nodes[1].at.y - out.nodes[0].at.y).abs() < 1e-9);
        // The boxes keep their own shape however the layout runs.
        assert!((out.nodes[0].width - 100.0).abs() < 1e-9);
        assert!((out.nodes[0].height - 40.0).abs() < 1e-9);
    }

    #[test]
    fn an_edge_naming_a_box_that_does_not_exist_draws_nothing() {
        let out = down(2, &[(0, 1), (0, 9)]);
        assert_eq!(out.edges.len(), 2);
        assert!(!out.edges[0].points.is_empty());
        assert!(out.edges[1].points.is_empty());
    }

    #[test]
    fn a_self_loop_is_drawn_beside_its_own_box() {
        let out = down(2, &[(0, 0), (0, 1)]);
        let looped = out.edges.first().cloned().unwrap_or_default();
        assert_eq!(looped.points.len(), 4);
        let box_of = out.nodes.first().copied().unwrap_or_default();
        for point in &looped.points {
            assert!(point.x >= box_of.at.x + box_of.width - 1e-6, "{point:?}");
        }
    }

    #[test]
    fn the_same_graph_lays_out_the_same_way_twice() {
        let input = graph(
            6,
            &[(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5)],
            Direction::Down,
        );
        assert_eq!(layout(&input), layout(&input));
    }

    #[test]
    fn a_graph_of_nothing_lays_out_to_nothing() {
        assert_eq!(layout(&Graph::default()), Placed::default());
    }

    #[test]
    fn a_graph_of_islands_puts_them_side_by_side() {
        let out = down(3, &[]);
        assert_eq!(out.nodes.len(), 3);
        let same_row = out
            .nodes
            .windows(2)
            .all(|pair| (pair[0].at.y - pair[1].at.y).abs() < 1e-9);
        assert!(same_row);
    }
}
