//! The CSS a C4 drawing carries with it.
//!
//! Every colour is a theme token, so a page restyles the diagram by changing one
//! variable and nothing is re-rendered.
//!
//! The hover behaviour is here rather than in a script for a hard reason: the
//! viewer embeds this SVG under a Content-Security-Policy with no inline
//! scripting, so a highlight that pairs a badge with its wire and its
//! description has to be declarative. `:has()` gives the cross-reference — hovering *either* member of
//! a step selects *both*, which is what makes the relationship work in either
//! direction. One rule per step is unavoidable: CSS cannot say "the element whose
//! attribute matches mine".

use crate::icons::OUTLINE_CLASS;

/// Rules that do not depend on the diagram.
const STATIC_RULES: &str = "\
text{font-family:Inter,system-ui,sans-serif}\
.c4-box{fill:var(--_node-fill);stroke:var(--_node-stroke);stroke-width:1}\
.c4-box-ext{stroke-dasharray:5 3}\
.c4-icon{color:var(--_text)}\
.c4-accent{fill:var(--ags-accent,var(--_arrow))}\
.c4-tag{fill:var(--_text-sec)}\
.c4-tag-person{fill:var(--ags-accent,var(--_arrow))}\
.c4-label{fill:var(--_text)}\
.c4-techn{fill:var(--_text-muted)}\
.c4-descr{fill:var(--_text-sec)}\
.c4-edge{fill:none;stroke:var(--_line);stroke-width:1}\
.c4-arrow-head{fill:var(--_arrow)}\
.c4-boundary{fill:none;stroke:var(--_node-stroke);stroke-dasharray:4 4}\
.c4-boundary-label{fill:var(--_text-sec)}\
.c4-title{fill:var(--_text)}\
.c4-badge{fill:var(--ags-bg);stroke:var(--_arrow);stroke-width:1.2}\
.c4-badge-text{fill:var(--_text)}\
.c4-tip-hit{fill:transparent}\
.c4-tip{opacity:0;pointer-events:none}\
.c4-tip-box{fill:var(--_arrow)}\
.c4-tip-text{fill:var(--ags-bg)}\
.c4-step{cursor:default}";

/// Keep a step marker to characters a selector can carry.
///
/// The marker comes from the author's own numbering, so it is not guaranteed to
/// be a bare digit — and it lands inside an attribute selector.
fn selector_safe(step: &str) -> String {
    step.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

/// Pair each badge with its wire and its description bubble.
fn step_rules(steps: &[String]) -> String {
    let mut seen: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for step in steps {
        let s = selector_safe(step);
        if s.is_empty() || seen.contains(&s) {
            continue;
        }
        seen.push(s.clone());
        let hovered = format!("svg:has(.c4-step[data-step=\"{s}\"]:hover)");
        out.push(format!(
            "{hovered} .c4-step[data-step=\"{s}\"] .c4-badge{{fill:var(--_arrow);stroke:var(--_arrow)}}\
             {hovered} .c4-step[data-step=\"{s}\"] .c4-badge-text{{fill:var(--ags-bg)}}\
             {hovered} .c4-edge[data-step=\"{s}\"]{{stroke:var(--_arrow);stroke-width:2}}\
             svg:has(.c4-step[data-step=\"{s}\"] .c4-badge:hover) .c4-tip[data-step=\"{s}\"][data-at=\"badge\"],\
             svg:has(.c4-step[data-step=\"{s}\"] .c4-badge-text:hover) .c4-tip[data-step=\"{s}\"][data-at=\"badge\"],\
             svg:has(.c4-step[data-step=\"{s}\"] .c4-tip-hit:hover) .c4-tip[data-step=\"{s}\"][data-at=\"tip\"]{{opacity:1}}"
        ));
    }
    out.concat()
}

/// The whole style block: derived tokens, the class rules, and one hover group
/// per step.
pub fn style(tokens: &str, steps: &[String]) -> String {
    format!(
        "{tokens}{STATIC_RULES}\
         .{OUTLINE_CLASS}{{stroke-linecap:round;stroke-linejoin:round}}{}",
        step_rules(steps)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_derived_tokens_come_first_so_the_rules_can_read_them() {
        let css = style("svg{--_text:red;}", &[]);
        assert!(css.starts_with("svg{--_text:red;}"), "{css}");
        assert!(css.contains(".c4-label{fill:var(--_text)}"), "{css}");
    }

    #[test]
    fn no_rule_names_a_literal_colour() {
        let css = style("", &["1".into()]);
        assert!(!css.contains('#'), "{css}");
        assert!(!css.contains("rgb("), "{css}");
    }

    #[test]
    fn hovering_either_member_of_a_step_selects_both() {
        let css = style("", &["3".into()]);
        // The badge lights up from a hover anywhere in the step ...
        assert!(
            css.contains(
                "svg:has(.c4-step[data-step=\"3\"]:hover) .c4-step[data-step=\"3\"] .c4-badge{"
            ),
            "{css}"
        );
        // ... and so does the wire, which is not inside the step group at all.
        assert!(
            css.contains("svg:has(.c4-step[data-step=\"3\"]:hover) .c4-edge[data-step=\"3\"]"),
            "{css}"
        );
    }

    #[test]
    fn a_bubble_is_revealed_by_the_target_it_belongs_to() {
        let css = style("", &["1".into()]);
        // Two targets, two bubbles: hovering the badge shows the one beside the
        // badge, hovering an arrowhead shows the one beside that head.
        assert!(css.contains("[data-at=\"badge\"]"), "{css}");
        assert!(css.contains("[data-at=\"tip\"]"), "{css}");
    }

    #[test]
    fn an_authors_own_marker_survives_into_the_selector() {
        let css = style("", &["3a".into()]);
        assert!(css.contains("[data-step=\"3a\"]"), "{css}");
    }

    #[test]
    fn a_marker_that_could_break_a_selector_is_stripped_or_dropped() {
        let css = style("", &["a\"]:hover){}".into()]);
        assert!(css.contains("[data-step=\"ahover\"]"), "{css}");
        assert!(!css.contains("a\"]:hover"), "{css}");
        // Nothing usable left at all means no rule rather than a broken one.
        assert_eq!(style("", &["<>".into()]), style("", &[]));
    }

    #[test]
    fn a_step_named_twice_emits_one_group_of_rules() {
        let once = style("", &["1".into()]);
        let twice = style("", &["1".into(), "1".into()]);
        assert_eq!(once, twice);
    }

    #[test]
    fn a_diagram_with_no_steps_still_carries_its_class_rules() {
        let css = style("", &[]);
        assert!(css.contains(".c4-box{"), "{css}");
        assert!(!css.contains(":has("), "{css}");
    }
}
