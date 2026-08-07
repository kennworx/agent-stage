//! Where each node sits, and the band each flow takes between two of them.
//!
//! Four steps, in order:
//!
//! 1. **Columns.** A node's column is the longest path in links from a node with
//!    no inflow, found by relaxation. The pass count is capped at the node count
//!    so a cycle cannot loop forever.
//! 2. **Weights.** A node is as thick as the larger of what flows in and what
//!    flows out — the two agree except at a source or a sink.
//! 3. **Scale.** One value-to-pixel scale for the whole diagram, chosen so the
//!    tightest column exactly fills the plot and no column overflows it.
//! 4. **Bands.** Outgoing bands stack down a node's right edge in link order,
//!    incoming bands down its left edge, so no two bands overlap at an edge.

use crate::round::count;
use crate::scene::Point;

use super::types::Diagram;

pub const PADDING: f64 = 28.0;
pub const NODE_WIDTH: f64 = 22.0;
/// Left edge to left edge between adjacent columns.
pub const COLUMN_GAP: f64 = 190.0;
/// Vertical gap between stacked nodes in one column.
pub const NODE_PADDING: f64 = 20.0;
pub const PLOT_HEIGHT: f64 = 420.0;
pub const LABEL_GAP: f64 = 8.0;
pub const LABEL_FONT: f64 = 14.0;
pub const LABEL_WEIGHT: u32 = 500;
/// The thinnest a band may be drawn, so a tiny flow is still visible.
pub const MIN_BAND: f64 = 1.5;

/// Which side of its node a name is written on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// To the right of the node, reading away from it.
    Right,
    /// To the left, which is where the final column writes so its names stay
    /// on the canvas.
    Left,
}

/// One node, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedNode {
    pub id: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    /// Index into the derived palette; also the order of first appearance.
    pub color_index: usize,
    pub label_at: Point,
    pub label_side: Side,
}

/// One flow, placed: the band it fills between two edges.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLink {
    pub source: String,
    pub target: String,
    pub value: f64,
    /// Top-left of the band where it leaves the source.
    pub from: Point,
    /// Top-left of the band where it meets the target.
    pub to: Point,
    pub thickness: f64,
    /// Taken from the source, so a flow reads as coming *from* somewhere.
    pub color_index: usize,
}

/// A laid-out sankey diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<PlacedNode>,
    pub links: Vec<PlacedLink>,
}

/// The index of each node name, so a link can be followed in one step.
fn index_of(nodes: &[String], name: &str) -> Option<usize> {
    nodes.iter().position(|n| n == name)
}

/// What flows in and out of each node.
fn flows(diagram: &Diagram) -> (Vec<f64>, Vec<f64>) {
    let n = diagram.nodes.len();
    let (mut inflow, mut outflow) = (vec![0.0; n], vec![0.0; n]);
    for link in &diagram.links {
        if let (Some(s), Some(t)) = (
            index_of(&diagram.nodes, &link.source),
            index_of(&diagram.nodes, &link.target),
        ) {
            if let Some(slot) = outflow.get_mut(s) {
                *slot += link.value;
            }
            if let Some(slot) = inflow.get_mut(t) {
                *slot += link.value;
            }
        }
    }
    (inflow, outflow)
}

/// The column each node falls in: its longest path from a node with no inflow.
///
/// Relaxed rather than solved, and capped at one pass per node — a cyclic graph
/// has no longest path at all, and the cap is what stops it looping forever.
fn columns(diagram: &Diagram) -> Vec<usize> {
    let n = diagram.nodes.len();
    let mut column = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for link in &diagram.links {
            let (Some(s), Some(t)) = (
                index_of(&diagram.nodes, &link.source),
                index_of(&diagram.nodes, &link.target),
            ) else {
                continue;
            };
            let (Some(from), Some(to)) = (column.get(s).copied(), column.get(t).copied()) else {
                continue;
            };
            if to < from + 1 {
                if let Some(slot) = column.get_mut(t) {
                    *slot = from + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    column
}

/// The one value-to-pixel scale the whole diagram uses.
///
/// The smallest candidate wins: a scale that lets a looser column fill the plot
/// would push the tightest one past the bottom of it.
fn value_scale(groups: &[Vec<usize>], weight: &[f64]) -> f64 {
    let mut scale = f64::INFINITY;
    for group in groups {
        if group.is_empty() {
            continue;
        }
        let sum: f64 = group.iter().filter_map(|i| weight.get(*i)).sum();
        // A column of nothing but zero-valued nodes would divide by zero.
        let sum = if sum == 0.0 { 1.0 } else { sum };
        let gaps = (count(group.len()) - 1.0) * NODE_PADDING;
        scale = scale.min((PLOT_HEIGHT - gaps) / sum);
    }
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// A node's drawn height: its weight, but never thinner than a hairline.
fn drawn_height(weight: f64, scale: f64) -> f64 {
    (weight * scale).max(MIN_BAND)
}

/// Place one column's nodes, stacked and centred against the plot height.
fn place_column(
    group: &[usize],
    at_column: usize,
    last_column: usize,
    diagram: &Diagram,
    weight: &[f64],
    scale: f64,
) -> Vec<(usize, PlacedNode)> {
    let stack: f64 = group
        .iter()
        .filter_map(|i| weight.get(*i))
        .map(|w| drawn_height(*w, scale))
        .sum();
    let height = stack + (count(group.len()) - 1.0) * NODE_PADDING;
    let mut y = PADDING + ((PLOT_HEIGHT - height) / 2.0).max(0.0);
    let x = PADDING + count(at_column) * COLUMN_GAP;

    // The final column writes leftward so its names stay on the canvas — but
    // only when there is a column to its left to write into.
    let side = if last_column > 0 && at_column == last_column {
        Side::Left
    } else {
        Side::Right
    };
    let label_x = match side {
        Side::Left => x - LABEL_GAP,
        Side::Right => x + NODE_WIDTH + LABEL_GAP,
    };

    let mut out = Vec::with_capacity(group.len());
    for &i in group {
        let h = drawn_height(weight.get(i).copied().unwrap_or_default(), scale);
        let name = diagram.nodes.get(i).cloned().unwrap_or_default();
        out.push((
            i,
            PlacedNode {
                id: name,
                at: Point::new(x, y),
                width: NODE_WIDTH,
                height: h,
                color_index: i,
                label_at: Point::new(label_x, y + h / 2.0),
                label_side: side,
            },
        ));
        y += h + NODE_PADDING;
    }
    out
}

/// Lay out a parsed sankey diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    let n = diagram.nodes.len();
    if n == 0 {
        return Placed {
            width: PADDING * 2.0,
            height: PADDING * 2.0,
            ..Placed::default()
        };
    }

    let (inflow, outflow) = flows(diagram);
    let column = columns(diagram);
    let last_column = column.iter().copied().max().unwrap_or(0);
    // A node is as thick as the busier of its two sides.
    let weight: Vec<f64> = (0..n)
        .map(|i| {
            inflow
                .get(i)
                .copied()
                .unwrap_or_default()
                .max(outflow.get(i).copied().unwrap_or_default())
                .max(0.0)
        })
        .collect();

    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); last_column + 1];
    for (i, c) in column.iter().enumerate() {
        if let Some(group) = groups.get_mut(*c) {
            group.push(i);
        }
    }
    let scale = value_scale(&groups, &weight);

    let mut nodes: Vec<Option<PlacedNode>> = vec![None; n];
    for (c, group) in groups.iter().enumerate() {
        for (i, node) in place_column(group, c, last_column, diagram, &weight, scale) {
            if let Some(slot) = nodes.get_mut(i) {
                *slot = Some(node);
            }
        }
    }
    let nodes: Vec<PlacedNode> = nodes.into_iter().flatten().collect();

    // Bands stack down each edge in link order, so two flows out of one node
    // sit against each other rather than on top of each other.
    let (mut out_offset, mut in_offset) = (vec![0.0; n], vec![0.0; n]);
    let links: Vec<PlacedLink> = diagram
        .links
        .iter()
        .filter_map(|link| {
            let s = index_of(&diagram.nodes, &link.source)?;
            let t = index_of(&diagram.nodes, &link.target)?;
            let (source, target) = (nodes.get(s)?, nodes.get(t)?);
            let thickness = (link.value * scale).max(MIN_BAND);
            let from = Point::new(
                source.at.x + source.width,
                source.at.y + out_offset.get(s).copied().unwrap_or_default(),
            );
            let to = Point::new(
                target.at.x,
                target.at.y + in_offset.get(t).copied().unwrap_or_default(),
            );
            if let Some(slot) = out_offset.get_mut(s) {
                *slot += thickness;
            }
            if let Some(slot) = in_offset.get_mut(t) {
                *slot += thickness;
            }
            Some(PlacedLink {
                source: link.source.clone(),
                target: link.target.clone(),
                value: link.value,
                from,
                to,
                thickness,
                color_index: source.color_index,
            })
        })
        .collect();

    // The right edge is the further of the last column's nodes and the widest
    // name written rightward from the column before it.
    let widest_right = nodes
        .iter()
        .filter(|node| node.label_side == Side::Right)
        .map(|node| crate::metrics::text_width(&node.id, LABEL_FONT, LABEL_WEIGHT))
        .fold(0.0_f64, f64::max);
    let last_edge = PADDING + count(last_column) * COLUMN_GAP + NODE_WIDTH;
    let label_edge = PADDING
        + count(last_column.saturating_sub(1)) * COLUMN_GAP
        + NODE_WIDTH
        + LABEL_GAP
        + widest_right;

    Placed {
        width: last_edge.max(label_edge) + PADDING,
        height: PLOT_HEIGHT + PADDING * 2.0,
        nodes,
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sankey::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const CHAIN: &str = "sankey-beta\nA,B,10\nB,C,10";

    #[test]
    fn a_chain_puts_each_node_in_its_own_column() {
        let out = placed(CHAIN);
        let xs: Vec<f64> = out.nodes.iter().map(|n| n.at.x).collect();
        assert!((xs[0] - PADDING).abs() < 1e-9);
        assert!((xs[1] - (PADDING + COLUMN_GAP)).abs() < 1e-9);
        assert!((xs[2] - (PADDING + COLUMN_GAP * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_node_takes_the_longest_path_to_it_not_the_shortest() {
        // A reaches C directly and through B; the long way is what decides.
        let out = placed("sankey\nA,B,1\nB,C,1\nA,C,1");
        assert!((out.nodes[2].at.x - (PADDING + COLUMN_GAP * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_cycle_terminates_rather_than_relaxing_forever() {
        // No longest path exists; the pass cap is what makes this return.
        let out = placed("sankey\nA,B,1\nB,A,1");
        assert_eq!(out.nodes.len(), 2);
    }

    #[test]
    fn a_node_is_as_thick_as_its_busier_side() {
        // B takes 10 in and passes 4 on, so it is drawn at 10, not 4.
        let out = placed("sankey\nA,B,10\nB,C,4");
        assert!(out.nodes[1].height > out.nodes[2].height);
    }

    #[test]
    fn no_column_overflows_the_plot() {
        let out = placed("sankey\nA,X,1\nB,X,1\nC,X,1\nD,X,1\nE,X,1");
        for node in &out.nodes {
            assert!(
                node.at.y + node.height <= PADDING + PLOT_HEIGHT + 1e-6,
                "{node:?}"
            );
        }
    }

    #[test]
    fn a_column_is_centred_against_the_plot() {
        let out = placed(CHAIN);
        let node = &out.nodes[0];
        let above = node.at.y - PADDING;
        let below = PADDING + PLOT_HEIGHT - (node.at.y + node.height);
        assert!((above - below).abs() < 1e-6, "{above} vs {below}");
    }

    #[test]
    fn a_tiny_flow_is_still_drawn_thick_enough_to_see() {
        let out = placed("sankey\nA,B,1000000\nA,C,0.000001");
        assert!((out.links[1].thickness - MIN_BAND).abs() < 1e-9);
    }

    #[test]
    fn bands_stack_down_an_edge_rather_than_overlapping_on_it() {
        let out = placed("sankey\nA,B,10\nA,C,10");
        let (first, second) = (&out.links[0], &out.links[1]);
        assert!((second.from.y - first.from.y - first.thickness).abs() < 1e-9);
        // Both leave the same edge, so the x agrees.
        assert!((second.from.x - first.from.x).abs() < 1e-9);
    }

    #[test]
    fn a_band_leaves_the_right_edge_and_arrives_at_the_left_one() {
        let out = placed(CHAIN);
        let link = &out.links[0];
        let (a, b) = (&out.nodes[0], &out.nodes[1]);
        assert!((link.from.x - (a.at.x + a.width)).abs() < 1e-9);
        assert!((link.to.x - b.at.x).abs() < 1e-9);
    }

    #[test]
    fn only_the_final_column_writes_its_names_leftward() {
        let out = placed(CHAIN);
        let sides: Vec<Side> = out.nodes.iter().map(|n| n.label_side).collect();
        assert_eq!(sides, [Side::Right, Side::Right, Side::Left]);
        // A single-column diagram has nothing to its left to write into.
        let alone = layout(&crate::sankey::Diagram {
            nodes: vec!["Only".into()],
            links: Vec::new(),
        });
        assert_eq!(alone.nodes[0].label_side, Side::Right);
    }

    #[test]
    fn a_name_sits_beside_its_node_on_the_side_it_was_given() {
        let out = placed(CHAIN);
        let (first, last) = (&out.nodes[0], &out.nodes[2]);
        assert!((first.label_at.x - (first.at.x + NODE_WIDTH + LABEL_GAP)).abs() < 1e-9);
        assert!((last.label_at.x - (last.at.x - LABEL_GAP)).abs() < 1e-9);
        // Vertically it is centred on the node either way.
        assert!((first.label_at.y - (first.at.y + first.height / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn the_canvas_makes_room_for_the_widest_rightward_name() {
        let narrow = placed("sankey\nA,Z,1");
        let wide = placed("sankey\nA very long node name indeed,Z,1");
        assert!(wide.width > narrow.width);
        // Height is fixed by the plot, whatever the names do.
        assert!((wide.height - narrow.height).abs() < 1e-9);
    }

    #[test]
    fn an_empty_diagram_is_padding_alone() {
        let out = placed("sankey");
        assert!((out.width - PADDING * 2.0).abs() < 1e-9);
        assert!((out.height - PADDING * 2.0).abs() < 1e-9);
        assert!(out.nodes.is_empty());
    }

    #[test]
    fn a_column_of_nothing_but_zero_weights_still_gets_a_scale() {
        // The sum would divide by zero; the fallback is what keeps it finite.
        let out = placed("sankey\nA,B,0");
        assert!(out.nodes.iter().all(|n| n.height.is_finite()));
        assert!((out.nodes[0].height - MIN_BAND).abs() < 1e-9);
    }
}
