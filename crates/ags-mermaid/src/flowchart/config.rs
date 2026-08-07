//! The measurements a flowchart is built from.
//!
//! Values rather than constants, so a caller can move one without editing the
//! renderer. A drawing embedded in a dense page and the same drawing exported on
//! its own want different type sizes, and a hard-coded number offers no way to
//! say so short of a fork.
//!
//! Gathered in one struct because layout and rendering must agree: a box drawn to
//! a different padding than it was *sized* to is a box whose text overflows it.
//! Passing one value through both stages is what makes that impossible rather
//! than merely unlikely.
//!
//! [`Config::default`] is the drawing this crate has always produced, so a caller
//! that says nothing gets exactly what it got before.

/// Everything a flowchart's geometry derives from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// Type for a node's own label.
    pub label_font: f64,
    pub label_weight: u32,
    /// Type for the words on an edge.
    ///
    /// An edge label carries as much meaning as a box does — `yes`/`no` on a
    /// decision is the whole reason the branch reads — so it is set only slightly
    /// smaller than a node's label and at the same weight. Set lighter than this
    /// and it is legible on a diff but not on a screen.
    pub edge_label_font: f64,
    pub edge_label_weight: u32,
    /// How much taller a line of text is than the type it is set in.
    pub line_height: f64,
    /// Space between a node's label and its outline.
    pub pad_x: f64,
    pub pad_y: f64,
    /// A diamond wastes its corners, so it needs more room than its text implies.
    pub diamond_extra: f64,
    /// The smallest a node may be drawn, whatever its label measures.
    pub min_width: f64,
    pub min_height: f64,
    /// Room reserved at an edge's end for the arrowhead.
    pub marker_size: f64,
    /// Space between a subgraph's frame and what it holds.
    pub group_pad: f64,
    /// Height of the band a subgraph's caption sits in.
    pub group_header: f64,
    pub group_label_font: f64,
    pub group_label_weight: u32,
    /// Inset of a subgraph's caption from its frame's left edge.
    pub group_label_pad_x: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            label_font: 13.0,
            label_weight: 500,
            edge_label_font: 12.0,
            edge_label_weight: 500,
            line_height: 1.3,
            pad_x: 20.0,
            pad_y: 10.0,
            diamond_extra: 24.0,
            min_width: 60.0,
            min_height: 36.0,
            marker_size: 28.0,
            group_pad: 14.0,
            group_header: 24.0,
            group_label_font: 12.0,
            group_label_weight: 600,
            group_label_pad_x: 10.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_drawing_this_crate_has_always_made() {
        // Pinned, because `Default` is the whole compatibility promise: a caller
        // that passes nothing must get what it got when these were constants.
        let c = Config::default();
        assert!((c.label_font - 13.0).abs() < f64::EPSILON);
        assert_eq!(c.label_weight, 500);
        assert!((c.edge_label_font - 12.0).abs() < f64::EPSILON);
        assert_eq!(c.edge_label_weight, 500);
        assert!((c.line_height - 1.3).abs() < f64::EPSILON);
        assert!((c.pad_x - 20.0).abs() < f64::EPSILON);
        assert!((c.pad_y - 10.0).abs() < f64::EPSILON);
        assert!((c.diamond_extra - 24.0).abs() < f64::EPSILON);
        assert!((c.min_width - 60.0).abs() < f64::EPSILON);
        assert!((c.min_height - 36.0).abs() < f64::EPSILON);
        assert!((c.marker_size - 28.0).abs() < f64::EPSILON);
        assert!((c.group_pad - 14.0).abs() < f64::EPSILON);
        assert!((c.group_header - 24.0).abs() < f64::EPSILON);
        assert!((c.group_label_font - 12.0).abs() < f64::EPSILON);
        assert_eq!(c.group_label_weight, 600);
        assert!((c.group_label_pad_x - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_config_can_be_moved_without_touching_the_renderer() {
        let bigger = Config {
            label_font: 20.0,
            ..Config::default()
        };
        assert!((bigger.label_font - 20.0).abs() < f64::EPSILON);
        // Everything it did not name is unchanged.
        assert!((bigger.pad_x - Config::default().pad_x).abs() < f64::EPSILON);
        assert_ne!(bigger, Config::default());
    }
}
