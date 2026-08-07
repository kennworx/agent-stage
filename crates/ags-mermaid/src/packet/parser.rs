//! Reading `packet` source.
//!
//! ```text
//! packet | packet-beta          the header, optionally carrying a title
//! title <text>
//! <start>-<end>: "Field name"   an inclusive bit range
//! <bit>: "Field name"           one bit
//! +<count>: "Field name"        `count` bits, continuing from the last field
//! ```

use super::types::{Diagram, Field};

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let trimmed = text.trim();
    let head = trimmed.strip_prefix(['"', '\'']).unwrap_or(trimmed);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

/// The text after a keyword, when the line opens with it as a whole word.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if !line.get(..keyword.len())?.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let tail = line.get(keyword.len()..)?;
    if tail.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some(tail.trim())
}

/// One field line, given where the previous field ended.
fn parse_field(line: &str, cursor: usize) -> Option<Field> {
    let (range, label) = line.split_once(':')?;
    let range = range.trim();
    let label = unquote(label).trim().to_string();
    if label.is_empty() {
        return None;
    }

    // `+count` continues from the cursor; anything else names its own bits.
    if let Some(count) = range.strip_prefix('+') {
        let count: usize = count.trim().parse().ok()?;
        if count < 1 {
            return None;
        }
        return Some(Field {
            start: cursor,
            end: cursor + count - 1,
            label,
        });
    }

    let (start, end) = if let Some((a, b)) = range.split_once('-') {
        (a.trim().parse().ok()?, b.trim().parse().ok()?)
    } else {
        let bit: usize = range.parse().ok()?;
        (bit, bit)
    };
    if end < start {
        return None;
    }
    Some(Field { start, end, label })
}

/// Parse a packet diagram. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    let mut cursor = 0usize;
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        let header = after_keyword(line, "packet-beta").or_else(|| after_keyword(line, "packet"));
        if let Some(rest) = header {
            if let Some(title) = after_keyword(rest, "title") {
                diagram.title = Some(unquote(title).to_string());
            }
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            diagram.title = Some(unquote(title).to_string());
            continue;
        }
        if let Some(field) = parse_field(line, cursor) {
            cursor = field.end + 1;
            diagram.fields.push(field);
        }
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(source: &str) -> Vec<(usize, usize, String)> {
        parse(source)
            .fields
            .into_iter()
            .map(|f| (f.start, f.end, f.label))
            .collect()
    }

    #[test]
    fn a_range_a_single_bit_and_a_count_all_read() {
        assert_eq!(
            fields("packet\n0-15: \"Source Port\"\n16: \"Flag\"\n+8: \"Length\""),
            [
                (0, 15, "Source Port".to_string()),
                (16, 16, "Flag".to_string()),
                (17, 24, "Length".to_string()),
            ]
        );
    }

    #[test]
    fn the_legacy_header_spelling_is_accepted() {
        assert_eq!(parse("packet-beta\n0: \"a\"").fields.len(), 1);
        assert_eq!(
            parse("packet-beta title Frame").title.as_deref(),
            Some("Frame")
        );
    }

    #[test]
    fn a_title_reads_from_the_header_or_its_own_line() {
        assert_eq!(parse("packet title Frame").title.as_deref(), Some("Frame"));
        assert_eq!(
            parse("packet\ntitle \"A frame\"").title.as_deref(),
            Some("A frame")
        );
    }

    #[test]
    fn a_count_continues_from_wherever_the_last_field_ended() {
        assert_eq!(
            fields("packet\n0-3: a\n+4: b\n+1: c"),
            [
                (0, 3, "a".to_string()),
                (4, 7, "b".to_string()),
                (8, 8, "c".to_string()),
            ]
        );
    }

    #[test]
    fn quotes_around_a_label_are_optional() {
        assert_eq!(fields("packet\n0: bare")[0].2, "bare");
        assert_eq!(fields("packet\n0: 'single'")[0].2, "single");
    }

    #[test]
    fn a_comment_is_stripped_before_the_line_is_read() {
        assert_eq!(fields("packet\n0-1: a %% two bits").len(), 1);
        assert_eq!(fields("packet\n%% just a comment\n0: a").len(), 1);
    }

    #[test]
    fn a_backwards_or_unlabelled_range_is_refused() {
        assert!(parse("packet\n9-2: a").fields.is_empty());
        assert!(parse("packet\n0-1: \"\"").fields.is_empty());
        assert!(parse("packet\n+0: a").fields.is_empty());
    }

    #[test]
    fn a_line_that_is_not_a_field_is_skipped_rather_than_fatal() {
        assert_eq!(fields("packet\nnonsense\n0: a\nalso nonsense").len(), 1);
    }

    #[test]
    fn nothing_in_yields_an_empty_diagram() {
        assert_eq!(parse(""), Diagram::default());
        assert_eq!(parse("packet"), Diagram::default());
    }
}
