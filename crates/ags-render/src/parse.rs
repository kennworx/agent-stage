//! Parse an artifact (markdown with fenced blocks) into [`Block`]s.
//!
//! This is pure structure: it finds fenced blocks, parses each info string into
//! `<type> [#id] [key=value | flag]*`, and records structural problems (unclosed
//! fences, malformed info-string tokens). It performs **no** semantic validation
//! — that is [`crate::validate`]'s job — and never invokes a diagram engine.

use crate::block::{is_block_type, Attr, AttrValue, Block, ValidationError, ValidationKind};

/// A fenced span whose type is not an addressable block — ordinary prose, kept
/// only so the validator can spot a near-miss typo (see [`crate::validate`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProseFence {
    /// The first info-string token, e.g. `"rust"` (empty for a bare fence).
    pub type_token: String,
    /// 1-based line of the opening fence.
    pub line: usize,
}

/// A parsed artifact: its fenced blocks plus structural errors found while parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Addressable fenced blocks in source order.
    pub blocks: Vec<Block>,
    /// Fenced spans that are prose, not blocks, in source order.
    pub prose_fences: Vec<ProseFence>,
    /// Structural errors (unclosed fences, malformed info strings).
    pub structural_errors: Vec<ValidationError>,
}

/// Parse `src` into an [`Artifact`]. Raw markdown between fences is implicit prose
/// and is ignored here (it carries no id or schema).
///
/// A fence whose first token is not an addressable type is prose too, and is
/// recorded only as a [`ProseFence`]. It is deliberately exempt from every
/// info-string rule: a GFM info string may carry arbitrary text after the
/// language (`json {"a": 1}`, `rust,no_run`), which the block grammar would
/// otherwise reject as malformed tokens. It is still scanned to its closing
/// delimiter, so a block-type name inside its body cannot open a phantom block.
#[must_use]
pub fn parse_artifact(src: &str) -> Artifact {
    let lines: Vec<&str> = src.lines().collect();
    let mut blocks = Vec::new();
    let mut prose_fences = Vec::new();
    let mut structural_errors = Vec::new();
    let mut ordinal = 0usize;
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines.get(i).copied().unwrap_or("");
        let Some((ch, len, rest)) = fence_marker(line) else {
            i += 1;
            continue;
        };

        let parsed = parse_info(rest.trim());
        let (body, close_idx) = collect_body(&lines, i + 1, ch, len);
        let open_line = i + 1;
        i = match close_idx {
            Some(j) => j + 1,
            None => lines.len(),
        };

        if !is_block_type(&parsed.type_token) {
            prose_fences.push(ProseFence {
                type_token: parsed.type_token,
                line: open_line,
            });
            continue;
        }

        let block = Block {
            type_token: parsed.type_token,
            id: parsed.id,
            attrs: parsed.attrs,
            body,
            line: open_line,
            // `i` has already advanced past the closing fence (or to the end of
            // the source, for an unclosed one), which is exactly where whatever
            // follows this block begins.
            end: i + 1,
            ordinal,
        };
        for detail in parsed.grammar_errors {
            structural_errors.push(ValidationError::new(
                block.anchor(),
                ValidationKind::InfoGrammar,
                detail,
            ));
        }
        if close_idx.is_none() {
            structural_errors.push(ValidationError::new(
                block.anchor(),
                ValidationKind::UnclosedFence,
                format!("opening fence at line {} is never closed", block.line),
            ));
        }
        blocks.push(block);
        ordinal += 1;
    }

    Artifact {
        blocks,
        prose_fences,
        structural_errors,
    }
}

/// If `line` opens or closes a fence, return `(fence_char, run_length, remainder)`.
/// The remainder is the text after the run of fence characters (untrimmed).
fn fence_marker(line: &str) -> Option<(char, usize, &str)> {
    let trimmed = line.trim_start();
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let run = trimmed.chars().take_while(|&c| c == first).count();
    if run < 3 {
        return None;
    }
    // Fence chars are ASCII, so the char count equals the byte offset.
    let rest = trimmed.get(run..).unwrap_or("");
    Some((first, run, rest))
}

/// Whether `line` closes a fence opened with `ch` repeated `len` times: same char,
/// at least as long, and nothing but the fence run on the line.
fn is_closing_fence(line: &str, ch: char, len: usize) -> bool {
    match fence_marker(line) {
        Some((c, l, rest)) => c == ch && l >= len && rest.trim().is_empty(),
        None => false,
    }
}

/// Collect body lines from `start` until a closing fence. Returns the joined body
/// and the index of the closing fence line (`None` if the fence is never closed).
fn collect_body(lines: &[&str], start: usize, ch: char, len: usize) -> (String, Option<usize>) {
    let mut body_lines: Vec<&str> = Vec::new();
    let mut j = start;
    while j < lines.len() {
        let line = lines.get(j).copied().unwrap_or("");
        if is_closing_fence(line, ch, len) {
            return (body_lines.join("\n"), Some(j));
        }
        body_lines.push(line);
        j += 1;
    }
    (body_lines.join("\n"), None)
}

/// The components extracted from an info string.
struct InfoParse {
    type_token: String,
    id: Option<String>,
    attrs: Vec<Attr>,
    grammar_errors: Vec<String>,
}

/// Parse an info string `<type> [#id] [key=value | flag]*` into its components,
/// collecting a detail string for every malformed token.
fn parse_info(info: &str) -> InfoParse {
    let mut tokens = split_info_tokens(info).into_iter();
    let type_token = tokens.next().unwrap_or_default();
    let mut id = None;
    let mut attrs = Vec::new();
    let mut grammar_errors = Vec::new();

    for tok in tokens {
        if let Some(rest) = tok.strip_prefix('#') {
            classify_id(rest, &tok, &mut id, &mut grammar_errors);
        } else if let Some((key, value)) = tok.split_once('=') {
            if key.is_empty() {
                grammar_errors.push(format!("attribute with empty key: '{tok}'"));
            } else {
                attrs.push(Attr {
                    key: key.to_string(),
                    value: AttrValue::Value(dequote(value)),
                });
            }
        } else if is_ident(&tok) {
            attrs.push(Attr {
                key: tok,
                value: AttrValue::Flag,
            });
        } else {
            grammar_errors.push(format!("malformed token '{tok}'"));
        }
    }

    InfoParse {
        type_token,
        id,
        attrs,
        grammar_errors,
    }
}

/// Record a `#id` token, or a grammar error if it is empty or a duplicate.
fn classify_id(rest: &str, tok: &str, id: &mut Option<String>, errors: &mut Vec<String>) {
    if rest.is_empty() {
        errors.push("empty '#id' token".to_string());
    } else if id.is_some() {
        errors.push(format!("duplicate id token '{tok}'"));
    } else {
        *id = Some(rest.to_string());
    }
}

/// Split an info string on whitespace, keeping double-quoted spans intact.
fn split_info_tokens(info: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in info.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            cur.push(c);
        } else if c.is_whitespace() && !in_quotes {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

/// Strip one layer of surrounding double quotes, if present.
fn dequote(v: &str) -> String {
    let bytes = v.as_bytes();
    if v.len() >= 2 && bytes.first() == Some(&b'"') && bytes.last() == Some(&b'"') {
        v.get(1..v.len() - 1).unwrap_or("").to_string()
    } else {
        v.to_string()
    }
}

/// Whether `s` is a bare-flag identifier (ASCII alphanumeric, `-`, or `_`).
fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_only_has_no_blocks() {
        let art = parse_artifact("just some prose\n\nmore prose");
        assert!(art.blocks.is_empty());
        assert!(art.structural_errors.is_empty());
    }

    #[test]
    fn parses_type_id_and_attrs() {
        let src =
            "```mermaid #flow feedback=annotate direction=TD collapsible\ngraph TD\n  A-->B\n```";
        let art = parse_artifact(src);
        assert_eq!(art.blocks.len(), 1);
        let b = &art.blocks[0];
        assert_eq!(b.type_token, "mermaid");
        assert_eq!(b.id.as_deref(), Some("flow"));
        assert_eq!(b.body, "graph TD\n  A-->B");
        assert_eq!(b.ordinal, 0);
        assert_eq!(b.line, 1);
        assert!(art.structural_errors.is_empty());
        assert!(b.attrs.contains(&Attr {
            key: "feedback".into(),
            value: AttrValue::Value("annotate".into())
        }));
        assert!(b.attrs.contains(&Attr {
            key: "collapsible".into(),
            value: AttrValue::Flag
        }));
    }

    #[test]
    fn quoted_attribute_value_keeps_spaces() {
        let src = "```code #c lang=rust title=\"a long title\"\nfn main() {}\n```";
        let art = parse_artifact(src);
        let b = &art.blocks[0];
        assert!(b.attrs.contains(&Attr {
            key: "title".into(),
            value: AttrValue::Value("a long title".into())
        }));
    }

    #[test]
    fn unclosed_fence_is_reported() {
        let src = "```code #c lang=rust\nfn main() {}";
        let art = parse_artifact(src);
        assert_eq!(art.blocks.len(), 1);
        assert_eq!(art.structural_errors.len(), 1);
        let e = &art.structural_errors[0];
        assert_eq!(e.kind, ValidationKind::UnclosedFence);
        assert_eq!(e.anchor, "#c");
    }

    #[test]
    fn empty_id_token_is_grammar_error() {
        let art = parse_artifact("```mermaid #\nx\n```");
        assert!(art
            .structural_errors
            .iter()
            .any(|e| e.kind == ValidationKind::InfoGrammar && e.detail.contains("empty '#id'")));
    }

    #[test]
    fn duplicate_id_token_is_grammar_error() {
        let art = parse_artifact("```mermaid #a #b\nx\n```");
        let b = &art.blocks[0];
        assert_eq!(b.id.as_deref(), Some("a"));
        assert!(art
            .structural_errors
            .iter()
            .any(|e| e.kind == ValidationKind::InfoGrammar && e.detail.contains("duplicate id")));
    }

    #[test]
    fn empty_key_and_malformed_tokens_are_grammar_errors() {
        let art = parse_artifact("```table =oops %$weird\nx\n```");
        let details: Vec<&str> = art
            .structural_errors
            .iter()
            .map(|e| e.detail.as_str())
            .collect();
        assert!(details.iter().any(|d| d.contains("empty key")));
        assert!(details.iter().any(|d| d.contains("malformed token")));
    }

    #[test]
    fn tilde_fences_and_longer_closer_work() {
        let src = "~~~code #c lang=text\nbody\n~~~~";
        let art = parse_artifact(src);
        assert_eq!(art.blocks.len(), 1);
        assert_eq!(art.blocks[0].body, "body");
        assert!(art.structural_errors.is_empty());
    }

    #[test]
    fn longer_outer_fence_contains_inner_backticks() {
        let src = "````code #c lang=md\n```\ninner\n```\n````";
        let art = parse_artifact(src);
        assert_eq!(art.blocks.len(), 1);
        assert_eq!(art.blocks[0].body, "```\ninner\n```");
    }

    #[test]
    fn short_backtick_run_is_not_a_fence() {
        // Two backticks is inline code, not a fence.
        let art = parse_artifact("`` not a fence ``");
        assert!(art.blocks.is_empty());
    }

    #[test]
    fn dequote_leaves_unquoted_and_partial_quotes() {
        assert_eq!(dequote("plain"), "plain");
        assert_eq!(dequote("\"quoted\""), "quoted");
        assert_eq!(dequote("\""), "\"");
        assert_eq!(dequote("\"unbalanced"), "\"unbalanced");
    }

    #[test]
    fn is_ident_accepts_flags_rejects_symbols() {
        assert!(is_ident("collapsible"));
        assert!(is_ident("mode-live_2"));
        assert!(!is_ident(""));
        assert!(!is_ident("has space"));
        assert!(!is_ident("weird$"));
    }

    #[test]
    fn two_blocks_get_sequential_ordinals() {
        let src = "```note #n\na\n```\ntext\n```code #c lang=rust\nb\n```";
        let art = parse_artifact(src);
        assert_eq!(art.blocks.len(), 2);
        assert_eq!(art.blocks[0].ordinal, 0);
        assert_eq!(art.blocks[1].ordinal, 1);
        assert_eq!(art.blocks[1].line, 5);
    }

    #[test]
    fn unrecognized_fence_is_prose_not_a_block() {
        let art = parse_artifact("```rust\nfn main() {}\n```");
        assert!(art.blocks.is_empty());
        assert!(art.structural_errors.is_empty());
        assert_eq!(art.prose_fences.len(), 1);
        assert_eq!(art.prose_fences[0].type_token, "rust");
        assert_eq!(art.prose_fences[0].line, 1);
    }

    #[test]
    fn bare_fence_is_prose() {
        let art = parse_artifact("```\nplain\n```");
        assert!(art.blocks.is_empty());
        assert_eq!(art.prose_fences.len(), 1);
        assert_eq!(art.prose_fences[0].type_token, "");
    }

    #[test]
    fn prose_fence_is_exempt_from_info_string_grammar() {
        // A GFM info string may carry arbitrary text after the language; the block
        // grammar would call these malformed tokens, which must not fire on prose.
        let art = parse_artifact("```json {\"a\": 1}\n{}\n```");
        assert!(art.blocks.is_empty());
        assert!(art.structural_errors.is_empty());
    }

    #[test]
    fn unclosed_prose_fence_is_not_an_error() {
        let art = parse_artifact("```yaml\na: 1");
        assert!(art.blocks.is_empty());
        assert!(art.structural_errors.is_empty());
        assert_eq!(art.prose_fences.len(), 1);
    }

    #[test]
    fn block_type_inside_a_prose_fence_does_not_open_a_block() {
        // The trap: the prose fence must still be scanned to its closer, or the
        // inner ```mermaid line would be read as a block opener.
        let art = parse_artifact("```text\n```mermaid\ngraph TD\n```");
        assert!(art.blocks.is_empty());
        assert_eq!(art.prose_fences.len(), 1);
        assert_eq!(art.prose_fences[0].type_token, "text");
    }

    #[test]
    fn prose_fences_do_not_consume_block_ordinals() {
        let art = parse_artifact("```rust\na\n```\n```note #n\nb\n```");
        assert_eq!(art.blocks.len(), 1);
        assert_eq!(art.blocks[0].ordinal, 0);
        assert_eq!(art.prose_fences.len(), 1);
    }
}
