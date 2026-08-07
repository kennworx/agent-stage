//! The block catalog — the agent-facing reference for the closed v1 vocabulary
//! (`artifact-authoring` D1 / §1.1). `ags catalog` prints it, and the authoring
//! skill has the agent read it *before* authoring so it never emits a block or
//! attribute Gate 1 would reject.
//!
//! Its **type list** and **attribute schema** are built from the validator's own
//! definitions — the [`crate::validate`] type set ([`BLOCK_TYPES`]) and
//! [`attr_summaries`] — so those cannot drift from what Gate 1 enforces (§1.3). Each
//! type's one-line **purpose** and **body rule** are hand-authored summaries: the
//! catalog test guards that every known type has both (so adding a type forces the
//! catalog to keep up), but *not* that the body-rule prose matches the validator —
//! so keep it in sync when a per-type body check in [`crate::validate`] changes.

use crate::affordances::affordance_summaries;
use crate::block::BLOCK_TYPES;
use crate::validate::attr_summaries;
use ags_mermaid::DiagramType;

/// One-line purpose for a block type — what it is for, so the agent picks the right one.
fn type_purpose(type_token: &str) -> &'static str {
    match type_token {
        "mermaid" => "a diagram; the accepted headers are listed at the end of this catalog",
        "question" => "ask the human a choice or free-text question; the answer routes back",
        "table" => "a markdown data table",
        "code" => "a read-only source/code excerpt",
        "html" => {
            "themed rich content — semantic HTML (kit classes + var(--token) colors) plus inline SVG art (logos, icons, custom diagrams)"
        }
        "note" => "an addressable callout — an info/warning aside, or a kind=claim the human can annotate and answer yes/no",
        "theme" => "define a color theme the viewer can apply (token: #hex lines, or a single seed: #hex the renderer expands into a full palette)",
        _ => "",
    }
}

/// One-line body rule for a block type — what Gate 1 requires of the body. A
/// hand-authored summary of [`crate::validate`]'s per-type body checks; only its
/// non-emptiness is tested, so keep the wording in sync when a body check changes.
fn body_rule(type_token: &str) -> &'static str {
    match type_token {
        "mermaid" => "diagram source whose first line names an accepted type, and which the renderer can draw — Gate 1 draws it to find out",
        "question" => "a prompt line, then `- option` lines (radio/checkbox/select need at least two)",
        "table" => "markdown rows; every row has the same cell count as the header",
        "code" => "non-empty source text",
        "html" => "whitelisted tags only, incl. a curated inline-SVG subset (svg/g/path/rect/circle/…); no <script>/handlers/unsafe URLs; themed HTML color via var(--tokens) — no hardcoded hex/rgb/named color, font-family, or absolute positioning; inline SVG is image-class (fixed fill/stroke fine; use fill=currentColor to theme-adapt); a custom-diagram `.ui-diagram-node` must carry a `data-id`",
        "note" => "non-empty markdown (a callout body)",
        "theme" => "`token: #hex` lines and/or a `seed: #hex` accent (optionally grouped under `dark:`/`light:` sections); a lone seed derives card/border/muted-foreground/primary per mode, and bg+fg seeds fill the middle tokens — explicit tokens always win",
        _ => "",
    }
}

/// Render the block catalog: the closed type set, each type's purpose, its attribute
/// schema (from the validator), and its body rule.
#[must_use]
pub fn block_catalog() -> String {
    let mut out = String::from(
        "agent-stage block catalog — the closed set of ADDRESSABLE block types (Gate 1).\n\n\
         Syntax: a fenced block  ```<type> #<id> [attr …]  then the body, then ```.\n\
         `#id` is required on any block whose `feedback` is not `none`. A `*` marks a\n\
         required attribute.\n\n\
         Prose vs blocks: everything between fences is implicit prose, rendered as\n\
         GitHub Flavored Markdown — headings, lists, tables, blockquotes, links, and\n\
         fenced code all work with no fence type and no id. A fence whose type is NOT\n\
         in this catalog (```rust, ```json, ```) is prose too: it renders as a plain\n\
         code block. That is valid, not an error.\n\
         So fence a block only when the human needs to ACT on it. In particular\n\
         ```rust is prose and carries no line-annotation affordance — use\n\
         ```code lang=rust when you want a reviewer to comment on a specific line.\n\
         A plain markdown table and a ```table block render identically; the only\n\
         difference is that a reviewer can annotate a cell of the latter.\n\
         A mistyped type one edit from a real one (```mermiad) is rejected, so a typo\n\
         cannot silently degrade into a code block.\n\n\
         The `human can:` line lists the human-side actions the block supports —\n\
         annotate (with its sub-target), answer, comment, or edit. These are review\n\
         affordances, NOT values of the `feedback` attribute (whose only values are\n\
         none/annotate/comment); a parenthesized hint is the annotation target or the\n\
         attribute that unlocks the verb.\n\
         Read this before authoring — only these types and attributes pass Gate 1.\n",
    );
    for &t in BLOCK_TYPES {
        out.push('\n');
        out.push_str(t);
        out.push_str(" — ");
        out.push_str(type_purpose(t));
        out.push_str("\n  attrs: ");
        out.push_str(&attr_summaries(t).join(", "));
        out.push_str("\n  body:  ");
        out.push_str(body_rule(t));
        let affordances = affordance_summaries(t);
        if !affordances.is_empty() {
            out.push_str("\n  human can: ");
            out.push_str(&affordances.join(", "));
        }
        out.push('\n');
    }
    out.push_str(&diagram_types());
    out
}

/// The diagram headers a `mermaid` block may open with.
///
/// Generated from the renderer's own [`DiagramType::ALL`] rather than listed here,
/// because this text exists to tell an agent what Gate 1 will accept and the
/// renderer *is* what Gate 1 asks. A hand-written list could say `sunburst` long
/// after the renderer stopped drawing one; this cannot.
///
/// Canonical spellings only — several types take more (a flowchart also answers to
/// `graph` and `stateDiagram`, C4 to four further headers) — so the list reads as
/// one line per idea instead of one per synonym.
fn diagram_types() -> String {
    let mut names: Vec<&str> = DiagramType::ALL.iter().map(|k| k.keyword()).collect();
    names.sort_unstable_by_key(|n| n.to_ascii_lowercase());
    format!(
        "\nmermaid diagram types ({}) — the header that opens each:\n  {}\n\
         A header outside this set is rejected, with a suggestion when it is one edit\n\
         from a real one. Some types accept further spellings (graph/stateDiagram for\n\
         flowchart; C4Container/C4Component/C4Dynamic/C4Deployment for C4Context) and a\n\
         `-beta`/`-v2` suffix is always accepted.\n",
        names.len(),
        names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_covers_every_known_type_with_purpose_and_body() {
        let cat = block_catalog();
        for &t in BLOCK_TYPES {
            assert!(cat.contains(t), "catalog lists '{t}'");
            assert!(!type_purpose(t).is_empty(), "'{t}' has a purpose");
            assert!(!body_rule(t).is_empty(), "'{t}' has a body rule");
        }
    }

    #[test]
    fn catalog_shows_type_specific_and_required_attrs_from_the_validator() {
        let cat = block_catalog();
        // enum + required-attr surface, formatted from the validator's own schema.
        assert!(cat.contains("direction=TD|LR|BT|RL"), "{cat}");
        assert!(cat.contains("*type=radio|checkbox|text|select"), "{cat}");
        assert!(cat.contains("*lang=<value>"), "{cat}");
        assert!(cat.contains("kind=info|warn|claim"), "{cat}");
        // universal attrs appear on every type.
        assert!(cat.contains("feedback=none|annotate|comment"), "{cat}");
    }

    #[test]
    fn catalog_shows_the_per_type_feedback_affordances() {
        let cat = block_catalog();
        // The affordance registry is surfaced per type, with sub-target/gate hints,
        // under a `human can:` label distinct from the `feedback=` attribute.
        assert!(cat.contains("human can: annotate (node)"), "{cat}");
        assert!(
            cat.contains("human can: annotate, comment, answer (kind=claim)"),
            "{cat}"
        );
        // `theme` is agent config with no human feedback, so it prints no affordance line.
        let theme_section = cat.split("\ntheme — ").nth(1).unwrap_or("");
        assert!(
            !theme_section.contains("human can:"),
            "theme has no affordance line: {cat}"
        );
    }

    #[test]
    fn catalog_lists_the_diagram_types_the_renderer_actually_draws() {
        let cat = block_catalog();
        // Generated from the renderer, so the list cannot name a type Gate 1 rejects
        // nor omit one it accepts.
        for kind in DiagramType::ALL {
            assert!(
                cat.contains(kind.keyword()),
                "catalog omits '{}'",
                kind.keyword()
            );
        }
        assert!(cat.contains("mermaid diagram types (27)"), "{cat}");
    }

    #[test]
    fn unknown_type_has_no_purpose_or_body() {
        // The drift-guard fallbacks: a type outside the set yields empty strings, so
        // the covers-every-type test above would fail if a type were added without one.
        assert_eq!(type_purpose("timeline"), "");
        assert_eq!(body_rule("timeline"), "");
    }
}
