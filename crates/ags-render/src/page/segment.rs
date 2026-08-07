//! An artifact as an ordered run of prose and blocks.
//!
//! The parser answers "which blocks are here"; a page also needs "and what is
//! between them", in order. Rather than scanning the source a second time — two
//! scanners disagreeing about where a fence ends is a bug that would show up as
//! a missing paragraph — the spans are derived from the blocks the parser
//! already found, and a test asserts that the spans tile the source exactly.

use crate::block::Block;
use crate::parse::Artifact;

/// One run of the document.
#[derive(Debug, Clone, PartialEq)]
pub enum Segment<'a> {
    /// Markdown between blocks, including any fence that is not an addressable
    /// type: a `rust` fence is prose and renders as a code block.
    Prose(&'a str),
    Block(&'a Block),
}

/// The document in order.
pub fn segments<'a>(source: &'a str, artifact: &'a Artifact) -> Vec<Segment<'a>> {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    // 0-based index of the first line not yet emitted.
    let mut cursor = 0usize;
    for block in &artifact.blocks {
        let start = block.line.saturating_sub(1);
        if start > cursor {
            if let Some(prose) = lines.get(cursor..start) {
                out.push(Segment::Prose(span(source, &lines, cursor, prose.len())));
            }
        }
        out.push(Segment::Block(block));
        cursor = block.end.saturating_sub(1);
    }
    if cursor < lines.len() {
        out.push(Segment::Prose(span(
            source,
            &lines,
            cursor,
            lines.len() - cursor,
        )));
    }
    out
}

/// The source text of `count` lines starting at `from`, as a slice of `source`.
///
/// Sliced rather than rejoined so a line ending or a trailing space survives
/// into the round-trip check below; a rejoin would quietly normalise them and
/// the check would then prove nothing.
fn span<'a>(source: &'a str, lines: &[&'a str], from: usize, count: usize) -> &'a str {
    let Some(first) = lines.get(from) else {
        return "";
    };
    let start = offset_of(source, first);
    let end = match lines.get(from + count) {
        Some(next) => offset_of(source, next),
        None => source.len(),
    };
    source.get(start..end).unwrap_or_default()
}

/// Where a line borrowed from `source` begins in it.
fn offset_of(source: &str, line: &str) -> usize {
    // Both are slices of the same allocation, so the difference of their
    // pointers is the offset — no searching, and no chance of matching an
    // identical line elsewhere in the document.
    (line.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_artifact;

    /// The `#n2` body deliberately ends in a blank line: the tiling assertion is
    /// only worth anything if a fixture exercises the span arithmetic's edge.
    const DOC: &str = "# Title\n\nSome prose.\n\n```mermaid #d1\nC4Context\nPerson(a,\"A\")\n```\n\nBetween.\n\n```rust\nlet x = 1;\n```\n\n```note #n1 kind=info\nMind this.\n```\n\n```note #n2\nAnd this.\n\n```\n\nTrailing prose.\n";

    /// Segment a source and hand the result to `f`, which keeps the artifact
    /// alive for exactly as long as the segments borrow it.
    fn with_parts<R>(source: &str, f: impl FnOnce(&[Segment<'_>]) -> R) -> R {
        let artifact = parse_artifact(source);
        f(&segments(source, &artifact))
    }

    #[test]
    fn the_document_comes_back_in_order() {
        let kinds = with_parts(DOC, |parts| {
            parts
                .iter()
                .map(|s| match s {
                    Segment::Prose(_) => "prose".to_string(),
                    Segment::Block(b) => b.type_token.clone(),
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(
            kinds,
            ["prose", "mermaid", "prose", "note", "prose", "note", "prose"],
            "a ```rust fence is prose, not a block"
        );
    }

    #[test]
    fn the_segments_tile_the_source_exactly() {
        // The whole reason the spans are derived rather than re-scanned: if this
        // holds, nothing was dropped, duplicated or reordered.
        //
        // This rebuilds a block's span from `end`, the same field `segments` uses.
        // It formerly rebuilt it by re-deriving the line count from the body — the
        // very function under test — so a block whose body ended blank was measured
        // wrong twice and the two wrongs agreed. A test that shares an assumption
        // with the code cannot check it; the fixtures below now include that case.
        let source = DOC;
        let rebuilt = with_parts(source, |parts| {
            parts
                .iter()
                .map(|s| match s {
                    Segment::Prose(text) => (*text).to_string(),
                    Segment::Block(b) => {
                        let start = b.line.saturating_sub(1);
                        let lines: Vec<&str> = source.lines().collect();
                        lines
                            .get(start..b.end.saturating_sub(1))
                            .unwrap_or_default()
                            .iter()
                            .map(|l| format!("{l}\n"))
                            .collect::<Vec<_>>()
                            .concat()
                    }
                })
                .collect::<String>()
        });
        assert_eq!(rebuilt, source);
    }

    #[test]
    fn a_document_of_pure_prose_is_one_segment() {
        with_parts("just words\n\nand more\n", |out| {
            assert_eq!(out.len(), 1);
            assert!(matches!(out.first(), Some(Segment::Prose(_))));
        });
    }

    #[test]
    fn a_document_that_is_only_a_block_has_no_prose_around_it() {
        with_parts("```note #n\nbody\n```\n", |out| {
            assert_eq!(out.len(), 1);
            assert!(matches!(out.first(), Some(Segment::Block(_))));
        });
    }

    #[test]
    fn an_empty_document_yields_nothing() {
        with_parts("", |out| assert!(out.is_empty()));
    }

    #[test]
    fn an_empty_block_body_still_measures_two_fences() {
        with_parts("before\n\n```note #n\n```\n\nafter\n", |out| {
            assert_eq!(out.len(), 3, "{out:?}");
            assert!(
                matches!(out.get(2), Some(Segment::Prose(text)) if *text == "\nafter\n"),
                "{out:?}"
            );
        });
    }

    #[test]
    fn a_body_ending_in_a_blank_line_does_not_leak_its_closing_fence() {
        // The body joins to "hello\n", whose `lines()` count is 1, not 2 — so a
        // span re-derived from the body stopped one line short, left the closing
        // fence in the prose that followed, and that fence opened a code block
        // which swallowed the rest of the document.
        with_parts(
            "# T\n\n```note #n\nhello\n\n```\n\nafter text here\n",
            |out| {
                assert_eq!(out.len(), 3, "{out:?}");
                assert!(
                    matches!(out.get(2), Some(Segment::Prose(text)) if *text == "\nafter text here\n"),
                    "the closing fence leaked into the prose: {out:?}"
                );
            },
        );
    }

    #[test]
    fn a_body_of_one_blank_line_is_not_mistaken_for_no_body() {
        // `[""].join("\n")` is "", which is what an absent body also produces —
        // so the two are indistinguishable downstream, and only the parser can
        // say which it saw.
        with_parts("```note #n\n\n```\n\nafter\n", |out| {
            assert_eq!(out.len(), 2, "{out:?}");
            assert!(
                matches!(out.get(1), Some(Segment::Prose(text)) if *text == "\nafter\n"),
                "{out:?}"
            );
        });
    }

    #[test]
    fn a_span_past_the_end_of_the_document_is_empty() {
        // Unreachable through `segments`, which never asks for a line it did
        // not find — but the guard is what makes that true rather than lucky.
        let source = "a\nb\n";
        let lines: Vec<&str> = source.lines().collect();
        assert_eq!(span(source, &lines, 9, 1), "");
        assert_eq!(span(source, &lines, 0, 1), "a\n");
        assert_eq!(span(source, &lines, 1, 1), "b\n");
    }

    #[test]
    fn two_blocks_running_together_have_no_prose_between_them() {
        with_parts("```note #a\nx\n```\n```note #b\ny\n```\n", |out| {
            assert_eq!(out.len(), 2);
            assert!(out.iter().all(|s| matches!(s, Segment::Block(_))));
        });
    }
}
