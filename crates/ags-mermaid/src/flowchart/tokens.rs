//! The two things a flowchart line is made of: a node, and an arrow.
//!
//! Both are read from the front of a line and hand back what is left, because a
//! line may chain them — `A --> B --> C` is two arrows and three nodes, read in
//! one pass.
//!
//! The delimiters are tried longest first. `(((` has to beat `((`, which has to
//! beat `(`, and `([` has to beat both — otherwise a stadium is read as a
//! rounded box containing a stray bracket.

use super::types::{EdgeStyle, Shape};

/// Every pair of delimiters, longest opening first.
const DELIMITERS: [(&str, &str, Shape); 12] = [
    ("(((", ")))", Shape::DoubleCircle),
    ("((", "))", Shape::Circle),
    ("([", "])", Shape::Stadium),
    ("[[", "]]", Shape::Subroutine),
    ("[(", ")]", Shape::Cylinder),
    ("{{", "}}", Shape::Hexagon),
    ("[/", "\\]", Shape::Trapezoid),
    ("[\\", "/]", Shape::TrapezoidAlt),
    ("[", "]", Shape::Rectangle),
    ("(", ")", Shape::Rounded),
    ("{", "}", Shape::Diamond),
    (">", "]", Shape::Asymmetric),
];

/// Whether `c` may appear in a node's name.
///
/// Letters from any script, because a state diagram is as likely to be written
/// in Chinese as in English, and the reference's `\p{L}` says so.
fn is_name(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// How far a node's name runs.
///
/// A name may contain a dash — `my-node` is one node — but `A-->B` is two and
/// an arrow, written without spaces. The two are told apart by what follows the
/// dash: a character that could continue an arrow operator ends the name.
fn name_end(text: &str) -> usize {
    let mut end = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if at == 0 && !(c.is_alphanumeric() || c == '_') {
            return 0;
        }
        if !is_name(c) {
            break;
        }
        if matches!(c, '-' | '=') {
            let next = chars.peek().map(|(_, c)| *c);
            if next.is_some_and(|c| matches!(c, '-' | '.' | '=' | '>')) {
                break;
            }
        }
        end = at + c.len_utf8();
    }
    end
}

/// One node as it was written: its name, its label, and its shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeToken {
    pub id: String,
    /// The text between the delimiters, or the name when there were none.
    pub label: String,
    pub shape: Shape,
    /// Whether the line gave a label, as against naming a node declared earlier.
    pub declared: bool,
}

/// Read a node from the front of `text`, and hand back what follows it.
pub fn node(text: &str) -> Option<(NodeToken, &str)> {
    let (id, rest) = text.split_at(name_end(text));
    if id.is_empty() {
        return None;
    }
    for (open, close, shape) in DELIMITERS {
        let Some(inner) = rest.strip_prefix(open) else {
            continue;
        };
        let Some(at) = inner.find(close) else {
            continue;
        };
        let (label, after) = inner.split_at(at);
        return Some((
            NodeToken {
                id: id.to_string(),
                label: crate::text::normalize_label(label),
                shape,
                declared: true,
            },
            after.get(close.len()..).unwrap_or("").trim_start(),
        ));
    }
    Some((
        NodeToken {
            id: id.to_string(),
            label: id.to_string(),
            shape: Shape::default(),
            declared: false,
        },
        rest.trim_start(),
    ))
}

/// One arrow as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrowToken {
    pub style: EdgeStyle,
    pub head_start: bool,
    pub head_end: bool,
    pub label: String,
}

/// The arrow operators, longest first so `-->` is not read as `--` and a stray
/// `>`.
const OPERATORS: [(&str, EdgeStyle, bool); 5] = [
    ("-->", EdgeStyle::Solid, true),
    ("==>", EdgeStyle::Thick, true),
    ("---", EdgeStyle::Solid, false),
    ("-.-", EdgeStyle::Dotted, false),
    ("===", EdgeStyle::Thick, false),
];

/// The dotted arrow, which the reference spells with an unescaped `.`.
///
/// So `-x->` is an arrow too. Kept, because a diagram in the wild may be leaning
/// on it and nothing is gained by being stricter than the renderer we replace.
fn dotted_arrow(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('-')?;
    let mut chars = rest.chars();
    chars.next()?;
    let rest = chars.as_str().strip_prefix('-')?;
    rest.strip_prefix('>')
}

/// `|label|` immediately after an arrow.
fn piped(text: &str) -> (String, &str) {
    let Some(rest) = text.strip_prefix('|') else {
        return (String::new(), text);
    };
    let Some(at) = rest.find('|') else {
        return (String::new(), text);
    };
    let (label, after) = rest.split_at(at);
    (
        crate::text::normalize_label(label.trim()),
        after.get(1..).unwrap_or("").trim_start(),
    )
}

/// Read an arrow from the front of `text`, and hand back what follows it.
pub fn arrow(text: &str) -> Option<(ArrowToken, &str)> {
    let (head_start, rest) = text
        .strip_prefix('<')
        .map_or((false, text), |tail| (true, tail));
    // The dotted arrow is tried first because its middle character is a
    // wildcard, and `-->` would otherwise be read as one.
    let (style, head_end, rest) = if let Some(tail) = rest.strip_prefix("-->") {
        (EdgeStyle::Solid, true, tail)
    } else if let Some(tail) = dotted_arrow(rest) {
        (EdgeStyle::Dotted, true, tail)
    } else {
        let found = OPERATORS.iter().skip(1).find_map(|(op, style, head)| {
            rest.strip_prefix(op).map(|tail| (*style, *head, tail))
        })?;
        found
    };
    let (label, rest) = piped(rest.trim_start());
    Some((
        ArrowToken {
            style,
            head_start,
            head_end,
            label,
        },
        rest.trim_start(),
    ))
}

/// An arrow written with its label in the middle: `A -- yes --> B`.
///
/// The two halves may disagree about the style; either one saying dotted or
/// thick makes it so, which is the reference's rule.
pub fn text_arrow(text: &str) -> Option<(ArrowToken, &str)> {
    let (head_start, rest) = text
        .strip_prefix('<')
        .map_or((false, text), |tail| (true, tail));
    let (open, rest) = ["--", "-.", "=="]
        .iter()
        .find_map(|op| rest.strip_prefix(op).map(|tail| (*op, tail)))?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    let (close, at) = ["-->", "---", ".->", "-.-", "==>", "==="]
        .iter()
        .filter_map(|op| rest.find(op).map(|at| (*op, at)))
        .min_by_key(|(_, at)| *at)?;
    let (label, after) = rest.split_at(at);
    if !label.ends_with(char::is_whitespace) {
        return None;
    }
    let style = if open == "-." || close == ".->" || close == "-.-" {
        EdgeStyle::Dotted
    } else if open == "==" || close == "==>" || close == "===" {
        EdgeStyle::Thick
    } else {
        EdgeStyle::Solid
    };
    Some((
        ArrowToken {
            style,
            head_start,
            head_end: close.ends_with('>'),
            label: crate::text::normalize_label(label.trim()),
        },
        after.get(close.len()..).unwrap_or("").trim_start(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> NodeToken {
        node(text).map_or_else(
            || NodeToken {
                id: String::new(),
                label: String::new(),
                shape: Shape::default(),
                declared: false,
            },
            |(token, _)| token,
        )
    }

    #[test]
    fn every_pair_of_delimiters_names_its_shape() {
        for (source, shape) in [
            ("A[Start]", Shape::Rectangle),
            ("A(Start)", Shape::Rounded),
            ("A{Start}", Shape::Diamond),
            ("A([Start])", Shape::Stadium),
            ("A((Start))", Shape::Circle),
            ("A[[Start]]", Shape::Subroutine),
            ("A(((Start)))", Shape::DoubleCircle),
            ("A{{Start}}", Shape::Hexagon),
            ("A[(Start)]", Shape::Cylinder),
            ("A>Start]", Shape::Asymmetric),
            ("A[/Start\\]", Shape::Trapezoid),
            ("A[\\Start/]", Shape::TrapezoidAlt),
        ] {
            let token = read(source);
            assert_eq!(token.shape, shape, "{source}");
            assert_eq!(token.label, "Start", "{source}");
            assert_eq!(token.id, "A", "{source}");
            assert!(token.declared, "{source}");
        }
    }

    #[test]
    fn a_longer_delimiter_wins_over_the_one_inside_it() {
        // `(((` must beat `((` must beat `(`, and `([` must beat both.
        assert_eq!(read("A(((x)))").shape, Shape::DoubleCircle);
        assert_eq!(read("A((x))").shape, Shape::Circle);
        assert_eq!(read("A(x)").shape, Shape::Rounded);
        assert_eq!(read("A([x])").shape, Shape::Stadium);
        assert_eq!(read("A[[x]]").shape, Shape::Subroutine);
        assert_eq!(read("A[(x)]").shape, Shape::Cylinder);
        assert_eq!(read("A[x]").shape, Shape::Rectangle);
    }

    #[test]
    fn a_bare_name_is_a_rectangle_labelled_with_itself() {
        let token = read("Alice");
        assert_eq!(token.id, "Alice");
        assert_eq!(token.label, "Alice");
        assert_eq!(token.shape, Shape::Rectangle);
        assert!(!token.declared, "no label was given");
    }

    #[test]
    fn a_name_may_be_written_in_any_script() {
        assert_eq!(read("启动[开始]").id, "启动");
        assert_eq!(read("启动[开始]").label, "开始");
    }

    #[test]
    fn a_label_keeps_its_line_breaks_and_loses_its_markup() {
        assert_eq!(read("A[one<br>two]").label, "one\ntwo");
        assert_eq!(read("A[**bold**]").label, "<b>bold</b>");
    }

    #[test]
    fn a_node_hands_back_what_follows_it() {
        let (token, rest) = node("A[Start] --> B").expect("a node");
        assert_eq!(token.id, "A");
        assert_eq!(rest, "--> B");
        let (bare, rest) = node("A --> B").expect("a node");
        assert_eq!(bare.id, "A");
        assert_eq!(rest, "--> B");
    }

    #[test]
    fn an_unclosed_delimiter_is_not_a_shape() {
        // Nothing closes it, so the name stands alone and the rest is left.
        let (token, rest) = node("A[Start").expect("a node");
        assert_eq!(token.shape, Shape::Rectangle);
        assert!(!token.declared);
        assert_eq!(rest, "[Start");
    }

    #[test]
    fn a_line_that_opens_with_no_name_is_not_a_node() {
        assert_eq!(node("--> B"), None);
        assert_eq!(node("==> B"), None);
        assert_eq!(node(""), None);
    }

    #[test]
    fn a_name_may_hold_a_dash_without_swallowing_an_arrow() {
        let (token, rest) = node("my-node-->other").expect("a node");
        assert_eq!(token.id, "my-node");
        assert_eq!(rest, "-->other");
        // And an arrow written without spaces still ends the name.
        let (tight, rest) = node("A-->B").expect("a node");
        assert_eq!(tight.id, "A");
        assert_eq!(rest, "-->B");
    }

    #[test]
    fn every_arrow_operator_is_read() {
        for (source, style, head) in [
            ("-->", EdgeStyle::Solid, true),
            ("---", EdgeStyle::Solid, false),
            ("-.->", EdgeStyle::Dotted, true),
            ("-.-", EdgeStyle::Dotted, false),
            ("==>", EdgeStyle::Thick, true),
            ("===", EdgeStyle::Thick, false),
        ] {
            let (token, _) = arrow(source).unwrap_or_else(|| panic!("{source}"));
            assert_eq!(token.style, style, "{source}");
            assert_eq!(token.head_end, head, "{source}");
            assert!(!token.head_start, "{source}");
        }
    }

    #[test]
    fn an_arrow_may_point_both_ways() {
        let (token, _) = arrow("<-->").expect("an arrow");
        assert!(token.head_start && token.head_end);
    }

    #[test]
    fn an_arrow_takes_a_label_between_bars() {
        let (token, rest) = arrow("-->|Yes| B").expect("an arrow");
        assert_eq!(token.label, "Yes");
        assert_eq!(rest, "B");
    }

    #[test]
    fn a_bar_that_never_closes_is_not_a_label() {
        let (token, rest) = arrow("-->|Yes B").expect("an arrow");
        assert_eq!(token.label, "");
        assert_eq!(rest, "|Yes B");
    }

    #[test]
    fn the_dotted_arrow_takes_whatever_is_in_its_middle() {
        // The reference leaves the `.` unescaped, so this is an arrow too.
        let (token, _) = arrow("-x->").expect("an arrow");
        assert_eq!(token.style, EdgeStyle::Dotted);
    }

    #[test]
    fn something_that_is_not_an_arrow_is_not_read_as_one() {
        assert_eq!(arrow("B"), None);
        assert_eq!(arrow("-"), None);
        assert_eq!(arrow(""), None);
    }

    #[test]
    fn an_arrow_may_carry_its_label_in_the_middle() {
        let (token, rest) = text_arrow("-- Yes --> B").expect("an arrow");
        assert_eq!(token.label, "Yes");
        assert_eq!(token.style, EdgeStyle::Solid);
        assert!(token.head_end);
        assert_eq!(rest, "B");
    }

    #[test]
    fn either_half_of_a_middle_label_may_set_the_style() {
        assert_eq!(
            text_arrow("-. Maybe .-> B").map(|(token, _)| token.style),
            Some(EdgeStyle::Dotted)
        );
        assert_eq!(
            text_arrow("== Always ==> B").map(|(token, _)| token.style),
            Some(EdgeStyle::Thick)
        );
        assert_eq!(
            text_arrow("-- Plain --- B").map(|(token, _)| token.style),
            Some(EdgeStyle::Solid)
        );
    }

    #[test]
    fn a_middle_label_needs_room_on_both_sides() {
        assert_eq!(text_arrow("--Yes--> B"), None, "no space after the opener");
        assert_eq!(
            text_arrow("-- Yes--> B"),
            None,
            "no space before the closer"
        );
    }

    #[test]
    fn a_middle_label_that_never_closes_is_not_an_arrow() {
        assert_eq!(text_arrow("-- Yes B"), None);
        assert_eq!(text_arrow("B"), None);
    }
}
