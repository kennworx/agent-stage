//! The parsed shape of a pie chart: a circle split into proportional wedges.

/// A parsed pie chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chart {
    pub title: Option<String>,
    /// When set, the legend shows raw values alongside labels (`pie showData`).
    pub show_data: bool,
    pub slices: Vec<Slice>,
}

/// One wedge, before it has an angle.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub label: String,
    pub value: f64,
}
