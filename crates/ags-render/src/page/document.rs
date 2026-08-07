//! The whole page, assembled ahead of serving.
//!
//! One HTML document with nothing outside it: no stylesheet to fetch, no script
//! bundle, no font request. That is the point of rendering ahead of time — a
//! reader on a phone, on a page served from a fresh port that shares no cache
//! with the last one, pays for exactly what is on the page and one round trip.
//!
//! Theming is CSS only, including the toggle. A page that renders without a
//! script should also theme without one, so the control is a checkbox and the
//! rule that reads it is `:has()`. Reaching for a script here would mean the
//! theme was the one thing on the page that needed JavaScript to work.

use crate::block::Block;
use crate::parse::{parse_artifact, Artifact};
use crate::prose::{Heading, Prose};

use super::blocks::{block_anchor_id, escape, render as render_block};
use super::segment::{segments, Segment};
use super::theme::{css as theme_tokens, Mode as ThemeMode, Theme};

/// The page's stylesheets: surface colours, the reading column, how a block sits
/// on the page.
///
/// They lived in `web/src/styles/`, reached by `include_str!` back when a viewer
/// used them too; nothing else does now, so they sit beside the code emitting
/// them. The review-only rules are not here: a baked page has no feedback loop
/// and carries none, a served one adds its own (`review.rs`), so the two differ
/// by exactly what they do rather than by a stylesheet each pretends not to use.
/// The stylesheet a rendered block needs to look like itself.
///
/// [`crate::render_one`] and [`crate::render_typed`] return content markup rather
/// than a document — no `<style>`, since a caller placing a block in its own page
/// has one. Without these rules that markup is an unstyled skeleton: a question
/// shows list bullets *and* radio buttons, a note loses its rule, a table its
/// borders. Concatenated in cascade order, as [`bake`] writes them.
#[must_use]
pub fn styles() -> String {
    format!("{BASE_CSS}\n{BLOCKS_CSS}\n{KIT_CSS}")
}

const BASE_CSS: &str = include_str!("styles/base.css");
const BLOCKS_CSS: &str = include_str!("styles/blocks.css");
const KIT_CSS: &str = include_str!("styles/kit.css");

/// The two palettes the page carries, as the viewer defines them.
///
/// Taken from `base.css` rather than invented: its `:root` block is the dark
/// theme and its `[data-theme='light']` block is the light one, and a page that
/// drifted from them would theme its chrome differently from its diagrams.
const DARK: &str = "--background:#0b0e14;--card:#12161f;--foreground:#dfe4ec;\
--muted-foreground:#8b95a6;--border:#232a37;--primary:#7aa2f7;\
--primary-foreground:#0b0e14";
const LIGHT: &str = "--background:#ffffff;--card:#f4f6fa;--foreground:#1e2430;\
--muted-foreground:#5a6472;--border:#b7c0cd;--primary:#3b5bdb;\
--primary-foreground:#ffffff";

/// A light/dark choice the reader can make.
struct Choice {
    /// The `value` its `<option>` carries, and what the rules select on.
    value: &'static str,
    label: &'static str,
    /// The tokens it sets, or `None` for the one that defers to the system.
    tokens: Option<&'static str>,
}

const CHOICES: [Choice; 3] = [
    Choice {
        value: "system",
        label: "System",
        tokens: None,
    },
    Choice {
        value: "light",
        label: "Light",
        tokens: Some(LIGHT),
    },
    Choice {
        value: "dark",
        label: "Dark",
        tokens: Some(DARK),
    },
];

/// The selector matching a `<select>` whose chosen option carries `value`.
///
/// `option:checked` tracks the selection with no script, which is what lets the
/// whole theming layer be CSS. It is the same trick the radios used before, and
/// the reason a dropdown costs nothing here.
fn picked(select: &str, value: &str) -> String {
    format!("#{select} option[value=\"{value}\"]:checked")
}

/// Theming with no script.
///
/// Three states rather than two, because "follow the system" is a real answer and
/// a two-way switch cannot hold it: a control labelled "light" means dark on a
/// light machine and light on a dark one, and neither position says "do whatever
/// the machine does".
///
/// The default is the system's preference and an explicit choice overrides it —
/// `:root:has(…)` outranks the bare `:root` the media query sets, so no
/// `!important` and no ordering trick is needed.
fn theme_css() -> String {
    let overrides: String = CHOICES
        .iter()
        .filter_map(|c| {
            c.tokens
                .map(|tokens| format!(":root:has({}){{{tokens}}}", picked("mode", c.value)))
        })
        .collect::<Vec<_>>()
        .concat();
    format!(
        ":root{{{DARK}}}\
         @media (prefers-color-scheme: light){{:root{{{LIGHT}}}}}\
         {overrides}"
    )
}

/// The `value` a theme's `<option>` carries.
fn theme_value(name: &str) -> String {
    format!("th-{name}")
}

/// The rules an artifact's own themes contribute.
///
/// Mode and palette are orthogonal — a palette has a dark and a light half and the
/// reader picks both — so the rules are a cross product, not a chain. Each names a
/// mode explicitly, including "system": `:has(a):has(b)` is one selector CSS can
/// weigh, where "no mode chosen" would need a `:not()` pair per rule and would
/// still be arguing with the media query. Two `:has()` arguments also outrank the
/// mode-only rules above, which is what lets a partial theme win where it sets a
/// token and fall through to the base palette where it does not.
fn agent_theme_css(themes: &[Theme]) -> String {
    themes
        .iter()
        .flat_map(|theme| {
            let on = picked("palette", &escape(&theme_value(&theme.name)));
            let dark = theme_tokens(theme, ThemeMode::Dark);
            let light = theme_tokens(theme, ThemeMode::Light);
            let system = picked("mode", "system");
            [
                // Following the machine: dark unless it asks for light.
                format!(":root:has({on}):has({system}){{{dark}}}"),
                format!(
                    "@media (prefers-color-scheme: light)\
                     {{:root:has({on}):has({system}){{{light}}}}}"
                ),
                format!(
                    ":root:has({on}):has({}){{{light}}}",
                    picked("mode", "light")
                ),
                format!(":root:has({on}):has({}){{{dark}}}", picked("mode", "dark")),
            ]
        })
        .collect::<Vec<_>>()
        .concat()
}

/// One `<select>`, labelled.
fn select(id: &str, label: &str, options: &str) -> String {
    format!("<label for=\"{id}\">{label}</label><select id=\"{id}\">{options}</select>")
}

/// The chrome: the palette picker and the light/dark picker, in one control.
///
/// Two dropdowns rather than a strip of buttons, and one box rather than two.
/// They are separate questions but the same question *kind* — how this page should
/// look — so they read as one control with two fields; stacked boxes read as two
/// unrelated widgets that happen to be near each other. A `<select>` also brings
/// keyboard behaviour, type-ahead and a scrolling popup for nothing, which matters
/// once an artifact declares eight palettes.
///
/// The palette field is absent when the artifact declares no theme: a control with
/// one option is chrome for its own sake.
pub(super) fn chrome(themes: &[Theme]) -> String {
    let modes: String = CHOICES
        .iter()
        .map(|c| {
            let selected = if c.tokens.is_none() { " selected" } else { "" };
            format!(
                "<option value=\"{}\"{selected}>{}</option>",
                c.value, c.label
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let palette = if themes.is_empty() {
        String::new()
    } else {
        // The first declared theme is selected by default, matching the viewer
        // this replaces: an agent that ships a palette meant it to be seen. "Base"
        // is last, as the way back rather than the starting point.
        let options: String = themes
            .iter()
            .enumerate()
            .map(|(i, theme)| {
                let selected = if i == 0 { " selected" } else { "" };
                format!(
                    "<option value=\"{}\"{selected}>{}</option>",
                    escape(&theme_value(&theme.name)),
                    escape(&theme.name)
                )
            })
            .chain(std::iter::once(
                "<option value=\"th-base\">Base</option>".to_string(),
            ))
            .collect::<Vec<_>>()
            .concat();
        select("palette", "Palette", &options)
    };
    format!(
        "<div class=\"look\">{palette}{}</div>",
        select("mode", "Mode", &modes)
    )
}

/// How the chrome is drawn: one pill in the corner holding both fields.
///
/// `appearance:none` removes the platform control so the two fields match each
/// other and the page; the chevron is a background image rather than an element,
/// so it cannot be selected along with the option text.
const CHROME_CSS: &str = ".look{position:fixed;right:1rem;bottom:1rem;z-index:20;\
display:flex;align-items:center;gap:.4rem;padding:.35rem .5rem;border-radius:999px;\
border:1px solid var(--border);background:var(--card);font-size:.78rem;\
color:var(--muted-foreground)}\
.look label{padding-left:.25rem}\
.look select{appearance:none;-webkit-appearance:none;font:inherit;\
color:var(--foreground);background-color:var(--background);\
border:1px solid var(--border);border-radius:999px;\
padding:.3rem 1.6rem .3rem .7rem;cursor:pointer;\
background-image:url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' \
viewBox='0 0 10 6'%3E%3Cpath d='M1 1l4 4 4-4' fill='none' stroke='%23888' \
stroke-width='1.5' stroke-linecap='round'/%3E%3C/svg%3E\");\
background-repeat:no-repeat;background-position:right .55rem center;\
background-size:.6rem}\
.look select:focus-visible{outline:2px solid var(--primary);outline-offset:2px}";

/// One navigable entry: a prose heading, or a block that carried a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    pub id: String,
    pub text: String,
    pub level: u8,
}

/// A rail is worth showing only when there is somewhere to go.
///
/// One entry is chrome for its own sake — a reader can see the whole structure
/// of a one-section document without help.
fn worth_showing(entries: &[Entry]) -> bool {
    entries.len() >= 2
}

/// The value of a block's `title`, when it has one.
fn title_of(block: &Block) -> Option<&str> {
    block
        .attrs
        .iter()
        .find(|a| a.key == "title")
        .and_then(|a| match &a.value {
            crate::block::AttrValue::Value(v) => Some(v.as_str()),
            crate::block::AttrValue::Flag => None,
        })
}

/// The rail, in document order.
///
/// Headings and titled blocks are gathered in one pass so their relative order
/// is the document's, not headings-then-blocks. A titled block nests one level
/// under whatever heading precedes it, which is what makes a diagram read as
/// belonging to its section rather than floating at the top level.
fn rail(entries: &[Entry]) -> String {
    if !worth_showing(entries) {
        return String::new();
    }
    let base = entries.iter().map(|e| e.level).min().unwrap_or(1);
    let items = entries
        .iter()
        .map(|e| {
            format!(
                "<li style=\"padding-left:{}rem\"><a href=\"#{}\">{}</a></li>",
                f64::from(e.level.saturating_sub(base)) * 0.75,
                escape(&e.id),
                escape(&e.text)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    format!("<nav class=\"toc\" aria-label=\"Table of contents\"><ul>{items}</ul></nav>")
}

/// What one pass over the document produced.
pub(super) struct Rendered {
    pub content: String,
    pub entries: Vec<Entry>,
    pub themes: Vec<Theme>,
}

/// The document body, and the rail entries gathered while building it.
///
/// `under` is appended after each block — nothing for a baked page, and the
/// recorded notes plus a composer for a served one. Passed as a function rather
/// than branched on a flag, so this pass has no opinion about which page it is
/// building.
pub(super) fn render_body(
    source: &str,
    artifact: &Artifact,
    answers: &[ags_feedback::FeedbackItem],
    interactive: bool,
    under: impl Fn(&Block) -> String,
) -> Rendered {
    let (content, entries) = body(source, artifact, answers, interactive, &under);
    Rendered {
        content,
        entries,
        themes: artifact_themes(artifact),
    }
}

fn body(
    source: &str,
    artifact: &Artifact,
    answers: &[ags_feedback::FeedbackItem],
    interactive: bool,
    under: &impl Fn(&Block) -> String,
) -> (String, Vec<Entry>) {
    let mut prose = Prose::new();
    let mut entries: Vec<Entry> = Vec::new();
    let mut out = String::new();
    // How many prose headings had been seen when the last block was reached, so
    // a titled block can nest under the heading above it.
    let mut heading_level: u8 = 0;
    for segment in segments(source, artifact) {
        match segment {
            Segment::Prose(text) => {
                let before = prose.headings().len();
                let html = prose.render(text);
                for Heading { id, text, level } in prose.headings().iter().skip(before) {
                    heading_level = *level;
                    entries.push(Entry {
                        id: id.clone(),
                        text: text.clone(),
                        level: *level,
                    });
                }
                if !html.trim().is_empty() {
                    out.push_str("<div class=\"prose\">");
                    out.push_str(&html);
                    out.push_str("</div>");
                }
            }
            Segment::Block(block) => {
                if let (Some(id), Some(title)) = (block.id.as_deref(), title_of(block)) {
                    entries.push(Entry {
                        id: block_anchor_id(id),
                        text: title.to_string(),
                        level: heading_level.saturating_add(1),
                    });
                }
                out.push_str(&render_block(
                    block,
                    &mut prose,
                    answers,
                    &under(block),
                    interactive,
                ));
            }
        }
    }
    (out, entries)
}

/// The document's name, taken from its first heading.
fn document_title(entries: &[Entry]) -> &str {
    entries
        .first()
        .map_or("Artifact", |entry| entry.text.as_str())
}

/// The palettes an artifact declares, in source order.
///
/// A theme block with no `#id` is skipped: the id is what the picker labels and
/// what a recorded choice names, so an anonymous palette could be shown but never
/// referred to. A theme that resolves to nothing is skipped too — it would emit an
/// empty rule and an option that visibly does nothing.
fn artifact_themes(artifact: &Artifact) -> Vec<Theme> {
    artifact
        .blocks
        .iter()
        .filter(|block| block.type_token == "theme")
        .filter_map(|block| {
            let theme = super::theme::parse(block.id.as_deref()?, &block.body);
            (!theme.is_empty()).then_some(theme)
        })
        .collect()
}

/// The document every page is poured into.
///
/// One place that knows what an `ags` page *is* — doctype, policy, the stylesheets
/// it carries, where the chrome sits — so a served page and a baked one cannot
/// drift into two different documents. What differs is passed in.
///
/// `script` is `None` for a baked page and rides under a nonce for a served one;
/// the policy is written to match, so a page that carries no script also forbids
/// one.
pub(super) fn shell(
    title: &str,
    extra_css: &str,
    top: &str,
    body: &str,
    script: Option<&str>,
) -> String {
    let (policy, tag) = match script {
        Some(js) => (
            format!("script-src 'nonce-{NONCE}'; connect-src 'self'; form-action 'self'"),
            format!("<script nonce=\"{NONCE}\">{js}</script>"),
        ),
        None => (
            "base-uri 'none'; form-action 'none'".to_string(),
            String::new(),
        ),
    };
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n\
         <meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; \
         style-src 'unsafe-inline'; img-src data:; {policy}\">\n\
         <title>{title}</title>\n\
         <style>{BASE_CSS}{BLOCKS_CSS}{KIT_CSS}{extra_css}</style>\n\
         </head>\n<body>\n\
         {top}\n<main id=\"artifact\">{body}</main>\n{tag}</body>\n</html>\n",
        title = escape(title),
    )
}

/// Nonce authorizing the served page's own script. Fixed rather than per-response:
/// the artifact never reaches the page as executable text, and the one script the
/// page carries is written here, so there is nothing for a guessed nonce to
/// authorize that is not already authorized.
const NONCE: &str = "ags-review";

/// The style the chrome needs, given what the artifact declares.
pub(super) fn chrome_css(themes: &[Theme]) -> String {
    format!("{CHROME_CSS}{}{}", theme_css(), agent_theme_css(themes))
}

/// The rail, for a caller that already has the entries.
pub(super) fn rail_of(entries: &[Entry]) -> String {
    rail(entries)
}

/// Assemble `source` into one self-contained page.
///
/// The tab is titled from the document's first heading. Use [`bake_named`] when
/// the caller knows the file it came from.
#[must_use]
pub fn bake(source: &str) -> String {
    bake_named(source, "")
}

/// As [`bake`], titled `ags · <name>`.
///
/// A row of tabs is only useful if each one says which file it is, and a heading
/// cannot: two artifacts about the same subject share it, and a document with no
/// heading has none to share.
#[must_use]
pub fn bake_named(source: &str, name: &str) -> String {
    let artifact = parse_artifact(source);
    let Rendered {
        content,
        entries,
        themes,
        ..
    } = render_body(source, &artifact, &[], false, |_| String::new());
    // Raw, not escaped: `shell` escapes what it is given, and escaping here too
    // turned an `&` in a heading into `&amp;amp;`.
    let title = if name.is_empty() {
        document_title(&entries).to_string()
    } else {
        format!("ags · {name}")
    };
    shell(
        &title,
        &chrome_css(&themes),
        &format!("{}{}", chrome(&themes), rail(&entries)),
        &content,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# Gallery\n\nSome prose.\n\n## Diagrams\n\n```mermaid #d1 title=\"A context\"\nC4Context\nPerson(a,\"A\")\nSystem(b,\"B\")\nRel(a,b,\"uses\")\n```\n\n## Notes\n\n```note #n1 kind=warn\nMind this.\n```\n";

    #[test]
    fn the_page_is_one_document_with_nothing_outside_it() {
        let page = bake(DOC);
        assert!(page.starts_with("<!doctype html>"), "{page}");
        assert!(page.trim_end().ends_with("</html>"));
        // Nothing to fetch: no stylesheet, no script, no font, no image.
        assert!(!page.contains("<link"), "{page}");
        assert!(!page.contains("<script"), "{page}");
        assert!(!page.contains("@import"), "{page}");
        // A `url()` may not leave the document. Two forms satisfy that: a
        // fragment, which is the arrowhead marker a diagram defines and then
        // references, and a `data:` URI, which *is* the content rather than a
        // reference to it. Anything else would be a fetch.
        for (i, _) in page.match_indices("url(") {
            let after = page.get(i + 4..).unwrap_or_default();
            let inline = after.starts_with('#')
                || after.starts_with("\"data:")
                || after.starts_with("data:");
            assert!(inline, "a url() off the page at {i}");
        }
    }

    #[test]
    fn the_only_absolute_reference_is_the_svg_namespace() {
        // An identifier the browser never fetches — but worth pinning, because
        // a font or a stylesheet arriving later would look just like it, and a
        // page that reaches the network is the failure this whole change exists
        // to avoid.
        let page = bake(DOC);
        for (i, _) in page.match_indices("//") {
            let before = page.get(..i).unwrap_or_default();
            assert!(
                before.ends_with("http:") || before.ends_with("https:"),
                "unexpected `//` at {i}"
            );
            let after = page.get(i..).unwrap_or_default();
            assert!(
                after.starts_with("//www.w3.org/2000/svg"),
                "a reference off the page: {}",
                after.get(..60).unwrap_or(after)
            );
        }
    }

    #[test]
    fn a_diagram_is_drawn_into_the_page_rather_than_left_to_the_reader() {
        let page = bake(DOC);
        assert!(page.contains("<figure class=\"diagram\"><svg"), "{page}");
        assert!(page.contains("data-id=\"a\""), "{page}");
    }

    #[test]
    fn prose_and_blocks_keep_their_order() {
        let page = bake(DOC);
        let at = |needle: &str| page.find(needle);
        assert!(
            at("Some prose.") < at("<figure class=\"diagram\""),
            "{page}"
        );
        assert!(at("<figure class=\"diagram\"") < at("note warn"), "{page}");
    }

    #[test]
    fn the_rail_lists_headings_and_titled_blocks_in_document_order() {
        let page = bake(DOC);
        let rail_html = page
            .split("<nav class=\"toc\"")
            .nth(1)
            .and_then(|s| s.split("</nav>").next())
            .unwrap_or_default();
        let order: Vec<&str> = ["Gallery", "Diagrams", "A context", "Notes"]
            .into_iter()
            .filter(|t| rail_html.contains(t))
            .collect();
        assert_eq!(order, ["Gallery", "Diagrams", "A context", "Notes"]);
        // The titled block nests under the heading above it.
        assert!(rail_html.contains("#block-d1"), "{rail_html}");
    }

    #[test]
    fn a_document_with_one_place_to_go_gets_no_rail() {
        let page = bake("# Only\n\nprose\n");
        assert!(!page.contains("<nav class=\"toc\""), "{page}");
    }

    #[test]
    fn a_title_is_escaped_once_and_not_twice() {
        // `shell` escapes what it is given, so a caller that escapes as well turns
        // an `&` in a heading into `&amp;amp;` — visible in the tab, and wrong.
        let page = bake("# Cost & benefit\n\ntext\n");
        assert!(page.contains("<title>Cost &amp; benefit</title>"), "{page}");
        assert!(!page.contains("&amp;amp;"), "{page}");
    }

    #[test]
    fn a_named_page_says_which_file_it_is() {
        // A row of tabs is only useful if each says which file it is, and a
        // heading cannot: two artifacts on one subject share it.
        let page = bake_named(DOC, "plan.md");
        assert!(page.contains("<title>ags · plan.md</title>"), "{page}");
        // An empty name falls back to the heading.
        assert_eq!(bake_named(DOC, ""), bake(DOC));
    }

    #[test]
    fn the_page_takes_its_name_from_the_first_heading() {
        assert!(bake(DOC).contains("<title>Gallery</title>"));
        assert!(bake("no headings here\n").contains("<title>Artifact</title>"));
    }

    #[test]
    fn theming_needs_no_script() {
        let page = bake(DOC);
        // The system's preference by default, and every explicit choice
        // overriding it through a selector rather than a script.
        assert!(
            page.contains("@media (prefers-color-scheme: light)"),
            "{page}"
        );
        for value in ["system", "light", "dark"] {
            assert!(
                page.contains(&format!("<option value=\"{value}\"")),
                "{value} missing"
            );
        }
        assert!(
            page.contains(":root:has(#mode option[value=\"light\"]:checked)"),
            "{page}"
        );
        assert!(
            page.contains(":root:has(#mode option[value=\"dark\"]:checked)"),
            "{page}"
        );
        assert!(!page.contains("<script"), "{page}");
    }

    #[test]
    fn following_the_system_is_the_default_and_sets_no_tokens_of_its_own() {
        let page = bake(DOC);
        // Selected, so a reader who never touches the control gets their
        // machine's preference ...
        assert!(
            page.contains("<option value=\"system\" selected>System</option>"),
            "{page}"
        );
        // ... and it overrides nothing, which is what makes that work.
        assert!(
            !page.contains(":root:has(#mode option[value=\"system\"]:checked){--"),
            "{page}"
        );
    }

    #[test]
    fn the_chrome_is_one_box_holding_its_fields() {
        // Two questions of the same kind — how this page should look — so one
        // control with two fields rather than two widgets that happen to be near
        // each other.
        let page = bake(THEMED);
        assert_eq!(page.matches("<div class=\"look\">").count(), 1, "{page}");
        assert_eq!(page.matches("<select id=").count(), 2, "{page}");
    }

    #[test]
    fn every_field_is_labelled_and_reachable() {
        // A native `<select>` brings keyboard behaviour and type-ahead for free;
        // the `for`/`id` pair is what a screen reader needs to name it.
        let page = bake(DOC);
        assert!(page.contains("<label for=\"mode\">Mode</label>"), "{page}");
        assert!(page.contains("<select id=\"mode\">"), "{page}");
        assert_eq!(page.matches("<option value=").count(), 3, "{page}");
    }

    #[test]
    fn an_empty_artifact_still_produces_a_page() {
        let page = bake("");
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("<main id=\"artifact\"></main>"), "{page}");
    }

    #[test]
    fn the_same_source_always_bakes_the_same_page() {
        assert_eq!(bake(DOC), bake(DOC));
    }

    const THEMED: &str = "# T\n\n```theme #ocean\ndark:\nbackground: #001018\n\
prima\
ry: #38bdf8\nlight:\nbackground: #f0f9ff\nprimary: #0284c7\n```\n\n\
```theme #dusk\nseed: #6a5acd\n```\n\nWords.\n";

    #[test]
    fn an_artifact_theme_reaches_the_page_as_css() {
        // The gap this closes: a `theme` block used to render as nothing, so an
        // agent could author a palette and get the default look with no diagnostic.
        let page = bake(THEMED);
        assert!(page.contains("--background:#001018"), "dark tokens missing");
        assert!(
            page.contains("--background:#f0f9ff"),
            "light tokens missing"
        );
        // The seed-only theme expands to a full palette rather than one token.
        assert!(page.contains("option[value=\"th-dusk\"]:checked"), "{page}");
        assert!(
            page.contains("--muted-foreground:#"),
            "the ramp did not run"
        );
    }

    #[test]
    fn each_theme_gets_a_rule_for_every_mode_it_can_be_read_in() {
        // Mode and palette are orthogonal, so two themes need eight rules: system,
        // system-under-a-light-machine, explicit light, explicit dark.
        let page = bake(THEMED);
        for value in ["th-ocean", "th-dusk"] {
            for mode in ["system", "light", "dark"] {
                let rule = format!(
                    ":root:has(#palette option[value=\"{value}\"]:checked):\
                     has(#mode option[value=\"{mode}\"]:checked)"
                );
                assert!(page.contains(&rule), "{value} has no rule for {mode}");
            }
        }
    }

    #[test]
    fn a_theme_rule_outranks_the_mode_rule_it_layers_over() {
        // Two compound `:has()` arguments against one. This is what lets a partial
        // theme set what it names and inherit the rest instead of blanking it.
        let page = bake(THEMED);
        let themed = page.contains(
            ":root:has(#palette option[value=\"th-ocean\"]:checked):\
             has(#mode option[value=\"dark\"]:checked)",
        );
        let mode_only = page.contains(":root:has(#mode option[value=\"dark\"]:checked){");
        assert!(themed && mode_only, "{page}");
    }

    #[test]
    fn the_palette_is_a_select_the_browser_draws_itself() {
        // A dropdown rather than a strip of buttons, and still no script: the rules
        // read the selection through `option:checked`.
        let page = bake(THEMED);
        assert!(page.contains("<select id=\"palette\">"), "{page}");
        assert!(page.contains("<label for=\"palette\">"), "{page}");
        assert!(!page.contains("<script"), "{page}");
    }

    #[test]
    fn the_first_theme_is_the_one_selected() {
        let page = bake(THEMED);
        assert!(
            page.contains("<option value=\"th-ocean\" selected>ocean</option>"),
            "{page}"
        );
        assert!(
            page.contains("<option value=\"th-dusk\">dusk</option>"),
            "{page}"
        );
        // And "Base" is offered last, as the way back rather than the start.
        assert!(
            page.contains("<option value=\"th-base\">Base</option>"),
            "{page}"
        );
    }

    #[test]
    fn an_artifact_with_no_theme_gets_no_picker() {
        // One option is not a choice; the control would be chrome for its own sake.
        let page = bake(DOC);
        assert!(!page.contains("th-base"), "{page}");
        assert!(!page.contains("<select id=\"palette\""), "{page}");
        // The mode field stays; it is not the artifact's to declare.
        assert!(page.contains("<select id=\"mode\">"), "{page}");
    }

    #[test]
    fn a_theme_that_says_nothing_usable_is_not_offered() {
        // An id-less block cannot be labelled or referred to, and one whose lines
        // are all junk would be an option that visibly does nothing.
        let page = bake("# T\n\n```theme\nprimary: #ff0000\n```\n\n```theme #junk\nwords\n```\n");
        assert!(!page.contains("<select id=\"palette\""), "{page}");
    }

    #[test]
    fn a_themed_page_labels_both_of_its_fields() {
        let page = bake(THEMED);
        assert!(
            page.contains("<label for=\"palette\">Palette</label>"),
            "{page}"
        );
        assert!(page.contains("<label for=\"mode\">Mode</label>"), "{page}");
    }
}
