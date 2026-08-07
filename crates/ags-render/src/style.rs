//! Themed-content style discipline for `html` blocks (Gate 1, `visual-system`).
//!
//! An `html` block is a *themed-content* block: all visual color must flow through
//! semantic CSS custom properties (`var(--…)`), never a hardcoded value. This
//! module scans an element's inline `style` attribute and rejects the channels
//! through which raw, off-theme styling would otherwise reach the page:
//! color literals (hex, the CSS color functions `rgb()`/`hsl()`/…, the CSS named
//! colors like `red`/`gold`, and the system-color keywords like `Canvas`/`ButtonText`),
//! `font-family` and the `font` shorthand (the renderer owns the font), and
//! absolute/fixed positioning (which escapes the renderer's flow layout). Only the
//! theme-neutral keywords `transparent` and `currentcolor` are allowed among color
//! words, since they carry no fixed hue. `url(...)`, quoted-string, and `var(...)`
//! spans are excluded from the color scan so a token reference or an SVG/image/text
//! value is not misread as a literal.
//!
//! Host-framework color *classes* (`bg-red-500`, `btn-primary`) are deliberately
//! **not** checked: the renderer serves a closed stylesheet (its own CSS plus the
//! kit) with no framework CSS, and `<style>`/`<script>` are whitelisted out (see
//! [`crate::html`]), so such a class selects no rule and paints nothing — an inert
//! channel a denylist would only guard with false positives.

use crate::block::{ValidationError, ValidationKind};
use crate::validate::is_hex_color;

/// Scan an inline `style` attribute value one `prop: value` declaration at a time,
/// collecting themed-content violations.
pub(crate) fn check_style(style: &str, anchor: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for decl in split_declarations(style) {
        let Some((prop, value)) = decl.split_once(':') else {
            continue;
        };
        check_declaration(
            &prop.trim().to_ascii_lowercase(),
            value.trim(),
            anchor,
            &mut errors,
        );
    }
    errors
}

/// Split an inline `style` into its `;`-separated declarations, ignoring a `;`
/// inside parentheses (`url(...)`, `calc(...)`) or a quoted string — so a data: URI
/// or a `content` string isn't torn apart and its tail mis-scanned as a declaration.
fn split_declarations(style: &str) -> Vec<&str> {
    let mut decls = Vec::new();
    let mut depth: u32 = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in style.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some(q) => match c {
                '\\' => escaped = true, // a `\"`/`\'` inside a string doesn't close it
                _ if c == q => quote = None,
                _ => {}
            },
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                ';' if depth == 0 => {
                    if let Some(d) = style.get(start..i) {
                        decls.push(d);
                    }
                    start = i + 1;
                }
                _ => {}
            },
        }
    }
    if let Some(d) = style.get(start..) {
        decls.push(d);
    }
    decls
}

/// Apply the themed-content rules to one CSS declaration.
fn check_declaration(prop: &str, value: &str, anchor: &str, errors: &mut Vec<ValidationError>) {
    // `font-family` and the `font` shorthand both set the family; the renderer owns it.
    if prop == "font-family" || prop == "font" {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlFontFamily,
            format!("themed content must not set '{prop}' — the renderer owns the font"),
        ));
    }
    // Match the leading keyword so a trailing `!important` — with or without a space
    // (`fixed !important`, `absolute!important`) — can't slip absolute/fixed past.
    let keyword = value
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_lowercase();
    if prop == "position" && matches!(keyword.as_str(), "absolute" | "fixed") {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlPositioning,
            format!("themed content must not use 'position: {value}' — it escapes flow layout"),
        ));
    }
    // A hex or color-function literal is unambiguous on any property; a bare color
    // WORD is only a color where a color is expected, so skip the named/system-color
    // pass on custom-ident properties (grid-area, animation-name, …) to avoid flagging
    // `grid-area: gold` or `animation: silver 1s`.
    if let Some(token) = color_literal(value, !IDENT_PROPS.contains(&prop)) {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::HtmlHardcodedColor,
            format!("hardcoded color '{token}' — reference a var(--token) instead"),
        ));
    }
}

/// Properties whose value is a user-chosen custom identifier (a grid line/area,
/// animation/counter/container name, …), where a word that happens to be a CSS color
/// name is NOT a color. The named/system-color word scan is skipped for these; the
/// unambiguous hex and color-function scans still run.
const IDENT_PROPS: &[&str] = &[
    "animation",
    "animation-name",
    "grid",
    "grid-area",
    "grid-row",
    "grid-row-start",
    "grid-row-end",
    "grid-column",
    "grid-column-start",
    "grid-column-end",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "counter-reset",
    "counter-increment",
    "counter-set",
    "will-change",
    "transition",
    "transition-property",
    "view-transition-name",
    "container",
    "container-name",
];

/// The first hardcoded color literal in a declaration value, if any: a CSS color
/// function (`rgb`/`hsl`/…), a `#`-hex token, or — when `scan_names` — a named or
/// system color. `url(...)`, quoted-string, and `var(...)` spans are all blanked
/// first, so an SVG ref like `url(#a1b2c3)`, text like `content: "#fff"`, and a token
/// reference like `var(--gold)` / `var(--x, teal)` are not misread as a literal.
fn color_literal(value: &str, scan_names: bool) -> Option<String> {
    let scrubbed = strip_fn_spans(&strip_quoted_spans(&strip_url_spans(value)), "var(");
    color_function(&scrubbed)
        .or_else(|| hex_token(&scrubbed))
        .or_else(|| scan_names.then(|| named_color(&scrubbed)).flatten())
}

/// Remove `url(...)` spans (case-insensitive) so their contents — SVG fragment ids
/// or image names — are not scanned as colors.
fn strip_url_spans(value: &str) -> String {
    strip_fn_spans(value, "url(")
}

/// Remove every `<open>...)` function-call span from `value` (case-insensitive;
/// `open` ends with `(`, e.g. `"url("` or `"var("`). The open token must be preceded
/// by a non-letter boundary (or start) so an identifier that merely ends in it
/// (`myvar(`, `myurl(`) is not mistaken for the function. Nested parens are balanced;
/// an unclosed span drops the rest of the value.
fn strip_fn_spans(value: &str, open: &str) -> String {
    let n = open.len();
    let mut out = String::with_capacity(value.len());
    let mut depth: u32 = 0;
    for c in value.chars() {
        if depth > 0 {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            continue;
        }
        out.push(c);
        if out.len() >= n {
            let matches_open = out
                .get(out.len() - n..)
                .is_some_and(|t| t.eq_ignore_ascii_case(open));
            // The open token must not be the tail of a longer identifier (`myvar(`).
            let on_boundary = out
                .get(..out.len() - n)
                .and_then(|pre| pre.chars().next_back())
                .is_none_or(|p| !p.is_ascii_alphabetic());
            if matches_open && on_boundary {
                out.truncate(out.len() - n);
                depth = 1;
            }
        }
    }
    out
}

/// Remove quoted-string spans (`"…"` / `'…'`) so a color-shaped substring inside a
/// string value (e.g. `content: "#fff"`) is not scanned as a color. A backslash
/// escapes the next char (so `\"` doesn't close the string); an unclosed quote drops
/// the rest of the value.
fn strip_quoted_spans(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some(q) => match c {
                '\\' => escaped = true,
                _ if c == q => quote = None,
                _ => {}
            },
            None if c == '"' || c == '\'' => quote = Some(c),
            None => out.push(c),
        }
    }
    out
}

/// CSS color-function notations that carry a literal color.
const COLOR_FUNCS: &[&str] = &[
    "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(", "color(",
];

/// The earliest CSS color-function call in `value` (case-insensitive), returned up
/// to its closing `)` (or end of value) as the offending token.
fn color_function(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    let idx = COLOR_FUNCS.iter().filter_map(|f| lower.find(f)).min()?;
    let rest = value.get(idx..)?;
    let end = rest.find(')').map_or(rest.len(), |o| o + 1);
    Some(rest.get(..end)?.trim().to_string())
}

/// The first `#`-prefixed hex color token (3/4/6/8 hex digits) in `value`.
fn hex_token(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        if b == b'#' {
            let mut j = i + 1;
            while bytes.get(j).is_some_and(u8::is_ascii_hexdigit) {
                j += 1;
            }
            if let Some(token) = value.get(i..j) {
                if is_hex_color(token) {
                    return Some(token.to_string());
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    None
}

/// The first CSS named or system color used as a whole alphabetic word in `value`.
/// `transparent`/`currentcolor` are theme-neutral and excluded from [`NAMED_COLORS`].
fn named_color(value: &str) -> Option<String> {
    value
        .split(|c: char| !c.is_ascii_alphabetic())
        .find(|w| {
            let lower = w.to_ascii_lowercase();
            !w.is_empty()
                && (NAMED_COLORS.contains(&lower.as_str())
                    || SYSTEM_COLORS.contains(&lower.as_str()))
        })
        .map(str::to_string)
}

/// CSS system-color keywords (Color Module 4, lowercased). They resolve to OS/theme
/// colors, so like a named color they paint outside the token system and are rejected.
/// The four short, ambiguous keywords `canvas`/`field`/`mark`/`highlight` are omitted:
/// they collide with common non-color idents (`animation-name: highlight`,
/// `grid-area: field`), so the bare form of those four is a documented residual.
const SYSTEM_COLORS: &[&str] = &[
    "accentcolor",
    "accentcolortext",
    "activetext",
    "buttonborder",
    "buttonface",
    "buttontext",
    "canvastext",
    "fieldtext",
    "graytext",
    "highlighttext",
    "linktext",
    "marktext",
    "selecteditem",
    "selecteditemtext",
    "visitedtext",
];

/// The CSS Color Module Level 4 named colors, minus the theme-neutral keywords
/// `transparent` and `currentcolor`. A hardcoded name is as off-theme as a hex.
const NAMED_COLORS: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(style: &str) -> Vec<ValidationKind> {
        check_style(style, "#h")
            .into_iter()
            .map(|e| e.kind)
            .collect()
    }

    #[test]
    fn hardcoded_hex_color_is_flagged() {
        assert_eq!(
            kinds("color:#fff;background:#112233"),
            vec![
                ValidationKind::HtmlHardcodedColor,
                ValidationKind::HtmlHardcodedColor
            ]
        );
    }

    #[test]
    fn color_functions_are_flagged() {
        assert_eq!(
            kinds("color: rgb(1,2,3)"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        assert_eq!(
            kinds("color: hsla(1,2%,3%,.5)"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        assert_eq!(
            kinds("color: oklch(0.5 0.1 200)"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
    }

    #[test]
    fn color_is_caught_in_any_property_not_just_color_and_background() {
        // The scan is property-agnostic: it reads every declaration's value.
        for decl in [
            "background-color:#abc",
            "border-color: red",
            "border: 1px solid #abcdef",
            "box-shadow: 0 0 4px rgba(0,0,0,.5)",
            "outline: 2px dashed teal",
            "fill:#f00",
            "stroke: navy",
            "caret-color: hotpink",
        ] {
            assert_eq!(
                kinds(decl),
                vec![ValidationKind::HtmlHardcodedColor],
                "expected {decl} to be flagged"
            );
        }
    }

    #[test]
    fn strip_url_spans_handles_nested_parens() {
        assert_eq!(strip_url_spans("url(a(b)c)after"), "after");
    }

    #[test]
    fn font_family_is_flagged() {
        assert_eq!(
            kinds("font-family: Comic Sans"),
            vec![ValidationKind::HtmlFontFamily]
        );
        // `font-size` is a length, not a family — allowed.
        assert!(kinds("font-size: 14px").is_empty());
    }

    #[test]
    fn absolute_and_fixed_positioning_are_flagged() {
        assert_eq!(
            kinds("position:absolute"),
            vec![ValidationKind::HtmlPositioning]
        );
        assert_eq!(
            kinds("position: FIXED"),
            vec![ValidationKind::HtmlPositioning]
        );
        // relative/sticky flow with the layout — allowed.
        assert!(kinds("position: relative").is_empty());
    }

    #[test]
    fn named_colors_are_flagged() {
        assert_eq!(
            kinds("color: red"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        assert_eq!(
            kinds("border: 1px solid darkslateblue"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        // a named color inside a gradient value is still caught.
        assert_eq!(
            kinds("background: linear-gradient(to right, gold, navy)"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
    }

    #[test]
    fn token_and_layout_styling_passes() {
        assert!(kinds("color: var(--foreground); background: var(--card)").is_empty());
        assert!(kinds("display:flex; gap:8px; padding:12px; border-radius:6px").is_empty());
        // theme-neutral color keywords carry no fixed hue and are allowed.
        assert!(kinds("background: transparent; color: currentcolor").is_empty());
        // non-color value words (layout/border keywords) are not mistaken for colors.
        assert!(kinds("border: 1px solid var(--border); align-items: center").is_empty());
    }

    #[test]
    fn named_color_requires_a_whole_word() {
        // substrings of longer identifiers are not colors.
        assert!(named_color("var(--foreground)").is_none());
        assert!(named_color("grid-template: subgrid").is_none());
        assert_eq!(named_color("border-color: gold").as_deref(), Some("gold"));
    }

    #[test]
    fn valueless_declaration_is_skipped() {
        // A malformed declaration with no ':' is not our concern (no panic, no error).
        assert!(kinds("not-a-declaration").is_empty());
    }

    #[test]
    fn url_fragment_ref_is_not_a_color() {
        // `url(#a1b2c3)` would look like a 6-digit hex without url-stripping.
        assert!(color_literal("fill: url(#a1b2c3)", true).is_none());
        // a genuine hex alongside a url() is still caught.
        assert_eq!(
            color_literal("fill: url(#grad); stroke:#f00", true).as_deref(),
            Some("#f00")
        );
    }

    #[test]
    fn strip_url_spans_handles_closed_unclosed_and_absent() {
        assert_eq!(strip_url_spans("a url(x) b"), "a  b");
        assert_eq!(strip_url_spans("a URL(x"), "a "); // unclosed drops the remainder
        assert_eq!(strip_url_spans("plain value"), "plain value");
    }

    #[test]
    fn hex_token_matches_only_valid_lengths() {
        assert_eq!(hex_token("#abc").as_deref(), Some("#abc"));
        assert_eq!(hex_token("#abcd").as_deref(), Some("#abcd"));
        assert_eq!(hex_token("#a1b2c3").as_deref(), Some("#a1b2c3"));
        assert_eq!(hex_token("#a1b2c3d4").as_deref(), Some("#a1b2c3d4"));
        // 5 hex digits is not a CSS color length.
        assert!(hex_token("#abcde").is_none());
        // a 6-hex prefix is a color even if a non-hex char follows.
        assert_eq!(hex_token("#abcdefg").as_deref(), Some("#abcdef"));
        // a bare '#', a non-hex run, a fragment id, and no '#' at all yield nothing.
        assert!(hex_token("#zzz").is_none());
        assert!(hex_token("url(#gradient)").is_none());
        assert!(hex_token("no hash here").is_none());
    }

    #[test]
    fn color_function_absent_returns_none() {
        assert!(color_function("display: flex").is_none());
    }

    #[test]
    fn positioning_survives_important_and_trailing_tokens() {
        // The rule matches the first token, so `!important` can't slip absolute/fixed past.
        assert_eq!(
            kinds("position: fixed !important"),
            vec![ValidationKind::HtmlPositioning]
        );
        assert_eq!(
            kinds("position:absolute!important"),
            vec![ValidationKind::HtmlPositioning]
        );
    }

    #[test]
    fn font_shorthand_is_flagged() {
        // The `font` shorthand sets the family too — not just `font-family`.
        assert_eq!(
            kinds("font: italic bold 14px/1.5 Georgia"),
            vec![ValidationKind::HtmlFontFamily]
        );
        // font-weight/size/style are not the family and stay allowed.
        assert!(kinds("font-weight: 600; font-style: italic").is_empty());
    }

    #[test]
    fn system_color_keywords_are_flagged() {
        // The unambiguous compound system colors are rejected…
        assert_eq!(
            kinds("color: CanvasText"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        assert_eq!(
            kinds("background: ButtonFace"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        assert_eq!(
            kinds("color: GrayText"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        // …but the four short, ident-colliding ones are NOT flagged, so common
        // layout/animation declarations that reuse those words still pass.
        assert!(kinds("animation-name: highlight").is_empty());
        assert!(kinds("grid-area: field; grid-area: canvas; grid-area: mark").is_empty());
    }

    #[test]
    fn color_named_token_reference_is_not_a_false_positive() {
        // A token whose name contains a color word is a reference, not a literal.
        assert!(kinds("color: var(--gold)").is_empty());
        assert!(kinds("background: var(--red-500); border-color: var(--blue)").is_empty());
        // A var() reference passes with OR without a fallback, consistently across the
        // color forms (named, hex, and function fallbacks alike).
        assert!(kinds("color: var(--x, teal)").is_empty());
        assert!(kinds("color: var(--x, #fff)").is_empty());
        assert!(kinds("color: var(--x, rgb(1,2,3))").is_empty());
        // a real literal alongside a var() is still caught.
        assert_eq!(
            kinds("background: var(--x) #f00"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
    }

    #[test]
    fn strip_fn_spans_requires_a_boundary_before_the_open() {
        // `myvar(` is an identifier ending in `var(`, not the var() function.
        assert_eq!(strip_fn_spans("myvar(navy)", "var("), "myvar(navy)");
        assert_eq!(strip_fn_spans("var(--x)", "var("), "");
        assert_eq!(strip_fn_spans("a var(--x) b", "var("), "a  b");
    }

    #[test]
    fn color_words_on_custom_ident_properties_are_not_flagged() {
        // A CSS color word used as a grid/animation/counter ident is not a color.
        for decl in [
            "grid-area: gold",
            "animation-name: silver",
            "animation: gold 1s",
            "counter-reset: teal 0",
            "grid-template-columns: [gold-start] 1fr",
            "will-change: transform",
        ] {
            assert!(kinds(decl).is_empty(), "expected {decl} to pass");
        }
        // …but an unambiguous hex/function literal is still caught even there.
        assert_eq!(
            kinds("grid-area: #f00"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
        // and a real color property with the same word IS still flagged.
        assert_eq!(
            kinds("color: gold"),
            vec![ValidationKind::HtmlHardcodedColor]
        );
    }

    #[test]
    fn escaped_quote_does_not_leak_a_color() {
        // A `\"` inside a string must not end the span and expose the text after it.
        assert!(kinds("content: \"Rated \\\"gold\\\" tier\"").is_empty());
        // …nor tear the declaration on a `;` inside the escaped string.
        assert!(kinds("content: \"a\\\"; color: red\"").is_empty());
        assert_eq!(strip_quoted_spans("\"a\\\"b\" red"), " red");
    }

    #[test]
    fn declarations_split_only_at_top_level() {
        // `;` inside url()/quotes doesn't split; top-level `;` does.
        assert_eq!(
            split_declarations("a: url(x;y); b: red"),
            vec!["a: url(x;y)", " b: red"]
        );
        assert_eq!(
            split_declarations("content: \"a;b\"; color: red"),
            vec!["content: \"a;b\"", " color: red"]
        );
        assert_eq!(split_declarations("color: red"), vec!["color: red"]);
    }

    #[test]
    fn color_shaped_string_literal_is_not_a_color() {
        // A `#hex` or color word inside a quoted string is text, not a painted color.
        assert!(kinds("content: \"#fff\"").is_empty());
        assert!(kinds("content: 'Gold tier'").is_empty());
    }

    #[test]
    fn url_span_with_semicolon_is_not_torn_into_a_false_color() {
        // The `;` inside a data: URI must not split the declaration and expose `navy`.
        assert!(kinds("background: url(data:image/svg+xml;stroke:navy)").is_empty());
    }

    #[test]
    fn strip_quoted_spans_drops_string_contents() {
        assert_eq!(strip_quoted_spans("a \"b;c\" d"), "a  d");
        assert_eq!(strip_quoted_spans("x 'unclosed"), "x "); // unclosed drops the rest
        assert_eq!(strip_quoted_spans("no quotes"), "no quotes");
    }
}
