//! Reading `treeView-beta` source.
//!
//! ```text
//! treeView-beta
//!   title <text>
//!   my-project/              a trailing slash marks a folder
//!       index.js             nesting comes from leading whitespace
//!       "name with spaces"   quotes are optional
//!       src/ ## description  an inline note, drawn italic
//!       index.js :::class    a highlight class, accepted and ignored
//! ```
//!
//! **Indentation is the syntax here**, so unlike every other type this one reads
//! the lines before they are trimmed. Trimming first would flatten the tree into
//! a list, silently and without error.

use super::types::{Tree, TreeNode};
use crate::keyword::{is_word, opens_with};
use crate::outline::{attach, indent_of, Outline};

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

/// Drop a trailing `:::class` highlight, which this renderer does not style.
fn strip_highlight(text: &str) -> &str {
    let Some(at) = text.rfind(":::") else {
        return text;
    };
    let Some(tail) = text.get(at + 3..) else {
        return text;
    };
    let name = tail.trim();
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

/// What one line says, before it is placed in the tree.
struct Entry {
    label: String,
    folder_mark: bool,
    description: Option<String>,
}

/// Read one already-trimmed line as an entry.
fn parse_entry(line: &str) -> Option<Entry> {
    // An inline note runs from the first `##` to the end of the line.
    let (head, description) = match line.split_once("##") {
        Some((head, note)) => {
            let note = note.trim();
            (head.trim(), (!note.is_empty()).then(|| note.to_string()))
        }
        None => (line, None),
    };
    let text = strip_highlight(head).trim();
    if text.is_empty() {
        return None;
    }
    let (text, folder_mark) = match text.strip_suffix('/') {
        Some(rest) => (rest.trim(), true),
        None => (text, false),
    };
    let label = unquote(text).trim();
    if label.is_empty() {
        return None;
    }
    Some(Entry {
        label: label.to_string(),
        folder_mark,
        description,
    })
}

/// A node while the tree is still being built, before paths are assigned.
struct Raw {
    entry: Entry,
    depth: usize,
    children: Vec<Raw>,
}

impl Outline for Raw {
    fn children_mut(&mut self) -> &mut Vec<Self> {
        &mut self.children
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

/// Turn a built node into its finished form, naming it by its path.
fn finish(raw: Raw, parent_path: &str, used: &mut Vec<String>) -> TreeNode {
    let path = unique_path(
        if parent_path.is_empty() {
            raw.entry.label.clone()
        } else {
            format!("{parent_path}/{}", raw.entry.label)
        },
        used,
    );
    let children: Vec<TreeNode> = raw
        .children
        .into_iter()
        .map(|child| finish(child, &path, used))
        .collect();
    TreeNode {
        label: raw.entry.label,
        // Marked as a folder, or holding something — either makes it one.
        is_folder: raw.entry.folder_mark || !children.is_empty(),
        description: raw.entry.description,
        depth: raw.depth,
        path,
        children,
    }
}

/// Parse a tree view. Reads `source` line by line **without trimming first**.
pub fn parse(source: &str) -> Tree {
    let mut tree = Tree::default();
    let mut roots: Vec<Raw> = Vec::new();
    // The chain of open ancestors: each entry is a child index, plus the indent
    // it was written at, which is what decides where the next line hangs.
    let mut open: Vec<(usize, usize)> = Vec::new();

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("%%") || opens_with(line, "treeview-beta") {
            continue;
        }
        if opens_with(line, "treeview") {
            continue;
        }
        // A line ending in a slash is a folder called `title …`, not a title.
        if !line.ends_with('/') {
            if let Some(title) = after_keyword(line, "title") {
                tree.title = Some(unquote(title).trim().to_string());
                continue;
            }
        }
        let Some(entry) = parse_entry(line) else {
            continue;
        };
        let indent = indent_of(raw_line);
        // Close every ancestor that is not strictly shallower than this line.
        while open.last().is_some_and(|(_, at)| *at >= indent) {
            open.pop();
        }
        let depth = open.len();
        let path: Vec<usize> = open.iter().map(|(i, _)| *i).collect();
        let index = {
            let mut level = &roots;
            for i in &path {
                let Some(next) = level.get(*i) else { break };
                level = &next.children;
            }
            level.len()
        };
        attach(
            &mut roots,
            &path,
            Raw {
                entry,
                depth,
                children: Vec::new(),
            },
        );
        open.push((index, indent));
    }

    let mut used = Vec::new();
    tree.nodes = roots
        .into_iter()
        .map(|root| finish(root, "", &mut used))
        .collect();
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_files_with_one_path_are_still_separately_addressable() {
        fn walk(node: &TreeNode, out: &mut Vec<String>) {
            out.push(node.path.clone());
            for child in &node.children {
                walk(child, out);
            }
        }
        // The path is the identity, so a folder listing the same name twice
        // would otherwise give two rows one anchor.
        let tree = parse("treeView\n  \"src/\"\n    \"a.ts\"\n    \"a.ts\"");
        let mut paths = Vec::new();
        for node in &tree.nodes {
            walk(node, &mut paths);
        }
        let before = paths.len();
        paths.sort_unstable();
        paths.dedup();
        assert_eq!(paths.len(), before, "{paths:?}");
    }

    const TREE: &str = "treeView-beta\n\
        title Project\n\
        my-project/\n    \
            src/ ## the source\n        \
                index.js\n        \
                util.js\n    \
            README.md";

    fn labels(node: &TreeNode, out: &mut Vec<(usize, String)>) {
        out.push((node.depth, node.label.clone()));
        for child in &node.children {
            labels(child, out);
        }
    }

    fn flat(source: &str) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for node in parse(source).nodes {
            labels(&node, &mut out);
        }
        out
    }

    #[test]
    fn indentation_builds_the_hierarchy() {
        assert_eq!(
            flat(TREE),
            [
                (0, "my-project".to_string()),
                (1, "src".to_string()),
                (2, "index.js".to_string()),
                (2, "util.js".to_string()),
                (1, "README.md".to_string()),
            ]
        );
    }

    #[test]
    fn a_tab_indents_as_four_columns() {
        // Mixed tabs and spaces have to agree, or a tab-indented file builds a
        // different tree from a space-indented one that looks identical.
        let tabbed = flat("treeView\nroot/\n\tchild");
        let spaced = flat("treeView\nroot/\n    child");
        assert_eq!(tabbed, spaced);
    }

    #[test]
    fn a_node_is_a_folder_when_marked_or_when_it_holds_something() {
        let tree = parse("treeView\nmarked/\nimplied\n    child\nplain");
        assert!(tree.nodes[0].is_folder, "marked with a slash");
        assert!(tree.nodes[1].is_folder, "has a child");
        assert!(!tree.nodes[2].is_folder);
    }

    #[test]
    fn a_path_names_a_node_by_where_it_sits() {
        let tree = parse("treeView\na/\n    index.ts\nb/\n    index.ts");
        assert_eq!(tree.nodes[0].children[0].path, "a/index.ts");
        assert_eq!(tree.nodes[1].children[0].path, "b/index.ts");
    }

    #[test]
    fn two_identical_paths_are_still_told_apart() {
        let tree = parse("treeView\nsame\nsame");
        assert_eq!(tree.nodes[0].path, "same");
        assert_eq!(tree.nodes[1].path, "same#2");
    }

    #[test]
    fn an_inline_note_reads_and_leaves_the_label_alone() {
        let tree = parse("treeView\nsrc/ ## where the code is");
        assert_eq!(tree.nodes[0].label, "src");
        assert_eq!(
            tree.nodes[0].description.as_deref(),
            Some("where the code is")
        );
    }

    #[test]
    fn a_highlight_class_is_accepted_and_ignored() {
        let tree = parse("treeView\nindex.js :::important");
        assert_eq!(tree.nodes[0].label, "index.js");
        // Something that is not a class name stays part of the label.
        assert_eq!(parse("treeView\na :::9bad").nodes[0].label, "a :::9bad");
    }

    #[test]
    fn quotes_around_a_label_are_optional() {
        assert_eq!(
            parse("treeView\n\"name with spaces\"").nodes[0].label,
            "name with spaces"
        );
        assert_eq!(parse("treeView\n\"a folder\"/").nodes[0].label, "a folder");
    }

    #[test]
    fn a_folder_called_title_is_not_a_title() {
        let tree = parse("treeView\ntitle something/");
        assert_eq!(tree.title, None);
        assert_eq!(tree.nodes[0].label, "title something");
    }

    #[test]
    fn several_roots_are_allowed() {
        let tree = parse("treeView\nfirst/\n    a\nsecond/\n    b");
        assert_eq!(tree.nodes.len(), 2);
    }

    #[test]
    fn dedenting_past_a_level_reattaches_to_the_right_ancestor() {
        assert_eq!(
            flat("treeView\na/\n        deep\n    shallow"),
            [
                (0, "a".to_string()),
                (1, "deep".to_string()),
                (1, "shallow".to_string()),
            ]
        );
    }

    #[test]
    fn a_comment_line_is_skipped_and_an_inline_one_is_not_a_comment() {
        assert!(parse("treeView\n%% a note").nodes.is_empty());
        // `%%` mid-line is part of the label: only `##` starts a note here.
        assert_eq!(parse("treeView\na %% b").nodes[0].label, "a %% b");
    }

    #[test]
    fn nothing_in_yields_an_empty_tree() {
        assert_eq!(parse(""), Tree::default());
        assert_eq!(parse("treeView-beta"), Tree::default());
    }
}
