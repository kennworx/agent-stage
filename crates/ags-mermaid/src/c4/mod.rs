//! C4 diagrams: context, container, component, dynamic and deployment.
//!
//! Import-only per the workspace conventions.

pub mod config;
mod geom;
mod labels;
mod lattice;
mod layout;
mod nudge;
mod pack;
mod parser;
mod place;
mod ports;
mod positioned;
mod quality;
mod render;
mod style;
mod types;

pub use geom::{Point, Rect, Side};
pub use layout::layout;
pub use parser::parse;
pub use place::kind_tag;
pub use positioned::{Placed, PlacedBoundary, PlacedElement, PlacedRelationship};
pub use render::scene;
pub use types::{
    Boundary, BoundaryKind, Diagram, Element, ElementKind, LayoutConfig, RelDirection,
    Relationship, Variant,
};
