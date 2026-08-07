//! Reading `ishikawa` source.
//!
//! ```text
//! ishikawa[-beta]
//! <effect>            the first content line: the fish's head
//!   <category>        one indent in: a major bone
//!     <cause>         deeper: a cause under that category
//!       <sub-cause>   deeper still, without limit
//! ```
//!
//! **Indentation is the syntax here**, so this parser reads the lines before
//! they are trimmed. Trimming first would make every line a category.

use super::types::{Cause, Diagram};
use crate::keyword::opens_with;
use crate::outline::{attach, level_len, Counted, Outline};

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// How deep a line is indented. A tab advances to the next two-column stop.
fn indent_of(line: &str) -> usize {
    let mut col = 0usize;
    for c in line.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += 2 - (col % 2),
            _ => break,
        }
    }
    col
}

/// The plain text of a line, with an `id["Label"]` wrapper or quotes removed.
fn strip_decorations(text: &str) -> String {
    // `id["Label"]` or `id[Label]`, where the part before the bracket carries
    // neither a bracket nor a quote — which is what makes this unambiguous.
    if let Some(body) = text.strip_suffix(']') {
        if let Some(open) = body.find('[') {
            let (head, inner) = (
                body.get(..open).unwrap_or_default(),
                body.get(open + 1..).unwrap_or_default(),
            );
            if !head.contains(['[', ']', '"']) && !inner.contains([']', '[']) {
                return unquote(inner.trim()).trim().to_string();
            }
        }
    }
    unquote(text).trim().to_string()
}

/// One content line: what it says and how far in it was written.
struct Line {
    text: String,
    indent: usize,
}

/// The content lines, in order, with the header and comments dropped.
fn content(source: &str) -> Vec<Line> {
    source
        .lines()
        .filter_map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("%%")
                || opens_with(trimmed, "ishikawa-beta")
                || opens_with(trimmed, "ishikawa")
            {
                return None;
            }
            Some(Line {
                text: strip_decorations(trimmed),
                indent: indent_of(raw),
            })
        })
        .collect()
}

/// A node while the tree is being built.
struct Raw {
    text: String,
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

/// Build the nesting tree from indent-tagged lines.
fn build(lines: &[Line]) -> Vec<Raw> {
    let mut roots: Vec<Raw> = Vec::new();
    // The chain of open ancestors: a child index and the indent it was at.
    let mut open: Vec<(usize, usize)> = Vec::new();
    for line in lines {
        while open.last().is_some_and(|(_, at)| *at >= line.indent) {
            open.pop();
        }
        let path: Vec<usize> = open.iter().map(|(i, _)| *i).collect();
        let index = level_len(&roots, &path);
        attach(
            &mut roots,
            &path,
            Raw {
                text: line.text.clone(),
                children: Vec::new(),
            },
        );
        open.push((index, line.indent));
    }
    roots
}

/// Turn a built node into a cause.
fn into_cause(raw: Raw) -> Cause {
    Cause {
        text: raw.text,
        causes: raw.children.into_iter().map(into_cause).collect(),
    }
}

/// Parse a fishbone. Reads `source` line by line **without trimming first**.
pub fn parse(source: &str) -> Diagram {
    let lines = content(source);
    let Some(head) = lines.first() else {
        return Diagram::default();
    };
    Diagram {
        effect: head.text.clone(),
        // Whatever follows the effect, at whatever indent — the shallowest of
        // those lines are the categories, however far in they were written.
        categories: build(lines.get(1..).unwrap_or_default())
            .into_iter()
            .map(into_cause)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_and_bracket_are_taken_off_only_when_they_are_unambiguous() {
        assert_eq!(strip_decorations("id[Label]"), "Label");
        assert_eq!(strip_decorations("id[\"Label\"]"), "Label");
        assert_eq!(strip_decorations("  Plain  "), "Plain");
        assert_eq!(strip_decorations("\"Quoted\""), "Quoted");
        // A bracket or quote before the opening bracket makes it ambiguous, so
        // the whole thing is the label rather than a guess at which part is.
        assert_eq!(strip_decorations("a]b[Label]"), "a]b[Label]");
        assert_eq!(strip_decorations("a\"b[Label]"), "a\"b[Label]");
        // As does a bracket inside it.
        assert_eq!(strip_decorations("id[La[bel]"), "id[La[bel]");
    }

    const FISH: &str = "ishikawa\n\
        Late delivery\n  \
          People\n    \
            Understaffed\n      \
              Hiring freeze\n  \
          Process\n    \
            Manual steps";

    #[test]
    fn the_first_content_line_is_the_effect() {
        let diagram = parse(FISH);
        assert_eq!(diagram.effect, "Late delivery");
        assert_eq!(diagram.categories.len(), 2);
        assert_eq!(diagram.categories[0].text, "People");
    }

    #[test]
    fn indentation_nests_causes_without_limit() {
        let diagram = parse(FISH);
        let people = &diagram.categories[0];
        assert_eq!(people.causes[0].text, "Understaffed");
        assert_eq!(people.causes[0].causes[0].text, "Hiring freeze");
    }

    #[test]
    fn a_tab_indents_to_the_next_two_column_stop() {
        let tabbed = parse("ishikawa\nEffect\n\tCategory");
        let spaced = parse("ishikawa\nEffect\n  Category");
        assert_eq!(tabbed, spaced);
    }

    #[test]
    fn the_shallowest_lines_after_the_effect_are_the_categories() {
        // However far in they are written, as long as they agree.
        let deep = parse("ishikawa\nEffect\n        A\n        B");
        assert_eq!(deep.categories.len(), 2);
    }

    #[test]
    fn a_bracketed_label_is_unwrapped() {
        assert_eq!(parse("ishikawa\nid[\"The effect\"]").effect, "The effect");
        assert_eq!(parse("ishikawa\nid[Bare]").effect, "Bare");
    }

    #[test]
    fn quotes_around_a_line_are_optional() {
        assert_eq!(parse("ishikawa\n\"Quoted effect\"").effect, "Quoted effect");
    }

    #[test]
    fn a_comment_line_is_skipped() {
        assert_eq!(parse("ishikawa\n%% a note\nEffect").effect, "Effect");
    }

    #[test]
    fn an_effect_with_no_categories_is_still_a_diagram() {
        let diagram = parse("ishikawa\nJust the effect");
        assert_eq!(diagram.effect, "Just the effect");
        assert!(diagram.categories.is_empty());
    }

    #[test]
    fn dedenting_past_a_level_reattaches_to_the_right_ancestor() {
        let diagram = parse("ishikawa\nE\n  A\n      deep\n    mid\n  B");
        assert_eq!(diagram.categories.len(), 2);
        assert_eq!(diagram.categories[0].causes.len(), 2, "deep and mid");
    }

    #[test]
    fn nothing_in_yields_an_empty_diagram() {
        assert_eq!(parse(""), Diagram::default());
        assert_eq!(parse("ishikawa-beta"), Diagram::default());
    }
}
