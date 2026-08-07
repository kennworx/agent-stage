//! Reading `treemap` source.
//!
//! ```text
//! treemap[-beta]
//!   title <text>
//!   "Branch"                 a node with no value of its own
//!       "Leaf" : 42          a leaf carries one
//!       "Leaf" : 42:::cls    a highlight class, accepted and ignored
//! ```
//!
//! **Indentation is the syntax here**, so this parser reads the lines before
//! they are trimmed. A branch's value is the sum of what it holds, computed
//! afterwards — writing one on a branch would be ignored.

use super::types::{Node, Treemap};
use crate::keyword::{is_word, opens_with};
use crate::outline::{attach, indent_of, level_len, Counted, Outline};

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
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

/// Whether `token` is a number in the form the syntax allows.
fn is_number(token: &str) -> bool {
    let body = token.strip_prefix('-').unwrap_or(token);
    let mut parts = body.splitn(2, '.');
    let whole = parts.next().unwrap_or_default();
    if !whole.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        Some(frac) => !frac.is_empty() && frac.chars().all(|c| c.is_ascii_digit()),
        None => !whole.is_empty(),
    }
}

/// A value the syntax allows and the layout can use: finite and not negative.
fn usable(token: &str) -> Option<f64> {
    if !is_number(token) {
        return None;
    }
    token
        .parse()
        .ok()
        .filter(|v: &f64| v.is_finite() && *v >= 0.0)
}

/// Drop a trailing `:::class` highlight, which this renderer does not style.
fn strip_highlight(text: &str) -> &str {
    let Some(at) = text.rfind(":::") else {
        return text;
    };
    let Some(name) = text.get(at + 3..).map(str::trim) else {
        return text;
    };
    let valid = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| is_word(c) || c == '-');
    if valid {
        text.get(..at).unwrap_or(text).trim_end()
    } else {
        text
    }
}

/// What one line declares.
struct Entry {
    label: String,
    value: Option<f64>,
}

/// Read one already-trimmed line.
fn parse_entry(line: &str) -> Option<Entry> {
    let text = strip_highlight(line).trim();
    // A quoted label may hold anything, including a colon.
    if let Some(rest) = text.strip_prefix('"') {
        let (label, tail) = rest.split_once('"')?;
        let tail = tail.trim();
        let value = match tail.strip_prefix(':') {
            // A value that is not usable leaves the node valueless rather than
            // dropping the line: the label was still written.
            Some(v) => usable(v.trim()),
            None if tail.is_empty() => None,
            None => return None,
        };
        return Some(Entry {
            label: label.trim().to_string(),
            value,
        });
    }
    // Unquoted, `Label : value`.
    if let Some((label, value)) = text.rsplit_once(':') {
        let label = label.trim();
        if label.is_empty() || label.contains(':') {
            return None;
        }
        return Some(Entry {
            label: label.to_string(),
            value: usable(value.trim()),
        });
    }
    // A bare branch name.
    (!text.is_empty()).then(|| Entry {
        label: unquote(text).to_string(),
        value: None,
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

/// A path no other node has claimed.
fn unique_path(path: String, used: &mut Vec<String>) -> String {
    if !used.contains(&path) {
        used.push(path.clone());
        return path;
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{path}#{n}");
        if !used.contains(&candidate) {
            used.push(candidate.clone());
            return candidate;
        }
        n += 1;
    }
}

/// Turn a built node into its finished form.
fn finish(raw: Raw, parent_path: &str, used: &mut Vec<String>) -> Node {
    let path = unique_path(
        if parent_path.is_empty() {
            raw.entry.label.clone()
        } else {
            format!("{parent_path}/{}", raw.entry.label)
        },
        used,
    );
    let children: Vec<Node> = raw
        .children
        .into_iter()
        .map(|child| finish(child, &path, used))
        .collect();
    Node {
        label: raw.entry.label,
        value: raw.entry.value.unwrap_or(0.0),
        path,
        children,
        color_index: None,
    }
}

/// A branch is worth what it holds. Depth-first, so a branch of branches adds
/// up correctly.
fn total(node: &mut Node) -> f64 {
    if !node.children.is_empty() {
        node.value = node.children.iter_mut().map(total).sum();
    }
    node.value
}

/// Everything under one top-level branch shares its colour, so a subtree reads
/// as one region rather than as a scatter.
fn paint(node: &mut Node, index: usize) {
    node.color_index = Some(index);
    for child in &mut node.children {
        paint(child, index);
    }
}

/// Parse a treemap. Reads `source` line by line **without trimming first**.
pub fn parse(source: &str) -> Treemap {
    let mut tree = Treemap::default();
    let mut roots: Vec<Raw> = Vec::new();
    let mut open: Vec<(usize, usize)> = Vec::new();

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("%%")
            || opens_with(line, "treemap-beta")
            || opens_with(line, "treemap")
        {
            continue;
        }
        // A quoted line or one carrying a colon is a node, even if it opens
        // with the word `title`.
        if !line.starts_with('"') && !line.contains(':') {
            if let Some(title) = after_keyword(line, "title") {
                tree.title = Some(unquote(title).trim().to_string());
                continue;
            }
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
    // One named root is the outermost box; several become a nameless container
    // holding the forest, which is not drawn.
    tree.root = if roots.len() == 1 {
        roots
            .pop()
            .map(|root| finish(root, "", &mut used))
            .unwrap_or_default()
    } else {
        Node {
            children: roots
                .into_iter()
                .map(|root| finish(root, "", &mut used))
                .collect(),
            ..Node::default()
        }
    };
    total(&mut tree.root);
    for (i, child) in tree.root.children.iter_mut().enumerate() {
        paint(child, i);
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_suffix_is_taken_off_only_when_it_is_a_class_name() {
        // `:::` inside a label is ordinary text, and taking it for a class would
        // silently truncate the label instead of styling anything.
        assert_eq!(strip_highlight("Sales:::warm"), "Sales");
        assert_eq!(strip_highlight("Sales :::warm"), "Sales");
        assert_eq!(strip_highlight("Sales:::warm-ish"), "Sales");
        assert_eq!(strip_highlight("Sales:::_warm"), "Sales");
        // Not a name: a digit first, punctuation inside, or nothing at all.
        assert_eq!(strip_highlight("Sales:::2warm"), "Sales:::2warm");
        assert_eq!(strip_highlight("Sales:::a b"), "Sales:::a b");
        assert_eq!(strip_highlight("Sales:::"), "Sales:::");
        // And a label that never mentions one is returned untouched.
        assert_eq!(strip_highlight("Sales"), "Sales");
    }

    const TREE: &str = "treemap-beta\n\
        title Disk usage\n\
        \"Projects\"\n    \
            \"rust\" : 40\n    \
            \"web\"\n        \
                \"src\" : 20\n        \
                \"dist\" : 5";

    #[test]
    fn a_whole_tree_reads() {
        let tree = parse(TREE);
        assert_eq!(tree.title.as_deref(), Some("Disk usage"));
        assert_eq!(tree.root.label, "Projects");
        assert_eq!(tree.root.children.len(), 2);
    }

    #[test]
    fn a_branch_is_worth_what_it_holds() {
        let tree = parse(TREE);
        assert!((tree.root.value - 65.0).abs() < 1e-9);
        assert!((tree.root.children[1].value - 25.0).abs() < 1e-9);
    }

    #[test]
    fn everything_under_one_top_level_branch_shares_its_colour() {
        let tree = parse(TREE);
        assert_eq!(tree.root.color_index, None, "the root belongs to nothing");
        assert_eq!(tree.root.children[0].color_index, Some(0));
        assert_eq!(tree.root.children[1].color_index, Some(1));
        assert_eq!(tree.root.children[1].children[0].color_index, Some(1));
    }

    #[test]
    fn several_roots_become_a_nameless_container() {
        let tree = parse("treemap\n\"a\" : 1\n\"b\" : 2");
        assert_eq!(tree.root.label, "");
        assert_eq!(tree.root.children.len(), 2);
        assert!((tree.root.value - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_path_names_a_leaf_by_where_it_sits() {
        let tree = parse("treemap\n\"top\"\n  \"a\"\n    \"x\" : 1\n  \"b\"\n    \"x\" : 2");
        assert_eq!(tree.root.children[0].children[0].path, "top/a/x");
        assert_eq!(tree.root.children[1].children[0].path, "top/b/x");
    }

    #[test]
    fn a_repeated_path_is_still_told_apart() {
        let tree = parse("treemap\n\"top\"\n  \"same\" : 1\n  \"same\" : 2");
        assert_eq!(tree.root.children[0].path, "top/same");
        assert_eq!(tree.root.children[1].path, "top/same#2");
    }

    #[test]
    fn an_unusable_value_leaves_the_node_valueless_rather_than_dropping_it() {
        // The label was still written, so the node still exists.
        for source in ["\"a\" : -5", "\"a\" : oops"] {
            let tree = parse(&format!("treemap\n\"top\"\n  {source}"));
            assert_eq!(tree.root.children.len(), 1, "{source}");
            assert!(tree.root.children[0].value.abs() < 1e-9, "{source}");
        }
    }

    #[test]
    fn an_unquoted_label_with_a_value_reads() {
        let tree = parse("treemap\ntop\n  leaf : 7");
        assert_eq!(tree.root.children[0].label, "leaf");
        assert!((tree.root.children[0].value - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_highlight_class_is_accepted_and_ignored() {
        let tree = parse("treemap\n\"a\" : 5:::important");
        assert_eq!(tree.root.label, "a");
        assert!((tree.root.value - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_quoted_line_is_a_node_even_when_it_opens_with_title() {
        let tree = parse("treemap\n\"title something\" : 1");
        assert_eq!(tree.title, None);
        assert_eq!(tree.root.label, "title something");
    }

    #[test]
    fn nothing_in_yields_an_empty_tree() {
        assert_eq!(parse(""), Treemap::default());
        assert_eq!(parse("treemap-beta"), Treemap::default());
    }
}
