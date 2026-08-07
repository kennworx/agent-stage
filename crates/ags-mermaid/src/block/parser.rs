//! Reading `block-beta` source.
//!
//! ```text
//! block-beta
//!   columns 4                       the grid width
//!   Id["Label"] space Other         one row of cells, filled left to right
//!   A --> B                         a wire between two cells
//! ```
//!
//! Every token on a row consumes a cell, including `space` — which is the only
//! way to leave a hole in the grid, so it has to take a slot to make one.

use super::types::{Block, Diagram, Edge};
use crate::keyword::{is_word, opens_with};

/// The run of word characters starting at `from`, and where it ends.
fn word_at(chars: &[char], from: usize) -> (String, usize) {
    let mut end = from;
    while chars.get(end).is_some_and(|c| is_word(*c)) {
        end += 1;
    }
    (
        chars.get(from..end).unwrap_or_default().iter().collect(),
        end,
    )
}

/// A `columns N` line, if that is what this is.
fn parse_columns(line: &str) -> Option<usize> {
    if !opens_with(line, "columns") {
        return None;
    }
    let n: usize = line.get("columns".len()..)?.trim().parse().ok()?;
    // A grid no cells wide has nowhere to put a block.
    Some(n.max(1))
}

/// An `A --> B` line, if that is what this is.
fn parse_edge(line: &str) -> Option<Edge> {
    let chars: Vec<char> = line.chars().collect();
    let (source, after) = word_at(&chars, 0);
    if source.is_empty() {
        return None;
    }
    let rest: String = chars.get(after..).unwrap_or_default().iter().collect();
    let tail = rest.trim_start().strip_prefix("-->")?;
    let target_chars: Vec<char> = tail.trim_start().chars().collect();
    let (target, _) = word_at(&target_chars, 0);
    if target.is_empty() {
        return None;
    }
    Some(Edge { source, target })
}

/// One cell token: an id, and the label it was written with.
struct Cell {
    id: String,
    label: Option<String>,
}

/// The cells on a row, left to right.
///
/// A token is `Id["Label"]` or a bare `Id`; anything between tokens is skipped.
/// An `Id[` with no closing `"]` falls back to the bare form rather than
/// swallowing the rest of the line, which is what the reference's backtracking
/// does with the same input.
fn cells(line: &str) -> Vec<Cell> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        if !chars.get(i).is_some_and(|c| is_word(*c)) {
            i += 1;
            continue;
        }
        let (id, after) = word_at(&chars, i);
        let bracketed = chars.get(after) == Some(&'[') && chars.get(after + 1) == Some(&'"');
        if bracketed {
            if let Some(close) = (after + 2..chars.len())
                .find(|&j| chars.get(j) == Some(&'"') && chars.get(j + 1) == Some(&']'))
            {
                let label: String = chars
                    .get(after + 2..close)
                    .unwrap_or_default()
                    .iter()
                    .collect();
                out.push(Cell {
                    id,
                    label: Some(label),
                });
                i = close + 2;
                continue;
            }
        }
        out.push(Cell { id, label: None });
        i = after;
    }
    out
}

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

/// Parse a block diagram. A line that matches nothing is still a row.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    let mut row = 0usize;
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if opens_with(line, "block-beta") || opens_with(line, "block") {
            continue;
        }
        if let Some(columns) = parse_columns(line) {
            diagram.columns = columns;
            continue;
        }
        if let Some(edge) = parse_edge(line) {
            diagram.edges.push(edge);
            continue;
        }
        for (col, cell) in cells(line).into_iter().enumerate() {
            // `space` holds a slot open without drawing anything. Only the bare
            // form is a hole: `space["Gap"]` names a block that happens to be
            // called space.
            let hole = cell.label.is_none() && cell.id.eq_ignore_ascii_case("space");
            if !hole {
                let label = cell.label.unwrap_or_else(|| cell.id.clone());
                diagram.blocks.push(Block {
                    id: cell.id,
                    label,
                    col,
                    row,
                });
            }
        }
        row += 1;
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(source: &str) -> Vec<(String, String, usize, usize)> {
        parse(source)
            .blocks
            .into_iter()
            .map(|b| (b.id, b.label, b.col, b.row))
            .collect()
    }

    #[test]
    fn a_row_fills_its_cells_left_to_right() {
        assert_eq!(
            placed("block-beta\nA[\"First\"] B[\"Second\"]"),
            [
                ("A".into(), "First".into(), 0, 0),
                ("B".into(), "Second".into(), 1, 0),
            ]
        );
    }

    #[test]
    fn a_space_holds_its_slot_open_without_drawing() {
        let blocks = placed("block-beta\nA space B");
        assert_eq!(blocks.len(), 2);
        // B is in the third cell, not the second — that is the whole point.
        assert_eq!(blocks[1].2, 2);
    }

    #[test]
    fn a_bare_id_is_its_own_label() {
        assert_eq!(
            placed("block-beta\nAlpha"),
            [("Alpha".into(), "Alpha".into(), 0, 0)]
        );
    }

    #[test]
    fn rows_advance_only_on_cell_lines() {
        let blocks = placed("block-beta\ncolumns 2\nA B\nA --> B\nC D");
        assert_eq!(blocks[2].3, 1, "the edge line is not a row: {blocks:?}");
    }

    #[test]
    fn columns_reads_and_never_falls_below_one() {
        assert_eq!(parse("block-beta\ncolumns 4").columns, 4);
        assert_eq!(parse("block-beta\ncolumns 0").columns, 1);
        // An unstated width is a single column.
        assert_eq!(parse("block-beta").columns, 1);
    }

    #[test]
    fn an_edge_reads_by_id_with_any_spacing() {
        assert_eq!(
            parse("block-beta\nA-->B\nC --> D").edges,
            [
                Edge {
                    source: "A".into(),
                    target: "B".into()
                },
                Edge {
                    source: "C".into(),
                    target: "D".into()
                },
            ]
        );
    }

    #[test]
    fn a_half_written_wire_is_a_row_of_cells_rather_than_an_edge() {
        // Each of these fails the edge shape at a different point, and each has
        // to fall through to the cell reader rather than being dropped.
        assert!(parse("block-beta\nA -->").edges.is_empty());
        assert!(parse("block-beta\n--> B").edges.is_empty());
        assert!(parse("block-beta\nA -- B").edges.is_empty());
        // Which means the words on those lines are still blocks.
        assert_eq!(placed("block-beta\nA -->")[0].0, "A");
    }

    #[test]
    fn a_named_block_called_space_is_still_a_block() {
        assert_eq!(placed("block-beta\nspace[\"Gap\"]").len(), 1);
    }

    #[test]
    fn an_unclosed_label_falls_back_to_the_bare_id() {
        // Two tokens, because the bracket never closes: the id, then the word
        // inside it. Matching the reference matters more than being tidy here.
        let blocks = placed("block-beta\nA[\"unterminated");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "A");
        assert_eq!(blocks[1].0, "unterminated");
    }

    #[test]
    fn a_comment_is_stripped_before_the_line_is_read() {
        assert_eq!(placed("block-beta\nA B %% two cells").len(), 2);
        assert!(placed("block-beta\n%% nothing here").is_empty());
    }

    #[test]
    fn the_header_is_skipped_but_a_word_starting_with_block_is_not() {
        assert!(parse("block-beta").blocks.is_empty());
        assert!(parse("block").blocks.is_empty());
        assert_eq!(placed("block-beta\nblocks")[0].0, "blocks");
    }

    #[test]
    fn nothing_in_yields_an_empty_grid() {
        assert_eq!(parse(""), Diagram::default());
    }
}
