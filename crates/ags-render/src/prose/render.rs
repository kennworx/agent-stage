//! Implicit prose to HTML.
//!
//! Everything between fenced blocks is GitHub Flavored Markdown, rendered by
//! `comrak` under a configuration chosen so that the emitted tag set is fixed by
//! the renderer and cannot be widened by an author.
//!
//! Two options are the security posture, and they replace an
//! escape-first-then-inject invariant that used to be held by hand:
//!
//! - **Raw HTML is escaped, not passed through.** `render.escape` on and
//!   `render.unsafe` off means `<script>` in the source arrives as text. This is
//!   the whole reason no DOM sanitiser is needed downstream: the output tag set
//!   is exactly what the formatter below emits, and the only mechanism for
//!   widening it is off.
//! - **A bare URL stays text.** Silently promoting one to a link is a surprise in
//!   a document the agent authored exactly as written.
//!
//! Smart punctuation is off for the same reason: an artifact quotes identifiers
//! and paths, and rewriting its punctuation is never wanted.

use std::fmt::Write as _;

use comrak::html::ChildRendering;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{create_formatter, parse_document, Arena, Options};

use super::boxart::has_box_drawing;
use super::slug::Slugger;

/// One navigable entry: a prose heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Fragment target, without the leading `#`.
    pub id: String,
    /// Label shown in the rail.
    pub text: String,
    /// Nesting depth, 1-based.
    pub level: u8,
}

/// What the formatter carries across one document.
#[derive(Debug, Default)]
struct State {
    slugs: Slugger,
    headings: Vec<Heading>,
}

/// Whether a link target can actually be reached from a single-artifact page.
///
/// Only two things can: an absolute `http(s)` or `mailto` destination, and a
/// pure `#fragment` pointing at a heading in this same document. Everything else
/// — a sibling `other-doc.md`, a root-relative path, a bare name — has nowhere
/// to go, because one artifact is one page. Those become hints instead of dead
/// anchors.
///
/// Stated as a closed allow-list rather than a `.md` deny-list, so no unforeseen
/// scheme can slip through and become an href.
fn is_reachable(href: &str) -> bool {
    let lower = href.to_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with('#')
}

/// Escape text for HTML content or a quoted attribute value.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// The plain text under a node, which is what a heading's id and rail label are
/// derived from.
fn text_of<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    let mut out = String::new();
    for child in node.descendants() {
        match &child.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::LineBreak | NodeValue::SoftBreak => out.push(' '),
            _ => {}
        }
    }
    out
}

create_formatter!(ProseFormatter<State>, {
    // Give every heading a stable id, so a `#fragment` link resolves and the
    // table of contents has something to navigate to.
    NodeValue::Heading(ref h) => |context, node, entering| {
        if entering {
            let text = text_of(node);
            let id = context.user.slugs.slug(&text);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                context.user.headings.push(Heading {
                    id: id.clone(),
                    text: trimmed.to_string(),
                    level: h.level,
                });
            }
            // `tabindex` makes the heading focusable, so following a
            // `#fragment` moves keyboard focus to the section rather than
            // leaving it at the top of the document.
            write!(
                context,
                "<h{} id=\"{}\" tabindex=\"-1\">",
                h.level,
                escape(&id)
            )?;
        } else {
            writeln!(context, "</h{}>", h.level)?;
        }
    },
    // Wrap every table in a horizontal scroll container, so a table wider than
    // the artifact column scrolls internally instead of escaping it. A scroll
    // container has to be a separate element: making the table itself
    // `overflow-x: auto` would require `display: block`, which drops the table
    // layout that sizes the columns in the first place.
    NodeValue::Table(..) => |context, node, entering| {
        // Wrapped around the default rendering rather than replacing it: a
        // table's `<tbody>` is opened and closed by the row renderer, so a
        // hand-written close would be unbalanced the moment a table had a
        // header and no body.
        if entering {
            context.write_str("<div class=\"table-scroll\">")?;
        }
        let child_rendering = comrak::html::format_node_default(context, node, entering)?;
        if !entering {
            context.write_str("</div>\n")?;
        }
        return Ok(child_rendering);
    },
    // A fenced code block, tagged when it holds box art so a prose fence tiles
    // the same way an addressable `code` block does.
    NodeValue::CodeBlock(ref cb) => |context, entering| {
        if entering {
            let lang = cb.info.split_whitespace().next().unwrap_or_default();
            let language = if lang.is_empty() {
                String::new()
            } else {
                format!(" class=\"language-{}\"", escape(lang))
            };
            let art = if has_box_drawing(&cb.literal) {
                " class=\"box-art\""
            } else {
                ""
            };
            writeln!(
                context,
                "<pre{art}><code{language}>{}</code></pre>",
                escape(&cb.literal)
            )?;
        }
        return Ok(ChildRendering::Skip);
    },
    // An unreachable destination becomes a hint rather than a dead anchor. The
    // closing tag has to change too, which is why this replaces the whole pair
    // rather than rewriting an attribute.
    NodeValue::Link(ref link) => |context, entering| {
        if is_reachable(&link.url) {
            if entering {
                write!(context, "<a href=\"{}\"", escape(&link.url))?;
                if !link.title.is_empty() {
                    write!(context, " title=\"{}\"", escape(&link.title))?;
                }
                context.write_str(">")?;
            } else {
                context.write_str("</a>")?;
            }
        } else if entering {
            write!(
                context,
                "<span class=\"link-hint\" title=\"{}\">",
                escape(&link.url)
            )?;
        } else {
            context.write_str("</span>")?;
        }
    },
    // `<s>` rather than GitHub's `<del>`, which is what the renderer this
    // replaces emitted and what the stylesheet selects on.
    NodeValue::Strikethrough => |context, entering| {
        context.write_str(if entering { "<s>" } else { "</s>" })?;
    },
    // `[[target]]` and `[[target|label]]`. Resolves to nothing for the same
    // reason a cross-document link does, and renders the same way.
    NodeValue::WikiLink(ref link) => |context, entering| {
        if entering {
            write!(
                context,
                "<span class=\"link-hint\" title=\"{}\">",
                escape(&link.url)
            )?;
        } else {
            context.write_str("</span>")?;
        }
    },
});

/// The parser and renderer configuration, built once per document.
fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    // Task lists stay off. The renderer this replaces has none, so switching
    // them on would silently turn a literal `- [x]` in an existing artifact
    // into a checkbox.
    options.extension.tasklist = false;
    options.extension.footnotes = true;
    // `[[target|label]]`: the label follows the pipe.
    options.extension.wikilinks_title_after_pipe = true;
    // A bare URL stays text.
    options.extension.autolink = false;
    // No smart quotes or dash substitution.
    options.parse.smart = false;
    // Raw HTML arrives as text rather than as markup.
    options.render.escape = true;
    options.render.r#unsafe = false;
    options
}

/// How many times an indented block may be re-read before giving up.
///
/// Each pass strips one level, so a pathological document cannot spin here.
const UNINDENT_PASSES: usize = 4;

/// Re-read indented blocks as prose rather than as code.
///
/// A fenced block is the only way to get a code block, because `code` is already
/// a first-class addressable block type and the indented form buys nothing —
/// while accepting it would silently turn a deeply-indented list continuation in
/// an existing artifact into code. The renderer this replaces simply switched
/// the rule off; `comrak` has no such switch, so the block is parsed and then
/// its content is parsed again as markdown, which is what the disabled rule
/// would have produced.
fn reread_indented_blocks<'a>(arena: &'a Arena<'a>, root: &'a AstNode<'a>, options: &Options) {
    for _ in 0..UNINDENT_PASSES {
        let indented: Vec<&'a AstNode<'a>> = root
            .descendants()
            .filter(
                |node| matches!(&node.data.borrow().value, NodeValue::CodeBlock(cb) if !cb.fenced),
            )
            .collect();
        if indented.is_empty() {
            return;
        }
        for node in indented {
            let literal = match &node.data.borrow().value {
                NodeValue::CodeBlock(cb) => cb.literal.clone(),
                _ => continue,
            };
            let reparsed = parse_document(arena, &literal, options);
            let children: Vec<&'a AstNode<'a>> = reparsed.children().collect();
            for child in children {
                child.detach();
                node.insert_before(child);
            }
            node.detach();
        }
    }
}

/// A prose renderer for one artifact.
///
/// Holds the id namespace, because an artifact is a single document: uniqueness
/// has to accumulate across prose runs, not reset between them.
#[derive(Debug, Default)]
pub struct Prose {
    state: State,
}

impl Prose {
    pub fn new() -> Self {
        Self::default()
    }

    /// Render one prose run to HTML.
    ///
    /// The caller injects the result directly, which is safe by the
    /// configuration above rather than by a downstream sanitising pass.
    pub fn render(&mut self, source: &str) -> String {
        let arena = Arena::new();
        let options = options();
        let doc = parse_document(&arena, source, &options);
        reread_indented_blocks(&arena, doc, &options);
        let mut out = String::new();
        let state = std::mem::take(&mut self.state);
        match ProseFormatter::format_document(doc, &options, &mut out, state) {
            Ok(state) => {
                self.state = state;
                out
            }
            // Formatting writes into a `String`, which cannot fail; a caller
            // still gets a document rather than a panic if that ever changes.
            Err(_) => String::new(),
        }
    }

    /// Every heading rendered so far, in document order.
    pub fn headings(&self) -> &[Heading] {
        &self.state.headings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(source: &str) -> String {
        Prose::new().render(source)
    }

    #[test]
    fn raw_html_arrives_as_text_rather_than_as_markup() {
        // The security posture, and the reason nothing downstream sanitises.
        let out = html("<script>alert(1)</script>\n\nplain <b>bold</b>");
        assert!(!out.contains("<script"), "{out}");
        assert!(out.contains("&lt;script&gt;"), "{out}");
        assert!(out.contains("&lt;b&gt;bold&lt;/b&gt;"), "{out}");
    }

    #[test]
    fn a_bare_url_stays_text() {
        let out = html("see https://example.com for more");
        assert!(!out.contains("<a "), "{out}");
    }

    #[test]
    fn punctuation_is_left_exactly_as_written() {
        // An artifact quotes identifiers and paths; rewriting its punctuation
        // into typographic forms is never wanted.
        let out = html("the \"quoted\" path -- and an ellipsis...");
        assert!(!out.contains('\u{201c}'), "{out}");
        assert!(!out.contains('\u{2014}'), "{out}");
        assert!(!out.contains('\u{2026}'), "{out}");
        assert!(out.contains("--"), "{out}");
        assert!(out.contains("..."), "{out}");
    }

    #[test]
    fn every_heading_gets_a_fragment_and_joins_the_rail() {
        let mut prose = Prose::new();
        let out = prose.render("# Title\n\n## Notes\n\n## Notes\n");
        assert!(
            out.contains("<h1 id=\"title\" tabindex=\"-1\">Title</h1>"),
            "{out}"
        );
        assert!(out.contains("<h2 id=\"notes\" tabindex=\"-1\">"), "{out}");
        assert!(
            out.contains("<h2 id=\"notes-1\">") || out.contains("id=\"notes-1\" tabindex"),
            "{out}"
        );
        assert_eq!(
            prose.headings(),
            &[
                Heading {
                    id: "title".into(),
                    text: "Title".into(),
                    level: 1
                },
                Heading {
                    id: "notes".into(),
                    text: "Notes".into(),
                    level: 2
                },
                Heading {
                    id: "notes-1".into(),
                    text: "Notes".into(),
                    level: 2
                },
            ]
        );
    }

    #[test]
    fn ids_keep_accumulating_across_prose_runs() {
        // Two runs either side of a diagram must not both claim `#provenance`.
        let mut prose = Prose::new();
        let first = prose.render("## Provenance");
        let second = prose.render("## Provenance");
        assert!(first.contains("id=\"provenance\""), "{first}");
        assert!(second.contains("id=\"provenance-1\""), "{second}");
    }

    #[test]
    fn an_empty_heading_is_addressable_but_not_navigable() {
        let mut prose = Prose::new();
        let out = prose.render("##\n\n## Real");
        assert!(out.contains("<h2 id=\"section\" tabindex=\"-1\">"), "{out}");
        // A blank row in the rail is noise; the anchor still exists.
        assert_eq!(prose.headings().len(), 1);
    }

    #[test]
    fn a_heading_id_reads_the_text_under_its_formatting() {
        let mut prose = Prose::new();
        prose.render("## The `kenn` **index**");
        assert_eq!(prose.headings()[0].id, "the-kenn-index");
        assert_eq!(prose.headings()[0].text, "The kenn index");
    }

    #[test]
    fn a_table_scrolls_inside_the_column_rather_than_escaping_it() {
        let out = html("| a | b |\n| - | - |\n| 1 | 2 |\n");
        assert!(out.contains("<div class=\"table-scroll\">"), "{out}");
        assert!(out.contains("<table>"), "{out}");
        // The body is closed by the default renderer, so the wrapper cannot
        // leave it dangling.
        assert!(out.contains("</tbody>"), "{out}");
        assert!(out.contains("</table>\n</div>"), "{out}");
        assert!(out.contains("<td>1</td>"), "{out}");
    }

    #[test]
    fn a_fence_carries_its_language_and_escapes_its_content() {
        let out = html("```rust\nlet x = a < b && c > d;\n```\n");
        assert!(out.contains("<code class=\"language-rust\">"), "{out}");
        assert!(out.contains("a &lt; b &amp;&amp; c &gt; d"), "{out}");
        assert!(!out.contains("class=\"box-art\""), "{out}");
    }

    #[test]
    fn a_fence_of_box_art_is_tagged_so_its_lines_tile() {
        let out = html("```\n┌───┐\n│ a │\n└───┘\n```\n");
        assert!(out.contains("<pre class=\"box-art\">"), "{out}");
        // No language was declared, so no language class is invented.
        assert!(out.contains("<code>"), "{out}");
    }

    #[test]
    fn a_reachable_link_stays_a_link() {
        assert!(html("[a](https://example.com)").contains("<a href=\"https://example.com\">"));
        assert!(html("[a](mailto:x@y.z)").contains("<a href=\"mailto:x@y.z\">"));
        assert!(html("[a](#notes)").contains("<a href=\"#notes\">"));
    }

    #[test]
    fn a_link_with_nowhere_to_go_becomes_a_hint() {
        // One artifact is one page, so a sibling document is not reachable.
        let out = html("see [the readme](README.md) and [a path](/docs/x)");
        assert!(!out.contains("<a "), "{out}");
        assert!(
            out.contains("<span class=\"link-hint\" title=\"README.md\">"),
            "{out}"
        );
        assert!(
            out.contains("<span class=\"link-hint\" title=\"/docs/x\">"),
            "{out}"
        );
        assert!(out.contains("the readme</span>"), "{out}");
    }

    #[test]
    fn an_unforeseen_scheme_is_a_hint_rather_than_an_href() {
        // The allow-list is closed on purpose.
        let out = html("[x](javascript:alert(1))");
        assert!(!out.contains("<a "), "{out}");
        assert!(out.contains("link-hint"), "{out}");
    }

    #[test]
    fn a_wiki_link_renders_like_any_other_unreachable_target() {
        let out = html("see [[design-notes]] and [[api|the API]]");
        assert!(
            out.contains("<span class=\"link-hint\" title=\"design-notes\">"),
            "{out}"
        );
        assert!(out.contains("the API</span>"), "{out}");
        assert!(!out.contains("<a "), "{out}");
    }

    #[test]
    fn gfm_survives_the_move() {
        assert!(html("~~gone~~").contains("<s>gone</s>"));
        // Tables and strikethrough come with GFM; task lists do not, because
        // the renderer this replaces had none.
        let out = html("- [x] done\n- [ ] todo");
        assert!(!out.contains("checkbox"), "{out}");
        assert!(out.contains("[x] done"), "{out}");
    }

    #[test]
    fn an_indented_block_is_prose_rather_than_code() {
        // A fenced block is the only way to get a code block; the indented form
        // would silently turn a deep list continuation into code.
        let out = html("a paragraph\n\n    indented *and* emphasised\n");
        assert!(!out.contains("<pre>"), "{out}");
        assert!(
            out.contains("<p>indented <em>and</em> emphasised</p>"),
            "{out}"
        );
    }

    #[test]
    fn a_fenced_block_is_still_code_however_deeply_indented() {
        let out = html("- item\n\n  ```\n  kept\n  ```\n");
        assert!(out.contains("<pre><code>kept"), "{out}");
    }

    #[test]
    fn ordinary_prose_renders_as_prose() {
        let out = html("A paragraph with *emphasis*, **strength** and `code`.\n\n- one\n- two\n");
        assert!(out.contains("<em>emphasis</em>"), "{out}");
        assert!(out.contains("<strong>strength</strong>"), "{out}");
        assert!(out.contains("<code>code</code>"), "{out}");
        assert!(out.contains("<ul>"), "{out}");
    }

    #[test]
    fn nothing_in_yields_nothing_out() {
        assert_eq!(html(""), "");
        assert!(Prose::new().headings().is_empty());
    }
}
