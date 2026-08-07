//! Reading `mindmap` source.
//!
//! ```text
//! mindmap
//!   Root
//!     id[Square]      (Round)      ((Circle))
//!     ))Bang((        )Cloud(      {{Hexagon}}
//!     Plain text      ::icon(fa fa-book)   :::className
//! ```
//!
//! **Indentation is the syntax here**, so this parser reads the lines before
//! they are trimmed.

use super::types::{Mindmap, Node, Shape};
use crate::keyword::opens_with;
use crate::outline::{attach, indent_of, level_len, Counted, Outline};

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// The delimiter pairs, longest first so `((x))` never reads as `(` … `)`.
const SHAPES: [(Shape, &str, &str); 6] = [
    (Shape::Circle, "((", "))"),
    (Shape::Bang, "))", "(("),
    (Shape::Hexagon, "{{", "}}"),
    (Shape::Cloud, ")", "("),
    (Shape::Square, "[", "]"),
    (Shape::Round, "(", ")"),
];

/// Drop a trailing `::icon(...)` or `:::class`, neither of which is drawn.
fn strip_decorations(line: &str) -> &str {
    let mut text = line.trim();
    if let Some(at) = text.rfind("::icon(") {
        if text.ends_with(')') {
            text = text.get(..at).unwrap_or(text).trim_end();
        }
    }
    if let Some(at) = text.rfind(":::") {
        let name = text.get(at + 3..).unwrap_or_default();
        if !name.is_empty() && !name.contains(char::is_whitespace) {
            text = text.get(..at).unwrap_or(text).trim_end();
        }
    }
    text
}

/// What one line declares.
struct Entry {
    label: String,
    shape: Shape,
}

/// Read one already-trimmed line.
fn parse_entry(line: &str) -> Option<Entry> {
    let text = strip_decorations(line);
    if text.is_empty() {
        return None;
    }
    for (shape, open, close) in SHAPES {
        if text.len() <= open.len() + close.len() || !text.ends_with(close) {
            continue;
        }
        let Some(inner) = text.get(..text.len() - close.len()) else {
            continue;
        };
        let Some(at) = inner.find(open) else { continue };
        let label = inner.get(at + open.len()..).unwrap_or_default().trim();
        // An empty pair is not a shape; fall through to the next candidate.
        if label.is_empty() {
            continue;
        }
        return Some(Entry {
            label: unquote(label).to_string(),
            shape,
        });
    }
    Some(Entry {
        label: unquote(text).to_string(),
        shape: Shape::Default,
    })
}

/// A node while the tree is being built.
struct Raw {
    entry: Entry,
    children: Vec<Raw>,
}

impl Outline for Raw {
    fn children_mut(&mut self) -> &mut Vec<Self> {
        &mut self.children
    }
}

impl Counted for Raw {
    fn children(&self) -> &[Self] {
        &self.children
    }
}

/// An id no other node has claimed.
fn unique_id(base: &str, used: &mut Vec<String>) -> String {
    let base = if base.is_empty() { "node" } else { base };
    if !used.iter().any(|u| u == base) {
        used.push(base.to_string());
        return base.to_string();
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{base}-{n}");
        if !used.contains(&candidate) {
            used.push(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

/// Turn a built node into its finished form.
fn finish(raw: Raw, depth: usize, used: &mut Vec<String>) -> Node {
    Node {
        id: unique_id(&raw.entry.label, used),
        label: raw.entry.label,
        shape: raw.entry.shape,
        depth,
        children: raw
            .children
            .into_iter()
            .map(|child| finish(child, depth + 1, used))
            .collect(),
    }
}

/// Parse a mindmap. Reads `source` line by line **without trimming first**.
pub fn parse(source: &str) -> Mindmap {
    let mut roots: Vec<Raw> = Vec::new();
    let mut open: Vec<(usize, usize)> = Vec::new();

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("%%") || opens_with(line, "mindmap") {
            continue;
        }
        let Some(entry) = parse_entry(line) else {
            continue;
        };
        let indent = indent_of(raw_line);
        while open.last().is_some_and(|(_, at)| *at >= indent) {
            open.pop();
        }
        let path: Vec<usize> = open.iter().map(|(i, _)| *i).collect();
        let index = level_len(&roots, &path);
        attach(
            &mut roots,
            &path,
            Raw {
                entry,
                children: Vec::new(),
            },
        );
        open.push((index, indent));
    }

    let mut used = Vec::new();
    // One root is the centre of the map; several become a nameless container,
    // which the layout lays out as a stack rather than around a pivot.
    let root = if roots.len() == 1 {
        roots
            .pop()
            .map(|root| finish(root, 0, &mut used))
            .unwrap_or_default()
    } else {
        Node {
            children: roots
                .into_iter()
                .map(|root| finish(root, 1, &mut used))
                .collect(),
            ..Node::default()
        }
    };
    Mindmap { root }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_nodes_with_one_label_get_ids_a_reader_can_tell_apart() {
        fn walk(node: &Node, out: &mut Vec<String>) {
            out.push(node.id.clone());
            for child in &node.children {
                walk(child, out);
            }
        }
        // An anchor names a node, so two nodes cannot answer to one id — and a
        // mindmap repeating a word is ordinary rather than a mistake.
        let tree = parse("mindmap\n  root\n    Plan\n    Plan\n    Plan");
        let mut ids = Vec::new();
        walk(&tree.root, &mut ids);
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
        assert!(ids.iter().any(|id| id == "Plan-2"), "{ids:?}");
    }

    #[test]
    fn a_node_with_no_label_still_gets_an_id() {
        assert_eq!(unique_id("", &mut Vec::new()), "node");
    }

    const MAP: &str = "mindmap\n\
        root((Mindmap))\n  \
          Origins\n    \
            Long history\n  \
          Research\n    \
            id[On effectiveness]";

    fn labels(node: &Node, out: &mut Vec<(usize, String)>) {
        out.push((node.depth, node.label.clone()));
        for child in &node.children {
            labels(child, out);
        }
    }

    fn flat(source: &str) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        labels(&parse(source).root, &mut out);
        out
    }

    #[test]
    fn indentation_builds_the_tree() {
        assert_eq!(
            flat(MAP),
            [
                (0, "Mindmap".to_string()),
                (1, "Origins".to_string()),
                (2, "Long history".to_string()),
                (1, "Research".to_string()),
                (2, "On effectiveness".to_string()),
            ]
        );
    }

    #[test]
    fn every_shape_reads() {
        let shapes: Vec<Shape> = parse(
            "mindmap\nr\n a[Square]\n (Round)\n ((Circle))\n ))Bang((\n )Cloud(\n {{Hexagon}}\n Plain",
        )
        .root
        .children
        .iter()
        .map(|c| c.shape)
        .collect();
        assert_eq!(
            shapes,
            [
                Shape::Square,
                Shape::Round,
                Shape::Circle,
                Shape::Bang,
                Shape::Cloud,
                Shape::Hexagon,
                Shape::Default,
            ]
        );
    }

    #[test]
    fn a_longer_delimiter_wins_over_a_prefix_of_itself() {
        // `((x))` would otherwise read as a round node labelled `(x)`.
        let node = &parse("mindmap\n((Circle))").root;
        assert_eq!(node.shape, Shape::Circle);
        assert_eq!(node.label, "Circle");
    }

    #[test]
    fn the_bracket_prefix_is_punctuation_and_the_label_is_the_identity() {
        let node = &parse("mindmap\nsomeId[The label]").root;
        assert_eq!(node.label, "The label");
        assert_eq!(node.id, "The label");
    }

    #[test]
    fn two_nodes_with_the_same_label_are_still_told_apart() {
        let map = parse("mindmap\nroot\n same\n same");
        let ids: Vec<&str> = map.root.children.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["same", "same-2"]);
    }

    #[test]
    fn an_icon_or_class_decoration_is_stripped() {
        assert_eq!(parse("mindmap\nBook ::icon(fa fa-book)").root.label, "Book");
        assert_eq!(parse("mindmap\nThing :::urgent").root.label, "Thing");
    }

    #[test]
    fn several_roots_become_a_nameless_container() {
        let map = parse("mindmap\nfirst\nsecond");
        assert_eq!(map.root.label, "");
        assert_eq!(map.root.children.len(), 2);
        // Their depth starts at one, because the container is depth zero.
        assert_eq!(map.root.children[0].depth, 1);
    }

    #[test]
    fn a_comment_line_is_skipped() {
        assert_eq!(parse("mindmap\n%% a note\nroot").root.label, "root");
    }

    #[test]
    fn nothing_in_yields_an_empty_map() {
        assert_eq!(parse(""), Mindmap::default());
        assert_eq!(parse("mindmap"), Mindmap::default());
    }
}
