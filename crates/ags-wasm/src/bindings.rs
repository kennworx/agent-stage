//! The exported surface: artifact text in, finished HTML out.

use wasm_bindgen::prelude::wasm_bindgen;

/// Render a whole artifact — prose and every block type — as one HTML document.
///
/// This is the interesting entry point. An artifact is markdown plus a closed set
/// of fenced blocks, and a page that can draw only the diagrams shows a reader
/// everything except the parts they were meant to act on: the questions, the
/// tables, the callouts, the code they were going to comment a line of.
///
/// What comes back is a complete document — `<!doctype html>` onward, styles
/// included, no script and nothing to fetch. Put it in an iframe, write it to a
/// file, or hand it to a print dialog. It is read-only: the feedback loop needs a
/// host to record replies, and there is no host in a page.
///
/// Never fails. A block Gate 1 would reject still renders — as a visible, labelled
/// placeholder rather than a hole — because a page that goes blank tells a reader
/// less than one that says which block is wrong. Call [`validate`] first if you
/// want to know before you draw.
#[wasm_bindgen]
#[must_use]
pub fn render_page(source: &str) -> String {
    ags_render::bake(source)
}

/// As [`render_page`], with `name` in the document title.
#[wasm_bindgen]
#[must_use]
pub fn render_named_page(source: &str, name: &str) -> String {
    ags_render::bake_named(source, name)
}

/// Check an artifact against Gate 1 without drawing it.
///
/// Returns TOON — the same rows `ags present --check` prints, so a page and the
/// command line report a problem identically — or an empty string when the
/// artifact is valid. TOON rather than JSON because the reader on the other end is
/// usually a model, and it is the format the rest of the loop already speaks.
#[wasm_bindgen]
#[must_use]
pub fn validate(source: &str) -> String {
    let errors = ags_render::validate_source(source);
    if errors.is_empty() {
        return String::new();
    }
    ags_render::errors_to_toon(&errors)
}

/// The block vocabulary an artifact may use, as the authoring agent reads it.
///
/// Generated from the validator, so an editor showing this to an author is showing
/// the rules that will actually be enforced.
#[wasm_bindgen]
#[must_use]
pub fn catalog() -> String {
    ags_render::block_catalog()
}

/// The stylesheet the rendered blocks need.
///
/// [`render_block`] and the typed entry points return content markup without a
/// `<style>`, because a caller placing a block into its own page already has one.
/// They still need these rules: without them a question shows list bullets *and*
/// radio buttons, a note loses its rule, a table its borders. Put this in a
/// `<style>` once, anywhere in the document.
///
/// [`render_page`] does not need it — a whole document carries its own.
#[wasm_bindgen]
#[must_use]
pub fn block_styles() -> String {
    ags_render::styles()
}

/// Every diagram type the engine draws, one keyword per line.
///
/// The keyword is spelled as an author writes it — `sequenceDiagram`, `xychart` —
/// so a caller can list what is supported, or check its own samples against what
/// the engine actually has. Variant spellings (`xychart-beta`, `stateDiagram-v2`)
/// are accepted on input but not listed: they name the same drawing.
#[wasm_bindgen]
#[must_use]
pub fn diagram_kinds() -> String {
    ags_mermaid::DiagramType::ALL
        .iter()
        .map(|kind| kind.keyword())
        .collect::<Vec<_>>()
        .join("\n")
}

/// One `theme` block's palette, as the body of a CSS rule.
///
/// The same resolution a served page does ahead of time: an explicit `token: #hex`
/// wins, a `seed: #hex` expands to the whole palette through an OKLCH lightness
/// ramp, and `background`/`foreground` seeds fill the middle by `color-mix()`. A
/// token the block does not reach is left out, so it falls through to the base
/// cascade rather than being pinned to a default that fights the theme.
///
/// `mode` is `"light"` or `"dark"`; anything else reads as dark, which is the
/// page's own default. What comes back is `--background:#111;--card:#222` — put it
/// on `<html>`'s `style`, or in a rule of your own.
///
/// Empty when the block sets nothing, which is a theme worth ignoring rather than
/// an empty rule worth emitting.
#[wasm_bindgen]
#[must_use]
pub fn theme_styles(name: &str, body: &str, mode: &str) -> String {
    let theme = ags_render::parse_theme(name, body);
    let mode = if mode == "light" {
        ags_render::ThemeMode::Light
    } else {
        ags_render::ThemeMode::Dark
    };
    ags_render::theme_css(&theme, mode)
}

/// Render any one block from its source, fence line included.
///
/// The general entry point: hand it ```` ```question #q type=radio ... ``` ```` and
/// it draws a question, a `table` fence and it draws a table. Use it when the type
/// comes from the input rather than from the call site — an editor rendering
/// whatever the author just typed.
///
/// Empty when the source holds no addressable block. A fence outside the closed
/// set is prose, and prose is not a block.
#[wasm_bindgen]
#[must_use]
pub fn render_block(source: &str) -> String {
    ags_render::render_one(source)
}

/// Render a block of a named type from its body alone.
///
/// `attrs` is spelled as it would be in a fence — `kind=claim`, `lang=rust` — and
/// may be empty. The typed wrappers below call this; it is exported too, for a
/// caller whose type is a variable.
#[wasm_bindgen]
#[must_use]
pub fn render_block_of(type_token: &str, body: &str, attrs: &str) -> String {
    ags_render::render_typed(type_token, body, attrs)
}

/// A diagram, as it appears inside a page — a `<figure>` around the SVG.
///
/// [`render_svg`] gives the bare SVG instead, for a caller placing it itself.
#[wasm_bindgen]
#[must_use]
pub fn render_mermaid(body: &str) -> String {
    ags_render::render_typed("mermaid", body, "")
}

/// A markdown table, as GitHub Flavored Markdown renders one.
#[wasm_bindgen]
#[must_use]
pub fn render_table(body: &str) -> String {
    ags_render::render_typed("table", body, "")
}

/// A callout. `kind` is `info`, `warn` or `claim`; anything else reads as `info`.
#[wasm_bindgen]
#[must_use]
pub fn render_note(body: &str, kind: &str) -> String {
    ags_render::render_typed("note", body, &format!("kind={kind}"))
}

/// A question — the prompt and its options.
///
/// Rendered as a reading rather than a form: a form needs a session to post to,
/// and a page holding one block has none. `type` is `radio`, `checkbox`, `text`
/// or `select`.
#[wasm_bindgen]
#[must_use]
pub fn render_question(body: &str, kind: &str) -> String {
    ags_render::render_typed("question", body, &format!("type={kind}"))
}

/// A code excerpt, shown verbatim. `lang` labels it; it is not highlighted.
#[wasm_bindgen]
#[must_use]
pub fn render_code(body: &str, lang: &str) -> String {
    ags_render::render_typed("code", body, &format!("lang={lang}"))
}

/// A themed HTML chunk, passed through as the markup it already is.
///
/// Gate 1 is what decides an `html` block is safe — no script, no event handler,
/// no unsafe URL — and this does not re-check it. Validate before rendering
/// anything an author did not write.
#[wasm_bindgen]
#[must_use]
pub fn render_html(body: &str) -> String {
    ags_render::render_typed("html", body, "")
}

/// Render one diagram to SVG with the colours written in, from a theme you supply.
///
/// [`render_svg`] leaves the colours to the page as `var()` references, which is
/// right for a diagram inside a document that owns a theme. This is for everywhere
/// else — and for anywhere the cascade cannot be trusted to reach the drawing.
///
/// Every fill and stroke comes back a literal, so the drawing looks the same in an
/// `<img src>`, an email, a canvas, or a page whose browser is recolouring what it
/// renders. Pass `#rrggbb` values; anything unparseable falls back to the default
/// theme rather than failing.
///
/// Changing theme means calling this again — there is nothing for a cascade to
/// re-evaluate once the colours are literals. That is the trade: independence from
/// the page costs a re-render per theme change.
///
/// # Errors
/// When the source is not a diagram this renderer can draw.
#[wasm_bindgen]
pub fn render_svg_themed(source: &str, bg: &str, fg: &str, accent: &str) -> Result<String, String> {
    let base = ags_mermaid::Theme::default();
    let pick = |given: &str, fallback: &String| {
        if ags_mermaid::is_valid_hex(given) {
            given.to_string()
        } else {
            fallback.clone()
        }
    };
    let options = ags_mermaid::Options {
        colors: ags_mermaid::ColorMode::Fixed,
        theme: ags_mermaid::Theme {
            bg: pick(bg, &base.bg),
            fg: pick(fg, &base.fg),
            accent: pick(accent, &base.accent),
        },
        ..ags_mermaid::Options::default()
    };
    ags_mermaid::render_svg(source, &options)
        .map(|rendered| rendered.svg)
        .map_err(|e| e.to_string())
}

/// Render one diagram to SVG, without an artifact around it.
///
/// For the case with no document: a diagram being edited in place, a preview
/// beside a source box. The output references theme tokens, so the page decides
/// the colours — see `examples/web` for the three variables it must define.
///
/// # Errors
/// When the source is not a diagram this renderer can draw. An `Err` rather than a
/// panic: a panic crossing the WebAssembly boundary aborts the instance, which
/// takes down the page that embedded it.
#[wasm_bindgen]
pub fn render_svg(source: &str) -> Result<String, String> {
    ags_mermaid::render_svg(source, &ags_mermaid::Options::default())
        .map(|rendered| rendered.svg)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARTIFACT: &str = "# Decision\n\nWe weighed two hosts.\n\n\
```mermaid #flow\ngraph TD\n  A[Rust] --> B[Ship]\n```\n\n\
```question #pick type=radio required\nWhich host?\n- Rust\n- TypeScript\n```\n\n\
```table #cost\n| option | cost |\n|---|---|\n| Rust | low |\n```\n\n\
```note #claim kind=claim\nOne binary is the right shape.\n```\n";

    #[test]
    fn a_whole_artifact_becomes_a_document() {
        let html = render_page(ARTIFACT);
        assert!(
            html.starts_with("<!doctype html>"),
            "{}",
            html.chars().take(40).collect::<String>()
        );
        assert!(html.contains("</html>"));
    }

    #[test]
    fn every_block_type_reaches_the_page_not_just_the_diagram() {
        // The reason this crate binds the renderer rather than the diagram engine.
        let html = render_page(ARTIFACT);
        assert!(html.contains("<svg"), "the diagram is drawn");
        assert!(html.contains("Which host?"), "the question is asked");
        assert!(html.contains("<table"), "the table is a table");
        assert!(
            html.contains("One binary is the right shape"),
            "the note is there"
        );
        assert!(
            html.contains("We weighed two hosts"),
            "and the prose around them"
        );
    }

    #[test]
    fn the_document_carries_no_script_and_nothing_to_fetch() {
        let html = render_page(ARTIFACT);
        assert!(!html.contains("<script"), "a baked page runs nothing");
        // The only URL is the SVG namespace, which names a spec rather than a
        // thing to download — so match what a fetch actually looks like.
        for fetch in ["src=\"http", "href=\"http", "url(http", "@import"] {
            assert!(
                !html.contains(fetch),
                "{fetch} would be a request: {html:.0}"
            );
        }
    }

    #[test]
    fn a_name_reaches_the_title() {
        assert!(render_named_page(ARTIFACT, "decision.md").contains("decision.md"));
    }

    #[test]
    fn a_valid_artifact_validates_to_nothing() {
        assert_eq!(validate(ARTIFACT), "");
    }

    #[test]
    fn a_broken_artifact_reports_rows_a_command_line_would_print() {
        let toon = validate("```question #q type=radio\nOnly one option\n- just this\n```\n");
        assert!(toon.contains("errors["), "{toon}");
        assert!(toon.contains("#q"), "{toon}");
    }

    #[test]
    fn the_catalog_names_the_closed_set() {
        let cat = catalog();
        for block in [
            "mermaid", "question", "table", "code", "html", "note", "theme",
        ] {
            assert!(cat.contains(block), "catalog omits {block}");
        }
    }

    #[test]
    fn a_themed_diagram_carries_its_colours_rather_than_referring_to_them() {
        let svg = render_svg_themed("graph TD\n  A-->B\n", "#101418", "#eef1f5", "#ff8800")
            .expect("renders");
        assert!(!svg.contains("var(--"), "nothing left for a cascade: {svg}");
        assert!(svg.contains("#101418"), "the background given is used");
        assert!(!svg.contains("color-mix"), "the mixes are resolved: {svg}");
    }

    #[test]
    fn an_unusable_colour_falls_back_rather_than_failing() {
        // A page passing an empty string, or `rebeccapurple`, gets a drawing.
        let svg = render_svg_themed("graph TD\n  A-->B\n", "", "not a colour", "#ff8800")
            .expect("renders");
        assert!(svg.contains("#ffffff"), "the default background: {svg}");
        assert!(svg.contains("#ff8800"), "and the accent that was usable");
    }

    #[test]
    fn every_block_type_has_its_own_entry_point() {
        assert!(render_table("| a |\n|---|\n| 1 |").contains("<table"));
        assert!(render_note("A claim.", "claim").contains("A claim."));
        assert!(render_question("Which?\n- a\n- b", "radio").contains("Which?"));
        assert!(render_code("fn main() {}", "rust").contains("fn main()"));
        assert!(render_mermaid("graph TD\n  A-->B").contains("<svg"));
        assert!(render_html("<p>hi</p>").contains("<p>hi</p>"));
    }

    #[test]
    fn the_general_entry_point_takes_the_type_from_the_fence() {
        assert!(render_block("```table #t\n| a |\n|---|\n| 1 |\n```").contains("<table"));
        assert!(render_block("```note #n kind=info\nhi\n```").contains("hi"));
        // Prose is not a block.
        assert_eq!(render_block("```rust\nfn main() {}\n```"), "");
    }

    #[test]
    fn a_type_can_come_from_a_variable() {
        for (kind, body) in [("note", "hi"), ("code", "x = 1"), ("table", "| a |\n|---|")] {
            assert!(
                !render_block_of(kind, body, "").is_empty(),
                "{kind} drew nothing"
            );
        }
    }

    #[test]
    fn every_kind_the_engine_draws_is_listed_and_none_twice() {
        let listed = diagram_kinds();
        let kinds: Vec<&str> = listed.split('\n').collect();
        assert_eq!(kinds.len(), ags_mermaid::DiagramType::ALL.len());
        for kind in ["flowchart", "sequenceDiagram", "pie", "xychart"] {
            assert!(kinds.contains(&kind), "{kind} is missing: {kinds:?}");
        }
        // Spelled as an author writes it, not as it is looked up.
        assert!(!kinds.contains(&"xychart-beta"), "{kinds:?}");
        // And every one of them is a header the detector accepts back.
        for kind in &kinds {
            let source = format!("{kind}\n");
            assert!(
                !matches!(
                    ags_mermaid::detect(&source),
                    ags_mermaid::Detection::Unknown { .. }
                ),
                "{kind} is listed but not detected"
            );
        }
    }

    #[test]
    fn a_seed_expands_into_a_palette_and_a_mode_changes_it() {
        let dark = theme_styles("t", "seed: #6366f1", "dark");
        let light = theme_styles("t", "seed: #6366f1", "light");
        assert!(dark.contains("--background:#"), "{dark}");
        assert!(dark.contains("--primary:#"), "{dark}");
        assert_ne!(dark, light, "a mode has to change the palette");
        // Anything that is not "light" is the page's own default.
        assert_eq!(theme_styles("t", "seed: #6366f1", "whatever"), dark);
    }

    #[test]
    fn a_theme_that_sets_nothing_emits_nothing() {
        // Rather than an empty rule, or a default that would fight the cascade.
        assert_eq!(theme_styles("t", "", "dark"), "");
        assert_eq!(theme_styles("t", "not a token: hello", "dark"), "");
    }

    #[test]
    fn the_stylesheet_the_blocks_need_is_available() {
        // Rendering a block without these gives an unstyled skeleton — a question
        // showing list bullets *and* radio buttons.
        let css = block_styles();
        assert!(css.contains(".question ul"), "the block rules are present");
        assert!(css.contains("--background"));
    }

    #[test]
    fn empty_input_is_an_error_not_a_panic() {
        assert_eq!(render_svg("").unwrap_err(), "no diagram type declared");
    }

    #[test]
    fn a_typo_crosses_the_boundary_as_a_message() {
        // The error has to survive as text: a caller in JavaScript cannot match
        // on a Rust enum, and a panic here would abort the whole instance.
        let err = render_svg("pae title X").unwrap_err();
        assert!(err.contains("did you mean `pie`"), "{err}");
    }

    #[test]
    fn a_header_nobody_recognises_reports_rather_than_panicking() {
        let err = render_svg("sunburstChart").unwrap_err();
        assert!(err.contains("unknown diagram type"), "{err}");
    }

    #[test]
    fn a_block_gate_one_would_reject_still_draws_something() {
        // A page that blanks tells a reader less than one that shows which block
        // is wrong, so rendering is total even where validation is not.
        let html = render_page("```mermaid #bad\nsunburstChart\n```\n");
        assert!(html.contains("<!doctype html>"));
        assert!(
            html.contains("unknown diagram type"),
            "it says what is wrong"
        );
    }
}
