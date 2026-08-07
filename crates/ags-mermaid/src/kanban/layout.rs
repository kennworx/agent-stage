//! Where each column and card sits.
//!
//! Columns run left to right, all the same width and all the same height as the
//! tallest — a board whose containers were ragged would read as an accident.
//! Card text wraps to the column's inner width.

use crate::round::count;
use crate::scene::Point;

use super::types::{Board, Card};

pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 36.0;
pub const TITLE_FONT: f64 = 18.0;
pub const COLUMN_GAP: f64 = 18.0;
/// Between a column's border and the cards inside it.
pub const COL_PAD: f64 = 10.0;
pub const HEADER_HEIGHT: f64 = 38.0;
pub const HEADER_FONT: f64 = 14.0;
pub const HEADER_WEIGHT: u32 = 600;
pub const CARD_GAP: f64 = 10.0;
pub const CARD_PAD_X: f64 = 12.0;
pub const CARD_PAD_Y: f64 = 10.0;
pub const CARD_FONT: f64 = 13.0;
pub const CARD_WEIGHT: u32 = 500;
pub const CARD_LINE_HEIGHT: f64 = 18.0;
pub const META_FONT: f64 = 11.0;
pub const META_WEIGHT: u32 = 400;
pub const META_LINE_HEIGHT: f64 = 16.0;
pub const META_GAP: f64 = 4.0;
pub const MIN_COL_WIDTH: f64 = 170.0;
pub const MAX_COL_WIDTH: f64 = 260.0;

/// One card, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCard {
    pub id: String,
    /// The card's text, wrapped to the column's inner width.
    pub lines: Vec<String>,
    pub meta_line: Option<String>,
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// One column, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedColumn {
    pub id: String,
    pub title: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub header_height: f64,
    pub cards: Vec<PlacedCard>,
}

/// A laid-out board.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub columns: Vec<PlacedColumn>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// Break `text` to `max_width`, greedily.
///
/// A single word wider than the column is left whole rather than split
/// mid-word: an over-wide line is easier to read than a broken identifier.
fn wrap(text: &str, max_width: f64, size: f64, weight: u32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in words {
        let candidate = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || crate::metrics::text_width(&candidate, size, weight) <= max_width {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The width every column takes: enough for the widest header and card, within
/// bounds. Text longer than the maximum wraps rather than widening the board.
fn column_width(board: &Board) -> f64 {
    let mut desired = MIN_COL_WIDTH;
    for column in &board.columns {
        let header = crate::metrics::text_width(&column.title, HEADER_FONT, HEADER_WEIGHT)
            + COL_PAD * 2.0
            + 8.0;
        desired = desired.max(header);
        for card in &column.cards {
            let text = crate::metrics::text_width(&card.text, CARD_FONT, CARD_WEIGHT)
                + COL_PAD * 2.0
                + CARD_PAD_X * 2.0;
            desired = desired.max(text.min(MAX_COL_WIDTH));
        }
    }
    desired.ceil().clamp(MIN_COL_WIDTH, MAX_COL_WIDTH)
}

/// How tall one card is once its text has wrapped.
fn card_height(lines: usize, has_meta: bool) -> f64 {
    CARD_PAD_Y * 2.0
        + count(lines) * CARD_LINE_HEIGHT
        + if has_meta {
            META_GAP + META_LINE_HEIGHT
        } else {
            0.0
        }
}

/// The cards of one column, stacked from under its header.
fn stack(cards: &[Card], top: f64, card_width: f64) -> (Vec<PlacedCard>, f64) {
    let text_width = card_width - CARD_PAD_X * 2.0;
    let mut out = Vec::with_capacity(cards.len());
    let mut y = top + HEADER_HEIGHT + CARD_GAP;
    for card in cards {
        let lines = wrap(&card.text, text_width, CARD_FONT, CARD_WEIGHT);
        let meta_line = card.meta_line();
        let height = card_height(lines.len(), meta_line.is_some());
        out.push(PlacedCard {
            id: card.id.clone(),
            lines,
            meta_line,
            // The x is filled in once the column's own position is known.
            at: Point::new(0.0, y),
            width: card_width,
            height,
        });
        y += height + CARD_GAP;
    }
    // `y` already sits one gap below the last card, which gives the column an
    // even bottom inset matching its top one.
    (out, y)
}

/// Lay out a parsed board.
pub fn layout(board: &Board) -> Placed {
    let top = PADDING
        + if board.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let width_of_column = column_width(board);
    let card_width = width_of_column - COL_PAD * 2.0;

    let stacks: Vec<(Vec<PlacedCard>, f64)> = board
        .columns
        .iter()
        .map(|column| {
            let (cards, bottom) = stack(&column.cards, top, card_width);
            let content = if column.cards.is_empty() {
                top + HEADER_HEIGHT
            } else {
                bottom
            };
            (cards, content - top)
        })
        .collect();
    // Every column is as tall as the tallest, so their containers line up.
    let column_height = stacks
        .iter()
        .map(|(_, h)| *h)
        .fold(HEADER_HEIGHT + CARD_GAP, f64::max);

    let columns = board
        .columns
        .iter()
        .zip(stacks)
        .enumerate()
        .map(|(i, (column, (cards, _)))| {
            let x = PADDING + count(i) * (width_of_column + COLUMN_GAP);
            PlacedColumn {
                id: column.id.clone(),
                title: column.title.clone(),
                at: Point::new(x, top),
                width: width_of_column,
                height: column_height,
                header_height: HEADER_HEIGHT,
                cards: cards
                    .into_iter()
                    .map(|card| PlacedCard {
                        at: Point::new(x + COL_PAD, card.at.y),
                        ..card
                    })
                    .collect(),
            }
        })
        .collect();

    // An empty board is still one column wide, so it is a board rather than a
    // sliver of padding.
    let cols = board.columns.len().max(1);
    let width = PADDING * 2.0 + count(cols) * width_of_column + (count(cols) - 1.0) * COLUMN_GAP;
    Placed {
        width,
        height: top + column_height + PADDING,
        title: board
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        columns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanban::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const BOARD: &str = "kanban\n\
        title Sprint\n\
        todo[To do]\n    \
            t1[One]\n    \
            t2[Two]\n\
        done[Done]\n    \
            d1[Three]";

    #[test]
    fn columns_run_left_to_right_all_the_same_width() {
        let out = placed(BOARD);
        assert_eq!(out.columns.len(), 2);
        let pitch = out.columns[1].at.x - out.columns[0].at.x;
        assert!((pitch - (out.columns[0].width + COLUMN_GAP)).abs() < 1e-9);
        assert!((out.columns[0].width - out.columns[1].width).abs() < 1e-9);
    }

    #[test]
    fn every_column_is_as_tall_as_the_tallest() {
        let out = placed(BOARD);
        assert!((out.columns[0].height - out.columns[1].height).abs() < 1e-9);
        // And that height comes from the fuller column, not the emptier one.
        assert!(out.columns[1].height > HEADER_HEIGHT + CARD_GAP);
    }

    #[test]
    fn cards_stack_under_their_header_with_an_even_gap() {
        let out = placed(BOARD);
        let cards = &out.columns[0].cards;
        assert!((cards[0].at.y - (out.columns[0].at.y + HEADER_HEIGHT + CARD_GAP)).abs() < 1e-9);
        assert!((cards[1].at.y - (cards[0].at.y + cards[0].height + CARD_GAP)).abs() < 1e-9);
    }

    #[test]
    fn a_card_is_inset_from_its_column_on_both_sides() {
        let out = placed(BOARD);
        let (column, card) = (&out.columns[0], &out.columns[0].cards[0]);
        assert!((card.at.x - (column.at.x + COL_PAD)).abs() < 1e-9);
        assert!((card.width - (column.width - COL_PAD * 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_column_widens_for_its_content_but_only_so_far() {
        let narrow = placed("kanban\na[A]");
        assert!((narrow.columns[0].width - MIN_COL_WIDTH).abs() < 1e-9);
        let wide = placed(
            "kanban\na[A]\n  c[A card whose text is far too long to fit on any single line]",
        );
        assert!((wide.columns[0].width - MAX_COL_WIDTH).abs() < 1e-9);
    }

    #[test]
    fn text_too_wide_for_a_column_wraps_and_makes_the_card_taller() {
        let out = placed(
            "kanban\na[A]\n  c[A card whose text is far too long to fit on any single line]",
        );
        let card = &out.columns[0].cards[0];
        assert!(card.lines.len() > 1);
        assert!((card.height - card_height(card.lines.len(), false)).abs() < 1e-9);
    }

    #[test]
    fn a_word_wider_than_the_column_is_left_whole() {
        // Breaking it would split an identifier, which is worse than an
        // over-wide line.
        let lines = wrap(
            "Supercalifragilisticexpialidocious",
            10.0,
            CARD_FONT,
            CARD_WEIGHT,
        );
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn a_metadata_line_makes_its_card_taller() {
        let plain = placed("kanban\na[A]\n  c[C]");
        let noted = placed("kanban\na[A]\n  c[C]@{ assigned: me }");
        let extra = noted.columns[0].cards[0].height - plain.columns[0].cards[0].height;
        assert!((extra - (META_GAP + META_LINE_HEIGHT)).abs() < 1e-9);
    }

    #[test]
    fn a_column_with_no_cards_is_still_a_column() {
        let out = placed("kanban\nempty[Nothing here]");
        assert_eq!(out.columns.len(), 1);
        assert!(out.columns[0].cards.is_empty());
        assert!((out.columns[0].height - (HEADER_HEIGHT + CARD_GAP)).abs() < 1e-9);
    }

    #[test]
    fn an_empty_board_is_still_one_column_wide() {
        let out = placed("kanban");
        assert!(out.columns.is_empty());
        assert!((out.width - (PADDING * 2.0 + MIN_COL_WIDTH)).abs() < 1e-9);
    }

    #[test]
    fn a_title_pushes_the_columns_down_and_centres_itself() {
        let out = placed(BOARD);
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "Sprint");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
        assert!((out.columns[0].at.y - (PADDING + TITLE_HEIGHT)).abs() < 1e-9);
    }
}
