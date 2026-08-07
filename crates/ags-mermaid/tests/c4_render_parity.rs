//! The C4 drawing, pinned against the renderer it replaces.
//!
//! Verified against the five real architecture diagrams by comparing every drawn
//! primitive — 1,138 of them — for tag, geometry, text, anchor, font size and the
//! classes that decide colour, after normalising the two things the renderers
//! spell differently and the page cannot tell apart: `translate(a b)` against
//! `translate(a,b)`, and a curve written with relative commands against the same
//! curve written with absolute ones. Every one matched, as did the set of
//! `data-id`, `data-from` and `data-to` values that feedback is keyed to.
//!
//! That check needed the renderer being replaced, so this stands in for it: the
//! same source, and the claims that would break silently if a later change
//! quietly dropped a layer, a hover rule, or an identity.

use ags_mermaid::{render_svg, ColorMode, Options};

const SOURCE: &str = r#"C4Container
title Render fixture
Person(dev, "Developer", "Explores a codebase")
System_Ext(mail, "Mail", "SMTP relay")
Container_Boundary(app, "Application") {
  Container(api, "API", "Rust", "Serves the graph")
  ContainerDb(store, "code.db", "SQLite")
}
Rel(dev, api, "1. Queries", "HTTPS")
BiRel(api, store, "Reads and writes", "SQLite")
Rel(api, mail, "Sends through", "SMTP")
"#;

fn rendered() -> String {
    render_svg(SOURCE, &Options::default())
        .map(|out| out.svg)
        .unwrap_or_default()
}

/// The drawing without its stylesheet.
///
/// Every class name appears in both, so a search over the whole document finds
/// the rule rather than the shape and answers a different question than the one
/// being asked.
fn body() -> String {
    rendered()
        .split("</style>")
        .nth(1)
        .unwrap_or_default()
        .to_string()
}

#[test]
fn every_element_keeps_the_identity_feedback_is_keyed_to() {
    let svg = rendered();
    for alias in ["dev", "mail", "api", "store"] {
        assert!(
            svg.contains(&format!("data-id=\"{alias}\"")),
            "{alias} lost its identity"
        );
    }
    // The relationship contract is the pair it joins, not an identity of its own.
    assert!(svg.contains("data-from=\"dev\" data-to=\"api\""), "{svg}");
    assert!(svg.contains("data-boundary=\"app\""), "{svg}");
}

#[test]
fn the_drawing_is_ordered_back_to_front() {
    let svg = body();
    let at = |needle: &str| svg.find(needle);
    // Frames behind wires behind boxes, and the description bubbles above all
    // of it — the layer stack, visible in document order because SVG has no
    // z-index to fall back on.
    assert!(at("c4-boundary") < at("c4-edge"), "{svg}");
    assert!(at("c4-edge") < at("class=\"node\""), "{svg}");
    assert!(at("class=\"node\"") < at("c4-badge"), "{svg}");
    assert!(at("c4-badge") < at("c4-tips"), "{svg}");
}

#[test]
fn a_step_pairs_its_badge_with_its_wire_without_a_script() {
    let svg = rendered();
    // The viewer serves this under a policy with no inline scripting, so the
    // cross-reference has to be a selector. Its root is the drawing's own scope
    // class, not a bare `svg` — one diagram's rules must not reach another's.
    assert!(
        svg.contains(":has(.c4-step[data-step=\"1\"]:hover)"),
        "{svg}"
    );
    assert!(
        !svg.contains("<style>svg:has("),
        "the hover pair must be confined to this drawing: {svg}"
    );
    assert!(svg.contains(".c4-edge[data-step=\"1\"]"), "{svg}");
    assert!(!svg.contains("<script"), "{svg}");
    assert!(!svg.contains("onmouseover"), "{svg}");
}

#[test]
fn a_description_appears_twice_because_the_arrowhead_is_far_from_the_badge() {
    let svg = body();
    // One bubble beside the badge, one beside each arrowhead — the two-headed
    // relationship therefore has three.
    assert_eq!(svg.matches("data-at=\"badge\"").count(), 3);
    assert_eq!(svg.matches("data-at=\"tip\"").count(), 4);
}

#[test]
fn no_colour_is_written_into_the_drawing_itself() {
    let svg = body();
    // Every fill and stroke reaches the shapes through a token, so a page
    // restyles the diagram by changing one variable and nothing re-renders.
    // A literal may appear only as the fallback inside a `var()`, which is what
    // keeps an extracted SVG from rendering unstyled.
    assert!(!svg.contains("fill=\"#"), "{svg}");
    assert!(!svg.contains("stroke=\"#"), "{svg}");
    assert!(!svg.contains("rgb("), "{svg}");
    for (i, _) in svg.match_indices('#') {
        let before = svg.get(..i).unwrap_or_default();
        // The two `#` a drawing may legitimately carry: a reference to the
        // arrowhead definition, and the fallback behind a token.
        let marker_ref = before.ends_with("url(");
        let token_fallback = before.ends_with(", ") && before.contains("var(--");
        assert!(
            marker_ref || token_fallback,
            "a literal colour outside a var() fallback at {i}: {svg}"
        );
    }
}

#[test]
fn a_standalone_image_carries_the_colours_a_page_would_have_supplied() {
    let svg = render_svg(
        SOURCE,
        &Options {
            colors: ColorMode::Fixed,
            ..Options::default()
        },
    )
    .map(|out| out.svg)
    .unwrap_or_default();
    // Nothing left for a missing cascade to resolve.
    assert!(svg.contains("--ags-bg:#ffffff"), "{svg}");
    assert!(!svg.contains("color-mix"), "{svg}");
    assert!(svg.contains("--_text:#1e2430"), "{svg}");
}

#[test]
fn the_same_source_always_draws_the_same_document() {
    assert_eq!(rendered(), rendered());
}

#[test]
fn a_diagram_with_nothing_in_it_still_produces_a_document() {
    let svg = render_svg("C4Context", &Options::default())
        .map(|out| out.svg)
        .unwrap_or_default();
    assert!(svg.starts_with("<svg"), "{svg}");
    assert!(svg.ends_with("</svg>"), "{svg}");
    assert!(!svg.contains("data-id="), "{svg}");
}
