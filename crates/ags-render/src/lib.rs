//! An agent-authored markdown artifact, rendered to a page a human can review.
//!
//! Prose is full GitHub Flavored Markdown, and every one of the closed set of
//! block types is drawn: a diagram to SVG, a question to a form, a table, a code
//! excerpt, a callout, a themed HTML chunk. What comes out is a finished page —
//! either the served review page or a standalone baked file.
//!
//! Validation lives here too, because it is the same reading: [`parse_artifact`] →
//! [`validate`] (or the combined [`validate_source`]) → [`errors_to_toon`], and the
//! parse that decides an artifact is well-formed is the parse that draws it.
//!
//! It knows what feedback *is* — [`ags_feedback`] supplies the model, and the page
//! renders recorded items into it — but nothing here stores or transports any. The
//! dependency runs one way: this crate answers whether an anchor still resolves
//! ([`anchors`]) and what a drawing gets wrong ([`render_findings`]); the feedback
//! crate carries those answers without knowing how they were reached.
//!
//! Per the workspace conventions, this file stays import-only: it declares
//! modules and re-exports their public surface. Logic (and its tests) lives in
//! named sibling submodules.

mod affordances;
mod anchors;
mod block;
mod catalog;
mod findings;
mod html;
mod page;
mod parse;
mod prose;
mod style;
mod toon;
mod validate;

pub use affordances::{affordance_summaries, affordances, Affordance, AffordanceSpec};
pub use anchors::{anchors, Anchors};
pub use block::{Attr, AttrValue, Block, ValidationError, ValidationKind};
pub use catalog::block_catalog;
pub use findings::{finding_updates, render_findings};
pub use page::{
    anchor_for, bake, bake_named, block_anchor_id, ended_notice, note_card, parse_theme,
    render_one, render_typed, resolve_theme, review, segments, styles, theme_css, Segment, Theme,
    ThemeMode,
};
pub use parse::{parse_artifact, Artifact};
pub use prose::{has_box_drawing, Heading, Prose, Slugger};
pub use toon::errors_to_toon;
pub use validate::{validate, validate_source};
