//! Label text: escaping, inline formatting, and word wrapping.
//!
//! Diagram sources carry labels written for people — `<br>` for a line break,
//! `**bold**`, a stray quote — and every diagram type needs the same handling of
//! them. Hand-rolled rather than regex-driven: one of the rules here uses a
//! lookbehind, which Rust's `regex` cannot express at all, and a regex engine is
//! a heavy passenger in a WebAssembly build that needs it for four substitutions.

use super::metrics::text_width;

/// Escape the characters that would otherwise be markup.
pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Tags that survive normalisation, because they map onto text styling.
const FORMATTING: [&str; 7] = ["b", "strong", "i", "em", "u", "s", "del"];

/// Tags dropped outright: no styling of ours corresponds to them.
const UNSUPPORTED: [&str; 4] = ["sub", "sup", "small", "mark"];

/// Whether `text` at `at` opens or closes one of `names`, and how long it is.
fn tag_at(text: &str, at: usize, names: &[&str]) -> Option<usize> {
    let rest = text.get(at..)?;
    if !rest.starts_with('<') {
        return None;
    }
    let close = rest.find('>')? + 1;
    let inner = rest
        .get(1..close - 1)?
        .trim()
        .trim_start_matches('/')
        .trim();
    let name = inner.trim_end_matches('/').trim();
    names
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
        .then_some(close)
}

/// Remove inline formatting tags, leaving the text they wrapped.
///
/// Measurement uses this: `<b>` occupies no width on screen, so leaving it in
/// would size every emphasised label too wide.
pub fn strip_formatting_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(len) = tag_at(text, i, &FORMATTING) {
            i += len;
            continue;
        }
        let Some(c) = text.get(i..).and_then(|s| s.chars().next()) else {
            break;
        };
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Strip one pair of surrounding double quotes, which a source uses to protect
/// a label containing separators.
fn unquote(label: &str) -> &str {
    let trimmed = label.strip_prefix('"').and_then(|s| s.strip_suffix('"'));
    match trimmed {
        Some(inner) if !label.is_empty() => inner,
        _ => label,
    }
}

/// Replace `<br>` in any spelling, and the literal two characters `\n`, with a
/// real line break; drop tags with no styling of ours behind them.
fn line_breaks_and_unsupported(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if let Some(len) = tag_at(text, i, &["br"]) {
            out.push('\n');
            i += len;
            continue;
        }
        if let Some(len) = tag_at(text, i, &UNSUPPORTED) {
            i += len;
            continue;
        }
        if text.get(i..).is_some_and(|s| s.starts_with("\\n")) {
            out.push('\n');
            i += 2;
            continue;
        }
        let Some(c) = text.get(i..).and_then(|s| s.chars().next()) else {
            break;
        };
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Rewrite `marker`-delimited spans as `<tag>` … `</tag>`.
///
/// A span is only recognised when its content neither begins nor ends with
/// whitespace and contains no marker character — so `2 * 3 * 4` stays
/// arithmetic rather than becoming italics.
fn emphasis(text: &str, marker: &str, tag: &str) -> String {
    let bytes = marker.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while let Some(rest) = text.get(i..).filter(|r| !r.is_empty()) {
        if !rest.starts_with(marker) {
            let Some(c) = rest.chars().next() else { break };
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        // A single marker directly after another is the tail of a doubled one,
        // never the start of a span: in `*a**b*` the `*` at index 3 closes
        // nothing, and reading it as an opener turns the whole thing into
        // `*a*<i>b</i>`.
        let after_doubled = bytes == 1
            && text
                .get(..i)
                .and_then(|s| s.chars().next_back())
                .is_some_and(|c| marker.starts_with(c));
        let span = if after_doubled {
            None
        } else {
            rest.get(bytes..)
                .and_then(|after| Some((after, close_of(after, marker)?)))
                .and_then(|(after, end)| after.get(..end).map(|body| (body, end)))
        };
        if let Some((body, end)) = span {
            out.push('<');
            out.push_str(tag);
            out.push('>');
            out.push_str(body);
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
            i += bytes + end + bytes;
        } else {
            out.push_str(marker);
            i += bytes;
        }
    }
    out
}

/// Offset of the closing `marker` for a span opening at the start of `after`,
/// if the content between them qualifies as emphasis.
fn close_of(after: &str, marker: &str) -> Option<usize> {
    let single = marker.len() == 1;
    let end = after.find(marker)?;
    let content = after.get(..end)?;
    if content.is_empty() {
        return None;
    }
    if single {
        // A single marker must not be part of a doubled one, and must hug its
        // content — `*a*` is emphasis, `* a *` is a bullet and a stray star.
        if content.starts_with('*') || content.contains('*') {
            return None;
        }
        if after.get(end + 1..).is_some_and(|s| s.starts_with(marker)) {
            return None;
        }
        let first = content.chars().next()?;
        let last = content.chars().next_back()?;
        if first.is_whitespace() || last.is_whitespace() {
            return None;
        }
    }
    Some(end)
}

/// Normalise a source label into text plus the formatting tags we render.
pub fn normalize_label(label: &str) -> String {
    let text = line_breaks_and_unsupported(unquote(label));
    let text = emphasis(&text, "**", "b");
    let text = emphasis(&text, "~~", "s");
    emphasis(&text, "*", "i")
}

/// Break `text` into at most `max_lines` lines that fit `max_width`.
///
/// The last line is ellipsised when the text does not fit, dropping its final
/// word to make room — a label that runs off its box is worse than one that
/// admits it was cut.
pub fn wrap(
    text: &str,
    max_width: f64,
    font_size: f64,
    font_weight: u32,
    max_lines: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let candidate = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if !cur.is_empty() && text_width(&candidate, font_size, font_weight) > max_width {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        } else {
            cur = candidate;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if max_lines == 0 || lines.len() <= max_lines {
        return lines;
    }
    lines.truncate(max_lines);
    if let Some(last) = lines.last_mut() {
        *last = ellipsise(last);
    }
    lines
}

/// Drop the trailing word, if there is one to drop, and mark the truncation.
fn ellipsise(line: &str) -> String {
    let kept = line
        .rfind(char::is_whitespace)
        .and_then(|cut| line.get(..cut))
        .map_or(line, str::trim_end);
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_the_characters_that_would_be_markup() {
        assert_eq!(escape_xml("a < b & c"), "a &lt; b &amp; c");
        assert_eq!(escape_xml("\"q\" 'r'"), "&quot;q&quot; &#39;r&#39;");
        assert_eq!(escape_xml("plain"), "plain");
    }

    #[test]
    fn escaping_does_not_double_up() {
        // `&` is rewritten once; the output must not then re-escape its own `&`.
        assert_eq!(escape_xml("&amp;"), "&amp;amp;");
    }

    #[test]
    fn line_break_tags_become_newlines() {
        for spelling in ["<br>", "<br/>", "<br />", "<BR>", "<Br />"] {
            assert_eq!(
                normalize_label(&format!("a{spelling}b")),
                "a\nb",
                "{spelling}"
            );
        }
    }

    #[test]
    fn a_literal_backslash_n_becomes_a_newline() {
        assert_eq!(normalize_label("a\\nb"), "a\nb");
    }

    #[test]
    fn surrounding_quotes_are_stripped_once() {
        assert_eq!(normalize_label("\"quoted\""), "quoted");
        // Only the outer pair — an inner quote is content.
        assert_eq!(normalize_label("\"say \"hi\"\""), "say \"hi\"");
        assert_eq!(normalize_label("\"unbalanced"), "\"unbalanced");
    }

    #[test]
    fn unsupported_tags_are_dropped_but_their_text_kept() {
        assert_eq!(normalize_label("H<sub>2</sub>O"), "H2O");
        assert_eq!(normalize_label("<mark>hot</mark>"), "hot");
    }

    #[test]
    fn markdown_emphasis_becomes_tags() {
        assert_eq!(normalize_label("**bold**"), "<b>bold</b>");
        assert_eq!(normalize_label("*italic*"), "<i>italic</i>");
        assert_eq!(normalize_label("~~gone~~"), "<s>gone</s>");
        assert_eq!(normalize_label("a **b** c"), "a <b>b</b> c");
    }

    #[test]
    fn a_lone_star_is_not_emphasis() {
        // The reason this is hand-rolled: arithmetic and bullets must survive.
        assert_eq!(normalize_label("2 * 3"), "2 * 3");
        assert_eq!(normalize_label("* item"), "* item");
        assert_eq!(normalize_label("a * b * c"), "a * b * c");
    }

    #[test]
    fn bold_wins_over_italic_at_the_same_position() {
        // `**x**` must not be read as two italic markers around nothing.
        assert_eq!(normalize_label("**x**"), "<b>x</b>");
    }

    #[test]
    fn formatting_tags_are_stripped_for_measurement() {
        assert_eq!(strip_formatting_tags("<b>bold</b> text"), "bold text");
        assert_eq!(strip_formatting_tags("<i>a</i><s>b</s>"), "ab");
        // Anything else is left alone — this strips styling, not markup.
        assert_eq!(strip_formatting_tags("<span>x</span>"), "<span>x</span>");
    }

    #[test]
    fn wrapping_breaks_on_width() {
        let lines = wrap("one two three four five", 60.0, 11.0, 400, 5);
        assert!(lines.len() > 1, "expected a break, got {lines:?}");
        assert_eq!(lines.join(" "), "one two three four five");
    }

    #[test]
    fn a_single_word_is_never_split() {
        // Better to overflow than to hyphenate an identifier.
        let lines = wrap("kenn-indexer::workflow", 10.0, 11.0, 400, 3);
        assert_eq!(lines, vec!["kenn-indexer::workflow"]);
    }

    #[test]
    fn wrapping_stops_at_the_line_limit_and_says_so() {
        let lines = wrap("one two three four five six seven", 40.0, 11.0, 400, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].ends_with('…'), "no truncation mark: {lines:?}");
    }

    #[test]
    fn ellipsis_drops_the_last_word_to_make_room() {
        assert_eq!(ellipsise("alpha beta"), "alpha…");
        // Nothing to drop: mark it anyway rather than returning it unchanged.
        assert_eq!(ellipsise("alpha"), "alpha…");
    }

    #[test]
    fn empty_and_blank_text_wrap_to_nothing() {
        assert!(wrap("", 100.0, 11.0, 400, 3).is_empty());
        assert!(wrap("   \n  ", 100.0, 11.0, 400, 3).is_empty());
    }

    #[test]
    fn a_zero_line_limit_does_not_truncate() {
        // Zero means "no limit" rather than "no lines", so a caller that has not
        // decided cannot silently lose the label.
        let lines = wrap("one two three", 30.0, 11.0, 400, 0);
        assert_eq!(lines.join(" "), "one two three");
    }
}
