//! Dump the C4 layout's geometry as JSON, for diffing against another renderer.
//!
//! Reads markdown on stdin (or from paths given as arguments), lays out every
//! fenced C4 diagram in it, and prints the result. Hand-rolled JSON so the
//! library keeps its no-dependency promise.
//!
//! With `--svg` as the first argument it prints the drawings instead, separated
//! by a form feed, so the same harness can diff either one.
//!
//! `cargo run -p ags-mermaid --example c4_geom -- doc.md`

use ags_mermaid::c4;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let as_svg = args.first().is_some_and(|a| a == "--svg");
    let mut out = if as_svg {
        String::new()
    } else {
        String::from("[\n")
    };
    let mut first = true;
    for path in args.into_iter().skip(usize::from(as_svg)) {
        let Ok(source) = std::fs::read_to_string(&path) else {
            eprintln!("cannot read {path}");
            continue;
        };
        for block in c4_blocks(&source) {
            if !first {
                out.push_str(if as_svg { "\u{c}" } else { ",\n" });
            }
            first = false;
            let placed = c4::layout(&c4::parse(&block));
            if as_svg {
                out.push_str(&ags_mermaid::svg(&c4::scene(
                    &placed,
                    &ags_mermaid::Theme::default(),
                    &ags_mermaid::ColorMode::Tokens,
                )));
            } else {
                out.push_str(&dump(&placed));
            }
        }
    }
    if !as_svg {
        out.push_str("\n]\n");
    }
    println!("{out}");
}

/// Every fenced mermaid block whose first line names a C4 diagram.
fn c4_blocks(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<&str>> = None;
    for line in md.lines() {
        let trimmed = line.trim();
        match cur.as_mut() {
            None => {
                if trimmed.starts_with("```mermaid") {
                    cur = Some(Vec::new());
                }
            }
            Some(lines) => {
                if trimmed == "```" {
                    let body = lines.join("\n");
                    if body.trim_start().starts_with("C4") {
                        out.push(body);
                    }
                    cur = None;
                } else {
                    lines.push(line);
                }
            }
        }
    }
    out
}

/// Three decimal places, which is finer than any coordinate is drawn at.
fn n(v: f64) -> String {
    let rounded = (v * 1000.0).round() / 1000.0;
    if rounded == 0.0 {
        "0".to_string()
    } else {
        format!("{rounded}")
    }
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn opt(s: Option<&String>) -> String {
    s.map_or_else(|| "null".to_string(), |v| quote(v))
}

fn list(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

fn dump(placed: &c4::Placed) -> String {
    let elements: Vec<String> = placed
        .elements
        .iter()
        .map(|e| {
            format!(
                "{{\"alias\":{},\"tag\":{},\"label\":{},\"techn\":{},\"descr\":{},\"external\":{},\"x\":{},\"y\":{},\"width\":{},\"height\":{}}}",
                quote(&e.alias),
                quote(&e.tag),
                quote(&e.label),
                opt(e.techn.as_ref()),
                list(&e.descr.iter().map(|d| quote(d)).collect::<Vec<_>>()),
                e.external,
                n(e.rect.x),
                n(e.rect.y),
                n(e.rect.width),
                n(e.rect.height)
            )
        })
        .collect();

    let boundaries: Vec<String> = placed
        .boundaries
        .iter()
        .map(|b| {
            format!(
                "{{\"alias\":{},\"label\":{},\"depth\":{},\"x\":{},\"y\":{},\"width\":{},\"height\":{}}}",
                quote(&b.alias),
                quote(&b.label),
                b.depth,
                n(b.rect.x),
                n(b.rect.y),
                n(b.rect.width),
                n(b.rect.height)
            )
        })
        .collect();

    let rels: Vec<String> = placed
        .relationships
        .iter()
        .map(|r| {
            let points: Vec<String> = r
                .points
                .iter()
                .map(|p| format!("[{},{}]", n(p.x), n(p.y)))
                .collect();
            format!(
                "{{\"from\":{},\"to\":{},\"label\":{},\"techn\":{},\"bidirectional\":{},\"step\":{},\"description\":{},\"x1\":{},\"y1\":{},\"x2\":{},\"y2\":{},\"labelX\":{},\"labelY\":{},\"points\":{}}}",
                quote(&r.from),
                quote(&r.to),
                quote(&r.label),
                opt(r.techn.as_ref()),
                r.bidirectional,
                quote(&r.step),
                quote(&r.description),
                n(r.start.x),
                n(r.start.y),
                n(r.end.x),
                n(r.end.y),
                n(r.badge_center.x),
                n(r.badge_center.y),
                list(&points)
            )
        })
        .collect();

    format!(
        "{{\"width\":{},\"height\":{},\"title\":{},\"elements\":{},\"boundaries\":{},\"relationships\":{}}}",
        n(placed.width),
        n(placed.height),
        opt(placed.title.as_ref()),
        list(&elements),
        list(&boundaries),
        list(&rels)
    )
}
