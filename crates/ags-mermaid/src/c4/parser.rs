//! Reading C4 source into a diagram.
//!
//! The grammar is a sequence of `Name(arg, arg, …)` calls, with boundaries
//! opening a brace block that may nest. Hand-rolled rather than regex-driven,
//! for the same reason as the label rules: a regex engine is a heavy passenger
//! in a WebAssembly build, and the grammar here is small enough not to need one.

use super::types::{
    Boundary, BoundaryKind, Diagram, Element, ElementKind, LayoutConfig, RelDirection,
    Relationship, Variant,
};

/// A `Name(args)` call, with whether the line opened a brace block.
struct Call {
    name: String,
    args: Vec<String>,
    open_brace: bool,
}

/// Parse `Name(args…)` optionally followed by `{`.
fn parse_call(line: &str) -> Option<Call> {
    let open = line.find('(')?;
    let name = line.get(..open)?.trim();
    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
        return None;
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let close = line.rfind(')')?;
    if close < open {
        return None;
    }
    let inner = line.get(open + 1..close)?;
    let tail = line.get(close + 1..)?.trim();
    if !tail.is_empty() && tail != "{" {
        return None;
    }
    Some(Call {
        name: name.to_string(),
        args: split_args(inner),
        open_brace: tail == "{",
    })
}

/// Split a comma-separated argument list, honouring quotes.
///
/// A comma inside quotes is content — labels routinely contain them — and the
/// surrounding quotes are dropped.
///
/// The two "push the character" arms are deliberately separate: inside quotes a
/// comma is content, outside them it separates. Merging them by their shared
/// body would hide that the whole point is *which* characters reach each one.
#[expect(
    clippy::match_same_arms,
    reason = "the arms distinguish quoted from unquoted context, not their bodies"
)]
fn split_args(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, c @ ('"' | '\'')) => quote = Some(c),
            (None, ',') => {
                args.push(cur.trim().to_string());
                cur.clear();
            }
            (None, c) => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !args.is_empty() {
        args.push(cur.trim().to_string());
    }
    args
}

/// Normalise an optional argument, dropping empties and named `$key=value` ones.
fn clean_arg(value: Option<&String>) -> Option<String> {
    let v = value?.trim();
    if v.is_empty() {
        return None;
    }
    // A named argument is styling metadata, not a positional value.
    let named = v.split_once('=').is_some_and(|(key, _)| {
        let k = key.trim().trim_start_matches('$');
        !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    if named {
        return None;
    }
    Some(v.to_string())
}

/// Whether a line opens a diagram, and should be consumed as the header.
fn is_header(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "c4context",
        "c4container",
        "c4component",
        "c4dynamic",
        "c4deployment",
    ]
    .iter()
    .any(|k| lower.starts_with(k))
}

/// Read a positive integer from `$key="n"` anywhere in a line.
fn read_config_value(line: &str, key: &str) -> Option<usize> {
    let lower = line.to_ascii_lowercase();
    let at = lower.find(&format!("${}", key.to_ascii_lowercase()))?;
    let rest = line.get(at + key.len() + 1..)?;
    let digits: String = rest
        .trim_start()
        .trim_start_matches('=')
        .trim_start()
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    // A malformed value leaves the current setting alone rather than collapsing
    // the diagram to zero columns.
    digits.parse().ok().filter(|n| *n > 0)
}

/// Apply `UpdateLayoutConfig` to the running configuration.
fn apply_layout_config(line: &str, config: &mut LayoutConfig) {
    if let Some(n) = read_config_value(line, "c4ShapeInRow") {
        config.shape_in_row = n;
    }
    if let Some(n) = read_config_value(line, "c4BoundaryInRow") {
        config.boundary_in_row = n;
    }
}

/// Map a boundary call name onto a coarse kind.
fn boundary_kind(name: &str) -> BoundaryKind {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("enterprise") {
        BoundaryKind::Enterprise
    } else if lower.starts_with("system") {
        BoundaryKind::System
    } else if lower.starts_with("container") {
        BoundaryKind::Container
    } else {
        BoundaryKind::Deployment
    }
}

/// Whether a call opens a boundary block.
fn is_boundary(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with("_boundary")
        || matches!(
            lower.as_str(),
            "deployment_node" | "node" | "node_l" | "node_r"
        )
}

/// The coarse kind an element call names.
fn element_kind(name: &str) -> Option<ElementKind> {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("person") {
        Some(ElementKind::Person)
    } else if lower.starts_with("system") {
        Some(ElementKind::System)
    } else if lower.starts_with("container") {
        Some(ElementKind::Container)
    } else if lower.starts_with("component") {
        Some(ElementKind::Component)
    } else {
        None
    }
}

/// The storage-shape variant a `*Db` or `*Queue` form carries.
fn variant_of(name: &str) -> Option<Variant> {
    let lower = name.to_ascii_lowercase();
    let base = lower.strip_suffix("_ext").unwrap_or(&lower);
    if base.ends_with("db") {
        Some(Variant::Db)
    } else if base.ends_with("queue") {
        Some(Variant::Queue)
    } else {
        None
    }
}

/// Build an element from a call, if the call names one.
fn parse_element(name: &str, args: &[String], boundary: Option<&String>) -> Option<Element> {
    let kind = element_kind(name)?;
    let alias = args.first()?.trim();
    if alias.is_empty() {
        return None;
    }
    let label = clean_arg(args.get(1)).unwrap_or_else(|| alias.to_string());
    // Containers and components carry a technology string before the description.
    let (techn, descr) = if matches!(kind, ElementKind::Container | ElementKind::Component) {
        (clean_arg(args.get(2)), clean_arg(args.get(3)))
    } else {
        (None, clean_arg(args.get(2)))
    };
    Some(Element {
        alias: alias.to_string(),
        kind,
        variant: variant_of(name),
        label,
        techn,
        descr,
        external: name.to_ascii_lowercase().ends_with("_ext"),
        boundary: boundary.cloned(),
    })
}

/// A direction hint from a `Rel_*` suffix.
fn rel_direction(name: &str) -> Option<RelDirection> {
    let lower = name.to_ascii_lowercase();
    let suffix = lower.rsplit_once('_')?.1;
    match suffix {
        "u" | "up" => Some(RelDirection::Up),
        "d" | "down" => Some(RelDirection::Down),
        "l" | "left" => Some(RelDirection::Left),
        "r" | "right" => Some(RelDirection::Right),
        _ => None,
    }
}

/// Build a relationship from a call, if both endpoints are named.
fn parse_relationship(name: &str, args: &[String]) -> Option<Relationship> {
    let lower = name.to_ascii_lowercase();
    // `RelIndex(index, from, to, …)` shifts every positional argument by one.
    let base = usize::from(lower == "relindex");
    let from = args.get(base)?.trim();
    let to = args.get(base + 1)?.trim();
    if from.is_empty() || to.is_empty() {
        return None;
    }
    Some(Relationship {
        from: from.to_string(),
        to: to.to_string(),
        label: args.get(base + 2).cloned().unwrap_or_default(),
        techn: clean_arg(args.get(base + 3)),
        direction: rel_direction(name),
        bidirectional: lower.starts_with("birel"),
    })
}

/// Parse C4 source into a diagram.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    // Boundaries currently open, innermost last.
    let mut stack: Vec<String> = Vec::new();
    // A boundary whose `{` is on the following line.
    let mut pending: Option<Boundary> = None;

    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") || is_header(line) {
            continue;
        }
        if let Some(rest) = strip_keyword(line, "title") {
            diagram.title = Some(rest.to_string());
            continue;
        }
        if line.to_ascii_lowercase().starts_with("updatelayoutconfig") {
            apply_layout_config(line, &mut diagram.config);
            continue;
        }
        // The rest of the `Update*` family is styling we do not model; consumed
        // here so it cannot be mistaken for an element call.
        if line.to_ascii_lowercase().starts_with("update") {
            continue;
        }
        if line == "{" {
            if let Some(boundary) = pending.take() {
                stack.push(boundary.alias.clone());
                diagram.boundaries.push(boundary);
            }
            continue;
        }
        if line.starts_with('}') {
            stack.pop();
            continue;
        }
        let Some(call) = parse_call(line) else {
            continue;
        };
        if is_boundary(&call.name) {
            let Some(alias) = call.args.first().filter(|a| !a.is_empty()) else {
                continue;
            };
            let boundary = Boundary {
                alias: alias.clone(),
                label: clean_arg(call.args.get(1)).unwrap_or_else(|| alias.clone()),
                kind: boundary_kind(&call.name),
                parent: stack.last().cloned(),
            };
            if call.open_brace {
                stack.push(boundary.alias.clone());
                diagram.boundaries.push(boundary);
            } else {
                pending = Some(boundary);
            }
            continue;
        }
        if call.name.to_ascii_lowercase().starts_with("rel")
            || call.name.to_ascii_lowercase().starts_with("birel")
        {
            if let Some(rel) = parse_relationship(&call.name, &call.args) {
                diagram.relationships.push(rel);
            }
            continue;
        }
        if let Some(element) = parse_element(&call.name, &call.args, stack.last()) {
            diagram.elements.push(element);
        }
    }
    diagram
}

/// The remainder of a line introduced by `keyword `, case-insensitively.
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let (head, rest) = line.split_at_checked(keyword.len())?;
    if !head.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let trimmed = rest.trim_start();
    (trimmed.len() < rest.len() && !trimmed.is_empty()).then_some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relationship_suffix_names_the_way_it_is_drawn() {
        // `Rel_U`, `Rel_Up`, and every other spelling the reference accepts —
        // both the letter and the word, in either case.
        for (name, want) in [
            ("Rel_U", RelDirection::Up),
            ("Rel_Up", RelDirection::Up),
            ("rel_d", RelDirection::Down),
            ("Rel_Down", RelDirection::Down),
            ("Rel_L", RelDirection::Left),
            ("REL_LEFT", RelDirection::Left),
            ("Rel_R", RelDirection::Right),
            ("Rel_Right", RelDirection::Right),
        ] {
            assert_eq!(rel_direction(name), Some(want), "{name}");
        }
    }

    #[test]
    fn a_relationship_without_a_direction_suffix_has_none() {
        assert_eq!(rel_direction("Rel"), None, "nothing after an underscore");
        assert_eq!(rel_direction("Rel_Back"), None, "not a direction");
        assert_eq!(rel_direction("BiRel_x"), None, "an unknown letter");
    }

    #[test]
    fn reads_elements_with_their_parts() {
        let d = parse(
            "C4Container\n\
             Container(web, \"Web App\", \"Java, Spring\", \"Serves pages\")",
        );
        assert_eq!(d.elements.len(), 1);
        let el = &d.elements[0];
        assert_eq!(el.alias, "web");
        assert_eq!(el.kind, ElementKind::Container);
        assert_eq!(el.label, "Web App");
        // A comma inside quotes is content, not a separator.
        assert_eq!(el.techn.as_deref(), Some("Java, Spring"));
        assert_eq!(el.descr.as_deref(), Some("Serves pages"));
        assert!(!el.external);
    }

    #[test]
    fn a_person_has_no_technology_field() {
        // The third argument means different things by kind: a description for a
        // person, a technology for a container.
        let d = parse("C4Context\nPerson(u, \"User\", \"Buys things\")");
        let el = &d.elements[0];
        assert_eq!(el.kind, ElementKind::Person);
        assert_eq!(el.techn, None);
        assert_eq!(el.descr.as_deref(), Some("Buys things"));
    }

    #[test]
    fn external_and_storage_forms_are_recognised() {
        let d = parse(
            "C4Container\n\
             System_Ext(mail, \"Mail\")\n\
             ContainerDb(db, \"Store\", \"Postgres\")\n\
             ContainerQueue_Ext(bus, \"Bus\", \"Kafka\")",
        );
        assert!(d.elements[0].external);
        assert_eq!(d.elements[0].kind, ElementKind::System);
        assert_eq!(d.elements[1].variant, Some(Variant::Db));
        // The variant changes the glyph, not the kind.
        assert_eq!(d.elements[1].kind, ElementKind::Container);
        assert_eq!(d.elements[2].variant, Some(Variant::Queue));
        assert!(d.elements[2].external);
    }

    #[test]
    fn an_element_with_no_label_falls_back_to_its_alias() {
        let d = parse("C4Context\nSystem(lonely)");
        assert_eq!(d.elements[0].label, "lonely");
    }

    #[test]
    fn boundaries_nest_and_claim_their_members() {
        let d = parse(
            "C4Deployment\n\
             Deployment_Node(host, \"Workstation\") {\n\
               Deployment_Node(shell, \"Session\") {\n\
                 Container(proc, \"Process\", \"Rust\")\n\
               }\n\
             }",
        );
        assert_eq!(d.boundaries.len(), 2);
        assert_eq!(d.boundaries[0].parent, None);
        // Without the parent link an outer node holding only other nodes has no
        // members of its own and vanishes.
        assert_eq!(d.boundaries[1].parent.as_deref(), Some("host"));
        assert_eq!(d.elements[0].boundary.as_deref(), Some("shell"));
    }

    #[test]
    fn a_brace_on_the_following_line_still_opens_the_block() {
        let d = parse(
            "C4Container\n\
             System_Boundary(s, \"Sys\")\n\
             {\n\
               Container(c, \"C\", \"Rust\")\n\
             }",
        );
        assert_eq!(d.boundaries.len(), 1);
        assert_eq!(d.elements[0].boundary.as_deref(), Some("s"));
    }

    #[test]
    fn relationships_carry_their_direction_and_arity() {
        let d = parse(
            "C4Context\n\
             Rel(a, b, \"calls\", \"HTTPS\")\n\
             BiRel(b, c, \"syncs\")\n\
             Rel_U(c, a, \"reports\")",
        );
        assert_eq!(d.relationships.len(), 3);
        assert_eq!(d.relationships[0].techn.as_deref(), Some("HTTPS"));
        assert!(!d.relationships[0].bidirectional);
        assert!(d.relationships[1].bidirectional);
        assert_eq!(d.relationships[2].direction, Some(RelDirection::Up));
    }

    #[test]
    fn rel_index_shifts_its_positional_arguments() {
        let d = parse("C4Dynamic\nRelIndex(1, a, b, \"first\")");
        let rel = &d.relationships[0];
        assert_eq!(rel.from, "a");
        assert_eq!(rel.to, "b");
        assert_eq!(rel.label, "first");
    }

    #[test]
    fn layout_config_is_read_and_bad_values_are_ignored() {
        let d =
            parse("C4Component\nUpdateLayoutConfig($c4ShapeInRow=\"3\", $c4BoundaryInRow=\"1\")");
        assert_eq!(d.config.shape_in_row, 3);
        assert_eq!(d.config.boundary_in_row, 1);

        // Zero columns would collapse the diagram; the default must survive.
        let bad = parse("C4Component\nUpdateLayoutConfig($c4ShapeInRow=\"0\")");
        assert_eq!(
            bad.config.shape_in_row,
            LayoutConfig::default().shape_in_row
        );
    }

    #[test]
    fn styling_directives_are_consumed_not_mistaken_for_elements() {
        let d = parse(
            "C4Context\n\
             UpdateRelStyle(a, b, $offsetY=\"10\")\n\
             UpdateElementStyle(a, $bgColor=\"red\")\n\
             System(a, \"A\")",
        );
        assert_eq!(d.elements.len(), 1);
        assert_eq!(d.relationships.len(), 0);
    }

    #[test]
    fn named_arguments_do_not_become_descriptions() {
        let d = parse("C4Container\nContainer(a, \"A\", \"Rust\", $tags=\"x\")");
        assert_eq!(d.elements[0].descr, None);
    }

    #[test]
    fn the_title_is_kept() {
        let d = parse("C4Context\ntitle Système de paiement\nSystem(a, \"A\")");
        assert_eq!(d.title.as_deref(), Some("Système de paiement"));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let d = parse("C4Context\n\n  %% a note\n\nSystem(a, \"A\")\n");
        assert_eq!(d.elements.len(), 1);
    }

    #[test]
    fn a_line_that_is_not_a_call_is_ignored_rather_than_fatal() {
        // Sources in the wild carry stray text; a diagram is better than an error.
        let d = parse("C4Context\nthis is not a call\nSystem(a, \"A\")");
        assert_eq!(d.elements.len(), 1);
    }

    #[test]
    fn empty_source_parses_to_an_empty_diagram() {
        let d = parse("");
        assert_eq!(d, Diagram::default());
    }
}
