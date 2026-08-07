//! Assembling an artifact into a page.
//!
//! Import-only per the workspace conventions.

mod blocks;
mod document;
mod review;
mod segment;
mod theme;

pub use blocks::{block_anchor_id, render_one, render_typed};
pub use document::{bake, bake_named, styles};
pub use review::{anchor_for, ended_notice, note_card, review};
pub use segment::{segments, Segment};
pub use theme::{
    css as theme_css, parse as parse_theme, resolve as resolve_theme, Mode as ThemeMode, Theme,
};
