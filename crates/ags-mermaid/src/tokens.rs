//! Writing a colour the way the target needs it.
//!
//! [`crate::ColorMode::Fixed`] promises literal colours throughout, for an image
//! with no document behind it. What it actually produced was literal values bound
//! to custom properties — `--_node-fill:#f8f8f9` — which the class rules then read
//! back through `var(--_node-fill)`. Inside a browser that resolves; in anything
//! else it does not, and a `fill` that fails to parse falls back to black.
//!
//! The symptom was a standalone SVG that rendered as black rectangles with no text
//! and no strokes. It affected every consumer of `Fixed`, not just the rasteriser
//! that found it — `ags-mermaid file.mmd` had been emitting it all along.
//!
//! Done as a pass over the finished document rather than in each of the
//! twenty-eight renderers, because the fault is not in any of them: each writes
//! `var(--token)` correctly, and the question is only whether a cascade will be
//! there to answer. One place decides that.

/// The value a `var(...)` call resolves to, given its inner text.
///
/// `var(--x)` yields `--x`'s binding; `var(--x, red)` yields it too, falling back
/// to `red` when nothing bound it. An unbound name with no fallback yields
/// `None`, and the call is left as it was — a wrong colour is a worse answer than
/// an unresolved one, because only the unresolved one is visible as a defect.
fn resolve(inner: &str, bound: &[(String, String)]) -> Option<String> {
    let (name, fallback) = match inner.split_once(',') {
        Some((name, rest)) => (name.trim(), Some(rest.trim())),
        None => (inner.trim(), None),
    };
    let name = name.strip_prefix("--")?;
    bound
        .iter()
        .find(|(bound_name, _)| bound_name == name)
        .map(|(_, value)| value.clone())
        .or_else(|| fallback.map(ToString::to_string))
}

/// Where a `var(` call closes, counting the calls nested inside it.
///
/// The first `)` is not the right one when the fallback is itself a call:
/// `var(--_node-fill,var(--ags-bg))` closes at the second. Taking the first left
/// the outer bracket stranded in the output — `fill:#f8f8f9)` — which is not a
/// colour, so the fill was dropped and the box was painted black. Seven diagram
/// types write a fallback that way, and every one of them shipped like that.
fn closing(body: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (at, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(at),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Replace one pass of `var(...)` calls. Returns the text and whether anything
/// changed.
#[expect(
    clippy::string_slice,
    reason = "every offset is a char boundary: `find` returns one, and the only \
              arithmetic on it adds the length of an ASCII literal"
)]
fn substitute(svg: &str, bound: &[(String, String)]) -> (String, bool) {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    let mut changed = false;
    while let Some(at) = rest.find("var(") {
        let (before, from_call) = rest.split_at(at);
        out.push_str(before);
        let body = &from_call["var(".len()..];
        let Some(close) = closing(body) else {
            out.push_str(from_call);
            return (out, changed);
        };
        match resolve(&body[..close], bound) {
            Some(value) => {
                out.push_str(&value);
                changed = true;
            }
            // Put the call back exactly as it was, closing paren included.
            None => out.push_str(&from_call[..=(close + "var(".len())]),
        }
        rest = &body[close + 1..];
    }
    out.push_str(rest);
    (out, changed)
}

/// How many times substitution may run before it is treated as circular.
///
/// A value may name another token, so one pass is not always enough; a token that
/// named itself would never settle. Four is far more nesting than the vocabulary
/// has and terminates regardless.
const MAX_PASSES: usize = 4;

/// Write every `var()` in `css` as `values` says it should be written.
///
/// The mapping is supplied by the caller — it is [`crate::Colors`], built from the
/// theme and the target — rather than discovered by reading the finished document.
/// That is the difference between resolving a colour and rewriting one: this
/// answers the name the renderer asked for, and cannot be fooled by a `--foo:`
/// that happens to appear inside a label.
pub(crate) fn resolve_all(css: &str, values: &[(String, String)]) -> String {
    let mut out = css.to_string();
    for _ in 0..MAX_PASSES {
        let (next, changed) = substitute(&out, values);
        out = next;
        if !changed {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Vec<(String, String)> {
        vec![
            ("_text".to_string(), "#1e2430".to_string()),
            ("_arrow".to_string(), "#40454f".to_string()),
        ]
    }

    #[test]
    fn a_named_token_becomes_its_value() {
        let css = ".n{fill:var(--_text)}";
        assert_eq!(resolve_all(css, &values()), ".n{fill:#1e2430}");
    }

    #[test]
    fn a_call_nested_in_a_fallback_closes_at_the_right_bracket() {
        // Seven diagram types write their fill this way. Closing at the first
        // `)` left the outer one behind — `fill:#1e2430)` is not a colour, so the
        // fill was dropped and the box came out black.
        let css = ".n{fill:var(--_text,var(--ags-bg))}";
        assert_eq!(resolve_all(css, &values()), ".n{fill:#1e2430}");
        assert!(
            !resolve_all(css, &values()).contains(')'),
            "no bracket survives"
        );
    }

    #[test]
    fn an_unbound_name_falls_back_to_the_call_inside_its_fallback() {
        // Nothing binds `--_group-fill`, so the fallback stands — and it is a
        // call of its own, which the next pass resolves.
        let bound = vec![("ags-bg".to_string(), "#ffffff".to_string())];
        assert_eq!(
            resolve_all(".n{fill:var(--_group-fill,var(--ags-bg))}", &bound),
            ".n{fill:#ffffff}"
        );
    }

    #[test]
    fn a_call_that_never_closes_is_left_alone() {
        let css = ".n{fill:var(--_text";
        assert_eq!(resolve_all(css, &values()), css);
    }

    #[test]
    fn a_fallback_is_used_only_when_the_config_has_no_answer() {
        let bound = values();
        assert_eq!(resolve("--_text, #999", &bound).as_deref(), Some("#1e2430"));
        assert_eq!(resolve("--absent, #999", &bound).as_deref(), Some("#999"));
        assert_eq!(resolve("--absent", &bound), None);
        assert_eq!(
            resolve("notaproperty", &bound),
            None,
            "must name a property"
        );
    }

    #[test]
    fn an_unanswerable_call_is_left_alone_rather_than_guessed() {
        // Visible beats wrong: a black box that should have been blue is a defect
        // nobody can see the cause of.
        let out = resolve_all(".n{fill:var(--nope)}", &values());
        assert!(out.contains("fill:var(--nope)"), "{out}");
    }

    #[test]
    fn attributes_are_written_as_well_as_rules() {
        // The arrow marker carries its paint as a presentation attribute.
        let css = r#"<polygon fill="var(--_arrow, #3b82f6)"/>"#;
        let out = resolve_all(css, &values());
        assert!(out.contains(r##"fill="#40454f""##), "{out}");
    }

    #[test]
    fn a_value_naming_another_token_settles() {
        let bound = vec![
            ("a".to_string(), "#0a0a0a".to_string()),
            ("b".to_string(), "var(--a)".to_string()),
        ];
        assert_eq!(resolve_all(".n{fill:var(--b)}", &bound), ".n{fill:#0a0a0a}");
    }

    #[test]
    fn a_value_naming_itself_terminates() {
        // Bounded rather than fixed-point, so a circular reference cannot hang.
        let bound = vec![("loop".to_string(), "var(--loop)".to_string())];
        let out = resolve_all(".n{fill:var(--loop)}", &bound);
        assert!(
            out.contains("var(--loop)"),
            "unsettled, but returned: {out}"
        );
    }

    #[test]
    fn css_with_no_references_is_returned_unchanged() {
        let css = ".n{fill:#123456}";
        assert_eq!(resolve_all(css, &values()), css);
    }

    #[test]
    fn an_unterminated_call_is_left_as_it_stands() {
        let out = resolve_all(".n{fill:var(--_text}", &values());
        assert!(out.contains("var(--_text}"), "{out}");
    }
}
