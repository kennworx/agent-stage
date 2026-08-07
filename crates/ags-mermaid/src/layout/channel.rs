//! Which lane each sideways run takes, so two runs sharing a gap do not cross.
//!
//! A run drops out of the upper layer at one column, crosses to another in a
//! lane of its own, and drops into the lower layer there. Its entry leg is drawn
//! *above* its lane and its exit leg *below*, so the lane it gets decides which
//! of its neighbours those two legs cut through:
//!
//! ```text
//!      a.from        b.from      a.to        b.to
//!         │             ┆          ┆           ┆
//!    ─────┴─────────────┼──────────┘           ┆     a, in the upper lane
//!                       └──────────────────────┘     b, in the lower
//! ```
//!
//! Here `b` enters inside the columns `a` travels between, so `b`'s entry leg
//! has to pass through `a`'s lane — unless `b` takes the higher lane itself.
//!
//! Ordering by where a run starts — the rule this replaces — gets that backwards
//! whenever two runs overlap without nesting: `b` starts further right and was
//! therefore put lower, which is the one arrangement that guarantees the
//! crossing. A CI diagram drew its `No` branch through its retry wire for
//! exactly this reason, and a state diagram drew `失败` through `重试`.

use super::route::Step;

/// Below this, a column is level with the end of a run rather than inside it.
///
/// Two runs are kept `spacing.edge` apart, so nothing legitimate sits within
/// half a pixel of a boundary; this only has to stop a run that starts exactly
/// where another ends from counting as crossing it.
const EPS: f64 = 0.5;

/// The columns a run travels between, in order.
fn span(run: &Step) -> (f64, f64) {
    let (from, to) = run.order;
    (from.min(to), from.max(to))
}

/// Whether a column falls strictly between the columns a run travels between.
fn inside(column: f64, run: &Step) -> bool {
    let (low, high) = span(run);
    column > low + EPS && column < high - EPS
}

/// Whether one run has to be drawn above another for the pair not to cross.
///
/// `a` has to be above `b` when `a` enters inside `b`'s span — its entry leg
/// would otherwise cut `b`'s lane — or when `b` leaves inside `a`'s span, which
/// is the same fault seen from the other end.
///
/// Both directions can hold at once: two runs that nest, or that genuinely swap
/// over, cross whatever lanes they are given. The caller settles those by the
/// order they already had rather than pretending there is an answer.
fn wants_above(a: &Step, b: &Step) -> bool {
    inside(a.order.0, b) || inside(b.order.1, a)
}

/// The runs of one gap, in the order their lanes are handed out.
///
/// Repeated selection rather than a sort: "must be above" is not an ordering —
/// it has ties and it has cycles — and handing it to a comparison sort would
/// give an answer that depends on the sort's internals. Taking the first run
/// nothing else needs to be above is a topological order where one exists, and
/// falls back to the order the runs came in where one does not.
fn stack(gap: &[usize], steps: &[Step]) -> Vec<usize> {
    let held = |over: usize, under: usize| match (steps.get(over), steps.get(under)) {
        (Some(over), Some(under)) => wants_above(over, under),
        _ => false,
    };
    let mut left: Vec<usize> = gap.to_vec();
    let mut out: Vec<usize> = Vec::with_capacity(left.len());
    while !left.is_empty() {
        let free = left
            .iter()
            .position(|at| left.iter().all(|other| other == at || !held(*other, *at)));
        // Nothing is free: every run left is under another, so they cross
        // whichever way round they go. Keep the one that was already first.
        let pick = free.unwrap_or(0);
        out.push(left.remove(pick));
    }
    out
}

/// Every run, gap by gap, ordered within each gap so as few as possible cross.
///
/// The incoming order is the tie-break: runs arrive sorted by where they start
/// and end, so two runs with no constraint between them keep the arrangement
/// that reads left to right.
pub(super) fn stacked(steps: Vec<Step>) -> Vec<Step> {
    let mut gaps: Vec<(usize, Vec<usize>)> = Vec::new();
    for (at, step) in steps.iter().enumerate() {
        match gaps.iter_mut().find(|(gap, _)| *gap == step.gap) {
            Some((_, held)) => held.push(at),
            None => gaps.push((step.gap, vec![at])),
        }
    }
    let order: Vec<usize> = gaps
        .iter()
        .flat_map(|(_, held)| stack(held, &steps))
        .collect();
    let mut taken: Vec<Option<Step>> = steps.into_iter().map(Some).collect();
    order
        .into_iter()
        .filter_map(|at| taken.get_mut(at).and_then(Option::take))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(edge: usize, gap: usize, from: f64, to: f64) -> Step {
        Step {
            edge,
            at: 0,
            gap,
            order: (from, to),
        }
    }

    fn edges(steps: &[Step]) -> Vec<usize> {
        steps.iter().map(|step| step.edge).collect()
    }

    #[test]
    fn a_run_entering_inside_another_takes_the_higher_lane() {
        // The measured CI case: the `No` branch runs 125.7 to 235.7 and the
        // retry wire enters at 215.9, inside it. Sorted by where they start, the
        // retry wire went second and the two were drawn through each other.
        let steps = vec![run(0, 0, 125.7, 235.7), run(1, 0, 215.9, 271.9)];
        assert_eq!(edges(&stacked(steps)), vec![1, 0]);
    }

    #[test]
    fn a_run_leaving_inside_another_takes_the_lower_lane() {
        // The same fault seen from the other end, and the same answer.
        let steps = vec![run(0, 0, 215.9, 271.9), run(1, 0, 125.7, 235.7)];
        assert_eq!(edges(&stacked(steps)), vec![0, 1]);
    }

    #[test]
    fn runs_that_do_not_meet_keep_the_order_they_came_in() {
        let steps = vec![run(0, 0, 0.0, 50.0), run(1, 0, 200.0, 250.0)];
        assert_eq!(edges(&stacked(steps)), vec![0, 1]);
    }

    #[test]
    fn two_runs_that_nest_cross_whichever_way_they_are_stacked() {
        // Neither order helps: the inner run's entry leg cuts the outer run's
        // lane, or the inner run's exit leg does. The incoming order stands.
        let outer = run(0, 0, 100.0, 300.0);
        let inner = run(1, 0, 150.0, 250.0);
        assert!(wants_above(&outer, &inner) && wants_above(&inner, &outer));
        assert_eq!(edges(&stacked(vec![outer, inner])), vec![0, 1]);
    }

    #[test]
    fn each_gap_is_stacked_on_its_own() {
        // A run in one gap constrains nothing in another, however its columns
        // line up.
        let steps = vec![
            run(0, 0, 125.7, 235.7),
            run(1, 1, 215.9, 271.9),
            run(2, 0, 215.9, 271.9),
        ];
        assert_eq!(edges(&stacked(steps)), vec![2, 0, 1]);
    }

    #[test]
    fn a_column_level_with_the_end_of_a_run_is_not_inside_it() {
        // One run leaving exactly where another arrives is two lines meeting,
        // not one cutting the other.
        let a = run(0, 0, 100.0, 200.0);
        assert!(!inside(200.0, &a));
        assert!(!inside(100.0, &a));
        assert!(inside(150.0, &a));
    }

    #[test]
    fn a_run_travelling_leftward_spans_the_same_columns() {
        let rightward = run(0, 0, 100.0, 200.0);
        let leftward = run(1, 0, 200.0, 100.0);
        assert_eq!(span(&rightward), span(&leftward));
    }

    #[test]
    fn nothing_to_stack_comes_back_empty() {
        assert!(stacked(Vec::new()).is_empty());
    }
}
