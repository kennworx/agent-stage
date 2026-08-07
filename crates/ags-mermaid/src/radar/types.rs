//! The parsed shape of a radar chart: axes, and a curve across them.

/// A parsed radar chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chart {
    pub title: Option<String>,
    /// The upper bound of the radial scale, when one was stated.
    pub max: Option<f64>,
    pub axes: Vec<Axis>,
    pub series: Vec<Series>,
}

/// One spoke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Axis {
    /// How a curve refers to it; unique within the chart.
    pub id: String,
    pub label: String,
}

/// One curve: a value per axis, in axis order.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub id: String,
    pub label: String,
    pub values: Vec<f64>,
}
