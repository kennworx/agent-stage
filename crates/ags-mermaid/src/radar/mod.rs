//! Radar charts: a value per axis, traced as a closed area.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, LegendRow, Placed, PlacedAxis, PlacedPoint, PlacedSeries};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Axis, Chart, Series};
