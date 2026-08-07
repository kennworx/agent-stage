//! Pulling a straight chain of dummies toward the boxes it actually joins.

use super::layers::Layering;
use super::place::separation;
use super::table::Table;
use super::types::Spacing;

/// Pull each chain of dummies toward the two real boxes it joins.
///
/// Brandes–Köpf makes a chain straight, which is what matters, and says nothing
/// about *where* the straight line goes — so a back edge can come out as a clean
/// run parked clear of both its own ends. `approved` dipped 43px below a source
/// at 122.6 and a target at 145.4 and travelled there and back for nothing.
///
/// The chain is rigid, so it moves as one and only as far as its narrowest layer
/// allows: `separation` is the rule that placed its neighbours, and it holds.
pub(super) fn pull_chains(
    layering: &Layering,
    layers: &[Vec<usize>],
    spacing: &Spacing,
    centres: &mut Table<f64>,
) {
    let mut pos = Table::<usize>::new(layering.nodes.len());
    for layer in layers {
        for (at, node) in layer.iter().enumerate() {
            pos.set(*node, at);
        }
    }
    let is_dummy = |node: usize| {
        layering
            .nodes
            .get(node)
            .is_some_and(super::layers::LayoutNode::is_dummy)
    };
    for chain in &layering.chains {
        let inner: Vec<usize> = chain.iter().copied().filter(|n| is_dummy(*n)).collect();
        if inner.is_empty() {
            continue;
        }
        let (Some(first), Some(last)) = (chain.first().copied(), chain.last().copied()) else {
            continue;
        };
        let want = f64::midpoint(centres.get(first), centres.get(last));
        let at = centres.get(inner.first().copied().unwrap_or(0));
        let step = want - at;
        if step.abs() < 1e-9 {
            continue;
        }
        // How far the whole chain may go before any of its dummies would sit
        // closer to a neighbour than the layout keeps two things.
        let mut room = step.abs();
        for node in &inner {
            let Some(layer) = layering.nodes.get(*node).and_then(|n| layers.get(n.layer)) else {
                continue;
            };
            let here = pos.get(*node);
            let beside = if step > 0.0 {
                here.checked_add(1).and_then(|n| layer.get(n))
            } else {
                here.checked_sub(1).and_then(|n| layer.get(n))
            };
            if let Some(other) = beside.copied() {
                let gap = (centres.get(other) - centres.get(*node)).abs()
                    - separation(layering, spacing, *node, other);
                room = room.min(gap.max(0.0));
            }
        }
        if room < 1e-9 {
            continue;
        }
        let by = room.min(step.abs()) * step.signum();
        for node in &inner {
            centres.update(*node, |x| x + by);
        }
    }
}
