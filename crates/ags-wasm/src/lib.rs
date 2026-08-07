//! WebAssembly bindings for agent-stage: render an artifact in the browser.
//!
//! Binds the whole renderer, not just the diagram engine. A page that can draw a
//! `mermaid` block but not a `question`, a `table` or a themed `html` chunk shows a
//! reader everything except the parts an artifact exists to put in front of them.
//!
//! Thin by design: it converts at the boundary and does nothing else, so there is
//! one renderer rather than one per host, and a page and the command line cannot
//! disagree about what an artifact looks like.
//!
//! Per the workspace conventions this file stays import-only.

mod bindings;

pub use bindings::{
    block_styles, catalog, render_block, render_block_of, render_code, render_html, render_mermaid,
    render_named_page, render_note, render_page, render_question, render_svg, render_svg_themed,
    render_table, validate,
};
