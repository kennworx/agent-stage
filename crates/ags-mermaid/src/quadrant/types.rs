//! The parsed shape of a quadrant chart: a unit square, and points inside it.

/// A parsed quadrant chart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Chart {
    pub title: Option<String>,
    /// The two ends of the horizontal axis, low to high.
    pub x_axis: Axis,
    /// The two ends of the vertical axis, low to high.
    pub y_axis: Axis,
    pub quadrants: Quadrants,
    pub points: Vec<DataPoint>,
}

/// The names given to an axis's ends, either of which may be unstated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Axis {
    pub low: Option<String>,
    pub high: Option<String>,
}

/// The names given to the four regions.
///
/// Numbered as the syntax numbers them, anticlockwise from the top right, which
/// is not the order anything is drawn in — hence the doc comment on each.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quadrants {
    /// Top right.
    pub q1: Option<String>,
    /// Top left.
    pub q2: Option<String>,
    /// Bottom left.
    pub q3: Option<String>,
    /// Bottom right.
    pub q4: Option<String>,
}

/// One plotted point, in the unit square.
#[derive(Debug, Clone, PartialEq)]
pub struct DataPoint {
    pub name: String,
    /// 0 at the left edge, 1 at the right.
    pub x: f64,
    /// 0 at the bottom edge, 1 at the top.
    pub y: f64,
}
