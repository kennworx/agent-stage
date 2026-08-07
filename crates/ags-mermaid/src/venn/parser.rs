//! Reading `venn-beta` source.
//!
//! ```text
//! venn-beta | venn
//!   title <text>
//!   set <Id>                    a bareword or a "quoted" name
//!   set <Id>["Display label"]   a separate label to write in the circle
//!   union <A>, <B> [, <C>]      a region where those sets meet
//!   text "<label>"              names whichever set or union came last
//! ```
//!
//! A trailing `:N` size is accepted and ignored — the beta grammar allows it and
//! this layout gives every set the same circle.

use super::types::{Diagram, Set, Union};
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

/// Drop a trailing `:N` or `:N.N` size.
fn strip_size(text: &str) -> &str {
    let Some(at) = text.rfind(':') else {
        return text;
    };
    let Some(tail) = text.get(at + 1..) else {
        return text;
    };
    let digits = tail.trim();
    let numeric = !digits.is_empty()
        && digits
            .split_once('.')
            .map_or(digits.chars().all(|c| c.is_ascii_digit()), |(a, b)| {
                !a.is_empty()
                    && a.chars().all(|c| c.is_ascii_digit())
                    && !b.is_empty()
                    && b.chars().all(|c| c.is_ascii_digit())
            });
    if numeric {
        text.get(..at).unwrap_or(text).trim()
    } else {
        text
    }
}

/// The text inside a trailing `[...]`, and what came before it.
fn split_bracket(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim_end();
    let body = trimmed.strip_suffix(']')?;
    let open = body.rfind('[')?;
    Some((body.get(..open)?, body.get(open + 1..)?))
}

/// The display label written inside a `[...]`.
fn bracket_label(inner: &str) -> String {
    unquote(inner.trim()).trim().to_string()
}

/// A `set` declaration: `Id`, `"Quoted"`, or either with a `["Display"]`.
fn parse_set(raw: &str) -> Option<Set> {
    let text = strip_size(raw);
    let (head, label) = match split_bracket(text) {
        Some((head, inner)) => (head.trim(), Some(bracket_label(inner))),
        None => (text.trim(), None),
    };
    // A quoted id may hold anything; a bare one runs until the first character
    // that could start something else.
    let id = if let Some(rest) = head.strip_prefix('"') {
        let (id, tail) = rest.split_once('"')?;
        if !tail.trim().is_empty() {
            return None;
        }
        id.trim()
    } else {
        if head.contains(|c: char| c.is_whitespace() || matches!(c, '[' | '"' | '\'' | ':')) {
            return None;
        }
        head
    };
    if id.is_empty() {
        return None;
    }
    Some(Set {
        id: id.to_string(),
        // An unlabelled set writes its own name.
        label: label
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| id.to_string()),
    })
}

/// A `union` declaration: two or more members, with an optional `["label"]`.
fn parse_union(raw: &str, sets: &[Set], taken: &[Union]) -> Option<Union> {
    let text = strip_size(raw);
    let (members, label) = match split_bracket(text) {
        Some((head, inner)) => (head.trim(), Some(bracket_label(inner))),
        None => (text.trim(), None),
    };
    let names: Vec<&str> = members
        .split(',')
        .map(|p| unquote(p.trim()).trim())
        .filter(|p| !p.is_empty())
        .collect();
    // One set is not an overlap.
    if names.len() < 2 {
        return None;
    }
    // A member is named by id, or failing that by label — and a name matching
    // neither is kept as written, so the region still says what was meant.
    let set_ids: Vec<String> = names
        .iter()
        .map(|name| {
            sets.iter()
                .find(|s| s.id == *name)
                .or_else(|| sets.iter().find(|s| s.label == *name))
                .map_or_else(|| (*name).to_string(), |s| s.id.clone())
        })
        .collect();
    Some(Union {
        id: unique_id(&set_ids.join("∩"), taken),
        set_ids,
        label: label.filter(|l| !l.is_empty()),
    })
}

/// A `text` declaration: `["label"]`, `"label"`, or a bare label.
fn parse_text(raw: &str) -> Option<String> {
    let label = match split_bracket(raw) {
        Some((head, inner)) if head.trim().is_empty() => bracket_label(inner),
        _ => unquote(raw.trim()).trim().to_string(),
    };
    (!label.is_empty()).then_some(label)
}

/// A union id not already claimed by another region.
fn unique_id(base: &str, taken: &[Union]) -> String {
    if !taken.iter().any(|u| u.id == base) {
        return base.to_string();
    }
    let mut n = 2usize;
    loop {
        let candidate = format!("{base}#{n}");
        if !taken.iter().any(|u| u.id == candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Which declaration a `text` line names.
enum Target {
    Set(usize),
    Union(usize),
    None,
}

/// Parse a Venn diagram. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    let mut current = Target::None;
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || opens_with(line, "venn-beta") || opens_with(line, "venn") {
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            diagram.title = Some(unquote(title).trim().to_string());
            continue;
        }
        if let Some(rest) = after_keyword(line, "set") {
            if let Some(set) = parse_set(rest) {
                diagram.sets.push(set);
                current = Target::Set(diagram.sets.len() - 1);
            }
            continue;
        }
        if let Some(rest) = after_keyword(line, "union") {
            if let Some(union) = parse_union(rest, &diagram.sets, &diagram.unions) {
                diagram.unions.push(union);
                current = Target::Union(diagram.unions.len() - 1);
            }
            continue;
        }
        if let Some(rest) = after_keyword(line, "text") {
            let Some(label) = parse_text(rest) else {
                continue;
            };
            match current {
                Target::Set(i) => {
                    if let Some(set) = diagram.sets.get_mut(i) {
                        set.label = label;
                    }
                }
                Target::Union(i) => {
                    if let Some(union) = diagram.unions.get_mut(i) {
                        union.label = Some(label);
                    }
                }
                Target::None => {}
            }
        }
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIAGRAM: &str = "venn-beta\n\
        title Skills\n\
        set Design\n\
        set Code[\"Engineering\"]\n\
        set Ops\n\
        union Design, Code\n\
        text Both";

    #[test]
    fn a_whole_diagram_reads() {
        let diagram = parse(DIAGRAM);
        assert_eq!(diagram.title.as_deref(), Some("Skills"));
        assert_eq!(diagram.sets.len(), 3);
        assert_eq!(diagram.sets[1].id, "Code");
        assert_eq!(diagram.sets[1].label, "Engineering");
        assert_eq!(diagram.unions.len(), 1);
        assert_eq!(diagram.unions[0].label.as_deref(), Some("Both"));
    }

    #[test]
    fn an_unlabelled_set_writes_its_own_name() {
        assert_eq!(parse("venn\nset Alpha").sets[0].label, "Alpha");
    }

    #[test]
    fn a_quoted_id_may_hold_spaces() {
        let sets = parse("venn\nset \"Two words\"").sets;
        assert_eq!(sets[0].id, "Two words");
        // A bare id may not, so the same line unquoted is refused.
        assert!(parse("venn\nset Two words").sets.is_empty());
    }

    #[test]
    fn a_trailing_size_is_accepted_and_ignored() {
        assert_eq!(parse("venn\nset A:50").sets[0].id, "A");
        assert_eq!(parse("venn\nset A:2.5").sets[0].id, "A");
        // Something that is not a number is part of the id's rejection, not a
        // size — a colon cannot appear in a bare id.
        assert!(parse("venn\nset A:big").sets.is_empty());
    }

    #[test]
    fn a_union_resolves_a_member_by_id_or_by_label() {
        let diagram = parse("venn\nset Code[\"Engineering\"]\nset Ops\nunion Engineering, Ops");
        assert_eq!(diagram.unions[0].set_ids, ["Code", "Ops"]);
        assert_eq!(diagram.unions[0].id, "Code∩Ops");
    }

    #[test]
    fn a_union_member_that_names_nothing_is_kept_as_written() {
        let diagram = parse("venn\nset A\nunion A, Ghost");
        assert_eq!(diagram.unions[0].set_ids, ["A", "Ghost"]);
    }

    #[test]
    fn one_member_is_not_an_overlap() {
        assert!(parse("venn\nset A\nunion A").unions.is_empty());
    }

    #[test]
    fn two_identical_unions_get_distinct_ids() {
        let diagram = parse("venn\nset A\nset B\nunion A, B\nunion A, B\nunion A, B");
        let ids: Vec<&str> = diagram.unions.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids, ["A∩B", "A∩B#2", "A∩B#3"]);
    }

    #[test]
    fn a_union_label_may_be_written_inline_or_on_a_text_line() {
        assert_eq!(
            parse("venn\nset A\nset B\nunion A, B[\"Overlap\"]").unions[0]
                .label
                .as_deref(),
            Some("Overlap")
        );
        assert_eq!(
            parse("venn\nset A\nset B\nunion A, B\ntext [\"Overlap\"]").unions[0]
                .label
                .as_deref(),
            Some("Overlap")
        );
    }

    #[test]
    fn a_text_line_names_whichever_declaration_came_last() {
        let diagram = parse("venn\nset A\ntext First\nset B\ntext Second");
        assert_eq!(diagram.sets[0].label, "First");
        assert_eq!(diagram.sets[1].label, "Second");
    }

    #[test]
    fn a_text_line_before_any_declaration_names_nothing() {
        // And does not panic looking for something to name.
        assert_eq!(parse("venn\ntext Homeless"), Diagram::default());
    }

    #[test]
    fn nothing_in_yields_an_empty_diagram() {
        assert_eq!(parse(""), Diagram::default());
        assert_eq!(parse("venn-beta"), Diagram::default());
    }
}
