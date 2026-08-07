//! Where each row of the tree sits.
//!
//! The forest is flattened depth-first into one row per node, the way a file
//! explorer shows it: indented by depth, stacked by order. Elbow connectors are
//! derived afterwards from the glyph centres, so the drawing and the hierarchy
//! cannot disagree.

use crate::round::count;
use crate::scene::Point;

use super::types::{Tree, TreeNode};

pub const PADDING: f64 = 24.0;
pub const ROW_HEIGHT: f64 = 26.0;
/// Added per level of nesting.
pub const INDENT_WIDTH: f64 = 24.0;
/// The square slot a folder or file glyph is drawn in.
pub const GLYPH_SIZE: f64 = 16.0;
pub const GLYPH_TEXT_GAP: f64 = 8.0;
pub const DESC_GAP: f64 = 10.0;
pub const TITLE_HEIGHT: f64 = 36.0;
pub const TITLE_FONT: f64 = 16.0;
pub const LABEL_FONT: f64 = 14.0;
pub const FOLDER_WEIGHT: u32 = 600;
pub const FILE_WEIGHT: u32 = 400;
pub const DESC_FONT: f64 = 13.0;
pub const DESC_WEIGHT: u32 = 400;

/// One row: a glyph, a name, and possibly a note.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub path: String,
    pub parent_path: Option<String>,
    pub label: String,
    pub description: Option<String>,
    pub is_folder: bool,
    pub depth: usize,
    /// Top-left of the glyph slot.
    pub glyph_at: Point,
    pub text_x: f64,
    pub desc_x: f64,
    pub centre_y: f64,
}

/// One elbow from a parent down to a child.
#[derive(Debug, Clone, PartialEq)]
pub struct Connector {
    pub from: String,
    pub to: String,
    /// Down from under the parent's glyph, then across to the child's.
    pub corner: [Point; 3],
}

/// A laid-out tree view.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub rows: Vec<Row>,
    pub connectors: Vec<Connector>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// How far right a row reaches, which is what the canvas has to cover.
fn row_extent(row: &Row) -> f64 {
    let weight = if row.is_folder {
        FOLDER_WEIGHT
    } else {
        FILE_WEIGHT
    };
    let label = crate::metrics::text_width(&row.label, LABEL_FONT, weight);
    match &row.description {
        Some(text) => row.desc_x + crate::metrics::text_width(text, DESC_FONT, DESC_WEIGHT),
        None => row.text_x + label,
    }
}

/// Walk the forest depth-first, laying out one row per node.
fn visit(node: &TreeNode, parent: Option<&str>, top: f64, index: &mut usize, out: &mut Vec<Row>) {
    let centre_y = top + count(*index) * ROW_HEIGHT + ROW_HEIGHT / 2.0;
    let glyph_x = PADDING + count(node.depth) * INDENT_WIDTH;
    let text_x = glyph_x + GLYPH_SIZE + GLYPH_TEXT_GAP;
    let weight = if node.is_folder {
        FOLDER_WEIGHT
    } else {
        FILE_WEIGHT
    };
    out.push(Row {
        path: node.path.clone(),
        parent_path: parent.map(str::to_string),
        label: node.label.clone(),
        description: node.description.clone(),
        is_folder: node.is_folder,
        depth: node.depth,
        glyph_at: Point::new(glyph_x, centre_y - GLYPH_SIZE / 2.0),
        text_x,
        desc_x: text_x + crate::metrics::text_width(&node.label, LABEL_FONT, weight) + DESC_GAP,
        centre_y,
    });
    *index += 1;
    for child in &node.children {
        visit(child, Some(&node.path), top, index, out);
    }
}

/// Lay out a parsed tree view.
pub fn layout(tree: &Tree) -> Placed {
    let top = PADDING
        + if tree.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let mut rows = Vec::new();
    let mut index = 0usize;
    for root in &tree.nodes {
        visit(root, None, top, &mut index, &mut rows);
    }

    let connectors = rows
        .iter()
        .filter_map(|row| {
            let parent = row.parent_path.as_ref()?;
            let parent_y = rows.iter().find(|r| r.path == *parent)?.centre_y;
            // The vertical run sits under the middle of the parent's glyph,
            // one indent level to the left of this row.
            let guide_x =
                PADDING + count(row.depth.saturating_sub(1)) * INDENT_WIDTH + GLYPH_SIZE / 2.0;
            Some(Connector {
                from: parent.clone(),
                to: row.path.clone(),
                corner: [
                    Point::new(guide_x, parent_y + GLYPH_SIZE / 2.0),
                    Point::new(guide_x, row.centre_y),
                    Point::new(row.glyph_at.x, row.centre_y),
                ],
            })
        })
        .collect();

    let right = rows.iter().map(row_extent).fold(0.0_f64, f64::max);
    Placed {
        width: right + PADDING,
        height: top + count(rows.len()) * ROW_HEIGHT + PADDING,
        title: tree
            .title
            .clone()
            // Left-aligned rather than centred: the tree itself is a left-hand
            // list, and a centred name over it would not line up with anything.
            .map(|text| (text, Point::new(PADDING, title_baseline()))),
        rows,
        connectors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treeview::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const TREE: &str = "treeView-beta\n\
        title Project\n\
        my-project/\n    \
            src/\n        \
                index.js\n    \
            README.md";

    #[test]
    fn the_forest_flattens_depth_first_into_stacked_rows() {
        let out = placed(TREE);
        let paths: Vec<&str> = out.rows.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "my-project",
                "my-project/src",
                "my-project/src/index.js",
                "my-project/README.md",
            ]
        );
        // Each row is one row-height below the one before it.
        for pair in out.rows.windows(2) {
            assert!((pair[1].centre_y - pair[0].centre_y - ROW_HEIGHT).abs() < 1e-9);
        }
    }

    #[test]
    fn depth_shifts_a_row_right_by_one_indent() {
        let out = placed(TREE);
        assert!((out.rows[1].glyph_at.x - out.rows[0].glyph_at.x - INDENT_WIDTH).abs() < 1e-9);
        assert!(
            (out.rows[2].glyph_at.x - out.rows[0].glyph_at.x - INDENT_WIDTH * 2.0).abs() < 1e-9
        );
    }

    #[test]
    fn a_name_clears_its_glyph_and_a_note_clears_the_name() {
        let out = placed("treeView\nsrc/ ## the source");
        let row = &out.rows[0];
        assert!((row.text_x - (row.glyph_at.x + GLYPH_SIZE + GLYPH_TEXT_GAP)).abs() < 1e-9);
        let label = crate::metrics::text_width("src", LABEL_FONT, FOLDER_WEIGHT);
        assert!((row.desc_x - (row.text_x + label + DESC_GAP)).abs() < 1e-9);
    }

    #[test]
    fn a_folder_name_is_measured_bold_and_a_file_name_is_not() {
        // The two weights give different widths, so a note after a folder name
        // sits further right than the same note after a file of the same name.
        let folder = placed("treeView\nsame/ ## note");
        let file = placed("treeView\nsame ## note");
        assert!(folder.rows[0].desc_x > file.rows[0].desc_x);
    }

    #[test]
    fn every_row_but_a_root_gets_an_elbow_from_its_parent() {
        let out = placed(TREE);
        assert_eq!(out.connectors.len(), 3);
        let elbow = &out.connectors[0];
        assert_eq!(elbow.from, "my-project");
        assert_eq!(elbow.to, "my-project/src");
        // Down, then across: the first two points share an x, the last two a y.
        assert!((elbow.corner[0].x - elbow.corner[1].x).abs() < 1e-9);
        assert!((elbow.corner[1].y - elbow.corner[2].y).abs() < 1e-9);
    }

    #[test]
    fn an_elbow_starts_below_its_parents_glyph_and_ends_at_its_childs() {
        let out = placed(TREE);
        let elbow = &out.connectors[0];
        assert!((elbow.corner[0].y - (out.rows[0].centre_y + GLYPH_SIZE / 2.0)).abs() < 1e-9);
        assert!((elbow.corner[2].x - out.rows[1].glyph_at.x).abs() < 1e-9);
        assert!((elbow.corner[2].y - out.rows[1].centre_y).abs() < 1e-9);
    }

    #[test]
    fn a_forest_of_roots_has_no_connectors_at_all() {
        assert!(placed("treeView\na\nb\nc").connectors.is_empty());
    }

    #[test]
    fn the_canvas_covers_the_widest_row_including_its_note() {
        let short = placed("treeView\na");
        let noted = placed("treeView\na ## with a long explanatory note");
        assert!(noted.width > short.width);
        // Height follows the row count, not the widths.
        assert!((noted.height - short.height).abs() < 1e-9);
    }

    #[test]
    fn height_follows_the_number_of_rows() {
        let one = placed("treeView\na");
        let two = placed("treeView\na\nb");
        assert!((two.height - one.height - ROW_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn a_title_pushes_the_rows_down_and_stays_left_aligned() {
        let out = placed(TREE);
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "Project");
        assert!((at.x - PADDING).abs() < 1e-9);
        assert!((out.rows[0].centre_y - (PADDING + TITLE_HEIGHT + ROW_HEIGHT / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn an_empty_tree_is_padding_alone() {
        let out = placed("treeView");
        assert!(out.rows.is_empty());
        assert!((out.width - PADDING).abs() < 1e-9);
        assert!((out.height - PADDING * 2.0).abs() < 1e-9);
    }
}
