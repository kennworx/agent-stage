//! Reading `gitGraph` source.
//!
//! ```text
//! gitGraph [LR:|TB:|BT:]
//!   commit [id: "…"] [tag: "…"] [type: NORMAL|REVERSE|HIGHLIGHT]
//!   branch <name>                   creates a lane and switches to it
//!   checkout <name> | switch <name>
//!   merge <name> [id: "…"] [tag: "…"]
//!   cherry-pick id: "…"
//! ```
//!
//! The current branch and each branch's head are carried forward as the lines
//! are read, which is what makes a commit's parent the previous one on *its*
//! branch rather than the previous one written.

use super::types::{Branch, Commit, CommitType, Graph, Orientation};
use crate::keyword::opens_with;

/// The branch that exists before any is declared.
const DEFAULT_MAIN: &str = "main";

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

/// Whatever follows `keyword`, when the line opens with it as a whole word.
fn tail_after<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if !opens_with(line, keyword) {
        return None;
    }
    let rest = line.get(keyword.len()..)?.trim();
    (!rest.is_empty()).then_some(rest)
}

/// The first whitespace-delimited token, or the first quoted run.
fn first_token(text: &str) -> &str {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return text.get(..end + 2).unwrap_or(text);
        }
    }
    text.split_whitespace().next().unwrap_or_default()
}

/// The value of `key: "value"` or `key: value`, wherever it appears.
fn option(rest: &str, key: &str) -> Option<String> {
    let lower = rest.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower.get(from..)?.find(&key.to_ascii_lowercase()) {
        let start = from + at;
        let after = rest.get(start + key.len()..)?;
        // The key has to be followed by a colon, or `id` would match `idle`.
        let value = after.trim_start();
        if let Some(value) = value.strip_prefix(':') {
            let value = value.trim_start();
            let text = if let Some(quoted) = value.strip_prefix('"') {
                quoted.split('"').next().unwrap_or_default()
            } else {
                value.split_whitespace().next().unwrap_or_default()
            };
            let text = text.trim();
            return (!text.is_empty()).then(|| text.to_string());
        }
        from = start + key.len();
    }
    None
}

/// The `type:` of a commit, when one was named.
fn option_type(rest: &str) -> Option<CommitType> {
    match option(rest, "type")?.to_ascii_uppercase().as_str() {
        "NORMAL" => Some(CommitType::Normal),
        "REVERSE" => Some(CommitType::Reverse),
        "HIGHLIGHT" => Some(CommitType::Highlight),
        _ => None,
    }
}

/// A short id derived from a commit's position, so the same source always
/// produces the same hashes.
fn generated_id(seq: usize) -> String {
    // Knuth's multiplicative hash over 32 bits, which is what the reference
    // uses; the wrap is the point, not an accident.
    let hash = u32::try_from(seq)
        .unwrap_or(u32::MAX)
        .wrapping_add(1)
        .wrapping_mul(2_654_435_761);
    let hex = format!("{hash:x}");
    let padded = if hex.len() < 7 {
        format!("{}{hex}", "0".repeat(7 - hex.len()))
    } else {
        hex
    };
    padded.get(..7).unwrap_or(&padded).to_string()
}

/// An id no other commit has claimed.
fn unique_id(base: &str, used: &mut Vec<String>) -> String {
    let mut id = base.to_string();
    let mut n = 2usize;
    while used.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    used.push(id.clone());
    id
}

/// The state carried from line to line.
struct State {
    graph: Graph,
    /// The last commit on each branch, by name.
    heads: Vec<(String, Option<String>)>,
    used: Vec<String>,
    current: String,
    seq: usize,
}

impl State {
    fn new() -> Self {
        let mut state = Self {
            graph: Graph::default(),
            heads: Vec::new(),
            used: Vec::new(),
            current: DEFAULT_MAIN.to_string(),
            seq: 0,
        };
        // Lane zero exists before anything is written.
        state.ensure_branch(DEFAULT_MAIN);
        state
    }

    fn ensure_branch(&mut self, name: &str) {
        if self.graph.branches.iter().any(|b| b.name == name) {
            return;
        }
        self.graph.branches.push(Branch {
            name: name.to_string(),
            order: self.graph.branches.len(),
        });
        self.heads.push((name.to_string(), None));
    }

    fn head(&self, name: &str) -> Option<String> {
        self.heads
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, head)| head.clone())
    }

    fn set_head(&mut self, name: &str, id: Option<String>) {
        if let Some(slot) = self.heads.iter_mut().find(|(n, _)| n == name) {
            slot.1 = id;
        } else {
            self.heads.push((name.to_string(), id));
        }
    }

    fn push_commit(
        &mut self,
        parents: Vec<String>,
        id: Option<String>,
        tag: Option<String>,
        kind: CommitType,
        is_merge: bool,
        is_cherry_pick: bool,
    ) {
        let id = unique_id(
            &id.unwrap_or_else(|| generated_id(self.seq)),
            &mut self.used,
        );
        self.seq += 1;
        let branch = self.current.clone();
        self.graph.commits.push(Commit {
            id: id.clone(),
            branch: branch.clone(),
            // A branch with no head yet contributes no parent at all rather
            // than an edge to nowhere.
            parents: parents.into_iter().filter(|p| !p.is_empty()).collect(),
            tag,
            kind,
            is_merge,
            is_cherry_pick,
        });
        self.set_head(&branch, Some(id));
    }
}

/// Parse a git graph. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Graph {
    let mut state = State::new();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if opens_with(line, "gitgraph") {
            let rest = line.get("gitgraph".len()..).unwrap_or_default();
            let word = rest.trim_matches(|c: char| c == ':' || c.is_whitespace());
            state.graph.orientation = match word.to_ascii_uppercase().as_str() {
                "TB" => Orientation::TopBottom,
                "BT" => Orientation::BottomTop,
                _ => Orientation::LeftRight,
            };
            continue;
        }
        if opens_with(line, "commit") {
            let rest = line.get("commit".len()..).unwrap_or_default();
            let parent = state.head(&state.current).unwrap_or_default();
            state.push_commit(
                vec![parent],
                option(rest, "id"),
                option(rest, "tag"),
                option_type(rest).unwrap_or_default(),
                false,
                false,
            );
            continue;
        }
        if let Some(rest) = tail_after(line, "branch") {
            let name = unquote(first_token(rest)).to_string();
            state.ensure_branch(&name);
            // A new branch starts where the current one is.
            let from = state.head(&state.current);
            state.set_head(&name, from);
            state.current = name;
            continue;
        }
        let switched = tail_after(line, "checkout").or_else(|| tail_after(line, "switch"));
        if let Some(rest) = switched {
            let name = unquote(first_token(rest)).to_string();
            state.ensure_branch(&name);
            state.current = name;
            continue;
        }
        if let Some(rest) = tail_after(line, "merge") {
            let name = unquote(first_token(rest)).to_string();
            state.ensure_branch(&name);
            let merged = state.head(&name).unwrap_or_default();
            let parent = state.head(&state.current).unwrap_or_default();
            state.push_commit(
                vec![parent, merged],
                option(rest, "id"),
                option(rest, "tag"),
                option_type(rest).unwrap_or_default(),
                true,
                false,
            );
            continue;
        }
        if opens_with(line, "cherry-pick") {
            let rest = line.get("cherry-pick".len()..).unwrap_or_default();
            let source_id = option(rest, "id").unwrap_or_default();
            let parent = state.head(&state.current).unwrap_or_default();
            state.push_commit(
                vec![parent, source_id],
                None,
                option(rest, "tag"),
                CommitType::Normal,
                false,
                true,
            );
        }
    }
    state.graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_option_is_read_however_it_is_written() {
        assert_eq!(option("id: \"one\"", "id").as_deref(), Some("one"));
        assert_eq!(option("id:one", "id").as_deref(), Some("one"));
        assert_eq!(option("ID: One", "id").as_deref(), Some("One"));
        assert_eq!(option("id: one two", "id").as_deref(), Some("one"));
        // A key needs its colon, or `id` would be found inside `idle`.
        assert_eq!(option("idle: yes", "id"), None);
        // …and the scan carries on past the false match to the real one.
        assert_eq!(option("idle stuff id: real", "id").as_deref(), Some("real"));
        assert_eq!(
            option("id:", "id"),
            None,
            "a key with no value is no option"
        );
        assert_eq!(option("tag: v1", "id"), None);
    }

    #[test]
    fn a_commit_type_is_read_by_name_and_an_unknown_one_is_refused() {
        assert_eq!(option_type("type: NORMAL"), Some(CommitType::Normal));
        assert_eq!(option_type("type: reverse"), Some(CommitType::Reverse));
        assert_eq!(option_type("type: Highlight"), Some(CommitType::Highlight));
        assert_eq!(option_type("type: sideways"), None);
        assert_eq!(option_type("id: one"), None);
    }

    #[test]
    fn a_branch_head_is_set_once_and_then_moved() {
        // Two arms: the first commit on a branch appends a head, every later one
        // moves the head that is already there. Only the appending arm is reached
        // by a diagram with one commit per branch.
        let mut state = State::new();
        state.set_head("main", Some("a".into()));
        state.set_head("feature", Some("b".into()));
        assert_eq!(state.heads.len(), 2, "two branches, two heads");
        state.set_head("main", Some("c".into()));
        assert_eq!(state.heads.len(), 2, "moving a head adds nothing");
        assert_eq!(
            state
                .heads
                .iter()
                .find(|(n, _)| n == "main")
                .map(|(_, id)| id.clone()),
            Some(Some("c".into())),
            "and it points at the newer commit"
        );
    }

    #[test]
    fn a_head_can_be_moved_to_nothing() {
        // A branch checked out before it has a commit has a head and no commit
        // for it to point at.
        let mut state = State::new();
        state.set_head("main", Some("a".into()));
        state.set_head("main", None);
        assert_eq!(state.heads.first().map(|(_, id)| id.clone()), Some(None));
    }

    #[test]
    fn a_quoted_first_token_keeps_its_quotes_and_its_spaces() {
        // A commit id may be quoted and contain spaces, so the first token is not
        // simply everything up to the first space.
        assert_eq!(first_token("\"a b\" rest"), "\"a b\"");
        assert_eq!(first_token("  \"only\""), "\"only\"");
    }

    #[test]
    fn an_unquoted_first_token_ends_at_the_first_space() {
        assert_eq!(first_token("commit id: x"), "commit");
        assert_eq!(first_token("   spaced   out"), "spaced");
    }

    #[test]
    fn a_quote_that_never_closes_falls_back_to_whitespace() {
        // Otherwise a stray quote would swallow the rest of the line.
        assert_eq!(first_token("\"unclosed rest"), "\"unclosed");
    }

    #[test]
    fn nothing_at_all_is_an_empty_token() {
        assert_eq!(first_token(""), "");
        assert_eq!(first_token("    "), "");
    }

    const GRAPH: &str = "gitGraph\n\
        commit id: \"one\"\n\
        branch feature\n\
        commit id: \"two\"\n\
        checkout main\n\
        commit id: \"three\"\n\
        merge feature id: \"four\" tag: \"v1\"";

    #[test]
    fn a_whole_graph_reads() {
        let graph = parse(GRAPH);
        let ids: Vec<&str> = graph.commits.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["one", "two", "three", "four"]);
        let branches: Vec<&str> = graph.branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(branches, ["main", "feature"]);
    }

    #[test]
    fn main_exists_before_anything_declares_it() {
        let graph = parse("gitGraph\ncommit");
        assert_eq!(graph.branches[0].name, "main");
        assert_eq!(graph.commits[0].branch, "main");
    }

    #[test]
    fn a_commit_follows_the_last_one_on_its_own_branch_not_the_last_written() {
        let graph = parse(GRAPH);
        // `three` is on main, so its parent is `one` — not `two`, which is the
        // commit written immediately before it.
        let three = &graph.commits[2];
        assert_eq!(three.parents, ["one"]);
    }

    #[test]
    fn the_first_commit_on_a_branch_has_no_parent_rather_than_an_empty_one() {
        assert!(parse("gitGraph\ncommit").commits[0].parents.is_empty());
    }

    #[test]
    fn a_new_branch_starts_where_the_current_one_is() {
        let graph = parse(GRAPH);
        assert_eq!(graph.commits[1].parents, ["one"]);
        assert_eq!(graph.commits[1].branch, "feature");
    }

    #[test]
    fn a_merge_names_both_the_branch_it_is_on_and_the_one_merged_in() {
        let merge = &parse(GRAPH).commits[3];
        assert!(merge.is_merge);
        assert_eq!(merge.parents, ["three", "two"]);
        assert_eq!(merge.tag.as_deref(), Some("v1"));
    }

    #[test]
    fn a_cherry_pick_names_the_commit_it_took() {
        let graph = parse(
            "gitGraph\n\
             commit id: \"a\"\n\
             branch f\n\
             commit id: \"b\"\n\
             checkout main\n\
             cherry-pick id: \"b\"",
        );
        let picked = &graph.commits[2];
        assert!(picked.is_cherry_pick);
        // Where it sits, then what it took.
        assert_eq!(picked.parents, ["a", "b"]);
    }

    #[test]
    fn switch_is_another_spelling_of_checkout() {
        let a = parse("gitGraph\nbranch f\ncheckout main\ncommit");
        let b = parse("gitGraph\nbranch f\nswitch main\ncommit");
        assert_eq!(a.commits[0].branch, b.commits[0].branch);
        assert_eq!(a.commits[0].branch, "main");
    }

    #[test]
    fn each_commit_kind_reads() {
        let kinds: Vec<CommitType> = parse(
            "gitGraph\ncommit type: NORMAL\ncommit type: REVERSE\ncommit type: HIGHLIGHT\ncommit",
        )
        .commits
        .iter()
        .map(|c| c.kind)
        .collect();
        assert_eq!(
            kinds,
            [
                CommitType::Normal,
                CommitType::Reverse,
                CommitType::Highlight,
                CommitType::Normal,
            ]
        );
    }

    #[test]
    fn an_unnamed_commit_gets_a_stable_generated_id() {
        let first = parse("gitGraph\ncommit\ncommit");
        let second = parse("gitGraph\ncommit\ncommit");
        assert_eq!(first.commits[0].id, second.commits[0].id);
        assert_ne!(first.commits[0].id, first.commits[1].id);
        assert_eq!(first.commits[0].id.len(), 7);
    }

    #[test]
    fn two_commits_given_the_same_id_are_still_told_apart() {
        let graph = parse("gitGraph\ncommit id: \"same\"\ncommit id: \"same\"");
        let ids: Vec<&str> = graph.commits.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["same", "same-2"]);
    }

    #[test]
    fn an_orientation_reads_from_the_header() {
        assert_eq!(parse("gitGraph TB:").orientation, Orientation::TopBottom);
        assert_eq!(parse("gitGraph BT:").orientation, Orientation::BottomTop);
        assert_eq!(parse("gitGraph").orientation, Orientation::LeftRight);
    }

    #[test]
    fn nothing_in_yields_a_graph_with_only_main() {
        let graph = parse("");
        assert_eq!(graph.branches.len(), 1);
        assert!(graph.commits.is_empty());
    }
}
