//! Mermaid diagram rendering.
//!
//! Diagram text in, SVG text out. Nothing here reads a filesystem, consults a
//! clock, or starts a thread, so the same code serves three targets: rendered
//! ahead of time on a server, compiled to WebAssembly and run in a browser, or
//! driven from a command line to produce a standalone image.
//!
//! Panics are not a failure mode this library is allowed to have. In a browser a
//! panic aborts the WebAssembly instance and takes the page with it, so
//! malformed input returns an error instead.
//!
//! Per the workspace conventions this file stays import-only.

mod api;
pub mod architecture;
pub mod block;
pub mod c4;
pub mod class;
mod color;
mod constraint;
mod detect;
mod emit;
pub mod er;
pub mod eventmodeling;
pub mod flowchart;
pub mod gantt;
pub mod gitgraph;
mod hover;
mod icons;
pub mod ishikawa;
pub mod journey;
pub mod kanban;
mod keyword;
mod label;
pub mod layout;
mod metrics;
pub mod mindmap;
mod outline;
pub mod packet;
pub mod pie;
pub mod quadrant;
pub mod radar;
#[cfg(feature = "raster")]
mod raster;
pub mod requirement;
mod round;
pub mod sankey;
mod scene;
pub mod sequence;
mod text;
mod theme;
pub mod timeline;
mod tokens;
pub mod treemap;
pub mod treeview;
pub mod venn;
pub mod wardley;
pub mod xychart;
pub mod zenuml;

pub use api::{inspect, render_svg, ColorMode, Measure, Options, RenderError, Rendered};
pub use color::{is_dark_background, is_valid_hex, mix_hex, series_color, CHART_ACCENT_FALLBACK};
pub use constraint::{check, Violation};
pub use detect::{detect, Detection, DiagramType};
pub use emit::svg;
pub use icons::{has_icon, icon};
pub use label::{beside, Placed as PlacedLabel};
pub use metrics::{mono_text_width, text_width};
#[cfg(feature = "raster")]
pub use raster::{png, RasterError};
pub use round::{coord, round_half_up};
pub use scene::{
    Anchor, Color, Content, Font, Layer, Marker, Node, Paint, Point, Role, Scene, Seg, Shape, Size,
    TextRun, Transform,
};
pub use text::{escape_xml, normalize_label, strip_formatting_tags, wrap};
pub use theme::{series_css, style_block, Theme};
