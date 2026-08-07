//! Recognising the pieces of a `zenuml` line.
//!
//! Matching is hand-rolled rather than regex-driven, as everywhere else in this
//! crate — see `text.rs` for why. Held apart from the reader above it because
//! none of it knows what a diagram is: every function here takes a string and
//! answers a question about its shape.

use crate::keyword::is_word;

use super::types::LineStyle;

pub(super) const OPENERS: [&str; 12] = [
    "if", "alt", "opt", "loop", "while", "for", "forEach", "par", "try", "critical", "group",
    "section",
];

/// Keywords that continue one. `else if` leads `else`, so the longer of the two
/// wins the same way the reference's alternation does.
pub(super) const DIVIDERS: [&str; 5] = ["else if", "else", "catch", "finally", "and"];

/// Whether an identifier may open with `c`.
pub(super) fn is_word_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// Whether `line` opens with `word`, ignoring case.
pub(super) fn starts_ci(line: &str, word: &str) -> bool {
    line.get(..word.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(word))
}

/// The text after `word` at the head of `line`, when a word boundary follows it.
///
/// A space inside `word` stands for a run of whitespace, so `"else if"` matches
/// `else   if` as the reference's `else\s+if` does.
pub(super) fn head<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    let mut rest = line;
    for (index, part) in word.split(' ').enumerate() {
        if index > 0 {
            let after = rest.trim_start();
            if after.len() == rest.len() {
                return None;
            }
            rest = after;
        }
        if !starts_ci(rest, part) {
            return None;
        }
        rest = rest.get(part.len()..)?;
    }
    (!rest.starts_with(is_word)).then_some(rest)
}

/// The first of `words` that heads `line`, and what follows it.
pub(super) fn head_of<'a>(
    line: &'a str,
    words: &[&'static str],
) -> Option<(&'static str, &'a str)> {
    words
        .iter()
        .find_map(|word| head(line, word).map(|rest| (*word, rest)))
}

/// The text after `word` and the whitespace that must follow it.
pub(super) fn after_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    if !starts_ci(line, word) {
        return None;
    }
    let rest = line.get(word.len()..)?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

/// Drop one surrounding pair of parentheses: `(ok)` is the condition `ok`.
pub(super) fn strip_parens(text: &str) -> &str {
    let trimmed = text.trim();
    trimmed
        .strip_prefix('(')
        .and_then(|inner| inner.strip_suffix(')'))
        .map_or(trimmed, str::trim)
}

/// Drop one surrounding pair of quotes.
pub(super) fn strip_quotes(text: &str) -> &str {
    let text = text.strip_prefix(['"', '\'']).unwrap_or(text);
    text.strip_suffix(['"', '\'']).unwrap_or(text)
}

/// Drop a trailing block opener, then the condition's parentheses.
pub(super) fn strip_trailing_brace(text: &str) -> &str {
    let trimmed = text.trim();
    strip_parens(trimmed.strip_suffix('{').map_or(trimmed, str::trim))
}

/// The head of a dotted reference: `Store.rows` names the participant `Store`.
pub(super) fn id_head(reference: &str) -> &str {
    strip_quotes(reference.split('.').next().unwrap_or(reference))
}

/// Collapse a keyword's whitespace and lowercase it, so `else   if` is written
/// `else if` wherever it is shown.
pub(super) fn normalize_keyword(keyword: &str) -> String {
    keyword
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase()
}

/// Split `text` at its first run of whitespace.
pub(super) fn split_token(text: &str) -> (&str, &str) {
    match text.find(char::is_whitespace) {
        Some(at) => {
            let (token, tail) = text.split_at(at);
            (token, tail.trim_start())
        }
        None => (text, ""),
    }
}

/// The label in an `as <label>` tail, when there is one.
///
/// `fold` mirrors the reference: the `participant` rule is case-insensitive, the
/// `@annotator` one is not, so `AS` names an alias only in the first.
pub(super) fn alias(rest: &str, fold: bool) -> Option<&str> {
    let after = if fold {
        if !starts_ci(rest, "as") {
            return None;
        }
        rest.get(2..)?
    } else {
        rest.strip_prefix("as")?
    };
    if !after.starts_with(char::is_whitespace) {
        return None;
    }
    let label = after.trim();
    (!label.is_empty()).then_some(label)
}

/// `<name>` or `<name> as <label>` — the tail of a declaration.
pub(super) fn declaration(tail: &str, fold: bool) -> Option<(String, String)> {
    let (token, rest) = split_token(tail);
    if token.is_empty() {
        return None;
    }
    let id = strip_quotes(token).to_string();
    if rest.is_empty() {
        return Some((id.clone(), id));
    }
    let label = strip_quotes(alias(rest, fold)?).to_string();
    Some((id, label))
}

/// A participant reference at the head of `text`, and what follows it.
pub(super) fn take_reference(text: &str) -> (&str, &str) {
    let end = text
        .find(|c: char| !(is_word(c) || c == '.'))
        .unwrap_or(text.len());
    let (reference, tail) = text.split_at(end);
    (reference, tail.trim_start())
}

/// `A->B: text`, in its dashed and double-headed spellings.
pub(super) fn arrow_message(content: &str) -> Option<(String, String, String, LineStyle)> {
    let (from, rest) = take_reference(content);
    if from.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix('-')?;
    let (dashed, rest) = rest
        .strip_prefix('-')
        .map_or((false, rest), |tail| (true, tail));
    let rest = rest.strip_prefix('>')?;
    let rest = rest.strip_prefix('>').unwrap_or(rest).trim_start();
    let (to, rest) = take_reference(rest);
    if to.is_empty() {
        return None;
    }
    let label = rest.strip_prefix(':')?.trim();
    if label.is_empty() {
        return None;
    }
    let style = if dashed {
        LineStyle::Dashed
    } else {
        LineStyle::Solid
    };
    Some((
        id_head(from).to_string(),
        id_head(to).to_string(),
        label.to_string(),
        style,
    ))
}

/// `Receiver.method(args)` — the receiver, and the call as written.
pub(super) fn method_call(content: &str) -> Option<(&str, &str)> {
    if !content.starts_with(is_word_start) {
        return None;
    }
    let at = content.find(|c: char| !is_word(c))?;
    let (receiver, rest) = content.split_at(at);
    let call = rest.strip_prefix('.')?;
    (!call.is_empty()).then(|| (receiver, call.trim()))
}

/// `@Stereotype Name [as Label]`.
pub(super) fn annotator_declaration(content: &str) -> Option<(String, String, String)> {
    let rest = content.strip_prefix('@')?;
    let at = rest.find(|c: char| !is_word(c))?;
    let (annotator, tail) = rest.split_at(at);
    if annotator.is_empty() || !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let (id, label) = declaration(tail.trim_start(), false)?;
    Some((annotator.to_string(), id, label))
}

/// Whether `content` is nothing but a bare participant name.
pub(super) fn is_bare_name(content: &str) -> bool {
    content.starts_with(is_word_start) && content.chars().all(is_word)
}
