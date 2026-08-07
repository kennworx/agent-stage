//! The C4 parser, pinned against the renderer it replaces.
//!
//! Every field of the expected output below is the reference implementation's
//! own answer for this source, not mine. The unit tests in the parser cover each
//! rule in isolation; this covers them interacting — a nested deployment node
//! inside another, a `_Ext` storage form, a `RelIndex` shifting its positional
//! arguments, and a named argument that must not be mistaken for a description.
//!
//! Verified once against the five real architecture diagrams as well (135 lines
//! of canonical output, identical). That check needed sources living outside the
//! repository, so this stands in for it permanently.

use ags_mermaid::c4;

const SOURCE: &str = r#"C4Deployment
title Everything at once
UpdateLayoutConfig($c4ShapeInRow="3", $c4BoundaryInRow="1")
UpdateElementStyle(a, $bgColor="red")
Person(dev, "Developer", "Writes, reviews")
Person_Ext(auditor, "Auditor")
System_Ext(mail, "Mail, external", "SMTP relay")
Deployment_Node(host, "Workstation")
{
  Container(cli, "kenn", "Rust binary", "Indexes a workspace")
  ContainerDb(store, "code.db", "SQLite", "Structural graph")
  Deployment_Node(bg, "Background") {
    ContainerQueue_Ext(bus, "Bus", "Kafka")
    Component(embed, "LazyEmbedder", "kenn-embed")
  }
}
System_Boundary(sys, "Platform") {
  System(api, "API")
}
Rel(dev, cli, "Runs [CLI]", "shell")
BiRel(cli, store, "Reads and writes", "SQLite")
Rel_U(embed, bus, "Publishes")
RelIndex(3, auditor, api, "Inspects", "HTTPS")
Rel(api, mail, "Notifies via", "SMTP", $tags="async")
"#;

const EXPECTED: &str = "\
title=Everything at once
config=3,1
E dev|person||Developer||Writes, reviews|false|
E auditor|person||Auditor|||true|
E mail|system||Mail, external||SMTP relay|true|
E cli|container||kenn|Rust binary|Indexes a workspace|false|host
E store|container|db|code.db|SQLite|Structural graph|false|host
E bus|container|queue|Bus|Kafka||true|bg
E embed|component||LazyEmbedder|kenn-embed||false|bg
E api|system||API|||false|sys
B host|Workstation|deployment|
B bg|Background|deployment|host
B sys|Platform|system|
R dev|cli|Runs [CLI]|shell||false
R cli|store|Reads and writes|SQLite||true
R embed|bus|Publishes||up|false
R auditor|api|Inspects|HTTPS||false
R api|mail|Notifies via|SMTP||false";

fn canonical(d: &c4::Diagram) -> String {
    let lower = |s: String| s.to_lowercase();
    let mut out = vec![
        format!("title={}", d.title.clone().unwrap_or_default()),
        format!(
            "config={},{}",
            d.config.shape_in_row, d.config.boundary_in_row
        ),
    ];
    for e in &d.elements {
        out.push(format!(
            "E {}|{}|{}|{}|{}|{}|{}|{}",
            e.alias,
            lower(format!("{:?}", e.kind)),
            e.variant
                .map(|v| lower(format!("{v:?}")))
                .unwrap_or_default(),
            e.label,
            e.techn.clone().unwrap_or_default(),
            e.descr.clone().unwrap_or_default(),
            e.external,
            e.boundary.clone().unwrap_or_default()
        ));
    }
    for b in &d.boundaries {
        out.push(format!(
            "B {}|{}|{}|{}",
            b.alias,
            b.label,
            lower(format!("{:?}", b.kind)),
            b.parent.clone().unwrap_or_default()
        ));
    }
    for r in &d.relationships {
        out.push(format!(
            "R {}|{}|{}|{}|{}|{}",
            r.from,
            r.to,
            r.label,
            r.techn.clone().unwrap_or_default(),
            r.direction
                .map(|x| lower(format!("{x:?}")))
                .unwrap_or_default(),
            r.bidirectional
        ));
    }
    out.join("\n")
}

#[test]
fn matches_the_reference_parse() {
    let got = canonical(&c4::parse(SOURCE));
    if got != EXPECTED {
        let g: Vec<&str> = got.lines().collect();
        let w: Vec<&str> = EXPECTED.lines().collect();
        let diffs: Vec<String> = (0..g.len().max(w.len()))
            .filter_map(|i| {
                let a = g.get(i).copied().unwrap_or("<missing>");
                let b = w.get(i).copied().unwrap_or("<missing>");
                (a != b).then(|| format!("line {i}:\n  ours: {a}\n  ref : {b}"))
            })
            .collect();
        panic!("{} divergences:\n{}", diffs.len(), diffs.join("\n"));
    }
}
