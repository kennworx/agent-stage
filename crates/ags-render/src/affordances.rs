//! The feedback-affordance registry (`block-taxonomy` §3.3).
//!
//! Maps each block type to the feedback verbs a human may perform on it — the closed
//! `{annotate, answer, comment, edit}` set from the design's type table — together with
//! the sub-target granularity or gating condition each verb carries (a diagram node, a
//! table cell, `kind=claim` for a note's answer).
//!
//! Gate 1 does **not** enforce affordances: they are a rendering + `feedback-transport`
//! concern, not a validity rule. This table is instead the single agent-facing
//! *declaration* of "what can a human do with each block type you author" — surfaced by
//! `ags catalog` (so the agent authors with the interaction model in mind) and available
//! to the feedback layer as the one place the mapping is defined. The catalog iterates
//! [`crate::validate::BLOCK_TYPES`], so every known type is guaranteed a row (a missing
//! one is caught by [`tests::every_known_fenced_type_has_a_declared_affordance_row`]);
//! the per-type verbs and hints, however, are hand-maintained here and mirror the
//! `block-taxonomy` design table — keep them in sync when an affordance changes.
//!
//! The `cell`/`line` annotation sub-targets below are the *designed* granularity from
//! that table; the viewer currently routes only node/block sub-targets, with per-cell
//! and per-line routing landing with `feedback-transport`'s remaining tiers.

/// A feedback verb a human may perform on a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affordance {
    /// Point at an element (block, diagram node, table cell, code line) and comment.
    Annotate,
    /// Give a structured response (a question's option, a claim's yes/no).
    Answer,
    /// Leave a free comment on the whole block.
    Comment,
}

impl Affordance {
    /// The lowercase verb as it appears in the catalog and the feedback channel.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Affordance::Annotate => "annotate",
            Affordance::Answer => "answer",
            Affordance::Comment => "comment",
        }
    }
}

/// One entry in a type's affordance list: a verb plus an optional hint — the
/// annotation sub-target (`node`/`cell`/`line`) or the attribute that gates the verb
/// (`kind=claim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffordanceSpec {
    /// The feedback verb.
    pub verb: Affordance,
    /// A sub-target granularity or gating condition, shown parenthesized in the catalog.
    pub hint: Option<&'static str>,
}

/// Shorthand for an entry, keeping the per-type tables below readable. Written as a
/// struct literal (not a helper fn) so it is pure compile-time data with no runtime
/// body — the tables are `const`.
macro_rules! aff {
    ($verb:ident) => {
        AffordanceSpec {
            verb: Affordance::$verb,
            hint: None,
        }
    };
    ($verb:ident, $hint:literal) => {
        AffordanceSpec {
            verb: Affordance::$verb,
            hint: Some($hint),
        }
    };
}

const MERMAID: &[AffordanceSpec] = &[aff!(Annotate, "node")];
/// A question takes a comment as well as an answer: picking an option says which,
/// and a reviewer often wants to say why — or that none of them fit.
const QUESTION: &[AffordanceSpec] = &[aff!(Answer), aff!(Comment)];
const TABLE: &[AffordanceSpec] = &[aff!(Annotate, "cell"), aff!(Comment)];
const CODE: &[AffordanceSpec] = &[aff!(Annotate, "line"), aff!(Comment)];
const HTML: &[AffordanceSpec] = &[aff!(Annotate, "node"), aff!(Comment)];
const NOTE: &[AffordanceSpec] = &[aff!(Annotate), aff!(Comment), aff!(Answer, "kind=claim")];

/// The feedback affordances a block type exposes, in catalog order. A type with no
/// human feedback (the agent-config `theme` type, or an unknown token) returns `&[]`.
#[must_use]
pub fn affordances(type_token: &str) -> &'static [AffordanceSpec] {
    match type_token {
        "mermaid" => MERMAID,
        "question" => QUESTION,
        "table" => TABLE,
        "code" => CODE,
        "html" => HTML,
        "note" => NOTE,
        _ => &[],
    }
}

/// One-line summaries of a type's affordances for the catalog, e.g.
/// `["annotate (node)", "edit (mode=live)"]` — the verb, with its hint parenthesized.
/// Empty when the type exposes no feedback.
#[must_use]
pub fn affordance_summaries(type_token: &str) -> Vec<String> {
    affordances(type_token)
        .iter()
        .map(|a| match a.hint {
            Some(hint) => format!("{} ({hint})", a.verb.label()),
            None => a.verb.label().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BLOCK_TYPES;

    #[test]
    fn label_covers_every_verb() {
        // Exercise each arm so the mapping stays total and the tokens are the expected
        // lowercase verbs.
        assert_eq!(Affordance::Annotate.label(), "annotate");
        assert_eq!(Affordance::Answer.label(), "answer");
        assert_eq!(Affordance::Comment.label(), "comment");
    }

    #[test]
    fn every_known_fenced_type_has_a_declared_affordance_row() {
        // Each type in the validator's set resolves through `affordances` (theme is the
        // one known type with an intentionally empty row — it is agent config, not
        // human-facing content).
        for &t in BLOCK_TYPES {
            let row = affordances(t);
            if t == "theme" {
                assert!(row.is_empty(), "theme exposes no feedback");
            } else {
                assert!(!row.is_empty(), "'{t}' declares at least one affordance");
            }
        }
    }

    #[test]
    fn unknown_type_has_no_affordances() {
        assert!(affordances("timeline").is_empty());
        assert!(affordance_summaries("timeline").is_empty());
    }

    #[test]
    fn summaries_render_verb_and_hint() {
        // A hinted verb is parenthesized; a bare verb is just its label.
        assert_eq!(
            affordance_summaries("mermaid"),
            vec!["annotate (node)".to_string()]
        );
        assert_eq!(
            affordance_summaries("question"),
            vec!["answer".to_string(), "comment".to_string()]
        );
        // A note carries the block-level annotate/comment pair plus the claim-gated answer.
        assert_eq!(
            affordance_summaries("note"),
            vec![
                "annotate".to_string(),
                "comment".to_string(),
                "answer (kind=claim)".to_string(),
            ]
        );
    }
}
