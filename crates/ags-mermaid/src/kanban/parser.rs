//! Reading `kanban` source.
//!
//! ```text
//! kanban
//!   title <text>
//!   todo[To do]                          a column
//!       t1[Write the parser]             a card, indented under it
//!       t2[Ship it]@{ assigned: 'me', priority: High }
//! ```
//!
//! **Indentation is the syntax here**: the first node's indent sets the column
//! level, and anything deeper is a card. So this parser reads the lines before
//! they are trimmed — trimming first would make every card a column.

use super::types::{Board, Card, Column};
use crate::keyword::{is_word, opens_with};
use crate::outline::indent_of;

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// Whether an identifier may hold `c`. Dashes are allowed; word characters are
/// not the same set here as for a keyword boundary.
fn is_id(c: char) -> bool {
    is_word(c) || c == '-'
}

/// The text after a keyword, when the line opens with it followed by a space.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if !line.get(..keyword.len())?.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let tail = line.get(keyword.len()..)?;
    if !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let text = tail.trim();
    (!text.is_empty()).then_some(text)
}

/// Read a `@{ key: value, key: 'value' }` block.
fn parse_metadata(block: &str) -> Vec<(String, String)> {
    let inner = block
        .trim()
        .strip_prefix("@{")
        .unwrap_or(block)
        .trim_end()
        .strip_suffix('}')
        .unwrap_or(block);
    let mut out = Vec::new();
    for pair in split_pairs(inner) {
        let Some((key, value)) = pair.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty()
            || !key.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            || !key.chars().all(is_id)
        {
            continue;
        }
        out.push((key.to_string(), unquote(value.trim()).trim().to_string()));
    }
    out
}

/// Split a metadata body on commas that are not inside quotes.
fn split_pairs(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in inner.chars() {
        if let Some(open) = quote {
            if c == open {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c == ',' {
            // A comma inside quotes belongs to the value, so only a bare one
            // ends the pair.
            if current.trim().is_empty() {
                current.clear();
            } else {
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(c);
    }
    if !current.trim().is_empty() {
        out.push(current);
    }
    out
}

/// What one line declares.
struct NodeLine {
    id: String,
    text: String,
    metadata: Vec<(String, String)>,
}

/// Read one already-trimmed line as a node.
fn parse_node(line: &str) -> Option<NodeLine> {
    let (head, metadata) = match line.find("@{") {
        Some(at) => (
            line.get(..at)?.trim(),
            parse_metadata(line.get(at..).unwrap_or_default()),
        ),
        None => (line, Vec::new()),
    };
    let id_end = head.find(|c: char| !is_id(c)).unwrap_or(head.len());
    let id = head.get(..id_end)?;
    if id.is_empty() {
        return None;
    }
    let rest = head.get(id_end..)?.trim();
    // `id[Label]`, or a bare id that is its own label.
    let text = if rest.is_empty() {
        id.to_string()
    } else {
        let inner = rest.strip_prefix('[')?.strip_suffix(']')?;
        unquote(inner.trim()).trim().to_string()
    };
    Some(NodeLine {
        id: id.to_string(),
        text,
        metadata,
    })
}

/// An id no other column or card has claimed.
fn unique_id(id: &str, used: &mut Vec<String>) -> String {
    if !used.iter().any(|u| u == id) {
        used.push(id.to_string());
        return id.to_string();
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{id}#{n}");
        if !used.contains(&candidate) {
            used.push(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

/// Parse a board. Reads `source` line by line **without trimming first**.
pub fn parse(source: &str) -> Board {
    let mut board = Board::default();
    let mut used: Vec<String> = Vec::new();
    // Set by the first node line; everything deeper than it is a card.
    let mut column_indent: Option<usize> = None;

    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") || opens_with(line, "kanban") {
            continue;
        }
        // A `title[…]` line declares a node called title, not a board title.
        if !line.contains('[') {
            if let Some(title) = after_keyword(line, "title") {
                board.title = Some(unquote(title).trim().to_string());
                continue;
            }
        }
        let Some(node) = parse_node(line) else {
            continue;
        };
        let indent = indent_of(raw);
        let at = *column_indent.get_or_insert(indent);

        // A card needs a column to sit in, so the first node is always one
        // however deeply it was written.
        if board.columns.is_empty() || indent <= at {
            board.columns.push(Column {
                id: unique_id(&node.id, &mut used),
                title: node.text,
                cards: Vec::new(),
            });
        } else if let Some(column) = board.columns.last_mut() {
            column.cards.push(Card {
                id: unique_id(&node.id, &mut used),
                text: node.text,
                metadata: node.metadata,
            });
        }
    }
    board
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOARD: &str = "kanban\n\
        title Sprint 12\n\
        todo[To do]\n    \
            t1[Write the parser]\n    \
            t2[Ship it]@{ assigned: 'me', priority: High }\n\
        done[Done]\n    \
            d1[Draft the plan]";

    #[test]
    fn a_whole_board_reads() {
        let board = parse(BOARD);
        assert_eq!(board.title.as_deref(), Some("Sprint 12"));
        assert_eq!(board.columns.len(), 2);
        assert_eq!(board.columns[0].title, "To do");
        assert_eq!(board.columns[0].cards.len(), 2);
        assert_eq!(board.columns[1].cards[0].text, "Draft the plan");
    }

    #[test]
    fn indentation_tells_a_card_from_a_column() {
        let board = parse("kanban\na[A]\n  c1[One]\nb[B]");
        assert_eq!(board.columns.len(), 2);
        assert_eq!(board.columns[0].cards.len(), 1);
        assert!(board.columns[1].cards.is_empty());
    }

    #[test]
    fn the_first_node_sets_the_column_level_however_deep_it_is() {
        let board = parse("kanban\n        a[A]\n            c[Card]");
        assert_eq!(board.columns.len(), 1);
        assert_eq!(board.columns[0].cards.len(), 1);
    }

    #[test]
    fn metadata_reads_with_or_without_quotes() {
        let card = &parse(BOARD).columns[0].cards[1];
        assert_eq!(
            card.metadata,
            [
                ("assigned".to_string(), "me".to_string()),
                ("priority".to_string(), "High".to_string()),
            ]
        );
    }

    #[test]
    fn a_comma_inside_a_quoted_value_does_not_split_the_pair() {
        let card =
            &parse("kanban\na[A]\n  c[C]@{ note: 'one, two', priority: Low }").columns[0].cards[0];
        assert_eq!(card.metadata[0].1, "one, two");
        assert_eq!(card.metadata.len(), 2);
    }

    #[test]
    fn only_three_keys_reach_the_drawn_line_and_in_a_fixed_order() {
        let card = &parse("kanban\na[A]\n  c[C]@{ priority: High, assigned: me, other: x }")
            .columns[0]
            .cards[0];
        assert_eq!(card.meta_line().as_deref(), Some("me · High"));
        // A card carrying nothing recognised shows no line at all.
        let bare = &parse("kanban\na[A]\n  c[C]@{ other: x }").columns[0].cards[0];
        assert_eq!(bare.meta_line(), None);
    }

    #[test]
    fn a_bare_id_is_its_own_label() {
        assert_eq!(parse("kanban\nbacklog").columns[0].title, "backlog");
    }

    #[test]
    fn every_id_is_unique_across_columns_and_cards_alike() {
        let board = parse("kanban\nx[A]\n  x[B]\n  x[C]");
        assert_eq!(board.columns[0].id, "x");
        let ids: Vec<&str> = board.columns[0]
            .cards
            .iter()
            .map(|c| c.id.as_str())
            .collect();
        assert_eq!(ids, ["x#2", "x#3"]);
    }

    #[test]
    fn a_node_called_title_is_not_the_board_title() {
        let board = parse("kanban\ntitle[A column]");
        assert_eq!(board.title, None);
        assert_eq!(board.columns[0].title, "A column");
    }

    #[test]
    fn a_comment_line_is_skipped() {
        assert!(parse("kanban\n%% a note").columns.is_empty());
    }

    #[test]
    fn nothing_in_yields_an_empty_board() {
        assert_eq!(parse(""), Board::default());
        assert_eq!(parse("kanban"), Board::default());
    }
}
