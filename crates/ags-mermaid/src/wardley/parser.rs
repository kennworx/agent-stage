//! Reading `wardley-beta` source.
//!
//! ```text
//! wardley-beta | wardley
//!   title <text>
//!   component <Name> [<visibility>, <evolution>]
//!   anchor <Name> [<visibility>, <evolution>]
//!   <Name> [<visibility>, <evolution>]          the keyword is optional
//!   A -> B          A --> B          a dependency
//!   A -.-> B                         drawn dashed
//!   A +> B                           a flow of value
//!   A -> B; label                    with an annotation
//! ```
//!
//! Coordinates read **visibility first**, which is the order the syntax uses and
//! the opposite of the screen's — visibility is the vertical one.

use super::types::{Component, Kind, Link, Map, Style};
use crate::keyword::opens_with;

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
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

/// A point outside the plane is pulled back onto its edge.
fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// The `[v, e]` half of a component line, if that is all that is left.
fn coordinates(rest: &str) -> Option<(f64, f64)> {
    let inner = rest.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (a, b) = inner.split_once(',')?;
    let (a, b) = (a.trim(), b.trim());
    if !is_number(a) || !is_number(b) {
        return None;
    }
    Some((clamp01(a.parse().ok()?), clamp01(b.parse().ok()?)))
}

/// A `[component|anchor] Name [v, e]` line.
fn parse_component(line: &str) -> Option<Component> {
    let (kind, rest) = if let Some(rest) = after_keyword(line, "anchor") {
        (Kind::Anchor, rest)
    } else if let Some(rest) = after_keyword(line, "component") {
        (Kind::Component, rest)
    } else {
        (Kind::Component, line)
    };
    // The name runs to the last `[`, so a name may itself contain brackets.
    let open = rest.rfind('[')?;
    let (name, coords) = (rest.get(..open)?, rest.get(open..)?);
    let (visibility, evolution) = coordinates(coords)?;
    let name = unquote(name.trim()).trim();
    if name.is_empty() {
        return None;
    }
    Some(Component {
        name: name.to_string(),
        visibility,
        evolution,
        kind,
    })
}

/// The operators, longest first so `-->` is never read as `->` plus a stray `-`.
const OPERATORS: [(&str, Style); 4] = [
    ("-.->", Style::Dashed),
    ("-->", Style::Solid),
    ("->", Style::Solid),
    ("+>", Style::Flow),
];

/// An `A -> B` line, with an optional `; label`.
fn parse_link(line: &str) -> Option<Link> {
    // The earliest operator wins, and at one position the longest one does —
    // which together is what the reference's lazy match plus ordered
    // alternation comes to.
    let (at, op, style) = (0..line.len()).find_map(|i| {
        let tail = line.get(i..)?;
        OPERATORS
            .iter()
            .find(|(op, _)| tail.starts_with(op))
            .map(|(op, style)| (i, *op, *style))
    })?;
    let from = unquote(line.get(..at)?.trim()).trim();
    let rest = line.get(at + op.len()..)?;
    // A label is separated by a semicolon, so the target cannot contain one.
    let (target, label) = if let Some((target, label)) = rest.split_once(';') {
        (target, Some(label.trim()))
    } else {
        (rest, None)
    };
    let to = unquote(target.trim()).trim();
    if from.is_empty() || to.is_empty() {
        return None;
    }
    Some(Link {
        from: from.to_string(),
        to: to.to_string(),
        label: label.filter(|l| !l.is_empty()).map(str::to_string),
        style,
    })
}

/// Parse a Wardley map. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Map {
    let mut map = Map::default();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || opens_with(line, "wardley-beta") || opens_with(line, "wardley") {
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            let title = unquote(title).trim();
            if !title.is_empty() {
                map.title = Some(title.to_string());
            }
            continue;
        }
        // Components are read first: a name may hold an arrow, but a line
        // ending in coordinates is never a link.
        if let Some(component) = parse_component(line) {
            map.components.push(component);
            continue;
        }
        if let Some(link) = parse_link(line) {
            map.links.push(link);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "wardley-beta\n\
        title Online photo service\n\
        anchor Customer [0.95, 0.63]\n\
        component Website [0.79, 0.61]\n\
        Platform [0.4, 0.75]\n\
        Customer -> Website\n\
        Website -.-> Platform\n\
        Website +> Platform; renders";

    #[test]
    fn a_whole_map_reads() {
        let map = parse(MAP);
        assert_eq!(map.title.as_deref(), Some("Online photo service"));
        assert_eq!(map.components.len(), 3);
        assert_eq!(map.components[0].kind, Kind::Anchor);
        assert_eq!(map.components[1].kind, Kind::Component);
        assert_eq!(map.links.len(), 3);
    }

    #[test]
    fn the_keyword_is_optional_and_defaults_to_a_component() {
        let map = parse("wardley\nBare [0.5, 0.5]");
        assert_eq!(map.components[0].name, "Bare");
        assert_eq!(map.components[0].kind, Kind::Component);
    }

    #[test]
    fn coordinates_read_visibility_first() {
        // The syntax's order is the opposite of the screen's, which is exactly
        // the kind of thing a port gets backwards.
        let c = &parse("wardley\nA [0.9, 0.1]").components[0];
        assert!((c.visibility - 0.9).abs() < 1e-9);
        assert!((c.evolution - 0.1).abs() < 1e-9);
    }

    #[test]
    fn a_coordinate_outside_the_plane_is_pulled_back_onto_it() {
        let c = &parse("wardley\nA [4, -2]").components[0];
        assert!((c.visibility - 1.0).abs() < 1e-9);
        assert!((c.evolution - 0.0).abs() < 1e-9);
    }

    #[test]
    fn each_operator_picks_its_own_style() {
        let styles: Vec<Style> = parse("wardley\nA -> B\nA --> B\nA -.-> B\nA +> B")
            .links
            .iter()
            .map(|l| l.style)
            .collect();
        assert_eq!(
            styles,
            [Style::Solid, Style::Solid, Style::Dashed, Style::Flow]
        );
    }

    #[test]
    fn a_longer_operator_wins_over_a_prefix_of_itself() {
        // `-.->` starts with `-`, and `-->` starts with `--`; reading either as
        // `->` would give the wrong style and a target beginning with a dash.
        let dashed = &parse("wardley\nA -.-> B").links[0];
        assert_eq!(dashed.style, Style::Dashed);
        assert_eq!(dashed.to, "B");
    }

    #[test]
    fn an_annotation_reads_and_an_empty_one_does_not() {
        assert_eq!(
            parse("wardley\nA -> B; because").links[0].label.as_deref(),
            Some("because")
        );
        assert_eq!(parse("wardley\nA -> B;").links[0].label, None);
        assert_eq!(parse("wardley\nA -> B").links[0].label, None);
    }

    #[test]
    fn a_line_ending_in_coordinates_is_a_component_even_with_an_arrow_in_it() {
        let map = parse("wardley\nA -> B [0.5, 0.5]");
        assert_eq!(map.components.len(), 1);
        assert_eq!(map.components[0].name, "A -> B");
        assert!(map.links.is_empty());
    }

    #[test]
    fn a_malformed_component_is_skipped_rather_than_fatal() {
        for source in ["A [0.5]", "A [a, b]", "A [0.5, 0.5", "[0.5, 0.5]"] {
            assert!(
                parse(&format!("wardley\n{source}")).components.is_empty(),
                "{source}"
            );
        }
    }

    #[test]
    fn a_link_missing_an_end_is_refused() {
        assert!(parse("wardley\n-> B").links.is_empty());
        assert!(parse("wardley\nA ->").links.is_empty());
    }

    #[test]
    fn quotes_around_a_name_are_optional() {
        assert_eq!(
            parse("wardley\n\"Quoted\" [0.1, 0.2]").components[0].name,
            "Quoted"
        );
        assert_eq!(parse("wardley\n\"A\" -> \"B\"").links[0].from, "A");
    }

    #[test]
    fn nothing_in_yields_an_empty_map() {
        assert_eq!(parse(""), Map::default());
        assert_eq!(parse("wardley-beta"), Map::default());
    }
}
