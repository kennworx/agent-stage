//! Placing things on a grid from the side letters that join them.
//!
//! This is the one diagram type in the set that does not go through the layered
//! engine, and the reason is in its own grammar. `a:R -- L:b` does not describe
//! a graph to be laid out — it says where b goes. A layered pass would read it
//! as an edge and put b wherever the crossing count came out lowest, which is
//! how a drawing that says "the cache is below the workers" comes out with the
//! cache beside them.
//!
//! So: whole cells, one relation at a time, in declaration order. Nothing here
//! knows about pixels, boxes or groups.

/// A square on the grid. Negative until the whole grid is shifted to fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cell {
    pub col: i64,
    pub row: i64,
}

impl Cell {
    pub const fn new(col: i64, row: i64) -> Self {
        Self { col, row }
    }

    const fn shifted(self, by: (i64, i64)) -> Self {
        Self::new(self.col + by.0, self.row + by.1)
    }
}

/// "`to` sits this many cells from `from`".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub from: usize,
    pub to: usize,
    pub step: (i64, i64),
}

/// Where a step lands, or the nearest free square across from it.
///
/// Across rather than along: a second thing on the right of a box belongs
/// beside the first, not past it, which is what keeps a fan-out looking like
/// one.
fn free_near(taken: &[Cell], want: Cell, step: (i64, i64)) -> Cell {
    if !taken.contains(&want) {
        return want;
    }
    let across = if step.0 == 0 { (1, 0) } else { (0, 1) };
    for distance in 1..=32 {
        for sign in [1_i64, -1] {
            let shift = (across.0 * distance * sign, across.1 * distance * sign);
            let candidate = want.shifted(shift);
            if !taken.contains(&candidate) {
                return candidate;
            }
        }
    }
    want
}

/// A free square to start an unconnected thing at: a fresh row of its own.
fn free_seed(taken: &[Cell]) -> Cell {
    let below = taken
        .iter()
        .map(|cell| cell.row)
        .max()
        .map_or(0, |row| row + 1);
    free_near(taken, Cell::new(0, below), (0, 1))
}

/// Everything a link says about a pair, whichever end is already placed.
fn reach(link: Link, at: usize) -> Option<(usize, (i64, i64))> {
    if link.from == at {
        return Some((link.to, link.step));
    }
    if link.to == at {
        return Some((link.from, (-link.step.0, -link.step.1)));
    }
    None
}

/// Walk out from one placed thing, placing everything it reaches.
fn spread(from: usize, links: &[Link], cells: &mut [Option<Cell>], taken: &mut Vec<Cell>) {
    let mut queue = vec![from];
    while let Some(at) = queue.pop() {
        let Some(Some(here)) = cells.get(at).copied() else {
            continue;
        };
        for link in links {
            let Some((other, step)) = reach(*link, at) else {
                continue;
            };
            let Some(slot) = cells.get_mut(other) else {
                continue;
            };
            if slot.is_some() {
                continue;
            }
            let landed = free_near(taken, here.shifted(step), step);
            *slot = Some(landed);
            taken.push(landed);
            queue.push(other);
        }
    }
}

/// Shift every cell so the grid starts at nought in both directions.
fn to_origin(cells: &mut [Cell]) {
    let left = cells.iter().map(|cell| cell.col).min().unwrap_or(0);
    let top = cells.iter().map(|cell| cell.row).min().unwrap_or(0);
    for cell in cells.iter_mut() {
        *cell = Cell::new(cell.col - left, cell.row - top);
    }
}

/// Place `count` things, honouring every link that can be honoured.
///
/// Deterministic throughout: things are seeded in the order they were declared,
/// each is reached in link order, and a square already taken is stepped past in
/// a fixed direction. The same source twice gives the same grid.
///
/// A link whose two ends are both already placed is dropped rather than
/// enforced. Two constraints can disagree — three boxes each declared to the
/// right of the last two — and the first one written wins, which is the rule a
/// reader can hold in their head.
pub fn place(count: usize, links: &[Link]) -> Vec<Cell> {
    let mut cells: Vec<Option<Cell>> = vec![None; count];
    let mut taken: Vec<Cell> = Vec::new();
    for at in 0..count {
        if cells.get(at).copied().flatten().is_some() {
            continue;
        }
        let seed = free_seed(&taken);
        if let Some(slot) = cells.get_mut(at) {
            *slot = Some(seed);
        }
        taken.push(seed);
        spread(at, links, &mut cells, &mut taken);
    }
    let mut out: Vec<Cell> = cells.into_iter().map(Option::unwrap_or_default).collect();
    to_origin(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(from: usize, to: usize, step: (i64, i64)) -> Link {
        Link { from, to, step }
    }

    #[test]
    fn one_thing_sits_at_the_origin() {
        assert_eq!(place(1, &[]), [Cell::new(0, 0)]);
        assert!(place(0, &[]).is_empty());
    }

    #[test]
    fn a_link_puts_the_second_thing_where_it_says() {
        assert_eq!(
            place(2, &[link(0, 1, (1, 0))]),
            [Cell::new(0, 0), Cell::new(1, 0)]
        );
        assert_eq!(
            place(2, &[link(0, 1, (0, 1))]),
            [Cell::new(0, 0), Cell::new(0, 1)]
        );
    }

    #[test]
    fn a_link_read_backwards_places_the_first_thing_too() {
        // The second thing is declared first, so the first is reached through
        // the link in reverse.
        let cells = place(2, &[link(1, 0, (1, 0))]);
        assert_eq!(cells, [Cell::new(1, 0), Cell::new(0, 0)]);
    }

    #[test]
    fn a_grid_always_starts_at_nought() {
        // Everything hangs to the left of the thing declared first.
        let cells = place(3, &[link(0, 1, (-1, 0)), link(1, 2, (-1, 0))]);
        assert_eq!(cells, [Cell::new(2, 0), Cell::new(1, 0), Cell::new(0, 0)]);
    }

    #[test]
    fn two_things_sent_to_one_square_are_stacked_across_the_step() {
        // Both are declared to the right of the first, so the second goes
        // beside the first rather than past it.
        let cells = place(3, &[link(0, 1, (1, 0)), link(0, 2, (1, 0))]);
        assert_eq!(cells, [Cell::new(0, 0), Cell::new(1, 0), Cell::new(1, 1)]);
        // And downwards, they stack sideways.
        let below = place(3, &[link(0, 1, (0, 1)), link(0, 2, (0, 1))]);
        assert_eq!(below, [Cell::new(0, 0), Cell::new(0, 1), Cell::new(1, 1)]);
    }

    #[test]
    fn a_thing_nothing_reaches_gets_a_row_to_itself() {
        let cells = place(3, &[link(0, 1, (1, 0))]);
        assert_eq!(cells, [Cell::new(0, 0), Cell::new(1, 0), Cell::new(0, 1)]);
    }

    #[test]
    fn a_diagonal_step_places_a_fan_either_side() {
        // A junction with one thing below-left and one below-right, which is
        // what `j:L -- T:api1` and `j:R -- T:api2` mean.
        let cells = place(3, &[link(0, 1, (-1, 1)), link(0, 2, (1, 1))]);
        assert_eq!(cells, [Cell::new(1, 0), Cell::new(0, 1), Cell::new(2, 1)]);
    }

    #[test]
    fn a_link_between_two_things_already_placed_is_left_alone() {
        // The third says it is right of the first and below the second; the
        // first constraint reached wins and the drawing keeps both lines.
        let cells = place(
            3,
            &[link(0, 1, (1, 0)), link(0, 2, (1, 0)), link(1, 2, (1, 0))],
        );
        assert_eq!(cells.len(), 3);
        // Whatever it resolved to, nothing shares a square.
        for (at, cell) in cells.iter().enumerate() {
            for other in cells.iter().skip(at + 1) {
                assert_ne!(cell, other);
            }
        }
    }

    #[test]
    fn nothing_ever_shares_a_square() {
        // A fan of eight from one point, all sent the same way.
        let links: Vec<Link> = (1..9).map(|to| link(0, to, (1, 0))).collect();
        let cells = place(9, &links);
        for (at, cell) in cells.iter().enumerate() {
            for other in cells.iter().skip(at + 1) {
                assert_ne!(cell, other, "two things in one square");
            }
        }
    }

    #[test]
    fn the_same_links_twice_give_the_same_grid() {
        let links = [link(0, 1, (1, 0)), link(0, 2, (1, 0)), link(2, 3, (0, 1))];
        assert_eq!(place(4, &links), place(4, &links));
    }

    #[test]
    fn a_link_naming_something_that_is_not_there_is_ignored() {
        let cells = place(2, &[link(0, 9, (1, 0)), link(0, 1, (1, 0))]);
        assert_eq!(cells, [Cell::new(0, 0), Cell::new(1, 0)]);
    }

    #[test]
    fn a_square_with_nowhere_free_across_it_falls_back_to_the_one_it_wanted() {
        // Thirty-three things all sent to the same square exhausts the search,
        // which returns the square rather than looping.
        let taken: Vec<Cell> = (-32..=32).map(|row| Cell::new(1, row)).collect();
        assert_eq!(free_near(&taken, Cell::new(1, 0), (1, 0)), Cell::new(1, 0));
    }
}
