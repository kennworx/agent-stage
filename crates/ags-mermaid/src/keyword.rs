//! Recognising a keyword at the head of a source line.
//!
//! Every diagram type reads its own grammar, and almost every one of them starts
//! by asking the same question: does this line open with `section`, or `title`,
//! or `commit`? The answer has to be whole-word and case-insensitive — `title` is
//! a keyword, `titles` is a label — which is what the reference's `\b` means.
//!
//! Seventeen parsers had written that themselves, identically, and none of the
//! copies was reachable from a test of its own: the helpers were exercised only
//! through whichever grammar happened to sit above them, so every one reported
//! nought per-function coverage while the parsers around them were fully covered.
//! One copy, tested once, answers both problems.

/// Whether `c` is what the reference's `\b` calls a word character.
pub(crate) fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether `line` opens with `keyword` as a whole word.
///
/// Case-insensitive, because the reference accepts `Title` and `title` alike, and
/// whole-word, because otherwise every keyword would also match every label that
/// happens to start with it.
pub(crate) fn opens_with(line: &str, keyword: &str) -> bool {
    let Some(head) = line.get(..keyword.len()) else {
        return false;
    };
    head.eq_ignore_ascii_case(keyword)
        && line
            .get(keyword.len()..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|c| !is_word(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_word_character_is_what_a_keyword_may_run_into() {
        assert!(is_word('a') && is_word('Z') && is_word('7') && is_word('_'));
        assert!(!is_word(' ') && !is_word('-') && !is_word(':') && !is_word('é'));
    }

    #[test]
    fn a_keyword_is_matched_whole_and_however_it_is_cased() {
        assert!(opens_with("title Shares", "title"));
        assert!(opens_with("TITLE Shares", "title"));
        assert!(
            opens_with("Title", "title"),
            "the whole line is the keyword"
        );
    }

    #[test]
    fn a_label_that_merely_starts_with_a_keyword_is_not_one() {
        // The reason this is not `starts_with`: `titles` is a label.
        assert!(!opens_with("titles are nice", "title"));
        assert!(!opens_with("title_case", "title"));
        assert!(!opens_with("title7", "title"));
    }

    #[test]
    fn punctuation_ends_a_word_so_the_keyword_still_matches() {
        assert!(opens_with("title: Shares", "title"));
        assert!(
            opens_with("title-of", "title"),
            "a dash is not a word character"
        );
    }

    #[test]
    fn a_line_shorter_than_the_keyword_cannot_open_with_it() {
        assert!(!opens_with("tit", "title"));
        assert!(!opens_with("", "title"));
    }

    #[test]
    fn an_empty_keyword_opens_every_line_that_does_not_start_a_word() {
        // Degenerate, but it must not panic on the empty slice.
        assert!(!opens_with("title", ""));
        assert!(opens_with(" x", ""));
    }
}
