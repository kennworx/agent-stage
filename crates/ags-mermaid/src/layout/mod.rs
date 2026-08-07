//! Layered graph layout — Sugiyama's method.
//!
//! Boxes and arrows in, coordinates out. Nothing under this module knows about
//! diagrams, themes, labels or SVG, and that separation is the point: a layout
//! bug otherwise shows up as a picture, which is the worst possible place to
//! debug one. Kept apart, the properties that matter are assertable directly —
//! no two nodes overlap, every edge runs the way the layout does, the same
//! input twice gives the same output — without rendering anything.
//!
//! Five passes, in order: break the cycles, assign the layers, order within
//! them, place, route. Each is its own submodule and each is testable alone.
//!
//! Import-only per the workspace conventions.

mod align;
mod channel;
mod cycles;
mod layers;
mod order;
mod place;
mod ports;
mod pull;
mod route;
mod run;
mod table;
mod types;

pub use cycles::{break_cycles, Acyclic, Arc};
pub use layers::{assign_layers, Layering, LayoutNode};
pub use order::{crossings, order_layers};
pub use place::{depth, extent, layer_tops, place};
pub use route::{route, route_loops, Placement};
pub use run::layout;
pub use table::{as_f64, Table};
pub use types::{
    Direction, Edge, Graph, Node, Placed, PlacedEdge, PlacedNode, Point, Port, Spacing,
};
