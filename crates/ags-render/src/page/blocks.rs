//! One block, drawn into the page.
//!
//! The markup here is the contract the stylesheet reads: a block is a
//! `<section class="block">` carrying its id, and its content is whichever shape
//! the type calls for. Those class names are not decoration — they are what
//! `blocks.css` selects on, and changing one silently unstyles a block type.

use ags_mermaid::{render_svg, Options};

use crate::block::{AttrValue, Block};
use crate::prose::{has_box_drawing, Prose};

/// Escape text for HTML content or a quoted attribute value.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The value of `key`, or `None` for an absent attribute or a bare flag.
fn attr<'a>(block: &'a Block, key: &str) -> Option<&'a str> {
    block
        .attrs
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| match &a.value {
            AttrValue::Value(v) => Some(v.as_str()),
            AttrValue::Flag => None,
        })
}

/// The DOM id given to a titled block so the rail can navigate to it.
///
/// Prefixed rather than using the raw block id, so a block called `intro` and a
/// heading slugged `intro` do not fight over one fragment.
pub fn block_anchor_id(block_id: &str) -> String {
    format!("block-{block_id}")
}

/// A diagram, drawn ahead of serving.
///
/// A type this build cannot draw becomes a visible, labelled placeholder rather
/// than a blank space: a diagram silently missing from a review page is worse
/// than one that says why it is missing.
fn diagram(body: &str) -> String {
    match render_svg(body, &Options::default()) {
        Ok(rendered) => format!("<figure class=\"diagram\">{}</figure>", rendered.svg),
        Err(err) => format!(
            "<figure class=\"diagram error\"><p>{}</p><pre class=\"code\">{}</pre></figure>",
            escape(&err.to_string()),
            escape(body)
        ),
    }
}

/// A code excerpt, tiled tighter when it holds box art.
fn code(body: &str) -> String {
    let class = if has_box_drawing(body) {
        "code box-art"
    } else {
        "code"
    };
    format!("<pre class=\"{class}\">{}</pre>", escape(body))
}

/// A callout.
fn note(block: &Block, prose: &mut Prose) -> String {
    let kind = attr(block, "kind").unwrap_or("info");
    format!(
        "<div class=\"note {}\"><span class=\"note-kind\">{}</span>{}</div>",
        escape(kind),
        escape(kind),
        prose.render(&block.body)
    )
}

/// A question, as the static form a reader sees before any script runs.
///
/// The prompt is the first line; every `- option` line after it is a choice.
/// Rendered as a real `<fieldset>` so the choices are readable and reachable by
/// keyboard on a page with no script at all — answering needs one, but seeing
/// what was asked does not.
fn question(block: &Block, answers: &[ags_feedback::FeedbackItem], interactive: bool) -> String {
    let mut lines = block.body.lines();
    let prompt = lines.next().unwrap_or_default();
    let kind = attr(block, "type").unwrap_or("text");
    let options: Vec<&str> = lines
        .filter_map(|l| l.trim().strip_prefix("- "))
        .map(str::trim)
        .collect();
    let name = block.id.clone().unwrap_or_else(|| block.anchor());
    // What the reviewer already answered, so a reload shows their choice rather
    // than an empty control that silently forgot it.
    let chosen = block.id.as_deref().and_then(|id| {
        answers
            .iter()
            .rfind(|a| a.block_id == id && a.kind == ags_feedback::FeedbackKind::Answer)
            .map(|a| a.body.clone())
    });
    let control = match kind {
        "text" => "<textarea class=\"q-text\" rows=\"3\"></textarea>".to_string(),
        "select" => format!(
            "<div class=\"q-select-wrap\"><select class=\"q-select\">{}</select></div>",
            options
                .iter()
                .map(|o| format!("<option>{}</option>", escape(o)))
                .collect::<Vec<_>>()
                .concat()
        ),
        // One option per row, in a list. The bare `<label>` run this replaced had
        // no structure for the stylesheet to reach: `.question li` matched nothing,
        // so the options wrapped into one line and each radio sat against the end
        // of the previous option's text rather than its own.
        _ => format!(
            "<ul>{}</ul>",
            options
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let _ = i;
                    let picked = if chosen.as_deref() == Some(*o) {
                        " checked"
                    } else {
                        ""
                    };
                    format!(
                        "<li><label><input type=\"{}\" name=\"body\" value=\"{}\"{picked}/> {}</label></li>",
                        if kind == "checkbox" {
                            "checkbox"
                        } else {
                            "radio"
                        },
                        escape(o),
                        escape(o)
                    )
                })
                .collect::<Vec<_>>()
                .concat()
        ),
    };
    // Wrapped in a form so an answer posts with no script. Without a server to
    // post to — a baked page, or a review already finished — the same controls are
    // rendered bare: readable, showing what was chosen, and not pretending to
    // accept a change nothing would receive.
    let prompt = escape(prompt);
    if !interactive {
        return format!("<div class=\"question\"><p class=\"prompt\">{prompt}</p>{control}</div>");
    }
    format!(
        "<div class=\"question\"><p class=\"prompt\">{prompt}</p>\
         <form class=\"answer\" method=\"post\" action=\"/feedback\">\
         <input type=\"hidden\" name=\"block_id\" value=\"{}\">\
         <input type=\"hidden\" name=\"kind\" value=\"answer\">\
         {control}<button type=\"submit\" class=\"quiet\">Answer</button></form></div>",
        escape(&name)
    )
}

/// The content of one block, by type.
fn content(
    block: &Block,
    prose: &mut Prose,
    answers: &[ags_feedback::FeedbackItem],
    interactive: bool,
) -> String {
    match block.type_token.as_str() {
        "mermaid" => diagram(&block.body),
        // A `table` block is a markdown table that a reviewer can annotate; it
        // renders exactly as the prose form does.
        "table" => prose.render(&block.body),
        "note" => note(block, prose),
        "question" => question(block, answers, interactive),
        // An `html` block is already themed markup, admitted by Gate 1.
        "html" => format!("<div class=\"htmlchunk\">{}</div>", block.body),
        // A `theme` block is configuration, not visible content.
        "theme" => String::new(),
        // `code`, and anything else the catalog grows: a body shown verbatim.
        _ => code(&block.body),
    }
}

/// Render one fenced block on its own, outside any artifact.
///
/// `source` is the block as an author writes it, fence line included. What comes
/// back is the block's content as HTML — no page around it, no section wrapper, no
/// anchor — so a caller can put it wherever it likes.
///
/// Empty when `source` holds no addressable block: a fence type outside the closed
/// set is prose, and prose is not a block.
///
/// Non-interactive by construction. A question renders as a reading of the
/// question rather than a form, because a form needs a host to post to and a
/// caller holding one block has no review session.
#[must_use]
pub fn render_one(source: &str) -> String {
    let artifact = crate::parse::parse_artifact(source);
    let Some(block) = artifact.blocks.first() else {
        return String::new();
    };
    let mut prose = Prose::default();
    content(block, &mut prose, &[], false)
}

/// Render a block of `type_token` from its body alone.
///
/// The per-type entry point: a caller that already knows what it is holding does
/// not have to write a fence around it to get it drawn. Attributes a type reads —
/// a note's `kind`, a code block's `lang` — come from `attrs`, spelled as they
/// would be in the fence (`kind=claim`).
///
/// An unknown type is drawn verbatim rather than refused, matching what the page
/// does with one.
#[must_use]
pub fn render_typed(type_token: &str, body: &str, attrs: &str) -> String {
    let fence = format!("```{type_token} {attrs}\n{body}\n```\n");
    render_one(&fence)
}

/// One block as a section of the page.
pub fn render(
    block: &Block,
    prose: &mut Prose,
    answers: &[ags_feedback::FeedbackItem],
    under: &str,
    interactive: bool,
) -> String {
    let body = format!("{}{under}", content(block, prose, answers, interactive));
    if body.is_empty() {
        return String::new();
    }
    let title = attr(block, "title");
    // `title` surfaces the block in the rail; `id` gives that entry something to
    // navigate to. Both are needed, so an untitled block stays out of the rail
    // entirely.
    let identity = match (&block.id, title) {
        (Some(id), Some(title)) => format!(
            " data-block-id=\"{}\" data-title=\"{}\" id=\"{}\"",
            escape(id),
            escape(title),
            escape(&block_anchor_id(id))
        ),
        (Some(id), None) => format!(" data-block-id=\"{}\"", escape(id)),
        (None, _) => String::new(),
    };
    format!("<section class=\"block\"{identity}>{body}</section>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_artifact;

    fn only_block(source: &str) -> Block {
        parse_artifact(source)
            .blocks
            .first()
            .cloned()
            .unwrap_or_else(|| Block {
                type_token: String::new(),
                id: None,
                attrs: Vec::new(),
                body: String::new(),
                line: 0,
                end: 0,
                ordinal: 0,
            })
    }

    fn html(source: &str) -> String {
        render(&only_block(source), &mut Prose::new(), &[], "", true)
    }

    #[test]
    fn every_character_that_could_end_an_attribute_is_escaped() {
        // A block title reaches an attribute and a note body reaches content;
        // one escape serves both, so it has to cover both cases.
        assert_eq!(
            escape("a & b < c > d \" e ' f"),
            "a &amp; b &lt; c &gt; d &quot; e &#39; f"
        );
        assert_eq!(escape("plain"), "plain");
    }

    #[test]
    fn a_block_carries_the_identity_feedback_is_keyed_to() {
        let out = html("```note #n1\nbody\n```\n");
        assert!(
            out.contains("<section class=\"block\" data-block-id=\"n1\">"),
            "{out}"
        );
    }

    #[test]
    fn a_titled_block_joins_the_rail_and_an_untitled_one_does_not() {
        let titled = html("```note #n1 title=\"A note\"\nbody\n```\n");
        assert!(titled.contains("data-title=\"A note\""), "{titled}");
        assert!(titled.contains("id=\"block-n1\""), "{titled}");
        let plain = html("```note #n1\nbody\n```\n");
        assert!(!plain.contains("data-title"), "{plain}");
        assert!(!plain.contains("id=\"block-"), "{plain}");
    }

    #[test]
    fn a_drawable_diagram_becomes_inline_svg() {
        let out = html(
            "```mermaid #d\nC4Context\nPerson(a,\"A\")\nSystem(b,\"B\")\nRel(a,b,\"x\")\n```\n",
        );
        assert!(out.contains("<figure class=\"diagram\"><svg"), "{out}");
        assert!(out.contains("data-id=\"a\""), "{out}");
    }

    #[test]
    fn a_diagram_that_cannot_be_read_says_so_rather_than_vanishing() {
        // Every type the detector names is drawn now, so the remaining way to
        // fail is a header nobody recognises.
        let out = html("```mermaid #d\nsunburstChart\n  a: 1\n```\n");
        assert!(out.contains("class=\"diagram error\""), "{out}");
        assert!(out.contains("unknown diagram type"), "{out}");
        // The source is still readable, so a reviewer can see what was meant.
        assert!(out.contains("sunburstChart"), "{out}");
    }

    #[test]
    fn a_diagram_with_a_typo_is_reported_as_a_typo() {
        let out = html("```mermaid #d\npae title Shares\n```\n");
        assert!(out.contains("did you mean"), "{out}");
    }

    #[test]
    fn code_is_escaped_and_box_art_is_tagged() {
        let plain = html("```code #c lang=rust\nlet a = b < c;\n```\n");
        assert!(
            plain.contains("<pre class=\"code\">let a = b &lt; c;</pre>"),
            "{plain}"
        );
        let art = html("```code #c lang=text\n┌─┐\n└─┘\n```\n");
        assert!(art.contains("<pre class=\"code box-art\">"), "{art}");
    }

    #[test]
    fn a_table_block_renders_as_the_prose_form_does() {
        let out = html("```table #t\n| a | b |\n| - | - |\n| 1 | 2 |\n```\n");
        assert!(out.contains("<div class=\"table-scroll\">"), "{out}");
        assert!(out.contains("<td>1</td>"), "{out}");
    }

    #[test]
    fn a_note_shows_its_kind_and_renders_its_body_as_markdown() {
        let out = html("```note #n kind=warn\nMind **this**.\n```\n");
        assert!(out.contains("<div class=\"note warn\">"), "{out}");
        assert!(
            out.contains("<span class=\"note-kind\">warn</span>"),
            "{out}"
        );
        assert!(out.contains("<strong>this</strong>"), "{out}");
        // An unstated kind is the default one, not an empty class.
        assert!(html("```note #n\nx\n```\n").contains("note info"));
    }

    #[test]
    fn a_question_is_readable_before_any_script_runs() {
        let out = html("```question #q type=radio\nWhich one?\n- first\n- second\n```\n");
        assert!(out.contains("<p class=\"prompt\">Which one?</p>"), "{out}");
        // `name="body"` is what the form posts under; the block id rides in a
        // hidden field beside it.
        assert!(out.contains("type=\"radio\" name=\"body\""), "{out}");
        assert!(out.contains("name=\"block_id\" value=\"q\""), "{out}");
        assert!(out.contains("first"), "{out}");
    }

    #[test]
    fn each_question_kind_gets_its_own_control() {
        assert!(html("```question #q type=text\nWhy?\n```\n").contains("<textarea"));
        assert!(
            html("```question #q type=checkbox\nWhich?\n- a\n- b\n```\n")
                .contains("type=\"checkbox\"")
        );
        let select = html("```question #q type=select\nWhich?\n- a\n- b\n```\n");
        assert!(select.contains("<select class=\"q-select\">"), "{select}");
        assert!(select.contains("<option>a</option>"), "{select}");
    }

    #[test]
    fn a_theme_block_is_configuration_and_draws_nothing() {
        assert_eq!(html("```theme #t\nseed: #3b82f6\n```\n"), "");
    }

    #[test]
    fn an_html_block_is_admitted_as_authored() {
        let out = html("```html #h\n<p class=\"lead\">Hello</p>\n```\n");
        assert!(
            out.contains("<div class=\"htmlchunk\"><p class=\"lead\">Hello</p>"),
            "{out}"
        );
    }

    #[test]
    fn a_title_that_could_break_an_attribute_is_escaped() {
        let out = html("```note #n title=\"a \\\" b\"\nx\n```\n");
        assert!(!out.contains("title=\"a \" b\""), "{out}");
    }
}
