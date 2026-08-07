//! Heading identity: text to a stable, GitHub-compatible id fragment.

use std::collections::HashMap;

/// Turns heading text into an id, disambiguating repeats deterministically.
///
/// State is per-slugger, so the caller controls the uniqueness scope: one
/// slugger over a whole artifact yields document-unique ids. That scope is not
/// a detail — two prose runs separated by a diagram can each open with
/// `## Provenance`, and those must not both claim the same fragment.
#[derive(Debug, Default)]
pub struct Slugger {
    seen: HashMap<String, usize>,
}

impl Slugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// The id for `text`, unique within this slugger.
    pub fn slug(&mut self, text: &str) -> String {
        let base = slug_base(text);
        let n = self.seen.entry(base.clone()).or_insert(0);
        let seen_before = *n;
        *n += 1;
        if seen_before == 0 {
            base
        } else {
            format!("{base}-{seen_before}")
        }
    }
}

/// The un-disambiguated slug for `text`.
///
/// Follows GitHub's rule — lower-case, drop everything that is not a letter,
/// number, space or hyphen, then collapse whitespace to hyphens — which is what
/// makes a `#fragment` copied from a rendered GitHub document resolve here too.
///
/// A heading with no sluggable characters, punctuation or emoji only, would
/// otherwise produce an empty id and not be addressable at all; those fall back
/// to a fixed stem and are disambiguated as usual.
fn slug_base(text: &str) -> String {
    let kept: String = text
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == '-')
        .collect();
    let hyphenated: String = kept.split_whitespace().collect::<Vec<_>>().join("-");
    if hyphenated.is_empty() {
        "section".to_string()
    } else {
        hyphenated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_becomes_the_fragment_github_would_give_it() {
        let mut s = Slugger::new();
        assert_eq!(
            s.slug("What came from the graph?"),
            "what-came-from-the-graph"
        );
        // A dropped character leaves a whitespace run, which collapses to one
        // hyphen rather than two.
        assert_eq!(
            s.slug("C4 Level 1 — System Context"),
            "c4-level-1-system-context"
        );
    }

    #[test]
    fn whitespace_collapses_and_edges_are_trimmed() {
        let mut s = Slugger::new();
        assert_eq!(s.slug("  Spaced   out  "), "spaced-out");
        assert_eq!(s.slug("Tabs\tand\nnewlines"), "tabs-and-newlines");
    }

    #[test]
    fn repeats_are_disambiguated_in_order() {
        // Two prose runs either side of a diagram can both open "## Provenance".
        let mut s = Slugger::new();
        assert_eq!(s.slug("Provenance"), "provenance");
        assert_eq!(s.slug("Provenance"), "provenance-1");
        assert_eq!(s.slug("Provenance"), "provenance-2");
    }

    #[test]
    fn a_heading_with_nothing_sluggable_still_gets_an_id() {
        let mut s = Slugger::new();
        assert_eq!(s.slug("!!!"), "section");
        assert_eq!(s.slug("🎉"), "section-1");
    }

    #[test]
    fn letters_outside_ascii_are_kept() {
        let mut s = Slugger::new();
        assert_eq!(s.slug("Überblick"), "überblick");
        assert_eq!(s.slug("日本語"), "日本語");
    }

    #[test]
    fn hyphens_survive_and_punctuation_does_not() {
        let mut s = Slugger::new();
        assert_eq!(
            s.slug("well-known: the `code` path"),
            "well-known-the-code-path"
        );
    }

    #[test]
    fn two_sluggers_do_not_share_a_namespace() {
        let mut a = Slugger::new();
        let mut b = Slugger::new();
        assert_eq!(a.slug("Notes"), "notes");
        assert_eq!(b.slug("Notes"), "notes");
    }
}
