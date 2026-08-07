//! Reading `graph` and `flowchart` source.
//!
//! ```text
//! flowchart TD
//!   A[Start] --> B{Ready?}
//!   B -->|Yes| C([Go])
//!   B -- No --> D[/Wait\]
//!   A & B --> E
//!   subgraph cluster [A group]
//!     direction LR
//!     E --> F
//!   end
//!   classDef warn fill:#fee,stroke:#f00
//!   class D warn
//! ```
//!
//! An edge line is read left to right rather than split: `A --> B --> C` is one
//! line naming three nodes, and `A & B --> C & D` is four edges. Reading it as a
//! chain of node-group and arrow is what makes both fall out of the same loop.
//!
//! Matching is hand-rolled rather than regex-driven, as everywhere else in this
//! crate — see `text.rs` for why.

use super::tokens::{arrow, node, text_arrow, ArrowToken};
use super::types::{Direction, Edge, Graph, Group, LinkTarget, Node, Style};

/// The keywords a line may open with, other than a node's own name.
const HEADERS: [&str; 2] = ["graph", "flowchart"];

/// The header that means the source is written the other way.
const STATE_HEADER: &str = "stateDiagram";

/// The text after `word` and the whitespace that must follow it.
fn after_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    if !line
        .get(..word.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(word))
    {
        return None;
    }
    let rest = line.get(word.len()..)?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

/// `fill:#f00,stroke:#333` as the pairs it names.
fn style_props(text: &str) -> Style {
    Style {
        props: text
            .split(',')
            .filter_map(|prop| {
                let (key, value) = prop.split_once(':')?;
                let (key, value) = (key.trim(), value.trim());
                (!key.is_empty() && !value.is_empty()).then(|| (key.to_string(), value.to_string()))
            })
            .collect(),
    }
}

/// A comma-separated list of names.
fn names(text: &str) -> Vec<String> {
    text.split(',')
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect()
}

/// A label as an identifier: `A group` becomes `Agroup`.
fn slug(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join("_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// The reading state: the graph so far, and the groups still open.
#[derive(Default)]
struct Reader {
    graph: Graph,
    /// Groups still open, outermost first.
    open: Vec<Group>,
}

impl Reader {
    /// Note a node, and put it in whichever group is open.
    ///
    /// A node may be named many times and declared anywhere. A later bare
    /// mention — `A --> C` after `A[Start] --> B` — leaves the label alone; a
    /// later mention carrying its own label replaces it, which is what lets a
    /// declaration come after the first use of the thing it declares.
    ///
    /// A node joins a group only when it is first seen, so naming it again
    /// inside one does not move it there.
    fn see(&mut self, token: &super::tokens::NodeToken) {
        if let Some(at) = self.graph.index_of(&token.id) {
            if token.declared {
                if let Some(known) = self.graph.nodes.get_mut(at) {
                    known.label.clone_from(&token.label);
                    known.shape = token.shape;
                }
            }
            return;
        }
        self.graph.nodes.push(Node {
            id: token.id.clone(),
            label: token.label.clone(),
            shape: token.shape,
            classes: Vec::new(),
        });
        if let Some(group) = self.open.last_mut() {
            group.nodes.push(token.id.clone());
        }
    }

    /// Read one or more nodes joined by `&`, and hand back what follows.
    fn node_group<'a>(&mut self, text: &'a str) -> Option<(Vec<String>, &'a str)> {
        let (first, mut rest) = node(text)?;
        self.see(&first);
        let mut ids = vec![first.id];
        while let Some(tail) = rest.strip_prefix('&') {
            let Some((next, after)) = node(tail.trim_start()) else {
                break;
            };
            self.see(&next);
            ids.push(next.id);
            rest = after;
        }
        Some((ids, rest))
    }

    /// Every edge between one group of nodes and the next.
    fn join(&mut self, sources: &[String], targets: &[String], link: &ArrowToken) {
        for source in sources {
            for target in targets {
                self.graph.edges.push(Edge {
                    source: source.clone(),
                    target: target.clone(),
                    label: link.label.clone(),
                    style: link.style,
                    head_start: link.head_start,
                    head_end: link.head_end,
                });
            }
        }
    }

    /// Read a line naming nodes and the arrows between them.
    fn chain(&mut self, line: &str) {
        let Some((mut sources, mut rest)) = self.node_group(line) else {
            return;
        };
        while !rest.is_empty() {
            // The piped form is tried first; the middle-label form is the
            // fallback, which is the order the reference tries them in.
            let Some((joiner, after)) = arrow(rest).or_else(|| text_arrow(rest)) else {
                break;
            };
            let Some((targets, tail)) = self.node_group(after) else {
                break;
            };
            self.join(&sources, &targets, &joiner);
            sources = targets;
            rest = tail;
        }
    }

    /// Open a `subgraph`.
    fn open_group(&mut self, rest: &str) {
        // `subgraph id [Label]` names both; anything else is a label that has to
        // supply its own id.
        let named = rest.split_once('[').and_then(|(head, tail)| {
            let id = head.trim();
            let label = tail.strip_suffix(']')?;
            let plain = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
            (!id.is_empty() && !label.is_empty() && id.chars().all(plain))
                .then(|| (id.to_string(), crate::text::normalize_label(label)))
        });
        let (id, label) = named.unwrap_or_else(|| (slug(rest), crate::text::normalize_label(rest)));
        self.open.push(Group {
            id,
            label,
            ..Group::default()
        });
    }

    /// Close the innermost group. Answers whether there was one.
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

    /// Read one line.
    fn line(&mut self, line: &str) {
        if let Some(rest) = after_word(line, "classDef") {
            let (name, props) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            self.graph
                .class_defs
                .push((name.to_string(), style_props(props)));
            return;
        }
        if let Some(rest) = after_word(line, "class") {
            if let Some((ids, name)) = rest.rsplit_once(char::is_whitespace) {
                for id in names(ids) {
                    if let Some(at) = self.graph.index_of(&id) {
                        if let Some(node) = self.graph.nodes.get_mut(at) {
                            node.classes.push(name.trim().to_string());
                        }
                    }
                }
            }
            return;
        }
        if let Some(rest) = after_word(line, "style") {
            if let Some((ids, props)) = rest.split_once(char::is_whitespace) {
                for id in names(ids) {
                    self.graph.node_styles.push((id, style_props(props)));
                }
            }
            return;
        }
        if let Some(rest) = after_word(line, "linkStyle") {
            if let Some((target, props)) = rest.split_once(char::is_whitespace) {
                let style = style_props(props);
                if target.trim().eq_ignore_ascii_case("default") {
                    self.graph.link_styles.push((LinkTarget::Every, style));
                } else {
                    for at in names(target).iter().filter_map(|n| n.parse::<usize>().ok()) {
                        self.graph
                            .link_styles
                            .push((LinkTarget::At(at), style.clone()));
                    }
                }
            }
            return;
        }
        if let Some(rest) = after_word(line, "direction") {
            if let Some(direction) = Direction::from_keyword(rest.trim()) {
                // Outside a group it names the whole drawing; inside, only that
                // group, which is laid out on its own.
                match self.open.last_mut() {
                    Some(group) => group.direction = Some(direction),
                    None => self.graph.direction = direction,
                }
            }
            return;
        }
        if let Some(rest) = after_word(line, "subgraph") {
            self.open_group(rest.trim());
            return;
        }
        if line == "end" && self.close_group() {
            return;
        }
        self.chain(line);
    }
}

/// Read a source, whichever of the two syntaxes it is written in.
///
/// A state diagram is a flowchart with a different spelling, so the two parsers
/// meet here and everything after this point is one pipeline.
pub fn read(source: &str) -> Graph {
    let header = source
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if header
        .get(..STATE_HEADER.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(STATE_HEADER))
    {
        return super::state::parse(source);
    }
    parse(source)
}

/// Parse a flowchart source.
pub fn parse(source: &str) -> Graph {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
        .collect();
    let mut reader = Reader::default();
    if let Some(header) = lines.first() {
        for keyword in HEADERS {
            if let Some(rest) = after_word(header, keyword) {
                if let Some(direction) = Direction::from_keyword(rest.trim()) {
                    reader.graph.direction = direction;
                }
                break;
            }
        }
    }
    for line in lines.iter().skip(1) {
        reader.line(line);
    }
    // A group left open still draws; the source simply forgot to close it.
    while reader.close_group() {}
    reader.graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flowchart::types::{EdgeStyle, Shape};

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
    fn a_state_diagram_is_read_by_the_other_parser() {
        let out = read("stateDiagram-v2\n[*] --> Idle");
        assert_eq!(out.nodes.len(), 2);
        assert_eq!(out.nodes[0].shape, crate::flowchart::Shape::StateStart);
        // And a flowchart still goes the usual way.
        let flow = read("graph TD\nA[Start] --> B");
        assert_eq!(flow.nodes[0].label, "Start");
        // Whatever case the header is written in.
        assert_eq!(read("STATEDIAGRAM\n[*] --> Idle").nodes.len(), 2);
        assert_eq!(read("").nodes.len(), 0);
    }

    #[test]
    fn a_header_names_the_direction() {
        assert_eq!(parse("graph TD\nA --> B").direction, Direction::Down);
        assert_eq!(parse("flowchart LR\nA --> B").direction, Direction::Right);
        assert_eq!(parse("graph BT\nA --> B").direction, Direction::Up);
        assert_eq!(parse("FLOWCHART rl\nA --> B").direction, Direction::Left);
        // No direction, and the default stands.
        assert_eq!(parse("graph\nA --> B").direction, Direction::Down);
    }

    #[test]
    fn a_line_names_its_nodes_and_the_arrow_between_them() {
        let out = parse("graph TD\nA[Start] --> B[Process]");
        assert_eq!(ids(&out), ["A", "B"]);
        assert_eq!(out.nodes[0].label, "Start");
        assert_eq!(wires(&out), [("A", "B", "")]);
        assert!(out.edges[0].head_end);
        assert!(!out.edges[0].head_start);
    }

    #[test]
    fn a_chain_on_one_line_is_read_end_to_end() {
        let out = parse("graph TD\nA[Start] --> B[Process] --> C[End]");
        assert_eq!(ids(&out), ["A", "B", "C"]);
        assert_eq!(wires(&out), [("A", "B", ""), ("B", "C", "")]);
    }

    #[test]
    fn an_ampersand_joins_every_source_to_every_target() {
        let out = parse("graph TD\nA & B --> C & D");
        assert_eq!(ids(&out), ["A", "B", "C", "D"]);
        assert_eq!(
            wires(&out),
            [
                ("A", "C", ""),
                ("A", "D", ""),
                ("B", "C", ""),
                ("B", "D", "")
            ]
        );
    }

    #[test]
    fn an_edge_carries_a_label_in_either_spelling() {
        let piped = parse("graph TD\nA -->|Yes| B");
        assert_eq!(wires(&piped), [("A", "B", "Yes")]);
        let middle = parse("graph TD\nA -- No --> B");
        assert_eq!(wires(&middle), [("A", "B", "No")]);
    }

    #[test]
    fn every_edge_style_survives_the_read() {
        let out = parse("graph TD\nA --> B\nB -.-> C\nC ==> D\nD --- E");
        let styles: Vec<EdgeStyle> = out.edges.iter().map(|edge| edge.style).collect();
        assert_eq!(
            styles,
            [
                EdgeStyle::Solid,
                EdgeStyle::Dotted,
                EdgeStyle::Thick,
                EdgeStyle::Solid
            ]
        );
        assert!(!out.edges[3].head_end, "`---` has no head");
    }

    #[test]
    fn a_node_named_again_later_keeps_the_label_it_was_given() {
        let out = parse("graph TD\nA[Start] --> B\nA --> C");
        assert_eq!(out.nodes[0].label, "Start");
        assert_eq!(ids(&out), ["A", "B", "C"]);
    }

    #[test]
    fn a_node_labelled_later_takes_the_label() {
        let out = parse("graph TD\nA --> B\nA[Start]");
        assert_eq!(out.nodes[0].label, "Start");
        assert_eq!(out.nodes[0].shape, Shape::Rectangle);
    }

    #[test]
    fn a_shape_is_read_from_the_delimiters_round_its_label() {
        let out = parse("graph TD\nA{Ready?} --> B([Go]) --> C[(Store)]");
        let shapes: Vec<Shape> = out.nodes.iter().map(|node| node.shape).collect();
        assert_eq!(shapes, [Shape::Diamond, Shape::Stadium, Shape::Cylinder]);
    }

    #[test]
    fn a_subgraph_holds_the_nodes_declared_inside_it() {
        let out = parse("graph TD\nA --> B\nsubgraph Inner\nC --> D\nend\nD --> A");
        assert_eq!(out.groups.len(), 1);
        assert_eq!(out.groups[0].nodes, ["C", "D"]);
        assert_eq!(out.groups[0].label, "Inner");
        assert_eq!(out.groups[0].id, "Inner");
    }

    #[test]
    fn a_subgraph_may_name_itself_and_its_label_apart() {
        let out = parse("graph TD\nsubgraph us-east [US East]\nA --> B\nend");
        assert_eq!(out.groups[0].id, "us-east");
        assert_eq!(out.groups[0].label, "US East");
    }

    #[test]
    fn a_subgraph_labelled_with_words_makes_itself_an_id() {
        let out = parse("graph TD\nsubgraph A group!\nA --> B\nend");
        assert_eq!(out.groups[0].label, "A group!");
        assert_eq!(out.groups[0].id, "A_group");
    }

    #[test]
    fn subgraphs_nest_innermost_first() {
        let out = parse("graph TD\nsubgraph Outer\nsubgraph Inner\nA --> B\nend\nC --> D\nend");
        assert_eq!(out.groups.len(), 2);
        assert_eq!(out.groups[0].label, "Inner");
        assert_eq!(out.groups[1].label, "Outer");
        assert_eq!(out.groups[1].groups, [0], "the outer one holds the inner");
        assert_eq!(out.groups[0].nodes, ["A", "B"]);
        assert_eq!(out.groups[1].nodes, ["C", "D"]);
    }

    #[test]
    fn a_direction_inside_a_group_belongs_to_that_group() {
        let out = parse("graph TD\nsubgraph Inner\ndirection LR\nA --> B\nend");
        assert_eq!(out.groups[0].direction, Some(Direction::Right));
        assert_eq!(out.direction, Direction::Down, "the drawing is unchanged");
    }

    #[test]
    fn a_direction_outside_a_group_belongs_to_the_drawing() {
        let out = parse("graph TD\ndirection LR\nA --> B");
        assert_eq!(out.direction, Direction::Right);
    }

    #[test]
    fn a_group_left_open_still_draws() {
        let out = parse("graph TD\nsubgraph Inner\nA --> B");
        assert_eq!(out.groups.len(), 1);
        assert_eq!(out.groups[0].nodes, ["A", "B"]);
    }

    #[test]
    fn an_end_with_no_group_open_is_read_as_a_node() {
        // The reference does the same: `end` only closes when something is open.
        let out = parse("graph TD\nend\nA --> B");
        assert!(out.groups.is_empty());
        assert!(ids(&out).contains(&"end"));
    }

    #[test]
    fn a_class_definition_and_its_assignment_are_both_read() {
        let out = parse("graph TD\nA --> B\nclassDef warn fill:#fee,stroke:#f00\nclass A,B warn");
        assert_eq!(out.class_defs.len(), 1);
        assert_eq!(out.class_defs[0].0, "warn");
        assert_eq!(
            out.class_defs[0].1.props,
            [
                ("fill".to_string(), "#fee".to_string()),
                ("stroke".to_string(), "#f00".to_string())
            ]
        );
        assert_eq!(out.nodes[0].classes, ["warn"]);
        assert_eq!(out.nodes[1].classes, ["warn"]);
    }

    #[test]
    fn a_style_line_is_kept_against_the_node_it_names() {
        let out = parse("graph TD\nA --> B\nstyle A fill:#f00");
        assert_eq!(out.node_styles.len(), 1);
        assert_eq!(out.node_styles[0].0, "A");
        assert_eq!(out.node_styles[0].1.props[0].1, "#f00");
    }

    #[test]
    fn a_link_style_names_either_one_edge_or_all_of_them() {
        let out = parse(
            "graph TD\nA --> B\nB --> C\nlinkStyle 0,1 stroke:#f00\nlinkStyle default stroke:#00f",
        );
        assert_eq!(out.link_styles.len(), 3);
        assert_eq!(out.link_styles[0].0, LinkTarget::At(0));
        assert_eq!(out.link_styles[1].0, LinkTarget::At(1));
        assert_eq!(out.link_styles[2].0, LinkTarget::Every);
    }

    #[test]
    fn a_property_with_no_value_is_dropped_rather_than_kept_empty() {
        let out = parse("graph TD\nA --> B\nstyle A fill:,stroke:#333,:red");
        assert_eq!(
            out.node_styles[0].1.props,
            [("stroke".to_string(), "#333".to_string())]
        );
    }

    #[test]
    fn a_comment_and_a_blank_line_are_dropped_before_reading() {
        let out = parse("graph TD\n\n%% a note\n  A --> B  \n");
        assert_eq!(wires(&out), [("A", "B", "")]);
    }

    #[test]
    fn a_source_of_nothing_reads_to_nothing() {
        assert_eq!(parse("graph TD"), Graph::default());
        assert_eq!(parse(""), Graph::default());
    }

    #[test]
    fn a_line_that_names_nothing_draws_nothing() {
        let out = parse("graph TD\n-->\n&\nA --> B");
        assert_eq!(wires(&out), [("A", "B", "")]);
    }

    #[test]
    fn a_line_that_trails_off_keeps_what_it_managed_to_say() {
        // An arrow with nothing after it.
        let dangling = parse("graph TD\nA -->");
        assert_eq!(ids(&dangling), ["A"]);
        assert!(dangling.edges.is_empty());
        // And two names with no arrow between them: the line stops at the second.
        let stray = parse("graph TD\nA B --> C");
        assert_eq!(ids(&stray), ["A"]);
        assert!(stray.edges.is_empty());
    }

    #[test]
    fn an_arrow_pointing_both_ways_says_so() {
        let out = parse("graph TD\nA <--> B");
        assert!(out.edges[0].head_start && out.edges[0].head_end);
    }
}
