//! XY charts: bars and smoothed curves against a pair of axes.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{
    layout, nice_ticks, Align, AxisTitle, Bar, Curve, LegendItem, Placed, PlacedAxis, Plot, Tick,
    Vertex,
};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{format_tick, Axis, Chart, Range, Series, SeriesKind};
