//! Gantt charts: dated bars on a day axis, grouped into sections.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, GridLine, Placed, PlacedSection, PlacedTask, Rect};
pub use parser::{add_days, parse};
pub use render::{render, scene};
pub use types::{Chart, Section, Status, Task};
