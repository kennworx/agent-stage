//! A vector addressed by node index, with no way to panic.
//!
//! A layered layout is almost entirely array work — layer of a node, order
//! within a layer, alignment, root, shift — and written with `v[i]` it is a
//! thicket of possible panics on a graph the caller built badly. `Table` makes
//! every read total: out of range answers with the default, which is the same
//! answer the algorithm would want for a node that is not there.
//!
//! This is not defensive padding. Every index the passes compute is derived
//! from a length they already hold, so out of range never happens; the type
//! exists so that fact does not have to be re-argued at each of a hundred
//! call sites.

/// A count of nodes or slots, as a float.
///
/// Positions and sizes are floats and counts are not; this is the one place the
/// two meet, so the conversion is argued once here rather than at each use.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts of nodes in a diagram, never near 2^53"
)]
pub fn as_f64(n: usize) -> f64 {
    n as f64
}

/// A value per node, addressed by the node's index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table<T> {
    values: Vec<T>,
}

impl<T: Clone + Default> Table<T> {
    /// A table of `len` defaults.
    pub fn new(len: usize) -> Self {
        Self {
            values: vec![T::default(); len],
        }
    }

    /// A table holding `values`.
    pub fn of(values: Vec<T>) -> Self {
        Self { values }
    }

    /// A table whose entry at each index is `f` of that index.
    pub fn from_fn(len: usize, f: impl Fn(usize) -> T) -> Self {
        Self {
            values: (0..len).map(f).collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The value at `at`, or the default when there is nothing there.
    pub fn get(&self, at: usize) -> T {
        self.values.get(at).cloned().unwrap_or_default()
    }

    /// Set the value at `at`. A write past the end is dropped.
    pub fn set(&mut self, at: usize, value: T) {
        if let Some(slot) = self.values.get_mut(at) {
            *slot = value;
        }
    }

    /// Change the value at `at` in place.
    pub fn update(&mut self, at: usize, f: impl FnOnce(T) -> T) {
        let next = f(self.get(at));
        self.set(at, next);
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.values.iter()
    }

    /// Every index, in order.
    pub fn indices(&self) -> std::ops::Range<usize> {
        0..self.values.len()
    }

    pub fn into_inner(self) -> Vec<T> {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_becomes_the_float_it_names() {
        assert!((as_f64(0) - 0.0).abs() < 1e-9);
        assert!((as_f64(7) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_new_table_is_all_defaults() {
        let table: Table<usize> = Table::new(3);
        assert_eq!(table.len(), 3);
        assert!(!table.is_empty());
        assert_eq!(table.get(0), 0);
        assert_eq!(table.get(2), 0);
    }

    #[test]
    fn a_table_of_nothing_is_empty() {
        let table: Table<f64> = Table::new(0);
        assert!(table.is_empty());
        assert_eq!(table.indices().count(), 0);
    }

    #[test]
    fn a_read_past_the_end_answers_with_the_default() {
        let table = Table::of(vec![7usize, 8]);
        assert_eq!(table.get(1), 8);
        assert_eq!(table.get(99), 0, "no panic, just the default");
    }

    #[test]
    fn a_write_past_the_end_is_dropped_rather_than_growing_the_table() {
        let mut table = Table::of(vec![1usize]);
        table.set(5, 9);
        assert_eq!(table.len(), 1);
        assert_eq!(table.get(0), 1);
    }

    #[test]
    fn a_value_can_be_changed_in_place() {
        let mut table = Table::of(vec![2usize, 3]);
        table.update(1, |v| v * 10);
        assert_eq!(table.get(1), 30);
        // Past the end the default is updated and the write dropped, which
        // leaves the table as it was.
        table.update(9, |v| v + 1);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn a_table_can_be_built_from_its_own_indices() {
        let table = Table::from_fn(4, |at| at * 2);
        assert_eq!(table.iter().copied().collect::<Vec<usize>>(), [0, 2, 4, 6]);
        assert_eq!(table.indices().collect::<Vec<usize>>(), [0, 1, 2, 3]);
    }

    #[test]
    fn a_table_gives_its_values_back() {
        assert_eq!(Table::of(vec![1usize, 2]).into_inner(), [1, 2]);
    }
}
