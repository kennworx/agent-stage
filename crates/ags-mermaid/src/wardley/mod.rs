//! Wardley maps: components placed by visibility and evolution.
//!
//! Import-only per the workspace conventions.

mod layout;
mod parser;
mod render;
mod types;

pub use layout::{layout, AxisLabel, Placed, PlacedComponent, PlacedLink, Rect};
pub use parser::parse;
pub use render::{render, scene};
pub use types::{Component, Kind, Link, Map, Style};
