//! The CSS a class drawing carries with it.
//!
//! Its own module, as C4's is: a stylesheet is a different kind of thing from
//! the code that builds the picture, and keeping the two apart is what lets the
//! hover pairing be read as one list of rules rather than found among them.
//!
//! Every colour is a theme token, so a page restyles the diagram by changing one
//! variable and nothing is re-rendered.

use crate::api::ColorMode;
use crate::theme::{style_block, Theme};

/// The rules a class diagram needs on top of the shared tokens, plus one hover
/// pair for each relationship that has something written on it.
pub(super) fn style(theme: &Theme, mode: &ColorMode, labelled: &[usize]) -> String {
    format!(
        "{}\
         .class-node rect{{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}}\
         .class-node .class-header{{fill:var(--_group-hdr)}}\
         .class-rule{{stroke:var(--_node-stroke);stroke-width:0.75}}\
         .class-name{{fill:var(--_text)}}\
         .class-annotation{{fill:var(--_text-muted)}}\
         .class-member{{font-family:'JetBrains Mono','SF Mono','Fira Code',ui-monospace,monospace}}\
         .class-vis{{fill:var(--_text-faint)}}\
         .class-member-name{{fill:var(--_text-sec)}}\
         .class-type{{fill:var(--_text-muted)}}\
         .class-static{{text-decoration:underline}}\
         .class-relationship polyline{{fill:none;stroke:var(--_line);stroke-width:1}}\
         .class-dependency polyline,.class-realization polyline{{stroke-dasharray:6 4}}\
         .class-edge-label{{fill:var(--_text-muted)}}\
         text{{font-family:Inter,system-ui,sans-serif}}{}",
        style_block(theme, mode),
        crate::hover::pairs(labelled)
    )
}
