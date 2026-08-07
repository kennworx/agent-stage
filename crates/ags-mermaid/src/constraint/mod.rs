//! Legibility checks over a laid-out scene.
//!
//! Every rule here is a defect that shipped while passing whatever checks were
//! in place at the time, and was caught by someone looking at the picture. They
//! run against the geometry the layout stage computed — never against emitted
//! SVG, because re-parsing a path attribute is how a diagram with twenty-three
//! edge crossings was measured as having four.
//!
//! Checks select on [`Role`](crate::scene::Role), which is why role is a field of
//! its own rather than being folded into paint order: an icon must not answer a
//! question asked about boxes.
//!
//! Import-only per the workspace conventions.

mod areas;
mod check;
mod edges;
mod report;
mod scene;

#[cfg(test)]
mod fixture;

pub use check::check;
pub use report::Violation;
