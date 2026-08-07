//! Where each node sits along its layer — Brandes and Köpf's method.
//!
//! The property worth having is that a long edge comes out *straight*: an edge
//! crossing four layers is four dummies, and placing each on its own arrives as
//! a staircase. Brandes–Köpf aligns each node with one neighbour above it,
//! compacts the resulting blocks as rigid units, and does that four times — from
//! the top and the bottom, biased left and right — then averages. Averaging is
//! what makes it symmetric: each pass pulls toward the corner it started from,
//! and the four cancel out.
//!
//! Two nodes may only align if the segment joining them crosses no *inner*
//! segment — one running between two dummies. That is the type-1 conflict of the
//! paper, and honouring it is what stops a long edge being bent aside by a short
//! one that happens to cross it.

use super::layers::Layering;
use super::table::{as_f64, Table};
use super::types::Spacing;

/// The four ways the alignment can be biased.
const PASSES: [(bool, bool); 4] = [(true, true), (true, false), (false, true), (false, false)];

/// One orientation of the graph: layers and neighbours as this pass sees them.
struct Frame {
    layers: Vec<Vec<usize>>,
    /// The neighbours of each node in the layer this pass treats as "before".
    before: Vec<Vec<usize>>,
    pos: Table<usize>,
    node_count: usize,
}

impl Frame {
    /// The graph as one pass sees it.
    ///
    /// `downward` walks the layers from the top; `leftward` reads each layer
    /// from its left end. A pass that does neither is the same code on a graph
    /// turned around, which is why there is one implementation and not four.
    fn of(layering: &Layering, layers: &[Vec<usize>], downward: bool, leftward: bool) -> Self {
        let node_count = layering.nodes.len();
        let mut ordered: Vec<Vec<usize>> = layers
            .iter()
            .map(|layer| {
                let mut layer = layer.clone();
                if !leftward {
                    layer.reverse();
                }
                layer
            })
            .collect();
        if !downward {
            ordered.reverse();
        }
        let mut before: Vec<Vec<usize>> = vec![Vec::new(); node_count];
        for arc in &layering.arcs {
            let (near, far) = if downward {
                (arc.to, arc.from)
            } else {
                (arc.from, arc.to)
            };
            if let Some(slot) = before.get_mut(near) {
                slot.push(far);
            }
        }
        let mut pos = Table::<usize>::new(node_count);
        for layer in &ordered {
            for (at, node) in layer.iter().enumerate() {
                pos.set(*node, at);
            }
        }
        // Each node's earlier neighbours, in the order they now appear.
        for slot in &mut before {
            slot.sort_by_key(|node| pos.get(*node));
        }
        Self {
            layers: ordered,
            before,
            pos,
            node_count,
        }
    }

    fn before_of(&self, node: usize) -> &[usize] {
        self.before.get(node).map_or(&[][..], Vec::as_slice)
    }
}

/// The pairs that may not align, as `(smaller, larger)` node index.
///
/// Held sorted so a lookup is a binary search rather than a hash — a hash set
/// would make the pass's output depend on iteration order.
type Conflicts = Vec<(usize, usize)>;

fn pair(a: usize, b: usize) -> (usize, usize) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Segments that cross an inner one, and so may not be aligned along.
///
/// An inner segment joins two dummies: it is part of a long edge, and the whole
/// point of the pass is to keep those straight. Anything crossing one gives way.
fn mark_conflicts(layering: &Layering, layers: &[Vec<usize>]) -> Conflicts {
    let node_count = layering.nodes.len();
    let mut pos = Table::<usize>::new(node_count);
    for layer in layers {
        for (at, node) in layer.iter().enumerate() {
            pos.set(*node, at);
        }
    }
    let mut above: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    for arc in &layering.arcs {
        if let Some(slot) = above.get_mut(arc.to) {
            slot.push(arc.from);
        }
    }
    for slot in &mut above {
        slot.sort_by_key(|node| pos.get(*node));
    }
    let is_dummy = |node: usize| {
        layering
            .nodes
            .get(node)
            .is_some_and(super::layers::LayoutNode::is_dummy)
    };

    let mut out: Conflicts = Vec::new();
    for pair_of_layers in layers.windows(2) {
        let Some(lower) = pair_of_layers.get(1) else {
            continue;
        };
        let mut last_inner = 0usize;
        let mut scanned = 0usize;
        for (at, node) in lower.iter().enumerate() {
            // The inner segment ending here, if there is one.
            let inner = is_dummy(*node)
                .then(|| {
                    above
                        .get(*node)
                        .and_then(|ups| ups.first().copied())
                        .filter(|up| is_dummy(*up))
                })
                .flatten();
            if at + 1 < lower.len() && inner.is_none() {
                continue;
            }
            let limit = inner.map_or_else(
                || pair_of_layers.first().map_or(0, std::vec::Vec::len),
                |up| pos.get(up),
            );
            while scanned <= at {
                let Some(scanning) = lower.get(scanned) else {
                    break;
                };
                for up in above.get(*scanning).map_or(&[][..], Vec::as_slice) {
                    let seen = pos.get(*up);
                    if seen < last_inner || seen > limit {
                        out.push(pair(*up, *scanning));
                    }
                }
                scanned += 1;
            }
            last_inner = limit;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Each node's block: the node it is aligned with, and the block's first node.
struct Blocks {
    root: Table<usize>,
    align: Table<usize>,
}

/// Align each node with one of the neighbours before it, preferring the middle.
fn align(frame: &Frame, conflicts: &Conflicts) -> Blocks {
    let mut out = Blocks {
        root: Table::from_fn(frame.node_count, |at| at),
        align: Table::from_fn(frame.node_count, |at| at),
    };
    for layer in &frame.layers {
        // How far along the previous layer this one has already committed to.
        // An alignment reaching back past it would cross one already made.
        let mut reached: Option<usize> = None;
        for node in layer {
            let neighbours = frame.before_of(*node);
            if neighbours.is_empty() {
                continue;
            }
            let last = neighbours.len() - 1;
            for middle in [last / 2, last.div_ceil(2)] {
                if out.align.get(*node) != *node {
                    break;
                }
                let Some(up) = neighbours.get(middle).copied() else {
                    continue;
                };
                let at = frame.pos.get(up);
                if conflicts.binary_search(&pair(up, *node)).is_ok() {
                    continue;
                }
                if reached.is_some_and(|last| at <= last) {
                    continue;
                }
                out.align.set(up, *node);
                out.root.set(*node, out.root.get(up));
                out.align.set(*node, out.root.get(*node));
                reached = Some(at);
            }
        }
    }
    out
}

/// How far apart two neighbours in a layer have to be, centre to centre.
pub(super) fn separation(layering: &Layering, spacing: &Spacing, left: usize, right: usize) -> f64 {
    let width = |node: usize| layering.nodes.get(node).map_or(0.0, |n| n.size.width);
    let dummy = |node: usize| {
        layering
            .nodes
            .get(node)
            .is_some_and(super::layers::LayoutNode::is_dummy)
    };
    // Two boxes want the room a reader needs to tell them apart. Anything else
    // is at least one wire, and a wire wants only the room that keeps it legible
    // beside its neighbour — the same `spacing.edge` kept between two wires.
    //
    // It used to be the box gap whenever *either* side was a box, which is why
    // `approved` could not run straight past `feature/ui` — 27px of clear air
    // and it was being held 40 away, so it dipped under the box instead and
    // crossed the wire below.
    //
    // A node with no width draws nothing either: it is where a wire crosses a
    // subgraph frame. Held a box apart, three wires left one box 14 apart and
    // crossed its frame 40 apart, and every one of them bent twice to cover the
    // difference.
    let drawn = |node: usize| !dummy(node) && width(node) > 0.0;
    let gap = if drawn(left) && drawn(right) {
        spacing.node
    } else {
        spacing.edge
    };
    f64::midpoint(width(left), width(right)) + gap
}

/// The running state of the compaction.
struct Compaction<'a> {
    frame: &'a Frame,
    blocks: &'a Blocks,
    layering: &'a Layering,
    spacing: &'a Spacing,
    sink: Table<usize>,
    shift: Table<f64>,
    x: Table<Option<f64>>,
}

impl Compaction<'_> {
    /// The node before `node` in its own layer.
    fn previous(&self, node: usize) -> Option<usize> {
        let at = self.frame.pos.get(node);
        let layer = self
            .frame
            .layers
            .iter()
            .find(|layer| layer.contains(&node))?;
        layer.get(at.checked_sub(1)?).copied()
    }

    /// Place one block and everything it leans on.
    ///
    /// Terminates without a depth guard: the block is given a coordinate before
    /// anything it leans on is visited, so a chain that comes back round finds
    /// it already placed and returns. The alignment ring always closes on its
    /// own root, because every node has exactly one node it aligns to and the
    /// root is the node the ring was built from.
    fn place_block(&mut self, root: usize) {
        if self.x.get(root).is_some() {
            return;
        }
        self.x.set(root, Some(0.0));
        let mut node = root;
        loop {
            if let Some(left) = self.previous(node) {
                let leader = self.blocks.root.get(left);
                self.place_block(leader);
                if self.sink.get(root) == root {
                    self.sink.set(root, self.sink.get(leader));
                }
                let gap = separation(self.layering, self.spacing, left, node);
                let (here, there) = (
                    self.x.get(root).unwrap_or(0.0),
                    self.x.get(leader).unwrap_or(0.0),
                );
                if self.sink.get(root) == self.sink.get(leader) {
                    self.x.set(root, Some(here.max(there + gap)));
                } else {
                    let slack = here - there - gap;
                    let sink = self.sink.get(leader);
                    self.shift.set(sink, self.shift.get(sink).min(slack));
                }
            }
            node = self.blocks.align.get(node);
            if node == root {
                break;
            }
        }
    }
}

/// Compact the blocks so nothing overlaps, keeping each block rigid.
fn compact(frame: &Frame, blocks: &Blocks, layering: &Layering, spacing: &Spacing) -> Table<f64> {
    let mut state = Compaction {
        frame,
        blocks,
        layering,
        spacing,
        sink: Table::from_fn(frame.node_count, |at| at),
        shift: Table::from_fn(frame.node_count, |_| f64::INFINITY),
        x: Table::new(frame.node_count),
    };
    for node in 0..frame.node_count {
        if blocks.root.get(node) == node {
            state.place_block(node);
        }
    }
    let mut out = Table::<f64>::new(frame.node_count);
    for node in 0..frame.node_count {
        let root = blocks.root.get(node);
        let mut at = state.x.get(root).unwrap_or(0.0);
        let shift = state.shift.get(state.sink.get(root));
        if shift.is_finite() {
            at += shift;
        }
        out.set(node, at);
    }
    out
}

/// One of the four biased placements.
fn one_pass(
    layering: &Layering,
    layers: &[Vec<usize>],
    conflicts: &Conflicts,
    spacing: &Spacing,
    downward: bool,
    leftward: bool,
) -> Table<f64> {
    let frame = Frame::of(layering, layers, downward, leftward);
    let blocks = align(&frame, conflicts);
    let placed = compact(&frame, &blocks, layering, spacing);
    if leftward {
        return placed;
    }
    // A rightward pass laid the graph out mirrored; turning it back is a
    // negation, and the shift into positive space happens when the four are
    // brought together.
    Table::from_fn(frame.node_count, |at| -placed.get(at))
}

/// Bring the four placements onto one scale and average the middle two.
///
/// The extremes of the four disagree the most, so dropping them is what the
/// paper recommends and what keeps one biased pass from dragging the drawing.
fn balance(passes: &[Table<f64>], node_count: usize) -> Table<f64> {
    let widths: Vec<(f64, f64, f64)> = passes
        .iter()
        .map(|pass| {
            let low = pass.iter().copied().fold(f64::INFINITY, f64::min);
            let high = pass.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (low, high, high - low)
        })
        .collect();
    let narrowest = widths
        .iter()
        .enumerate()
        .filter(|(_, (_, _, width))| width.is_finite())
        .min_by(|a, b| {
            a.1 .2
                .partial_cmp(&b.1 .2)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(at, _)| at);
    let Some((reference_low, reference_high, _)) = narrowest.and_then(|at| widths.get(at).copied())
    else {
        return Table::new(node_count);
    };

    let aligned: Vec<Table<f64>> = passes
        .iter()
        .zip(&widths)
        .enumerate()
        .map(|(at, (pass, (low, high, _)))| {
            // A leftward pass is pinned by its left edge and a rightward one by
            // its right, which is the alignment the paper prescribes.
            let leftward = PASSES.get(at).is_some_and(|(_, left)| *left);
            let shift = if leftward {
                reference_low - low
            } else {
                reference_high - high
            };
            let shift = if shift.is_finite() { shift } else { 0.0 };
            Table::from_fn(node_count, |node| pass.get(node) + shift)
        })
        .collect();

    Table::from_fn(node_count, |node| {
        let mut values: Vec<f64> = aligned.iter().map(|pass| pass.get(node)).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        match (values.get(1), values.get(2)) {
            (Some(low), Some(high)) => f64::midpoint(*low, *high),
            _ => values.first().copied().unwrap_or(0.0),
        }
    })
}

/// Place every node along its layer, and answer each one's centre.
///
/// The result is shifted so the leftmost box's edge sits at zero.
pub fn place(layering: &Layering, layers: &[Vec<usize>], spacing: &Spacing) -> Table<f64> {
    let node_count = layering.nodes.len();
    if node_count == 0 {
        return Table::new(0);
    }
    let conflicts = mark_conflicts(layering, layers);
    let passes: Vec<Table<f64>> = PASSES
        .iter()
        .map(|(downward, leftward)| {
            one_pass(layering, layers, &conflicts, spacing, *downward, *leftward)
        })
        .collect();
    let mut centres = balance(&passes, node_count);
    super::pull::pull_chains(layering, layers, spacing, &mut centres);

    let left = (0..node_count)
        .map(|node| {
            let half = layering.nodes.get(node).map_or(0.0, |n| n.size.width) / 2.0;
            centres.get(node) - half
        })
        .fold(f64::INFINITY, f64::min);
    let shift = if left.is_finite() { -left } else { 0.0 };
    Table::from_fn(node_count, |node| centres.get(node) + shift)
}

/// The width every layer needs, once placed.
pub fn extent(layering: &Layering, centres: &Table<f64>) -> f64 {
    (0..layering.nodes.len())
        .map(|node| {
            let half = layering.nodes.get(node).map_or(0.0, |n| n.size.width) / 2.0;
            centres.get(node) + half
        })
        .fold(0.0, f64::max)
}

/// How tall each layer is, and where its top sits.
/// How tall the space between two layers has to be to hold its lanes.
///
/// A lane sits at `(i + 1) / (n + 1)` of the gap, so `n` of them divide it into
/// `n + 1` parts and the pitch is the gap over `n + 1`. Leaving the gap at
/// `spacing.layer` regardless therefore packs a busy gap tighter and tighter:
/// five runs across the default 50 gives a 8.3px pitch, which is not the 12 that
/// `spacing.edge` says two runs sharing this space are kept apart by. Seven
/// would put them under the width at which the checker stops calling them two
/// edges at all.
fn gap_height(spacing: &Spacing, lanes: usize) -> f64 {
    let wanted = lanes
        .checked_add(1)
        .map_or(spacing.layer, |parts| as_f64(parts) * spacing.edge);
    spacing.layer.max(wanted)
}

/// Where each layer's band starts, and how tall it is.
///
/// `lanes` says how many sideways runs cross the gap *after* each layer, which
/// is what those gaps are sized from. An empty slice asks for the bare
/// `spacing.layer` everywhere, which is what a caller with no routing yet wants.
pub fn layer_tops(
    layering: &Layering,
    layers: &[Vec<usize>],
    spacing: &Spacing,
    lanes: &[usize],
) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(layers.len());
    let mut top = 0.0;
    for (at, layer) in layers.iter().enumerate() {
        let height = layer
            .iter()
            .map(|node| layering.nodes.get(*node).map_or(0.0, |n| n.size.height))
            .fold(0.0, f64::max);
        out.push((top, height));
        top += height + gap_height(spacing, lanes.get(at).copied().unwrap_or(0));
    }
    out
}

/// The height of the whole drawing.
pub fn depth(tops: &[(f64, f64)]) -> f64 {
    tops.iter()
        .map(|(top, height)| top + height)
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cycles::break_cycles;
    use crate::layout::layers::assign_layers;
    use crate::layout::order::order_layers;
    use crate::layout::types::{Edge, Node};

    struct Case {
        layering: Layering,
        layers: Vec<Vec<usize>>,
        centres: Table<f64>,
    }

    fn placed(sizes: &[Node], pairs: &[(usize, usize)]) -> Case {
        let edges: Vec<Edge> = pairs.iter().map(|(a, b)| Edge::new(*a, *b)).collect();
        let acyclic = break_cycles(sizes.len(), &edges);
        let layering = assign_layers(sizes, &acyclic.arcs, edges.len());
        let layers = order_layers(&layering);
        let centres = place(&layering, &layers, &Spacing::default());
        Case {
            layering,
            layers,
            centres,
        }
    }

    fn boxes(n: usize) -> Vec<Node> {
        vec![Node::new(100.0, 40.0); n]
    }

    #[test]
    fn a_single_node_sits_at_its_own_half_width() {
        let case = placed(&boxes(1), &[]);
        assert!((case.centres.get(0) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn a_chain_comes_out_in_one_straight_column() {
        let case = placed(&boxes(3), &[(0, 1), (1, 2)]);
        let first = case.centres.get(0);
        assert!((case.centres.get(1) - first).abs() < 1e-9);
        assert!((case.centres.get(2) - first).abs() < 1e-9);
    }

    #[test]
    fn two_nodes_in_a_layer_clear_each_other() {
        let case = placed(&boxes(3), &[(0, 1), (0, 2)]);
        let gap = (case.centres.get(1) - case.centres.get(2)).abs();
        assert!(gap >= 100.0 + Spacing::default().node - 1e-9, "{gap}");
    }

    #[test]
    fn a_parent_sits_between_the_children_it_feeds() {
        let case = placed(&boxes(3), &[(0, 1), (0, 2)]);
        let middle = f64::midpoint(case.centres.get(1), case.centres.get(2));
        assert!((case.centres.get(0) - middle).abs() < 1.0, "{middle}");
    }

    #[test]
    fn a_long_edge_comes_out_straight() {
        // `0 -> 3` crosses two layers, so it becomes a chain of two dummies.
        // The point of the whole pass is that they line up.
        let case = placed(&boxes(4), &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let chain = case.layering.chains.get(3).cloned().unwrap_or_default();
        assert!(chain.len() > 2, "the long edge bends: {chain:?}");
        let along: Vec<f64> = chain.iter().map(|node| case.centres.get(*node)).collect();
        let bends = along
            .windows(2)
            .filter(|pair| {
                pair.first()
                    .zip(pair.get(1))
                    .is_some_and(|(a, b)| (a - b).abs() > 1e-6)
            })
            .count();
        // The two ends may sit off the line; the dummies between must not.
        let dummies: Vec<f64> = chain
            .iter()
            .filter(|node| {
                case.layering
                    .nodes
                    .get(**node)
                    .is_some_and(super::super::layers::LayoutNode::is_dummy)
            })
            .map(|node| case.centres.get(*node))
            .collect();
        for pair in dummies.windows(2) {
            let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            assert!((a - b).abs() < 1e-6, "the dummies drift: {dummies:?}");
        }
        assert!(bends <= 2, "{along:?}");
    }

    #[test]
    fn no_two_nodes_in_a_layer_overlap() {
        let case = placed(&boxes(7), &[(0, 1), (0, 2), (0, 3), (1, 4), (2, 5), (3, 6)]);
        for layer in &case.layers {
            for pair in layer.windows(2) {
                let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                let need = separation(&case.layering, &Spacing::default(), *a, *b);
                let gap = (case.centres.get(*b) - case.centres.get(*a)).abs();
                assert!(
                    gap >= need - 1e-6,
                    "{a} and {b} are {gap} apart, need {need}"
                );
            }
        }
    }

    #[test]
    fn the_drawing_starts_at_the_left_edge() {
        let case = placed(&boxes(4), &[(0, 1), (0, 2), (1, 3)]);
        let left = (0..4)
            .map(|node| case.centres.get(node) - 50.0)
            .fold(f64::INFINITY, f64::min);
        assert!(left.abs() < 1e-9, "{left}");
    }

    #[test]
    fn boxes_of_different_widths_still_clear_each_other() {
        let sizes = vec![
            Node::new(300.0, 40.0),
            Node::new(40.0, 40.0),
            Node::new(200.0, 40.0),
        ];
        let case = placed(&sizes, &[(0, 1), (0, 2)]);
        let gap = (case.centres.get(2) - case.centres.get(1)).abs();
        assert!(
            gap >= f64::midpoint(40.0, 200.0) + Spacing::default().node - 1e-6,
            "{gap}"
        );
    }

    #[test]
    fn layers_stack_by_the_tallest_box_in_each() {
        let sizes = vec![
            Node::new(100.0, 40.0),
            Node::new(100.0, 90.0),
            Node::new(100.0, 40.0),
        ];
        let case = placed(&sizes, &[(0, 1), (1, 2)]);
        let tops = layer_tops(&case.layering, &case.layers, &Spacing::default(), &[]);
        assert_eq!(tops.len(), 3);
        assert!((tops.get(1).map_or(0.0, |t| t.1) - 90.0).abs() < 1e-9);
        let second = tops.get(1).map_or(0.0, |t| t.0);
        assert!((second - (40.0 + Spacing::default().layer)).abs() < 1e-9);
        assert!((depth(&tops) - (40.0 + 50.0 + 90.0 + 50.0 + 40.0)).abs() < 1e-9);
    }

    #[test]
    fn a_gap_grows_only_when_its_lanes_would_not_fit() {
        let spacing = Spacing::default();
        // Three lanes divide the bare 50 into four parts of 12.5, which already
        // clears the 12 `spacing.edge` asks for. Nothing to do.
        assert!((gap_height(&spacing, 0) - spacing.layer).abs() < 1e-9);
        assert!((gap_height(&spacing, 3) - spacing.layer).abs() < 1e-9);
        // Five would sit 8.3 apart, and five gaps of 12 is 72.
        assert!((gap_height(&spacing, 5) - 72.0).abs() < 1e-9);
    }

    #[test]
    fn the_lanes_a_gap_carries_push_the_layers_below_it_down() {
        let case = placed(&boxes(3), &[(0, 1), (1, 2)]);
        let bare = layer_tops(&case.layering, &case.layers, &Spacing::default(), &[]);
        // Six lanes across the first gap, none across the second.
        let busy = layer_tops(&case.layering, &case.layers, &Spacing::default(), &[6, 0]);
        assert!(
            (busy.first().map_or(0.0, |t| t.0) - bare.first().map_or(0.0, |t| t.0)).abs() < 1e-9
        );
        let grew = busy.get(1).map_or(0.0, |t| t.0) - bare.get(1).map_or(0.0, |t| t.0);
        assert!((grew - (7.0 * 12.0 - 50.0)).abs() < 1e-9, "{grew}");
        // And the quiet gap below is left exactly as it was.
        let after = busy.get(2).map_or(0.0, |t| t.0) - busy.get(1).map_or(0.0, |t| t.0);
        let before = bare.get(2).map_or(0.0, |t| t.0) - bare.get(1).map_or(0.0, |t| t.0);
        assert!((after - before).abs() < 1e-9);
    }

    #[test]
    fn the_drawing_is_as_wide_as_its_widest_row() {
        let case = placed(&boxes(3), &[(0, 1), (0, 2)]);
        let width = extent(&case.layering, &case.centres);
        assert!(
            width >= 100.0 + Spacing::default().node + 100.0 - 1e-6,
            "{width}"
        );
    }

    #[test]
    fn the_same_graph_places_the_same_way_twice() {
        let a = placed(&boxes(6), &[(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5)]);
        let b = placed(&boxes(6), &[(0, 1), (0, 2), (1, 3), (2, 4), (3, 5), (4, 5)]);
        assert_eq!(a.centres, b.centres);
    }

    #[test]
    fn blocks_that_belong_to_separate_runs_are_pulled_together_afterwards() {
        // Two towers side by side with a rung between them: the blocks end up in
        // different sink classes, and the slack between them is closed by the
        // shift pass rather than by the direct comparison.
        let case = placed(
            &boxes(10),
            &[
                (0, 1),
                (1, 2),
                (2, 3),
                (4, 5),
                (5, 6),
                (6, 7),
                (0, 5),
                (4, 1),
                (3, 8),
                (7, 8),
                (8, 9),
            ],
        );
        // Nothing may overlap however the blocks were compacted.
        for layer in &case.layers {
            for pair in layer.windows(2) {
                let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                let need = separation(&case.layering, &Spacing::default(), *a, *b);
                let gap = (case.centres.get(*b) - case.centres.get(*a)).abs();
                assert!(
                    gap >= need - 1e-6,
                    "{a} and {b} are {gap} apart, need {need}"
                );
            }
        }
        assert!(case.centres.iter().all(|at| at.is_finite()));
    }

    #[test]
    fn a_block_leaning_on_two_separate_runs_is_closed_up_by_the_shift() {
        // Built by hand, because the shape is specific: a block spanning two
        // layers whose left-hand neighbour in each layer belongs to a different
        // run. The first neighbour adopts the block into its run; the second
        // then disagrees, and the slack between the two is recorded as a shift
        // rather than resolved on the spot.
        use crate::layout::layers::LayoutNode;
        let node = |layer| LayoutNode {
            size: Node::new(100.0, 40.0),
            real: Some(0),
            layer,
        };
        let layering = Layering {
            nodes: vec![node(0), node(1), node(0), node(1)],
            arcs: Vec::new(),
            layers: vec![vec![0, 2], vec![1, 3]],
            chains: Vec::new(),
        };
        let frame = Frame {
            layers: vec![vec![0, 2], vec![1, 3]],
            before: vec![Vec::new(); 4],
            pos: Table::of(vec![0usize, 0, 1, 1]),
            node_count: 4,
        };
        let blocks = Blocks {
            root: Table::of(vec![0usize, 1, 2, 2]),
            align: Table::of(vec![0usize, 1, 3, 2]),
        };
        let out = compact(&frame, &blocks, &layering, &Spacing::default());
        assert!(out.iter().all(|at| at.is_finite()), "{out:?}");
        // The block stays rigid: both its nodes share a coordinate.
        assert!((out.get(2) - out.get(3)).abs() < 1e-9);
        // And it still clears the two it leans on.
        let need = 100.0 + Spacing::default().node;
        assert!(out.get(2) - out.get(0) >= need - 1e-6, "{out:?}");
        assert!(out.get(3) - out.get(1) >= need - 1e-6, "{out:?}");
    }

    #[test]
    fn a_graph_of_nothing_places_nothing() {
        let layering = assign_layers(&[], &[], 0);
        assert!(place(&layering, &[], &Spacing::default()).is_empty());
        assert!((extent(&layering, &Table::new(0)) - 0.0).abs() < 1e-9);
        assert!((depth(&[]) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn two_edges_passing_between_layers_need_only_an_edges_room() {
        // Both are dummies, so the gap is the edge spacing and not the node one.
        let case = placed(&boxes(6), &[(0, 4), (1, 5), (0, 1), (4, 5)]);
        let spacing = Spacing::default();
        for layer in &case.layers {
            for pair in layer.windows(2) {
                let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                let both_dummies = [*a, *b].iter().all(|node| {
                    case.layering
                        .nodes
                        .get(*node)
                        .is_some_and(super::super::layers::LayoutNode::is_dummy)
                });
                if both_dummies {
                    let need = separation(&case.layering, &spacing, *a, *b);
                    assert!((need - spacing.edge).abs() < 1e-9, "{need}");
                }
            }
        }
    }
}
