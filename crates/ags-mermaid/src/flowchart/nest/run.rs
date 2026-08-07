//! Laying each container out, its children first.

use super::tree::{members, router, toward, Member, Tree};
use super::wire::{joined, Piece, Side};
use crate::flowchart::config::Config;
use crate::flowchart::types::Graph;
use crate::layout::{self, Point};

struct Laid {
    width: f64,
    height: f64,
    /// Where each of the drawing's nodes ended up.
    nodes: Vec<(usize, layout::PlacedNode)>,
    pieces: Vec<Piece>,
    /// Where a crossing edge met this container's side, as a fraction of it.
    ports: Vec<(usize, f64)>,
}

struct Wire {
    edge: usize,
    from: usize,
    to: usize,
    /// Which end of the edge is a port on this container's own boundary, when
    /// the edge is only passing through.
    side: Side,
    /// Whether the engine draws this wire against the way it is travelled.
    flip: bool,
}

/// Which of two members a wire descends from, before anything has a size.
///
/// A child has to place its port before its parent is laid out, but which *side*
/// of the child the parent's wire will arrive on is the parent's business, not
/// the child's — and getting it wrong is what makes a wire leave by the face
/// pointing away from where it is going and then travel back round.
///
/// The parent can answer it early because layering needs only the topology:
/// break the cycles, assign the layers, and ask which of the two ends came out
/// higher. Sizes move boxes about within a layer; they never change which layer
/// a box is in.
fn descending(count: usize, edges: &[layout::Edge]) -> Vec<bool> {
    let sizes = vec![layout::Node::new(0.0, 0.0); count];
    let acyclic = layout::break_cycles(count, edges);
    let layering = layout::assign_layers(&sizes, &acyclic.arcs, edges.len());
    let layer = |node: usize| layering.nodes.get(node).map_or(0, |found| found.layer);
    edges
        .iter()
        .map(|edge| layer(edge.from) < layer(edge.to))
        .collect()
}

/// What one container has to draw, worked out before anything has a size.
struct Plan {
    members: Vec<Member>,
    wires: Vec<Wire>,
    /// Each crossing edge and the index of the port standing in for it.
    ports: Vec<(usize, usize)>,
    /// Crossing edges reaching a node directly, for a container running across
    /// its parent's grain: the edge, the member, which end, and which side of
    /// this container the parent arrives at.
    aims: Vec<(usize, usize, Side, bool)>,
}

/// Which wires a container draws, and which of its edges only pass through it.
fn plan(
    graph: &Graph,
    tree: &Tree,
    container: Option<usize>,
    meets: &[(usize, bool)],
    athwart: bool,
) -> Plan {
    let members = members(graph, tree, container);
    let at_of = |member: Member| members.iter().position(|held| *held == member);
    let mut aims: Vec<(usize, usize, Side, bool)> = Vec::new();
    let mut wires: Vec<Wire> = Vec::new();
    let mut ports: Vec<(usize, usize)> = Vec::new();
    for (edge, found) in graph.edges.iter().enumerate() {
        let ends = (graph.index_of(&found.source), graph.index_of(&found.target));
        let (Some(source), Some(target)) = ends else {
            continue;
        };
        let routed_by = router(tree, graph, edge);
        if routed_by == container {
            let (Some(from), Some(to)) = (
                toward(tree, container, source).and_then(at_of),
                toward(tree, container, target).and_then(at_of),
            ) else {
                continue;
            };
            wires.push(Wire {
                edge,
                from,
                to,
                side: Side::Whole,
                flip: false,
            });
            continue;
        }
        // Only a container *strictly below* the one routing this edge carries a
        // port for it — that is, one the router encloses. Testing chain
        // membership alone would be true at the drawing for every edge, and
        // testing it downward would be true at every group between the router
        // and the endpoint's own group, which is how an edge wholly inside a
        // child got a spurious port in its parent and a second piece that ran
        // off across the page.
        let Some(here) = container else { continue };
        if !tree.over(here).contains(&routed_by) {
            continue;
        }
        let holds = |node: usize| tree.chain(node).contains(&Some(here));
        let (inside, side) = if holds(source) {
            (source, Side::Source)
        } else if holds(target) {
            (target, Side::Target)
        } else {
            // The edge passes this container by entirely.
            continue;
        };
        let Some(member) = toward(tree, container, inside).and_then(at_of) else {
            continue;
        };
        if athwart {
            if let Some(Member::Node(_)) = toward(tree, container, inside) {
                let above = meets
                    .iter()
                    .find(|(crossing, _)| *crossing == edge)
                    .map_or(side == Side::Target, |(_, above)| *above);
                aims.push((edge, member, side, above));
                continue;
            }
        }
        let port = members.len() + ports.len();
        ports.push((edge, port));
        // The port goes in an earlier layer than the node when the parent's wire
        // arrives at this container's top. Absent an answer — the drawing has no
        // parent — the natural reading holds: what enters comes from above.
        let from_above = meets
            .iter()
            .find(|(crossing, _)| *crossing == edge)
            .map_or(side == Side::Target, |(_, above)| *above);
        let (from, to) = if from_above {
            (port, member)
        } else {
            (member, port)
        };
        wires.push(Wire {
            edge,
            from,
            to,
            side,
            // The engine draws from the earlier layer down; the wire is
            // travelled from whichever end the edge itself starts at.
            flip: from_above != (side == Side::Target),
        });
    }
    Plan {
        members,
        wires,
        ports,
        aims,
    }
}

/// Where along this container's side something sits, as the parent will see it.
///
/// The parent places a group as a box one header taller than the layout inside
/// it, and everything inside drops by that header to make room for the caption.
/// A fraction taken of the bare layout is therefore a fraction of the wrong
/// thing: the parent multiplies it by the taller box and lands somewhere else,
/// which put a step at every boundary a wire crossed. Only visible when the
/// parent runs across the page, because that is the axis the header adds to.
fn fraction(
    box_: &layout::PlacedNode,
    placed: &layout::Placed,
    outer: layout::Direction,
    cfg: &Config,
) -> f64 {
    let along = if outer.across() {
        let header = cfg.group_header;
        (box_.at.y + header + box_.height / 2.0) / (placed.height + header).max(1.0)
    } else {
        (box_.at.x + box_.width / 2.0) / placed.width.max(1.0)
    };
    along.clamp(0.0, 1.0)
}

/// Where each member ended up, and the pieces its children already drew.
///
/// A group's contents move with the box it was placed as, and drop below the
/// band its caption sits in.
fn gather(
    members: &[Member],
    children: &[Option<Laid>],
    placed: &layout::Placed,
    cfg: &Config,
) -> (Vec<(usize, layout::PlacedNode)>, Vec<Piece>) {
    let mut nodes: Vec<(usize, layout::PlacedNode)> = Vec::new();
    let mut pieces: Vec<Piece> = Vec::new();
    for (at, member) in members.iter().enumerate() {
        let Some(box_) = placed.nodes.get(at) else {
            continue;
        };
        match *member {
            Member::Node(node) => nodes.push((node, *box_)),
            Member::Group(_) => {
                let Some(Some(child)) = children.get(at) else {
                    continue;
                };
                let shift = Point::new(box_.at.x, box_.at.y + cfg.group_header);
                let moved = |at: &Point| Point::new(at.x + shift.x, at.y + shift.y);
                for (node, inner) in &child.nodes {
                    nodes.push((
                        *node,
                        layout::PlacedNode {
                            at: moved(&inner.at),
                            ..*inner
                        },
                    ));
                }
                for piece in &child.pieces {
                    pieces.push(Piece {
                        edge: piece.edge,
                        side: piece.side,
                        depth: piece.depth,
                        points: piece.points.iter().map(moved).collect(),
                    });
                }
            }
        }
    }
    (nodes, pieces)
}

/// The boundary fractions and connecting pieces for edges aimed at a node.
///
/// An aimed edge has no port: the parent attaches at the fraction of this
/// container where the *node* sits, and the stretch from the boundary to the
/// node's own face — through the header band, where nothing is drawn — is this
/// container's whole piece of it.
fn aim(
    aims: &[(usize, usize, Side, bool)],
    placed: &layout::Placed,
    outer: layout::Direction,
    pieces: &mut Vec<Piece>,
    depth: usize,
    cfg: &Config,
) -> Vec<(usize, f64)> {
    let mut out: Vec<(usize, f64)> = Vec::new();
    for (edge, member, side, above) in aims {
        let Some(box_) = placed.nodes.get(*member) else {
            continue;
        };
        let along = fraction(box_, placed, outer, cfg);
        out.push((*edge, along));
        // Straight in from the side the parent said it would arrive at. Reading
        // the nearest side off the node's own position instead sends the wire
        // past the node and back up into its far face.
        let run = if outer.across() {
            let mid = box_.at.y + box_.height / 2.0;
            let (from, to) = if *above {
                (0.0, box_.at.x)
            } else {
                (placed.width, box_.at.x + box_.width)
            };
            (Point::new(from, mid), Point::new(to, mid))
        } else {
            let mid = box_.at.x + box_.width / 2.0;
            let (from, to) = if *above {
                (0.0, box_.at.y)
            } else {
                (placed.height, box_.at.y + box_.height)
            };
            (Point::new(mid, from), Point::new(mid, to))
        };
        let mut points = vec![run.0, run.1];
        if *side == Side::Source {
            points.reverse();
        }
        pieces.push(Piece {
            edge: *edge,
            side: *side,
            depth,
            points,
        });
    }
    out
}

/// Each member's size, laying every child group out first.
///
/// A child is told, for each wire touching it, which side of itself the parent
/// will arrive at — which is the one thing it cannot work out alone.
#[expect(
    clippy::too_many_arguments,
    reason = "each is a distinct input the recursion needs; bundling them would only move the list"
)]
fn grown(
    graph: &Graph,
    cfg: &Config,
    tree: &Tree,
    members: &[Member],
    wires: &[Wire],
    falls: &[bool],
    ports: &[(usize, usize)],
    depth: usize,
    direction: crate::flowchart::types::Direction,
) -> (Vec<layout::Node>, Vec<Option<Laid>>) {
    let mut sizes: Vec<layout::Node> = Vec::with_capacity(members.len() + ports.len());
    let mut children: Vec<Option<Laid>> = Vec::with_capacity(members.len());
    for (at, member) in members.iter().enumerate() {
        match *member {
            Member::Node(node) => {
                let found = graph.nodes.get(node);
                sizes.push(found.map_or(layout::Node::new(0.0, 0.0), |node| {
                    crate::flowchart::layout::measure(&node.label, node.shape, cfg)
                }));
                children.push(None);
            }
            Member::Group(group) => {
                let inner: Vec<(usize, bool)> = wires
                    .iter()
                    .enumerate()
                    .filter_map(|(index, wire)| {
                        let falling = falls.get(index).copied().unwrap_or(true);
                        // Entering this member from above, or leaving it
                        // downward — either way, which side of it is met.
                        if wire.to == at {
                            Some((wire.edge, falling))
                        } else if wire.from == at {
                            Some((wire.edge, !falling))
                        } else {
                            None
                        }
                    })
                    .collect();
                let laid = lay(
                    graph,
                    cfg,
                    tree,
                    Some(group),
                    depth + 1,
                    &inner,
                    direction.as_layout(),
                );
                // The frame's caption sits in a band above everything inside it.
                sizes.push(layout::Node::new(
                    laid.width,
                    laid.height + cfg.group_header,
                ));
                children.push(Some(laid));
            }
        }
    }
    // A port is a place on the boundary, not a box.
    for _ in ports {
        sizes.push(layout::Node::new(0.0, 0.0));
    }
    (sizes, children)
}

/// Lay one container out, its children first.
///
/// `meets` says, for each edge crossing this container's own boundary, whether
/// the parent's wire arrives at the top of it. That decides which layer this
/// container puts the port in, and so which side of the box it comes out on.
fn lay(
    graph: &Graph,
    cfg: &Config,
    tree: &Tree,
    container: Option<usize>,
    depth: usize,
    meets: &[(usize, bool)],
    outer: layout::Direction,
) -> Laid {
    let direction = container
        .and_then(|group| graph.groups.get(group))
        .and_then(|group| group.direction)
        .unwrap_or(graph.direction);
    // A group with its own `direction` may run across its parent's grain. Then a
    // port is the wrong tool: it is a node in this container's layered pass, so
    // it lands on a face at right angles to the one the parent can attach to,
    // and the wire has to go round the outside of the frame to reach it. What
    // the parent wants in that case is to aim straight at the node itself.
    let athwart = direction.as_layout().across() != outer.across();
    let Plan {
        members,
        wires,
        ports,
        aims,
    } = plan(graph, tree, container, meets, athwart);

    // Which way each wire runs, so a child can be told where its port must come
    // out. Asked before any child is laid out, and answerable because layering
    // needs only the topology — see `descending`.
    let falls = descending(
        members.len() + ports.len(),
        &wires
            .iter()
            .map(|wire| layout::Edge::new(wire.from, wire.to))
            .collect::<Vec<layout::Edge>>(),
    );

    let (sizes, children) = grown(
        graph, cfg, tree, &members, &wires, &falls, &ports, depth, direction,
    );

    let pins: Vec<layout::Port> = children
        .iter()
        .enumerate()
        .filter_map(|(at, child)| child.as_ref().map(|laid| (at, laid)))
        .flat_map(|(at, laid)| {
            wires
                .iter()
                .enumerate()
                .filter_map(move |(index, wire)| {
                    let meets = (wire.from == at)
                        .then_some(true)
                        .or((wire.to == at).then_some(false));
                    let along = laid
                        .ports
                        .iter()
                        .find(|(edge, _)| *edge == wire.edge)
                        .map(|(_, fraction)| *fraction)?;
                    Some(layout::Port {
                        edge: index,
                        source: meets?,
                        at: along,
                    })
                })
                .collect::<Vec<layout::Port>>()
        })
        .collect();

    let placed = layout::layout(&layout::Graph {
        nodes: sizes,
        edges: wires
            .iter()
            .map(|wire| layout::Edge::new(wire.from, wire.to))
            .collect(),
        direction: direction.as_layout(),
        spacing: layout::Spacing::default(),
        ports: pins,
    });

    let (nodes, mut pieces) = gather(&members, &children, &placed, cfg);
    for (at, wire) in wires.iter().enumerate() {
        let Some(route) = placed.edges.get(at) else {
            continue;
        };
        let mut points = route.points.clone();
        if wire.flip {
            points.reverse();
        }
        pieces.push(Piece {
            edge: wire.edge,
            side: wire.side,
            depth,
            points,
        });
    }

    let aimed = aim(&aims, &placed, outer, &mut pieces, depth, cfg);

    let out_ports = ports
        .iter()
        .filter_map(|(edge, at)| {
            let box_ = placed.nodes.get(*at)?;
            // Along the axis the *parent* pins on. A group that overrides the
            // direction runs across its parent's grain, so its port sits on a
            // face the parent's engine cannot attach to at all; giving the
            // fraction in the child's own axis put the parent's wire on the
            // opposite side of the box from the port it was meant to meet.
            Some((*edge, fraction(box_, &placed, outer, cfg)))
        })
        .chain(aimed)
        .collect();

    Laid {
        width: placed.width,
        height: placed.height,
        nodes,
        pieces,
        ports: out_ports,
    }
}

pub fn layout(graph: &Graph, cfg: &Config) -> layout::Placed {
    let tree = Tree::of(graph);
    let laid = lay(graph, cfg, &tree, None, 0, &[], graph.direction.as_layout());

    let mut nodes = vec![
        layout::PlacedNode {
            at: Point::new(0.0, 0.0),
            width: 0.0,
            height: 0.0,
        };
        graph.nodes.len()
    ];
    for (node, box_) in &laid.nodes {
        if let Some(slot) = nodes.get_mut(*node) {
            *slot = *box_;
        }
    }
    let edges = (0..graph.edges.len())
        .map(|edge| layout::PlacedEdge {
            points: joined(
                laid.pieces
                    .iter()
                    .filter(|piece| piece.edge == edge)
                    .collect(),
                layout::Spacing::default().edge,
            ),
        })
        .collect();
    layout::Placed {
        width: laid.width,
        height: laid.height,
        nodes,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowchart::parse;

    fn placed(source: &str) -> layout::Placed {
        layout(&parse(source), &Config::default())
    }

    /// Where a node ended up, by id.
    fn box_of(graph: &Graph, out: &layout::Placed, id: &str) -> layout::PlacedNode {
        let at = graph.index_of(id).expect("declared");
        *out.nodes.get(at).expect("placed")
    }

    const NESTED: &str = "graph TD\n  subgraph outer\n    subgraph inner\n      A --> B\n    end\n  end\n  C --> A\n";

    #[test]
    fn a_frame_holds_exactly_its_members_and_nothing_between_them() {
        // The defect this whole module exists for: `Fix & Retry` is in the group
        // and fed from below it, so a flat layering drops it to the bottom and
        // the frame drawn round the result spans the strangers in between.
        let source =
            "graph TD\n  A --> B\n  subgraph g\n    B --> R\n  end\n  B --> E\n  E --> R\n";
        let graph = parse(source);
        let out = layout(&graph, &Config::default());
        let (b, r, e) = (
            box_of(&graph, &out, "B"),
            box_of(&graph, &out, "R"),
            box_of(&graph, &out, "E"),
        );
        // The frame is drawn round wherever the members landed, so the test is
        // whether the stranger fell inside that rectangle — on both axes, since
        // E is free to sit beside the group at the same height.
        let frame = (
            b.at.x.min(r.at.x),
            b.at.y.min(r.at.y),
            (b.at.x + b.width).max(r.at.x + r.width),
            (b.at.y + b.height).max(r.at.y + r.height),
        );
        let inside = e.at.x >= frame.0
            && e.at.y >= frame.1
            && e.at.x + e.width <= frame.2
            && e.at.y + e.height <= frame.3;
        assert!(
            !inside,
            "E is not in the group, so the frame must not span it"
        );
    }

    #[test]
    fn a_group_that_overrides_the_direction_runs_across_its_parent() {
        let graph =
            parse("graph TD\n  subgraph g\n    direction LR\n    A --> B\n  end\n  C --> A\n");
        let out = layout(&graph, &Config::default());
        let (a, b) = (box_of(&graph, &out, "A"), box_of(&graph, &out, "B"));
        assert!(b.at.x > a.at.x, "LR inside, so B is to the right of A");
        assert!((a.at.y - b.at.y).abs() < 1.0, "and on the same row");
        // The parent still reads downwards.
        assert!(a.at.y > box_of(&graph, &out, "C").at.y);
    }

    #[test]
    fn a_group_across_the_grain_is_aimed_at_from_either_side() {
        // The mirror of the case above: a page running across, holding a group
        // that runs down. One wire enters the group and one leaves it, so both
        // sides of the aim are drawn.
        let graph = parse(
            "graph LR\n  subgraph g\n    direction TB\n    A --> B\n  end\n  C --> A\n  B --> D\n",
        );
        let out = layout(&graph, &Config::default());
        let (a, b) = (box_of(&graph, &out, "A"), box_of(&graph, &out, "B"));
        assert!(b.at.y > a.at.y, "TB inside, so B is below A");
        // The page still reads across it.
        assert!(box_of(&graph, &out, "C").at.x < a.at.x);
        assert!(box_of(&graph, &out, "D").at.x > b.at.x);
        for wire in &out.edges {
            for pair in wire.points.windows(2) {
                let square =
                    (pair[0].x - pair[1].x).abs() < 0.5 || (pair[0].y - pair[1].y).abs() < 0.5;
                assert!(square, "every run stays axis-aligned: {:?}", wire.points);
            }
        }
    }

    #[test]
    fn a_wire_crossing_a_boundary_arrives_at_the_node_and_not_the_frame() {
        let graph = parse("graph TD\n  C --> A\n  subgraph g\n    A --> B\n  end\n");
        let out = layout(&graph, &Config::default());
        let a = box_of(&graph, &out, "A");
        let wire = out.edges.first().expect("routed");
        let end = wire.points.last().copied().expect("has points");
        assert!(
            end.y >= a.at.y - 1.0 && end.y <= a.at.y + a.height + 1.0,
            "the wire reaches A's own box, not the frame round it: {end:?}"
        );
    }

    #[test]
    fn a_graph_with_no_groups_is_laid_out_exactly_as_before() {
        let out = placed("graph TD\n  A --> B\n  B --> C\n");
        assert_eq!(out.nodes.len(), 3);
        assert!(out.nodes[1].at.y > out.nodes[0].at.y);
        assert!(out.nodes[2].at.y > out.nodes[1].at.y);
    }

    #[test]
    fn the_same_source_nests_the_same_way_twice() {
        assert_eq!(placed(NESTED).nodes, placed(NESTED).nodes);
    }

    #[test]
    fn which_way_a_wire_falls_is_known_before_anything_has_a_size() {
        // Layering needs the topology only, which is what lets a parent tell a
        // child which side its port has to come out on.
        let edges = vec![layout::Edge::new(0, 1), layout::Edge::new(2, 1)];
        let falls = descending(3, &edges);
        assert_eq!(falls, vec![true, true]);
        // A back edge is turned by the cycle break, so it does not descend.
        let cyclic = vec![
            layout::Edge::new(0, 1),
            layout::Edge::new(1, 2),
            layout::Edge::new(2, 0),
        ];
        let broken = descending(3, &cyclic);
        assert_eq!(broken.iter().filter(|down| !**down).count(), 1);
    }
}
