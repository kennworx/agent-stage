//! Quadrant charts: a square split in four, with points plotted in it.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, AxisLabel, Placed, PlacedPoint, Rect, Region};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Axis, Chart, DataPoint, Quadrants};
