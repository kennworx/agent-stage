//! Prose rendering, pinned against the renderer it replaces.
//!
//! The expected HTML below is markdown-it's own output for this source, not
//! mine. Verified against every reference artifact as well — the five C4
//! documents, their README, and the reasoning demo — with output identical on
//! all seven. Those live outside the repository, so this fixture stands in for
//! them: it exercises each rule the configuration decides, and each one it
//! deliberately switches off.
//!
//! Compared after collapsing whitespace *between* tags, which a browser does not
//! render, and comparing `<pre>` content exactly, which it does.

use ags_render::Prose;

const SOURCE: &str = r#"# Prose fixture — every rule

A paragraph with *emphasis*, **strength**, `code`, ~~struck~~ and a "quoted"
path -- plus an ellipsis...

## Links

- [reachable](https://example.com)
- [mail](mailto:a@b.c)
- [fragment](#links)
- [sibling doc](README.md)
- [root path](/docs/x)
- [[design-notes]] and [[api|the API]]

Bare https://example.com stays text. Raw <b>html</b> and <script>alert(1)</script> do not.

## Links

| column | meaning |
| ------ | ------- |
| `a`    | first   |
| `b`    | second  |

```rust
let x = a < b && c > d;
```

```
┌───┐
│ a │
└───┘
```

    an indented block

- [x] done
- [ ] todo

##

> a quote
"#;

const EXPECTED: &str = r##"<h1 id="prose-fixture-every-rule" tabindex="-1">Prose fixture — every rule</h1><p>A paragraph with <em>emphasis</em>, <strong>strength</strong>, <code>code</code>, <s>struck</s> and a &quot;quoted&quot;
path -- plus an ellipsis...</p><h2 id="links" tabindex="-1">Links</h2><ul><li><a href="https://example.com">reachable</a></li><li><a href="mailto:a@b.c">mail</a></li><li><a href="#links">fragment</a></li><li><span class="link-hint" title="README.md">sibling doc</span></li><li><span class="link-hint" title="/docs/x">root path</span></li><li><span class="link-hint" title="design-notes">design-notes</span> and <span class="link-hint" title="api">the API</span></li></ul><p>Bare https://example.com stays text. Raw &lt;b&gt;html&lt;/b&gt; and &lt;script&gt;alert(1)&lt;/script&gt; do not.</p><h2 id="links-1" tabindex="-1">Links</h2><div class="table-scroll"><table><thead><tr><th>column</th><th>meaning</th></tr></thead><tbody><tr><td><code>a</code></td><td>first</td></tr><tr><td><code>b</code></td><td>second</td></tr></tbody></table></div><pre><code class="language-rust">let x = a &lt; b &amp;&amp; c &gt; d;
</code></pre><pre class="box-art"><code>┌───┐
│ a │
└───┘
</code></pre><p>an indented block</p><ul><li>[x] done</li><li>[ ] todo</li></ul><h2 id="section" tabindex="-1"></h2><blockquote><p>a quote</p></blockquote>"##;

/// Whitespace a browser ignores, ignored — except inside `<pre>`, where it is
/// the content.
fn canonical(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<pre") {
        let (before, from_pre) = rest.split_at(start);
        out.push_str(collapse(before).trim());
        let Some(end) = from_pre.find("</pre>") else {
            out.push_str(from_pre);
            return out;
        };
        let (block, tail) = from_pre.split_at(end + "</pre>".len());
        out.push_str(block);
        rest = tail;
    }
    out.push_str(collapse(rest).trim());
    out.trim().to_string()
}

/// Drop a run of whitespace that sits between two tags.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '>' {
            out.push(c);
            continue;
        }
        out.push('>');
        let mut run = String::new();
        while chars.peek().is_some_and(|n| n.is_whitespace()) {
            run.extend(chars.next());
        }
        if chars.peek() != Some(&'<') {
            out.push_str(&run);
        }
    }
    out
}

#[test]
fn matches_reference() {
    let mut prose = Prose::new();
    assert_eq!(canonical(&prose.render(SOURCE)), EXPECTED);
}

#[test]
fn the_headings_it_collected_are_the_ones_it_emitted() {
    let mut prose = Prose::new();
    let html = prose.render(SOURCE);
    for heading in prose.headings() {
        assert!(
            html.contains(&format!("id=\"{}\"", heading.id)),
            "{} is in the rail but not in the document",
            heading.id
        );
    }
    // The empty heading is addressable but not navigable, so it is not here.
    assert_eq!(prose.headings().len(), 3);
    // Two sections are both called "Links"; the second takes a distinct id.
    let ids: Vec<&str> = prose.headings().iter().map(|h| h.id.as_str()).collect();
    assert_eq!(ids, ["prose-fixture-every-rule", "links", "links-1"]);
}
