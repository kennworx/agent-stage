//! Semantic Gate-1 validation of a parsed [`Artifact`].
//!
//! Enforces the `block-format` v1 rules — the closed type set, info-string
//! attribute schema, unique ids, and per-type body schema — plus the structural
//! HTML-chunk safety delegated to [`crate::html`]. The v1 fenced-type set is
//! `{mermaid, question, table, code, html, note}`, plus the `theme` config block.
//!
//! Diagram validity **is** checked here, by drawing the diagram. It used to be
//! deferred to a browser gate on the grounds that the CLI shipped no engine; the
//! CLI is the engine now, so deferring it would only mean discovering in the page
//! what could have been said at the gate. What is still not checked is legibility:
//! `render_svg` reports what a drawing gets wrong alongside the drawing, and a
//! diagram that reads badly is a finding for the agent rather than grounds to
//! refuse the review.

use std::collections::HashSet;

use crate::block::{AttrValue, Block, ValidationError, ValidationKind};

mod body;
mod fence;

use crate::parse::Artifact;
use body::validate_body;
pub(crate) use body::{is_hex_color, THEME_TOKENS};
use fence::near_miss_error;

/// Parse and validate `src` in one step (the CLI's Gate-1 entry point). An empty
/// result means the artifact is structurally valid.
#[must_use]
pub fn validate_source(src: &str) -> Vec<ValidationError> {
    validate(&crate::parse::parse_artifact(src))
}

/// Validate an artifact, returning every rule violation (empty = valid). Includes
/// the structural errors surfaced during parsing.
#[must_use]
pub fn validate(art: &Artifact) -> Vec<ValidationError> {
    let mut errors = art.structural_errors.clone();
    errors.extend(duplicate_id_errors(&art.blocks));
    for block in &art.blocks {
        errors.extend(validate_block(block));
    }
    for fence in &art.prose_fences {
        errors.extend(near_miss_error(fence));
    }
    errors
}

/// Validate a single block: attributes, feedback-needs-id, and per-type body
/// schema. Type membership is settled during parsing — a non-member fence never
/// becomes a [`Block`].
fn validate_block(block: &Block) -> Vec<ValidationError> {
    let anchor = block.anchor();
    let mut errors = Vec::new();
    errors.extend(feedback_needs_id(block, &anchor));
    errors.extend(validate_attrs(block, &anchor));
    errors.extend(validate_body(block, &anchor));
    errors
}

fn duplicate_id_errors(blocks: &[Block]) -> Vec<ValidationError> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut errors = Vec::new();
    for block in blocks {
        if let Some(id) = &block.id {
            if !seen.insert(id.as_str()) {
                errors.push(ValidationError::new(
                    block.anchor(),
                    ValidationKind::DuplicateId,
                    format!("id '#{id}' is already used by an earlier block"),
                ));
            }
        }
    }
    errors
}

/// When `feedback` is set to a value other than `none`, the block must carry an id.
fn feedback_needs_id(block: &Block, anchor: &str) -> Option<ValidationError> {
    let value = attr_value(block, "feedback")?;
    if value != "none" && block.id.is_none() {
        return Some(ValidationError::new(
            anchor,
            ValidationKind::FeedbackNeedsId,
            format!("feedback='{value}' requires an '#id' to route to"),
        ));
    }
    None
}

/// Validate every attribute against the type's schema, and check required keys.
fn validate_attrs(block: &Block, anchor: &str) -> Vec<ValidationError> {
    let specs = attr_specs(&block.type_token);
    let mut errors = Vec::new();
    for attr in &block.attrs {
        match specs.iter().find(|s| s.key == attr.key) {
            None => errors.push(ValidationError::new(
                anchor,
                ValidationKind::UnknownAttr,
                format!(
                    "'{}' is not a valid attribute for a '{}' block",
                    attr.key, block.type_token
                ),
            )),
            Some(spec) => errors.extend(check_attr_value(anchor, spec, &attr.value)),
        }
    }
    for key in required_keys(&block.type_token) {
        if !block.attrs.iter().any(|a| a.key == *key) {
            errors.push(ValidationError::new(
                anchor,
                ValidationKind::MissingAttr,
                format!(
                    "a '{}' block requires the '{key}' attribute",
                    block.type_token
                ),
            ));
        }
    }
    errors
}

/// Check one attribute's value shape against its spec.
fn check_attr_value(anchor: &str, spec: &AttrSpec, value: &AttrValue) -> Option<ValidationError> {
    let bad = |detail: String| {
        Some(ValidationError::new(
            anchor,
            ValidationKind::BadAttrValue,
            detail,
        ))
    };
    match (&spec.values, value) {
        (AttrValues::Flag, AttrValue::Flag) | (AttrValues::Any, AttrValue::Value(_)) => None,
        (AttrValues::Flag, AttrValue::Value(_)) => {
            bad(format!("'{}' is a flag and takes no value", spec.key))
        }
        (AttrValues::Any, AttrValue::Flag) => bad(format!("'{}' requires a value", spec.key)),
        (AttrValues::OneOf(set), AttrValue::Value(v)) if set.contains(&v.as_str()) => None,
        (AttrValues::OneOf(set), AttrValue::Value(v)) => bad(format!(
            "'{}={v}' is not one of [{}]",
            spec.key,
            set.join(", ")
        )),
        (AttrValues::OneOf(set), AttrValue::Flag) => bad(format!(
            "'{}' requires one of [{}]",
            spec.key,
            set.join(", ")
        )),
    }
}

pub(super) fn attr_value<'a>(block: &'a Block, key: &str) -> Option<&'a str> {
    block.attrs.iter().find_map(|a| match &a.value {
        AttrValue::Value(v) if a.key == key => Some(v.as_str()),
        _ => None,
    })
}

/// The shape a given attribute value may take.
enum AttrValues {
    /// A bare flag (no value).
    Flag,
    /// Any non-empty value.
    Any,
    /// One of a fixed set.
    OneOf(&'static [&'static str]),
}

/// One attribute's schema: its key and permitted value shape.
struct AttrSpec {
    key: &'static str,
    values: AttrValues,
}

impl AttrSpec {
    fn flag(key: &'static str) -> Self {
        Self {
            key,
            values: AttrValues::Flag,
        }
    }
    fn any(key: &'static str) -> Self {
        Self {
            key,
            values: AttrValues::Any,
        }
    }
    fn one_of(key: &'static str, set: &'static [&'static str]) -> Self {
        Self {
            key,
            values: AttrValues::OneOf(set),
        }
    }
}

/// The attribute schema for a block type: universal attributes plus its own.
fn attr_specs(type_token: &str) -> Vec<AttrSpec> {
    let mut specs = vec![
        AttrSpec::one_of("feedback", &["none", "annotate", "comment"]),
        AttrSpec::any("title"),
        AttrSpec::flag("collapsible"),
    ];
    match type_token {
        "mermaid" => {
            specs.push(AttrSpec::one_of("direction", &["TD", "LR", "BT", "RL"]));
        }
        "question" => {
            specs.push(AttrSpec::one_of(
                "type",
                &["radio", "checkbox", "text", "select"],
            ));
            specs.push(AttrSpec::flag("required"));
        }
        "table" => specs.push(AttrSpec::flag("sort")),
        "code" => {
            specs.push(AttrSpec::any("lang"));
            specs.push(AttrSpec::flag("wrap"));
        }
        "note" => specs.push(AttrSpec::one_of("kind", &["info", "warn", "claim"])),
        _ => {}
    }
    specs
}

/// The attribute keys a block type requires.
fn required_keys(type_token: &str) -> &'static [&'static str] {
    match type_token {
        "question" => &["type"],
        "code" => &["lang"],
        _ => &[],
    }
}

/// Human-readable one-line summaries of a type's attribute schema, derived from the
/// same [`attr_specs`]/[`required_keys`] the validator enforces — so the
/// [`crate::catalog`] shows exactly what Gate 1 accepts. A flag renders as its key,
/// a free value as `key=<value>`, an enum as `key=a|b|c`; a required key is prefixed
/// with `*` (distinct from any attribute literally named `required`).
pub(crate) fn attr_summaries(type_token: &str) -> Vec<String> {
    let required = required_keys(type_token);
    attr_specs(type_token)
        .iter()
        .map(|spec| {
            let shape = match &spec.values {
                AttrValues::Flag => spec.key.to_string(),
                AttrValues::Any => format!("{}=<value>", spec.key),
                AttrValues::OneOf(set) => format!("{}={}", spec.key, set.join("|")),
            };
            if required.contains(&spec.key) {
                format!("*{shape}")
            } else {
                shape
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::body::{
        diagram_error, is_dash_cell, is_separator_row, parse_row, unknown_diagram_detail,
    };
    use super::fence::{is_one_edit_apart, FENCE_LANGUAGES};
    use super::*;
    use crate::block::BLOCK_TYPES;
    use crate::parse::parse_artifact;
    use ags_mermaid::DiagramType;

    fn errors_for(src: &str) -> Vec<ValidationError> {
        validate(&parse_artifact(src))
    }

    fn kinds(src: &str) -> Vec<ValidationKind> {
        errors_for(src).into_iter().map(|e| e.kind).collect()
    }

    #[test]
    fn valid_artifact_has_no_errors() {
        let src = "\
```mermaid #flow feedback=annotate direction=TD
graph TD
  A-->B
```
Some prose.
```question #commit type=radio required
Which host?
- TypeScript
- Rust
```
```table #t
| a | b |
| 1 | 2 |
```
```code #c lang=rust
fn main() {}
```";
        assert!(
            errors_for(src).is_empty(),
            "expected clean, got {:?}",
            errors_for(src)
        );
    }

    #[test]
    fn unknown_type_is_prose_and_carries_no_block_rules() {
        // `timeline` is not a block type and is not one edit from one, so the fence
        // is ordinary prose — and its block-only attribute is not judged.
        assert!(errors_for("```timeline #x bogus=1\nbody\n```").is_empty());
    }

    #[test]
    fn language_tags_are_not_near_misses() {
        for src in [
            "```rust\nx\n```",
            "```json\nx\n```",
            "```bash\nx\n```",
            "```text\nx\n```",
            "```sh\nx\n```",
            "```go\nx\n```",
            "```toml\nx\n```",
            "```yaml\nx\n```",
            "```diff\nx\n```",
            "```\nx\n```",
            // The six that really do sit one edit from a block type. Each is a
            // language someone writes on purpose, so each must pass.
            "```node\nconsole.log(1)\n```",
            "```none\nx\n```",
            "```htm\n<p>x</p>\n```",
            "```html5\n<p>x</p>\n```",
            "```xhtml\n<p>x</p>\n```",
            "```haml\n%p x\n```",
        ] {
            assert!(errors_for(src).is_empty(), "{src} should validate as prose");
        }
    }

    #[test]
    fn the_excluded_languages_are_exactly_the_ones_that_collide() {
        // The list earns its place by being the collisions, not a hunch: every
        // entry must actually be one edit from a block type, or it is dead weight
        // silencing nothing.
        for lang in FENCE_LANGUAGES {
            assert!(
                BLOCK_TYPES
                    .iter()
                    .any(|known| is_one_edit_apart(lang, known)),
                "'{lang}' is not one edit from any block type, so excluding it is pointless"
            );
        }
    }

    #[test]
    fn near_miss_fence_types_are_rejected() {
        for (src, want) in [
            ("```mermiad\ngraph TD\n```", "mermaid"),
            ("```tabel\n| a |\n```", "table"),
            ("```nte\nhi\n```", "note"),
            // `hmtl`, not `htm`: the transposition is the typo, whereas `htm` is
            // the file extension and now validates as the prose it is.
            ("```hmtl\n<p>x</p>\n```", "html"),
            ("```codes\nx\n```", "code"),
        ] {
            let errs = errors_for(src);
            assert_eq!(errs.len(), 1, "{src}");
            assert_eq!(errs[0].kind, ValidationKind::NearMissType);
            assert!(errs[0].detail.contains(want), "{src} → {}", errs[0].detail);
        }
    }

    #[test]
    fn near_miss_error_anchors_to_type_and_line() {
        let errs = errors_for("prose\n\n```mermiad\ngraph TD\n```");
        assert_eq!(errs[0].anchor, "mermiad@3");
    }

    #[test]
    fn one_edit_apart_covers_each_shape() {
        // Substitution, deletion, insertion, transposition — then identity and too-far.
        assert!(is_one_edit_apart("nose", "note"));
        assert!(is_one_edit_apart("nte", "note"));
        assert!(is_one_edit_apart("notes", "note"));
        assert!(is_one_edit_apart("nto", "not"));
        assert!(!is_one_edit_apart("note", "note"));
        assert!(!is_one_edit_apart("rust", "note"));
        assert!(!is_one_edit_apart("", "note"));
        assert!(!is_one_edit_apart("transposed", "note"));
    }

    #[test]
    fn transposition_is_one_edit_though_levenshtein_calls_it_two() {
        // The case the rule exists for: two substitutions under plain edit
        // distance, one swap under Damerau.
        assert!(is_one_edit_apart("mermiad", "mermaid"));
        assert!(is_one_edit_apart("tabel", "table"));
        // A non-adjacent swap is genuinely two edits.
        assert!(!is_one_edit_apart("dermaim", "mermaid"));
        // Equal length with more than two differences is not a transposition.
        assert!(!is_one_edit_apart("xxxxxxx", "mermaid"));
    }

    #[test]
    fn one_insertion_rejects_a_second_divergence() {
        // Length differs by one, so the insertion path runs — but after skipping
        // the leading `x` the tails still disagree, so it is more than one edit.
        assert!(!is_one_edit_apart("xnoet", "note"));
        // Two characters longer never reaches the insertion path at all.
        assert!(!is_one_edit_apart("noteXY", "note"));
    }

    #[test]
    fn note_block_validates_body_and_optional_kind() {
        // A bare note (kind is optional) with a non-empty markdown body passes.
        assert!(errors_for("```note #n\nA reasoning claim.\n```").is_empty());
        // Each allowed kind is accepted.
        assert!(errors_for("```note #n kind=warn\nheads up\n```").is_empty());
        assert!(errors_for("```note #n kind=claim\nthe host is portable\n```").is_empty());
        // An out-of-set kind is a bad enum value.
        assert_eq!(
            kinds("```note #n kind=bogus\nx\n```"),
            vec![ValidationKind::BadAttrValue]
        );
        // An empty body is rejected like the other non-empty-body types.
        assert_eq!(kinds("```note #n\n\n```"), vec![ValidationKind::EmptyBody]);
    }

    #[test]
    fn duplicate_ids_are_flagged_once_for_the_second() {
        let src = "```code #dup lang=rust\na\n```\n```table #dup\n| x |\n```";
        assert_eq!(
            kinds(src)
                .iter()
                .filter(|k| **k == ValidationKind::DuplicateId)
                .count(),
            1
        );
    }

    #[test]
    fn feedback_without_id_fails() {
        assert_eq!(
            kinds("```code lang=rust feedback=annotate\na\n```"),
            vec![ValidationKind::FeedbackNeedsId]
        );
    }

    #[test]
    fn feedback_none_without_id_is_fine() {
        assert!(errors_for("```code lang=rust feedback=none\na\n```").is_empty());
    }

    #[test]
    fn unknown_attr_is_rejected() {
        assert_eq!(
            kinds("```mermaid #m type=radio\ngraph TD\n  A-->B\n```"),
            vec![ValidationKind::UnknownAttr]
        );
    }

    #[test]
    fn bad_enum_value_flag_and_value_shape_are_checked() {
        // Bodies are real diagrams, not placeholders: the mermaid body is checked
        // by drawing it, so `x` would add a diagram-type error to every assertion
        // and stop these from testing the attribute rule alone.
        assert_eq!(
            kinds("```mermaid #m direction=XX\ngraph TD\n  A-->B\n```"),
            vec![ValidationKind::BadAttrValue]
        );
        // flag given a value
        assert_eq!(
            kinds("```mermaid #m collapsible=yes\ngraph TD\n  A-->B\n```"),
            vec![ValidationKind::BadAttrValue]
        );
        // value attr given as flag
        assert_eq!(
            kinds("```code #c lang\nx\n```"),
            vec![ValidationKind::BadAttrValue]
        );
        // enum attr given as flag
        assert_eq!(
            kinds("```mermaid #m direction\ngraph TD\n  A-->B\n```"),
            vec![ValidationKind::BadAttrValue]
        );
    }

    #[test]
    fn required_attr_missing_is_flagged() {
        assert_eq!(
            kinds("```code #c\nx\n```"),
            vec![ValidationKind::MissingAttr]
        );
    }

    #[test]
    fn empty_bodies_are_flagged() {
        assert_eq!(
            kinds("```mermaid #m\n\n```"),
            vec![ValidationKind::EmptyBody]
        );
        assert_eq!(
            kinds("```code #c lang=rust\n\n```"),
            vec![ValidationKind::EmptyBody]
        );
    }

    #[test]
    fn question_needs_prompt_and_enough_options() {
        // choice with one option
        assert_eq!(
            kinds("```question #q type=radio\nPick?\n- only\n```"),
            vec![ValidationKind::QuestionOptions]
        );
        // no prompt, only options
        assert_eq!(
            kinds("```question #q type=checkbox\n- a\n- b\n```"),
            vec![ValidationKind::EmptyBody]
        );
        // text type needs no options
        assert!(errors_for("```question #q type=text\nYour name?\n```").is_empty());
    }

    #[test]
    fn table_rectangularity_with_separator_row() {
        // separator row is ignored; the short data row is flagged
        let src = "```table #t\n| a | b | c |\n| - | - | - |\n| 1 | 2 |\n```";
        assert_eq!(kinds(src), vec![ValidationKind::TableArity]);
    }

    #[test]
    fn table_without_rows_is_empty() {
        assert_eq!(
            kinds("```table #t\nno pipes here\n```"),
            vec![ValidationKind::EmptyBody]
        );
    }

    #[test]
    fn parse_row_strips_outer_pipes() {
        assert_eq!(parse_row("| a | b |"), vec!["a", "b"]);
        assert_eq!(parse_row("a | b"), vec!["a", "b"]);
    }

    #[test]
    fn separator_detection() {
        assert!(is_separator_row(&["---".to_string(), ":-:".to_string()]));
        assert!(!is_separator_row(&["a".to_string()]));
        assert!(!is_dash_cell(":::"));
        assert!(is_dash_cell(" :--: "));
    }

    #[test]
    fn attr_value_finds_value_not_flag() {
        let art = parse_artifact("```code #c lang=rust wrap\nx\n```");
        let b = &art.blocks[0];
        assert_eq!(attr_value(b, "lang"), Some("rust"));
        assert_eq!(attr_value(b, "wrap"), None);
        assert_eq!(attr_value(b, "missing"), None);
    }

    #[test]
    fn mermaid_mode_live_is_accepted_in_grammar() {
        // `mode` is gone: the viewer no longer edits a diagram, the agent does.
        assert!(!errors_for("```mermaid #m mode=live\ngraph TD\n A-->B\n```").is_empty());
    }

    #[test]
    fn an_unrenderable_diagram_is_rejected_rather_than_left_to_the_page() {
        // This used to pass Gate 1 on body-non-empty alone and fail later in a
        // browser. There is no later now — the gate draws it.
        assert_eq!(
            kinds("```mermaid #m\nthis is not valid mermaid at all !!!\n```"),
            vec![ValidationKind::DiagramType]
        );
    }

    #[test]
    fn an_unknown_diagram_type_names_the_supported_set() {
        let errors = errors_for("```mermaid #m\nsunburstChart\n  a: 1\n```");
        assert_eq!(errors.len(), 1);
        let detail = &errors[0].detail;
        assert!(detail.contains("sunburstchart"), "{detail}");
        // The set is spelled out, from the renderer's own list.
        for kind in [
            DiagramType::Pie,
            DiagramType::Sequence,
            DiagramType::Wardley,
        ] {
            assert!(detail.contains(kind.keyword()), "{detail}");
        }
    }

    #[test]
    fn a_mistyped_diagram_type_is_reported_as_the_typo_it_is() {
        let errors = errors_for("```mermaid #m\nmindmpa\n  root\n```");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, ValidationKind::DiagramType);
        // The suggestion replaces the full set: one guess beats twenty-seven.
        assert!(
            errors[0].detail.contains("did you mean 'mindmap'"),
            "{:?}",
            errors[0]
        );
        assert!(!errors[0].detail.contains("supported:"), "{:?}", errors[0]);
    }

    #[test]
    fn a_diagram_with_no_header_at_all_says_so() {
        // `found` is empty here, which reads as a different sentence from a header
        // that was present but unrecognised.
        let detail = unknown_diagram_detail("", None);
        assert!(detail.contains("declares no type"), "{detail}");
        assert!(detail.contains("supported:"), "{detail}");
    }

    #[test]
    fn a_malformed_diagram_reports_the_line_it_broke_on() {
        // Constructed directly because nothing in the renderer produces one: every
        // parser is lenient today. The mapping still has to be right for the day one
        // is not, and this is the only way to exercise it.
        let err = diagram_error(
            "#m",
            &ags_mermaid::RenderError::Malformed {
                line: 3,
                message: "bad token".into(),
            },
        );
        assert_eq!(err.kind, ValidationKind::DiagramMalformed);
        assert_eq!(err.anchor, "#m");
        assert!(err.detail.contains("line 3"), "{err:?}");
        assert!(err.detail.contains("bad token"), "{err:?}");
    }

    #[test]
    fn a_drawable_diagram_passes_even_when_it_reads_badly() {
        // Legibility violations ride alongside the SVG and are deliberately not
        // gate failures — 15 of the 117 gallery diagrams trip one, and refusing
        // them would refuse artifacts a reviewer would happily read.
        assert!(errors_for("```mermaid #m\ngraph TD\n  A-->B\n```").is_empty());
        assert!(errors_for("```mermaid #x\nxychart-beta\n  bar [1, 2, 3]\n```").is_empty());
    }

    #[test]
    fn validate_body_returns_no_body_errors_for_unhandled_types() {
        // `validate_body` is only reached for known types via `validate_block`, each of
        // which has a body arm; its defensive fallback yields no body errors when called
        // directly with a token outside the match (an unknown type never reaches here in
        // the normal flow, since `validate_block` short-circuits it first).
        let block = Block {
            type_token: "timeline".into(),
            id: Some("n".into()),
            attrs: vec![],
            body: "hi".into(),
            line: 1,
            end: 4,
            ordinal: 0,
        };
        assert!(validate_body(&block, "#n").is_empty());
    }

    #[test]
    fn theme_block_accepts_tokens_sections_and_rejects_bad_lines() {
        // known tokens (3-/4-/6-/8-digit hex) plus dark:/light: mode sections pass.
        assert!(errors_for(
            "```theme #t\nprimary: #bb9af7\nforeground: #abcd\ndark:\nbackground: #101\nlight:\nbackground: #11223344\n```"
        )
        .is_empty());
        // a lone `seed: #hex` accent is a complete theme — the renderer derives the rest.
        assert!(errors_for("```theme #t\nseed: #6a5acd\n```").is_empty());
        // unknown token, non-hex values, a colon-less line, and a non-hex seed each flag theme-token.
        assert!(kinds("```theme #t\nzoom: #fff\n```").contains(&ValidationKind::ThemeToken));
        assert!(kinds("```theme #t\nprimary: red\n```").contains(&ValidationKind::ThemeToken));
        assert!(kinds("```theme #t\nprimary: #12x\n```").contains(&ValidationKind::ThemeToken));
        assert!(kinds("```theme #t\nprimary #fff\n```").contains(&ValidationKind::ThemeToken));
        assert!(
            kinds("```theme #t\nseed: rebeccapurple\n```").contains(&ValidationKind::ThemeToken)
        );
        // sections but no tokens has nothing to apply.
        assert!(kinds("```theme #t\ndark:\nlight:\n```").contains(&ValidationKind::EmptyBody));
    }
}
