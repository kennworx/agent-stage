//! Reading `stateDiagram-v2` source into the flowchart's own structure.
//!
//! ```text
//! stateDiagram-v2
//!   [*] --> Closed
//!   Closed --> Connecting : connect
//!   state "Waiting for you" as Connecting
//!   state Connected {
//!     Idle --> Busy
//!   }
//!   Closed --> [*]
//! ```
//!
//! A state is a box and a transition is an arrow, so the drawing is a flowchart
//! whatever the source calls itself. Only three things need translating: `[*]`
//! is a marker rather than a name and every occurrence of it is its own node, a
//! composite state is a group rather than a box, and a state's description is
//! written on a line of its own rather than in brackets.

use super::types::{Direction, Edge, EdgeStyle, Graph, Group, Node, Shape};

/// Whether `c` may appear in a state's name.
fn is_name(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// The text after `word` and the whitespace that must follow it.
fn after_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    if line.get(..word.len())? != word {
        return None;
    }
    let rest = line.get(word.len()..)?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

/// The contents of a leading `"…"`, and what follows it.
fn quoted(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some((rest.get(..end)?, rest.get(end + 1..)?.trim_start()))
}

/// A state's name at the head of `text`, and what follows it.
fn name(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|c: char| !is_name(c)).unwrap_or(text.len());
    let (found, rest) = text.split_at(end);
    (!found.is_empty()).then_some((found, rest.trim_start()))
}

/// `[*]`, or a plain name.
fn endpoint(text: &str) -> Option<(&str, &str)> {
    match text.strip_prefix("[*]") {
        Some(rest) => Some(("[*]", rest.trim_start())),
        None => name(text),
    }
}

/// The reading state.
struct Reader {
    graph: Graph,
    open: Vec<Group>,
    /// Names that turned out to be composite states, so a transition naming one
    /// does not also draw it as a box.
    composites: Vec<String>,
    starts: usize,
    ends: usize,
}

impl Reader {
    fn new() -> Self {
        Self {
            graph: Graph {
                direction: Direction::Down,
                ..Graph::default()
            },
            open: Vec::new(),
            composites: Vec::new(),
            starts: 0,
            ends: 0,
        }
    }

    /// Add a state, or relabel one a transition already brought into being.
    fn declare(&mut self, id: &str, label: &str, shape: Shape) {
        if let Some(at) = self.graph.index_of(id) {
            if let Some(known) = self.graph.nodes.get_mut(at) {
                if !label.is_empty() {
                    known.label = label.to_string();
                }
                known.shape = shape;
            }
            return;
        }
        self.graph.nodes.push(Node {
            id: id.to_string(),
            label: label.to_string(),
            shape,
            classes: Vec::new(),
        });
        if let Some(group) = self.open.last_mut() {
            group.nodes.push(id.to_string());
        }
    }

    /// A state a transition named, drawn as a plain rounded box.
    fn ensure(&mut self, id: &str) {
        if self.composites.iter().any(|name| name == id) {
            return;
        }
        if self.graph.index_of(id).is_none() {
            self.declare(id, id, Shape::Rounded);
        }
    }

    /// One end of a transition, as the node it names.
    ///
    /// `[*]` is a marker rather than a name, and every occurrence is its own
    /// node — the one at the top of a diagram and the one at the bottom are two
    /// different things, however alike they are written.
    fn endpoint_node(&mut self, written: &str, entering: bool) -> String {
        if written != "[*]" {
            self.ensure(written);
            return written.to_string();
        }
        let (count, shape, stem) = if entering {
            self.starts += 1;
            (self.starts, Shape::StateStart, "_start")
        } else {
            self.ends += 1;
            (self.ends, Shape::StateEnd, "_end")
        };
        let id = if count > 1 {
            format!("{stem}{count}")
        } else {
            stem.to_string()
        };
        self.declare(&id, "", shape);
        id
    }

    fn open_group(&mut self, id: &str, label: &str) {
        self.composites.push(id.to_string());
        // A transition may have named it before it was declared, which drew a
        // box for something that is really a frame.
        if let Some(at) = self.graph.index_of(id) {
            self.graph.nodes.remove(at);
        }
        self.open.push(Group {
            id: id.to_string(),
            label: label.to_string(),
            ..Group::default()
        });
    }

    fn close_group(&mut self) -> bool {
        let Some(group) = self.open.pop() else {
            return false;
        };
        let at = self.graph.groups.len();
        self.graph.groups.push(group);
        if let Some(parent) = self.open.last_mut() {
            parent.groups.push(at);
        }
        true
    }

    /// `state "Description" as s1 {` — a composite state, named or not.
    fn composite(&mut self, rest: &str) -> bool {
        let Some(head) = rest.strip_suffix('{') else {
            return false;
        };
        let head = head.trim_end();
        let (label, head) = match quoted(head) {
            Some((label, after)) => match after_word(after, "as") {
                Some(tail) => (Some(label.to_string()), tail),
                None => return false,
            },
            None => (None, head),
        };
        let Some((id, rest)) = name(head) else {
            return false;
        };
        if !rest.is_empty() {
            return false;
        }
        let shown = label.unwrap_or_else(|| id.to_string());
        self.open_group(id, &crate::text::normalize_label(&shown));
        true
    }

    /// `state "Description" as s1` — a name for a state, with no block.
    fn alias(&mut self, rest: &str) -> bool {
        let Some((label, after)) = quoted(rest) else {
            return false;
        };
        let Some(tail) = after_word(after, "as") else {
            return false;
        };
        let Some((id, rest)) = name(tail) else {
            return false;
        };
        if !rest.is_empty() {
            return false;
        }
        self.declare(id, &crate::text::normalize_label(label), Shape::Rounded);
        true
    }

    /// `s1 --> s2 : label`.
    fn transition(&mut self, line: &str) -> bool {
        let Some((from, rest)) = endpoint(line) else {
            return false;
        };
        let Some(rest) = rest.strip_prefix("-->") else {
            return false;
        };
        let Some((to, rest)) = endpoint(rest.trim_start()) else {
            return false;
        };
        let label = match rest.strip_prefix(':') {
            Some(text) => crate::text::normalize_label(text.trim()),
            None if rest.is_empty() => String::new(),
            None => return false,
        };
        let (from, to) = (from.to_string(), to.to_string());
        let source = self.endpoint_node(&from, true);
        let target = self.endpoint_node(&to, false);
        self.graph.edges.push(Edge {
            source,
            target,
            label,
            style: EdgeStyle::Solid,
            head_start: false,
            head_end: true,
        });
        true
    }

    /// `s1 : Description`.
    fn description(&mut self, line: &str) -> bool {
        let Some((id, rest)) = name(line) else {
            return false;
        };
        let Some(text) = rest.strip_prefix(':') else {
            return false;
        };
        let label = text.trim();
        if label.is_empty() {
            return false;
        }
        self.declare(id, &crate::text::normalize_label(label), Shape::Rounded);
        true
    }

    fn line(&mut self, line: &str) {
        if let Some(rest) = after_word(line, "direction") {
            if let Some(direction) = Direction::from_keyword(rest.trim()) {
                self.graph.direction = direction;
            }
            return;
        }
        if let Some(rest) = after_word(line, "state") {
            if self.composite(rest) || self.alias(rest) {
                return;
            }
        }
        if line == "}" && self.close_group() {
            return;
        }
        if self.transition(line) {
            return;
        }
        self.description(line);
    }
}

/// Parse a state diagram source.
pub fn parse(source: &str) -> Graph {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
        .collect();
    let mut reader = Reader::new();
    for line in lines.iter().skip(1) {
        reader.line(line);
    }
    while reader.close_group() {}
    reader.graph
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(graph: &Graph) -> Vec<&str> {
        graph.nodes.iter().map(|node| node.id.as_str()).collect()
    }

    fn wires(graph: &Graph) -> Vec<(&str, &str, &str)> {
        graph
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.source.as_str(),
                    edge.target.as_str(),
                    edge.label.as_str(),
                )
            })
            .collect()
    }

    #[test]
    fn a_transition_names_two_states_and_the_arrow_between_them() {
        let out = parse("stateDiagram-v2\nIdle --> Busy : work");
        assert_eq!(ids(&out), ["Idle", "Busy"]);
        assert_eq!(wires(&out), [("Idle", "Busy", "work")]);
        assert_eq!(out.nodes[0].shape, Shape::Rounded);
        assert!(out.edges[0].head_end && !out.edges[0].head_start);
    }

    #[test]
    fn a_transition_needs_no_label() {
        let out = parse("stateDiagram-v2\nIdle --> Busy");
        assert_eq!(wires(&out), [("Idle", "Busy", "")]);
    }

    #[test]
    fn a_marker_at_each_end_is_its_own_state() {
        let out = parse("stateDiagram-v2\n[*] --> Idle\nIdle --> [*]");
        assert_eq!(ids(&out), ["_start", "Idle", "_end"]);
        assert_eq!(out.nodes[0].shape, Shape::StateStart);
        assert_eq!(out.nodes[2].shape, Shape::StateEnd);
        assert_eq!(out.nodes[0].label, "");
    }

    #[test]
    fn two_markers_of_the_same_kind_are_two_states() {
        // The one at the top and the one at the bottom are different things,
        // however alike they are written.
        let out = parse("stateDiagram-v2\n[*] --> A\n[*] --> B\nA --> [*]\nB --> [*]");
        assert_eq!(ids(&out), ["_start", "A", "_start2", "B", "_end", "_end2"]);
    }

    #[test]
    fn a_description_names_a_state_written_elsewhere() {
        let out = parse("stateDiagram-v2\nIdle --> Busy\nBusy : Doing the work");
        assert_eq!(out.nodes[1].label, "Doing the work");
        assert_eq!(ids(&out), ["Idle", "Busy"], "and declares nothing new");
    }

    #[test]
    fn an_alias_names_a_state_the_other_way_round() {
        let out = parse("stateDiagram-v2\nstate \"Doing the work\" as Busy\nIdle --> Busy");
        assert_eq!(out.nodes[0].id, "Busy");
        assert_eq!(out.nodes[0].label, "Doing the work");
    }

    #[test]
    fn a_composite_state_is_a_frame_rather_than_a_box() {
        let out = parse("stateDiagram-v2\nstate Working {\nIdle --> Busy\n}");
        assert_eq!(out.groups.len(), 1);
        assert_eq!(out.groups[0].id, "Working");
        assert_eq!(out.groups[0].label, "Working");
        assert_eq!(out.groups[0].nodes, ["Idle", "Busy"]);
        assert!(!ids(&out).contains(&"Working"), "and not also a box");
    }

    #[test]
    fn a_composite_state_may_be_named_before_it_is_opened() {
        // The transition draws a box for it, and opening the block takes it back.
        let out = parse("stateDiagram-v2\nA --> Working\nstate Working {\nIdle --> Busy\n}");
        assert!(!ids(&out).contains(&"Working"));
        assert_eq!(wires(&out)[0], ("A", "Working", ""));
    }

    #[test]
    fn a_composite_state_may_carry_a_name_of_its_own() {
        let out = parse("stateDiagram-v2\nstate \"Hard at it\" as Working {\nIdle --> Busy\n}");
        assert_eq!(out.groups[0].id, "Working");
        assert_eq!(out.groups[0].label, "Hard at it");
    }

    #[test]
    fn composite_states_nest() {
        let out = parse("stateDiagram-v2\nstate Outer {\nstate Inner {\nA --> B\n}\nC --> D\n}");
        assert_eq!(out.groups.len(), 2);
        assert_eq!(out.groups[0].id, "Inner");
        assert_eq!(out.groups[1].id, "Outer");
        assert_eq!(out.groups[1].groups, [0]);
    }

    #[test]
    fn a_block_left_open_still_draws() {
        let out = parse("stateDiagram-v2\nstate Working {\nIdle --> Busy");
        assert_eq!(out.groups.len(), 1);
        assert_eq!(out.groups[0].nodes, ["Idle", "Busy"]);
    }

    #[test]
    fn a_composite_state_that_is_not_one_is_not_opened() {
        for line in [
            "state \"Named\" Working {", // no `as` between them
            "state \"Named\" as {",      // nothing to name
            "state Working extra {",     // more than a name before the brace
        ] {
            let out = parse(&format!("stateDiagram-v2\n{line}\nA --> B\n}}"));
            assert!(out.groups.is_empty(), "{line}");
        }
    }

    #[test]
    fn an_alias_that_is_not_one_names_nothing() {
        for line in [
            "state Working",           // no quotes at all
            "state \"Named\" Working", // no `as`
            "state \"Named\" as ",     // nothing to name
            "state \"Named\" as A B",  // more than a name after it
        ] {
            let out = parse(&format!("stateDiagram-v2\n{line}\nX --> Y"));
            assert_eq!(ids(&out), ["X", "Y"], "{line}");
        }
    }

    #[test]
    fn a_transition_naming_a_composite_state_does_not_also_draw_a_box() {
        // The block comes first this time, so the name is known to be a frame
        // before anything points at it.
        let out = parse("stateDiagram-v2\nstate Working {\nIdle --> Busy\n}\nA --> Working");
        assert!(!ids(&out).contains(&"Working"));
        assert_eq!(ids(&out), ["Idle", "Busy", "A"]);
    }

    #[test]
    fn a_direction_line_turns_the_whole_drawing() {
        assert_eq!(
            parse("stateDiagram-v2\ndirection LR\nA --> B").direction,
            Direction::Right
        );
        assert_eq!(parse("stateDiagram-v2\nA --> B").direction, Direction::Down);
    }

    #[test]
    fn a_name_may_be_written_in_any_script() {
        let out = parse("stateDiagram-v2\n启动 --> 运行 : 开始");
        assert_eq!(ids(&out), ["启动", "运行"]);
        assert_eq!(out.edges[0].label, "开始");
    }

    #[test]
    fn a_line_that_is_neither_a_transition_nor_a_description_draws_nothing() {
        let out = parse("stateDiagram-v2\nsomething odd here\nA --> B");
        assert_eq!(wires(&out), [("A", "B", "")]);
        assert_eq!(ids(&out), ["A", "B"]);
    }

    #[test]
    fn a_state_line_that_is_neither_a_block_nor_an_alias_falls_through() {
        // `state X` on its own is not a form the reference knows, so the line is
        // read as whatever else it might be — here, nothing.
        let out = parse("stateDiagram-v2\nstate Alone\nA --> B");
        assert!(!ids(&out).contains(&"Alone"));
    }

    #[test]
    fn a_comment_and_a_blank_line_are_dropped_before_reading() {
        let out = parse("stateDiagram-v2\n\n%% a note\n  A --> B  \n");
        assert_eq!(wires(&out), [("A", "B", "")]);
    }

    #[test]
    fn a_source_of_nothing_reads_to_nothing() {
        assert_eq!(parse("stateDiagram-v2").nodes.len(), 0);
        assert_eq!(parse("").nodes.len(), 0);
    }

    #[test]
    fn a_closing_brace_with_nothing_open_is_not_a_state() {
        let out = parse("stateDiagram-v2\n}\nA --> B");
        assert!(out.groups.is_empty());
        assert_eq!(ids(&out), ["A", "B"]);
    }
}
