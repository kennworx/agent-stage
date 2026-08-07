//! Rendering the prose between a document's fenced blocks.
//!
//! Import-only per the workspace conventions.

mod boxart;
mod render;
mod slug;

pub use boxart::has_box_drawing;
pub use render::{Heading, Prose};
pub use slug::Slugger;
