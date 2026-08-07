//! Structural safety check for agent-authored HTML chunks (Gate 1).
//!
//! Parses the chunk with the `tl` HTML parser and rejects `<script>`,
//! non-whitelisted tags, `on*` event-handler attributes, and unsafe URL schemes
//! (`javascript:`, `vbscript:`, `data:text/html`).
//!
//! It deliberately does **not** judge HTML *well-formedness* — HTML is lenient by
//! design (a real parser fixes up unbalanced tags rather than rejecting them), so
//! that concern belongs to the browser (Gate 2, the real parser) plus a served
//! Content-Security-Policy, exactly as diagram parse-validity is deferred to the
//! rendering engine.
//!
//! The `html` block is the **themed-content** block (`visual-system`): all visual
//! color must flow through semantic tokens, never a hardcoded value. So every
//! non-SVG element's inline `style` attribute is also handed to [`crate::style`],
//! which rejects color literals, `font-family`, and absolute/fixed positioning.
//!
//! It also admits a curated inline-SVG subset ([`SVG_TAGS`]) for images/logos/custom
//! diagrams. Inline SVG is **image-class**: exempt from the themed-color `style` scan
//! (a logo keeps its brand colors), while the `<script>`/`on*`/unsafe-URL checks still
//! apply inside it. The dangerous elements (`foreignObject`, SMIL, external refs) stay
//! off the whitelist and are rejected as disallowed tags.

use crate::block::{ValidationError, ValidationKind};

/// Semantic/layout tags permitted in an HTML chunk. Interactive and embedding
/// tags (`script`, `style`, `iframe`, `object`, `embed`, `form`, `input`) are
/// deliberately excluded.
const WHITELIST: &[&str] = &[
    "div",
    "span",
    "p",
    "a",
    "ul",
    "ol",
    "li",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "strong",
    "em",
    "b",
    "i",
    "u",
    "s",
    "small",
    "sub",
    "sup",
    "code",
    "pre",
    "blockquote",
    "hr",
    "br",
    "img",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
    "caption",
    "section",
    "article",
    "header",
    "footer",
    "main",
    "aside",
    "nav",
    "figure",
    "figcaption",
    "details",
    "summary",
    "mark",
    "abbr",
    "dl",
    "dt",
    "dd",
    "kbd",
    "samp",
    "var",
    "time",
    "cite",
    "q",
    "del",
    "ins",
    "button",
    "label",
];

/// The curated static inline-SVG subset (visual-system: "Inline SVG is safe image-class
/// content"). Drawing / paint / text elements only. The script-bearing and
/// HTML-embedding elements are deliberately absent — `script` (caught by name),
/// `foreignObject`, `style`, the SMIL animation tags (`animate`/`set`/…), and the
/// external-reference tags (`use`/`image`/`textPath`) — so they fall through to the
/// disallowed-tag error. Names are lowercased before lookup, so the camelCase SVG tags
/// (`linearGradient`, `clipPath`) appear here folded to lower case.
const SVG_TAGS: &[&str] = &[
    "svg",
    "g",
    "defs",
    "title",
    "desc",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "lineargradient",
    "radialgradient",
    "stop",
    "clippath",
    "mask",
];

/// Whether `name` (already ASCII-lowercased) is a curated inline-SVG element. Because
/// `foreignObject` is excluded from the subset, an SVG subtree contains only SVG
/// elements, so this doubles as the "inside an `<svg>`" test used to grant the
/// image-class `style` exemption.
fn is_svg_tag(name: &str) -> bool {
    SVG_TAGS.contains(&name)
}

/// Attribute keys whose value is a URL and must be scheme-checked.
const URL_ATTRS: &[&str] = &[
    "href",
    "src",
    "xlink:href",
    "action",
    "formaction",
    "poster",
];

/// Check an HTML chunk body for structural-safety violations. A parse failure
/// (extremely rare for the lenient `tl` parser) yields no Gate-1 errors — the
/// browser gate remains the source of truth for how the chunk actually parses.
pub(crate) fn check_html(body: &str, anchor: &str) -> Vec<ValidationError> {
    tl::parse(body, tl::ParserOptions::default())
        .map(|dom| scan(&dom, anchor))
        .unwrap_or_default()
}

/// Walk every element in the parsed DOM and collect violations.
fn scan(dom: &tl::VDom, anchor: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for node in dom.nodes() {
        if let Some(tag) = node.as_tag() {
            check_tag(tag, anchor, &mut errors);
        }
    }
    errors
}

/// Check one element's tag name and its attributes.
fn check_tag(tag: &tl::HTMLTag, anchor: &str, errors: &mut Vec<ValidationError>) {
    let name = tag.name().as_utf8_str().to_ascii_lowercase();
    if name == "script" {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlScript,
            "'<script>' is not allowed in an HTML chunk",
        ));
    } else if !WHITELIST.contains(&name.as_str()) && !is_svg_tag(&name) {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlDisallowedTag,
            format!("'<{name}>' is not a whitelisted tag"),
        ));
    }
    // An element inside an inline `<svg>` is image-class: its `style` paint is art, not
    // themed content, so it skips the token check (visual-system). The `<script>`,
    // `on*`, and unsafe-URL checks in `check_attr` still apply regardless.
    let image_class = is_svg_tag(&name);
    for (key, value) in tag.attributes().iter() {
        check_attr(key.as_ref(), value.as_deref(), image_class, anchor, errors);
    }
    check_diagram_node(tag, anchor, errors);
}

/// A custom-diagram node (an element whose `class` includes `ui-diagram-node`) is an
/// annotation target, so it must carry a non-empty `data-id` the reviewer's feedback
/// keys to — otherwise the annotation would dangle (visual-system §3.2). Layout-only
/// primitives (grid/row/region/…) are not nodes and need no id.
fn check_diagram_node(tag: &tl::HTMLTag, anchor: &str, errors: &mut Vec<ValidationError>) {
    let mut is_node = false;
    let mut has_data_id = false;
    for (key, value) in tag.attributes().iter() {
        // HTML attribute names are case-insensitive — the browser lowercases them — so
        // normalize the key before matching, mirroring `check_attr`. Accumulate with `|=`
        // so no attribute masks an earlier match. Caveat: `tl` collapses a *duplicated*
        // `class` to the last occurrence while a browser keeps the first, so a malformed
        // `class="ui-diagram-node" class="x"` can still slip past — a `tl` limitation the
        // check can't see (the first value is already gone); agents don't author dup attrs.
        match key.as_ref().to_ascii_lowercase().as_str() {
            "class" => {
                is_node |= value
                    .as_deref()
                    .is_some_and(|v| v.split_whitespace().any(|c| c == "ui-diagram-node"));
            }
            "data-id" => has_data_id |= value.as_deref().is_some_and(|v| !v.trim().is_empty()),
            _ => {}
        }
    }
    if is_node && !has_data_id {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlDiagramNodeNeedsId,
            "a '.ui-diagram-node' needs a non-empty 'data-id' so an annotation can key to it",
        ));
    }
}

/// Flag `on*` event-handler attributes and unsafe URL-scheme values. `image_class`
/// (the element is inside an inline SVG) exempts only the themed-color `style` scan —
/// the handler and URL-scheme checks always run, so a script vector inside SVG is still
/// caught.
fn check_attr(
    key: &str,
    value: Option<&str>,
    image_class: bool,
    anchor: &str,
    errors: &mut Vec<ValidationError>,
) {
    let key = key.to_ascii_lowercase();
    if key.len() > 2 && key.starts_with("on") {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlEventHandler,
            format!("event-handler attribute '{key}' is not allowed"),
        ));
    }
    if URL_ATTRS.contains(&key.as_str()) {
        if let Some(url) = value {
            if is_unsafe_url(url) {
                errors.push(ValidationError::new(
                    anchor,
                    ValidationKind::HtmlUnsafeUrl,
                    format!("attribute '{key}' uses an unsafe URL scheme"),
                ));
            }
        }
    }
    if key == "style" && !image_class {
        if let Some(style) = value {
            errors.extend(crate::style::check_style(style, anchor));
        }
    }
}

/// Whether a URL value uses a script-executing or HTML-smuggling scheme.
fn is_unsafe_url(value: &str) -> bool {
    let v = value.trim_start().to_ascii_lowercase();
    v.starts_with("javascript:") || v.starts_with("vbscript:") || v.starts_with("data:text/html")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(body: &str) -> Vec<ValidationKind> {
        check_html(body, "#h").into_iter().map(|e| e.kind).collect()
    }

    #[test]
    fn clean_semantic_html_passes() {
        let body = "<section><h2>Title</h2><p>Hello <strong>world</strong> and <a href=\"/x\">link</a>.</p></section>";
        assert!(check_html(body, "#h").is_empty());
    }

    #[test]
    fn void_and_self_closing_are_fine() {
        assert!(check_html("<p>one<br>two</p><hr><img src=\"a.png\" alt=\"a\"/>", "#h").is_empty());
    }

    #[test]
    fn script_is_flagged() {
        assert_eq!(
            kinds("<div><script>alert(1)</script></div>"),
            vec![ValidationKind::HtmlScript]
        );
    }

    #[test]
    fn non_whitelisted_tag_is_flagged() {
        assert_eq!(
            kinds("<marquee>hi</marquee>"),
            vec![ValidationKind::HtmlDisallowedTag]
        );
    }

    #[test]
    fn event_handler_attribute_is_flagged() {
        assert_eq!(
            kinds("<div onclick=\"steal()\">x</div>"),
            vec![ValidationKind::HtmlEventHandler]
        );
    }

    #[test]
    fn diagram_node_without_data_id_is_flagged() {
        // A `.ui-diagram-node` is an annotation target and needs an id to key to.
        assert_eq!(
            kinds("<div class=\"ui-diagram-node\">A</div>"),
            vec![ValidationKind::HtmlDiagramNodeNeedsId]
        );
        // an empty data-id doesn't count.
        assert_eq!(
            kinds("<div class=\"ui-diagram-grid ui-diagram-node\" data-id=\" \">A</div>"),
            vec![ValidationKind::HtmlDiagramNodeNeedsId]
        );
        // with a data-id it passes; layout-only primitives never need one.
        assert!(
            check_html("<div class=\"ui-diagram-node\" data-id=\"a\">A</div>", "#h").is_empty()
        );
        assert!(check_html(
            "<div class=\"ui-diagram-grid\"><span class=\"ui-diagram-label\">x</span></div>",
            "#h"
        )
        .is_empty());
        // HTML attribute names are case-insensitive: an uppercased CLASS is still a node
        // (flag it), and an uppercased DATA-ID is a valid id (pass it).
        assert_eq!(
            kinds("<div CLASS=\"ui-diagram-node\">A</div>"),
            vec![ValidationKind::HtmlDiagramNodeNeedsId]
        );
        assert!(check_html(
            "<div class=\"ui-diagram-node\" DATA-ID=\"n1\">A</div>",
            "#h"
        )
        .is_empty());
    }

    #[test]
    fn handler_detection_is_quote_aware() {
        assert_eq!(
            kinds("<div onmouseover=\"do a thing\">x</div>"),
            vec![ValidationKind::HtmlEventHandler]
        );
    }

    #[test]
    fn javascript_url_is_flagged() {
        assert_eq!(
            kinds("<a href=\"javascript:steal()\">x</a>"),
            vec![ValidationKind::HtmlUnsafeUrl]
        );
    }

    #[test]
    fn data_text_html_url_is_flagged() {
        assert_eq!(
            kinds("<a href=\"data:text/html;base64,PHNjcmlwdD4=\">y</a>"),
            vec![ValidationKind::HtmlUnsafeUrl]
        );
    }

    #[test]
    fn safe_and_data_image_urls_pass() {
        assert!(check_html(
            "<a href=\"/ok\"><img src=\"data:image/png;base64,AAAA\" alt=\"x\"></a>",
            "#h"
        )
        .is_empty());
    }

    #[test]
    fn comments_are_ignored() {
        assert!(check_html("<!-- a note --><div>ok</div>", "#h").is_empty());
    }

    #[test]
    fn inline_style_hardcoded_color_is_flagged() {
        // The themed-content rule reaches element `style` attributes via check_style.
        assert_eq!(
            kinds("<div style=\"color:#fff\">x</div>"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
    }

    #[test]
    fn inline_style_with_only_tokens_passes() {
        assert!(check_html(
            "<div style=\"color: var(--foreground); display: flex\">x</div>",
            "#h"
        )
        .is_empty());
    }

    #[test]
    fn short_on_prefixed_key_is_not_a_handler() {
        assert!(check_html("<div on>x</div>", "#h").is_empty());
    }

    #[test]
    fn unbalanced_markup_no_longer_errors() {
        // Well-formedness is dropped: the browser (Gate 2) is the real parser.
        assert!(check_html("<div><span>text</div>", "#h").is_empty());
    }

    #[test]
    fn unsafe_url_scheme_detection() {
        assert!(is_unsafe_url("javascript:x"));
        assert!(is_unsafe_url("  VBScript:x"));
        assert!(is_unsafe_url("data:text/html,x"));
        assert!(!is_unsafe_url("/relative/path"));
        assert!(!is_unsafe_url("data:image/png;base64,AA"));
    }

    #[test]
    fn curated_inline_svg_art_passes() {
        // Image-class SVG: curated shapes with paint literals are art, not themed
        // content, so `fill`/`stroke` color literals raise no violation.
        let svg = "<svg viewBox=\"0 0 96 96\"><g><path d=\"M0 0 L9 9\" fill=\"#f5f5f4\" \
                   stroke=\"#101114\"/><rect width=\"8\" height=\"8\" fill=\"#f59e0b\"/>\
                   <circle cx=\"5\" cy=\"5\" r=\"3\"/><polygon points=\"0,0 9,0 5,9\"/></g></svg>";
        assert!(check_html(svg, "#h").is_empty(), "curated svg art passes");
    }

    #[test]
    fn script_vectors_inside_svg_are_still_flagged() {
        // The image-class exemption is only for `style` color; the script surface is
        // caught inside SVG exactly as in HTML.
        assert_eq!(
            kinds("<svg><script>steal()</script></svg>"),
            vec![ValidationKind::HtmlScript]
        );
        assert_eq!(
            kinds("<svg onload=\"steal()\"></svg>"),
            vec![ValidationKind::HtmlEventHandler]
        );
        assert_eq!(
            kinds("<svg><rect onclick=\"x\"/></svg>"),
            vec![ValidationKind::HtmlEventHandler]
        );
        assert_eq!(
            kinds("<svg><a xlink:href=\"javascript:x\">y</a></svg>"),
            vec![ValidationKind::HtmlUnsafeUrl]
        );
    }

    #[test]
    fn html_embedding_and_smil_svg_elements_are_rejected() {
        // The dangerous static/animation elements are absent from the subset, so they
        // fall through to the disallowed-tag error.
        for body in [
            "<svg><foreignObject><p>x</p></foreignObject></svg>",
            "<svg><style>rect{fill:red}</style></svg>",
            "<svg><animate attributeName=\"x\"/></svg>",
            "<svg><set attributeName=\"onload\" to=\"alert(1)\"/></svg>",
        ] {
            assert_eq!(
                kinds(body),
                vec![ValidationKind::HtmlDisallowedTag],
                "rejected: {body}"
            );
        }
    }

    #[test]
    fn image_class_style_exemption_is_svg_only() {
        // A hardcoded color in a `style` inside SVG is allowed (image-class)...
        assert!(check_html("<svg><rect style=\"fill:#f00\"/></svg>", "#h").is_empty());
        // ...while the same literal on a non-SVG element is still rejected.
        assert_eq!(
            kinds("<div style=\"color:#fff\">x</div>"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
    }

    #[test]
    fn svg_currentcolor_passes_and_a_diagram_node_still_needs_an_id() {
        // currentColor lets a mark adapt to the theme.
        assert!(check_html("<svg><path d=\"M0 0\" fill=\"currentColor\"/></svg>", "#h").is_empty());
        // An annotatable SVG element still needs a `data-id` (custom-diagram rule).
        assert_eq!(
            kinds("<svg><g class=\"ui-diagram-node\">A</g></svg>"),
            vec![ValidationKind::HtmlDiagramNodeNeedsId]
        );
        assert!(check_html(
            "<svg><g class=\"ui-diagram-node\" data-id=\"n1\">A</g></svg>",
            "#h"
        )
        .is_empty());
    }
}
