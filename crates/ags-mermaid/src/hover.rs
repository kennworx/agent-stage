//! Lighting a wire and the label that names it, together, from the stylesheet.
//!
//! A flowchart needs none of this: its title is a child of its wire's own group,
//! so `:has()` reads the pairing structurally and four static rules cover every
//! drawing. A class, ER or requirement diagram cannot do that — a label inside
//! the line's group paints *behind every box*, so the label has to be a separate
//! top-level node, and CSS has no way to say "the element whose attribute
//! matches mine". That leaves what C4 does: name the pair on both halves and
//! write one rule for each.
//!
//! Done in CSS rather than a script because the viewer embeds these drawings
//! under a Content-Security-Policy with no inline scripting.
//!
//! Only the pairs that have something written on them get rules. An unlabelled
//! wire has nothing to hover and is left alone.

/// The attribute naming which wire a label belongs to, on both halves.
///
/// Bare, because `Node::tagged` writes the `data-` prefix itself; the selectors
/// below spell it out.
pub const PAIR: &str = "rel";

/// The highlight colour, falling back to the arrow ink where a page sets no
/// accent of its own.
const LIT: &str = "var(--ags-accent,var(--_arrow))";

/// The parts of a wire, and what lighting each one means.
///
/// Named by element rather than by class so that one generator serves three
/// diagram types whose lines and labels are classed differently: a `polyline` or
/// `line` is the wire, a `circle` is the ring on a "zero or one" end, a
/// `polygon` is an arrowhead, and `text` is what is written on it.
const PARTS: [(&str, &str); 5] = [
    ("polyline", "stroke:{LIT};stroke-width:2"),
    ("line", "stroke:{LIT}"),
    ("circle", "stroke:{LIT}"),
    ("polygon", "fill:{LIT};stroke:{LIT}"),
    ("text", "fill:{LIT}"),
];

/// One rule set per labelled wire: hovering either half lights both.
///
/// Each part is matched two ways, because the diagram types differ in how much
/// they could group. Class and ER put a wire's pieces in one group and tag the
/// group; a requirement diagram cannot group its pieces at all — its label plate
/// is a decoration and its verb a label, and both have to stay top-level to
/// paint in the right order — so there the tag lands on each piece itself.
pub fn pairs(labelled: &[usize]) -> String {
    let mut out: Vec<String> = Vec::new();
    for id in labelled {
        let hovered = format!("svg:has([data-{PAIR}=\"{id}\"]:hover)");
        for (part, paint) in &PARTS {
            let paint = paint.replace("{LIT}", LIT);
            out.push(format!(
                "{hovered} [data-{PAIR}=\"{id}\"] {part},\
                 {hovered} {part}[data-{PAIR}=\"{id}\"]{{{paint}}}"
            ));
        }
        out.push(format!("[data-{PAIR}=\"{id}\"]{{cursor:default}}"));
    }
    out.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hovering_either_half_of_a_pair_lights_both() {
        let css = pairs(&[3]);
        // The `:has()` is on the drawing's root and names neither half in
        // particular, which is what makes it work from the label or the line.
        assert!(css.contains("svg:has([data-rel=\"3\"]:hover)"), "{css}");
        assert!(
            css.contains("svg:has([data-rel=\"3\"]:hover) [data-rel=\"3\"] polyline"),
            "{css}"
        );
        assert!(
            css.contains("svg:has([data-rel=\"3\"]:hover) [data-rel=\"3\"] text"),
            "{css}"
        );
    }

    #[test]
    fn a_wire_with_nothing_written_on_it_gets_no_rule() {
        assert!(pairs(&[]).is_empty());
        let css = pairs(&[0, 2]);
        assert!(css.contains("data-rel=\"0\""), "{css}");
        assert!(css.contains("data-rel=\"2\""), "{css}");
        assert!(!css.contains("data-rel=\"1\""), "{css}");
    }

    #[test]
    fn no_rule_names_a_literal_colour() {
        let css = pairs(&[0]);
        assert!(!css.contains('#'), "{css}");
        assert!(!css.contains("rgb("), "{css}");
    }

    #[test]
    fn the_crows_feet_of_a_relationship_light_with_its_line() {
        // ER draws its feet as plain `line` elements inside the wire's group,
        // so they have to be named or half the wire stays dark.
        let css = pairs(&[0]);
        assert!(css.contains("[data-rel=\"0\"] line,"), "{css}");
    }

    #[test]
    fn a_part_is_matched_whether_it_is_the_tagged_node_or_inside_one() {
        // Class and ER tag the group; a requirement diagram, which cannot group
        // its pieces, tags each piece. One rule has to cover both.
        let css = pairs(&[0]);
        for part in PARTS.map(|(part, _)| part) {
            assert!(
                css.contains(&format!("[data-rel=\"0\"] {part},")),
                "no descendant form for {part}: {css}"
            );
            assert!(
                css.contains(&format!("{part}[data-rel=\"0\"]")),
                "no self form for {part}: {css}"
            );
        }
    }
}
