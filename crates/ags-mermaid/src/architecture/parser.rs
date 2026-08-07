//! Reading `architecture-beta` source.
//!
//! Four kinds of line: a group, a service, a junction, and an edge. The three
//! declarations share a shape — `<keyword> <id>(<icon>)[<Title>] in <group>` —
//! so they share a reader; only the keyword and what it produces differ.

use super::types::{Diagram, Edge, Item, Kind, Side};

/// The header, with or without the `-beta` the grammar is still shipped under.
const HEADER: &str = "architecture";

/// Every arrow, longest first so `<-->` is not read as `<-`.
const ARROWS: [&str; 8] = ["<-->", "<--", "-->", "<->", "<-", "->", "--", "-"];

/// Whether a character may appear in an identifier.
fn is_name(letter: char) -> bool {
    letter.is_ascii_alphanumeric() || letter == '_'
}

/// The text after a leading keyword, when the line starts with it as a word.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim())
}

/// The leading identifier, and what follows it.
fn name(text: &str) -> Option<(&str, &str)> {
    let end = text.find(|letter| !is_name(letter)).unwrap_or(text.len());
    let id = text.get(..end)?;
    (!id.is_empty()).then_some((id, text.get(end..)?.trim_start()))
}

/// A bracketed run at the start of the text, and what follows it.
fn bracketed(text: &str, open: char, close: char) -> (&str, &str) {
    let Some(body) = text.strip_prefix(open) else {
        return ("", text);
    };
    match body.find(close) {
        Some(end) => (
            body.get(..end).unwrap_or("").trim(),
            body.get(end + close.len_utf8()..)
                .unwrap_or("")
                .trim_start(),
        ),
        None => ("", text),
    }
}

/// A declaration: `<keyword> <id>(<icon>)[<Title>] [in <group>]`.
///
/// The icon, the title and the group are each optional and always in that
/// order, which is what lets one reader serve all three keywords.
fn declaration(line: &str, keyword: &str, kind: Kind) -> Option<Item> {
    let rest = after_keyword(line, keyword)?;
    let (id, rest) = name(rest)?;
    let (icon, rest) = bracketed(rest, '(', ')');
    let (title, rest) = bracketed(rest, '[', ']');
    let parent = match after_keyword(rest, "in") {
        Some(text) => name(text).map(|(group, _)| group).unwrap_or_default(),
        None => "",
    };
    Some(Item {
        id: id.to_string(),
        kind,
        icon: icon.to_string(),
        title: if title.is_empty() {
            id.to_string()
        } else {
            crate::text::normalize_label(title)
        },
        parent: parent.to_string(),
    })
}

/// Where an arrow sits in a line, and what it is.
///
/// No whitespace is required round it: an identifier is letters, digits and
/// underscores, and a side is one letter after a colon, so none of `-`, `<` or
/// `>` can appear anywhere else on the line.
fn find_arrow(line: &str) -> Option<(usize, &'static str)> {
    for (at, _) in line.char_indices() {
        let tail = line.get(at..)?;
        if let Some(arrow) = ARROWS.into_iter().find(|arrow| tail.starts_with(arrow)) {
            return Some((at, arrow));
        }
    }
    None
}

/// One end of an edge: `id`, `id:S`, `S:id`, and any of them with a `{group}`
/// marker that the layout has no use for.
///
/// `leading` says which order to expect: the side comes after the name at the
/// source end and before it at the target end.
fn endpoint(text: &str, leading: bool) -> Option<(String, Option<Side>)> {
    let text = text.trim();
    let (head, tail) = match text.split_once(':') {
        Some((head, tail)) => (head.trim(), tail.trim()),
        None => (text, ""),
    };
    let (written, side_text) = if leading && !tail.is_empty() {
        (tail, head)
    } else {
        (head, tail)
    };
    // `a{grp}` names a group the endpoint belongs to, which the placement
    // already knows from the declaration.
    let (id, _) = name(written)?;
    let side = side_text.chars().next().and_then(Side::from_letter);
    Some((id.to_string(), side))
}

/// An edge line: `a:R --> L:b`, or any of it left out but the arrow.
pub fn edge(line: &str) -> Option<Edge> {
    let (at, arrow) = find_arrow(line)?;
    let (from, from_side) = endpoint(line.get(..at)?, false)?;
    let (to, to_side) = endpoint(line.get(at + arrow.len()..)?, true)?;
    Some(Edge {
        from,
        from_side,
        to,
        to_side,
        arrow_start: arrow.starts_with('<'),
        arrow_end: arrow.ends_with('>'),
    })
}

/// Read an architecture diagram.
///
/// A line nobody recognises is dropped rather than rejected: a diagram with one
/// bad line should still draw the rest of itself.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with("%%")
            || line.to_ascii_lowercase().starts_with(HEADER)
        {
            continue;
        }
        if let Some(item) = declaration(line, "group", Kind::Group)
            .or_else(|| declaration(line, "service", Kind::Service))
            .or_else(|| declaration(line, "junction", Kind::Junction))
        {
            diagram.items.push(item);
            continue;
        }
        if let Some(found) = edge(line) {
            diagram.edges.push(found);
        }
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keyword_has_to_be_a_whole_word() {
        assert_eq!(after_keyword("group cloud", "group"), Some("cloud"));
        assert_eq!(after_keyword("grouped cloud", "group"), None);
        assert_eq!(after_keyword("group", "group"), None);
    }

    #[test]
    fn a_name_stops_at_the_first_character_it_cannot_hold() {
        assert_eq!(
            name("cloud(cloud)[Cloud]"),
            Some(("cloud", "(cloud)[Cloud]"))
        );
        assert_eq!(name("web_1 in net"), Some(("web_1", "in net")));
        assert_eq!(name("(nothing)"), None);
        assert_eq!(name(""), None);
    }

    #[test]
    fn a_bracketed_run_is_taken_only_when_it_opens_the_text() {
        assert_eq!(bracketed("(cloud)[Cloud]", '(', ')'), ("cloud", "[Cloud]"));
        assert_eq!(bracketed("[Cloud]", '(', ')'), ("", "[Cloud]"));
        // Unclosed, so nothing is taken and the text is left whole.
        assert_eq!(bracketed("(cloud", '(', ')'), ("", "(cloud"));
        assert_eq!(bracketed("()rest", '(', ')'), ("", "rest"));
    }

    #[test]
    fn a_group_is_read_with_its_glyph_its_title_and_its_parent() {
        let item = declaration(
            "group region(server)[Region A] in cloud",
            "group",
            Kind::Group,
        )
        .expect("a group");
        assert_eq!(item.id, "region");
        assert_eq!(item.kind, Kind::Group);
        assert_eq!(item.icon, "server");
        assert_eq!(item.title, "Region A");
        assert_eq!(item.parent, "cloud");
    }

    #[test]
    fn everything_after_the_name_is_optional() {
        let bare = declaration("group cloud", "group", Kind::Group).expect("a group");
        assert_eq!(bare.icon, "");
        // With nothing written, a thing is drawn under its own name.
        assert_eq!(bare.title, "cloud");
        assert_eq!(bare.parent, "");
        let titled = declaration("service db[DB]", "service", Kind::Service).expect("a service");
        assert_eq!(titled.title, "DB");
        assert_eq!(titled.icon, "");
        let junction = declaration("junction j in net", "junction", Kind::Junction).expect("one");
        assert_eq!(junction.id, "j");
        assert_eq!(junction.parent, "net");
    }

    #[test]
    fn a_declaration_of_another_kind_is_not_read_as_this_one() {
        assert_eq!(declaration("service web", "group", Kind::Group), None);
        assert_eq!(declaration("group", "group", Kind::Group), None);
    }

    #[test]
    fn every_arrow_is_found_and_the_longest_wins() {
        assert_eq!(find_arrow("a:R -- L:b"), Some((4, "--")));
        assert_eq!(find_arrow("a:R --> L:b"), Some((4, "-->")));
        assert_eq!(find_arrow("a <--> b"), Some((2, "<-->")));
        assert_eq!(find_arrow("a:R<--L:b"), Some((3, "<--")));
        assert_eq!(find_arrow("a b"), None);
    }

    #[test]
    fn an_endpoint_reads_its_side_from_whichever_end_it_is() {
        assert_eq!(
            endpoint("web:R", false),
            Some(("web".to_string(), Some(Side::Right)))
        );
        assert_eq!(
            endpoint(" L:db", true),
            Some(("db".to_string(), Some(Side::Left)))
        );
        // No side written at all.
        assert_eq!(endpoint("web", false), Some(("web".to_string(), None)));
        assert_eq!(endpoint("db", true), Some(("db".to_string(), None)));
        // A group marker is tolerated and has nothing to add.
        assert_eq!(
            endpoint("web{cloud}:R", false),
            Some(("web".to_string(), Some(Side::Right)))
        );
        assert_eq!(endpoint("", false), None);
    }

    #[test]
    fn an_edge_carries_both_sides_and_both_arrowheads() {
        let plain = edge("web:R -- L:db").expect("an edge");
        assert_eq!(plain.from, "web");
        assert_eq!(plain.from_side, Some(Side::Right));
        assert_eq!(plain.to, "db");
        assert_eq!(plain.to_side, Some(Side::Left));
        assert!(!plain.arrow_start);
        assert!(!plain.arrow_end);
        let directed = edge("cdn:L --> T:web").expect("an edge");
        assert!(directed.arrow_end);
        assert!(!directed.arrow_start);
        let back = edge("a <-- b").expect("an edge");
        assert!(back.arrow_start);
        assert!(!back.arrow_end);
        let both = edge("a <--> b").expect("an edge");
        assert!(both.arrow_start && both.arrow_end);
    }

    #[test]
    fn a_line_with_no_arrow_is_not_an_edge() {
        assert_eq!(edge("service web"), None);
        assert_eq!(edge("web db"), None);
    }

    #[test]
    fn a_whole_diagram_reads_in_declaration_order() {
        let diagram = parse(
            "architecture-beta\n    group cloud(cloud)[Cloud]\n    service web(server)[Web] in cloud\n    service db(database)[DB] in cloud\n    web:R -- L:db",
        );
        assert_eq!(
            diagram
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<String>>(),
            ["cloud", "web", "db"]
        );
        assert_eq!(diagram.items.first().expect("a group").kind, Kind::Group);
        assert_eq!(diagram.edges.len(), 1);
    }

    #[test]
    fn the_header_is_skipped_with_or_without_its_suffix() {
        assert_eq!(parse("architecture-beta\n  service a").items.len(), 1);
        assert_eq!(parse("architecture\n  service a").items.len(), 1);
    }

    #[test]
    fn comments_blank_lines_and_nonsense_are_stepped_over() {
        let diagram = parse(
            "architecture-beta\n\n  %% a note\n  service a\n  ??? nothing ???\n  service b\n  a -- b",
        );
        assert_eq!(diagram.items.len(), 2);
        assert_eq!(diagram.edges.len(), 1);
    }
}
