//! The C4 layout, pinned — no longer against the renderer it replaces.
//!
//! The unit tests inside the layout cover each pass in isolation; this covers them
//! interacting — box sizing feeding the grid, the grid feeding the routing
//! lattice, routing feeding separation, separation feeding badge placement, and
//! all of it feeding the canvas. It is a regression anchor: a change here means
//! every diagram moved, and that should be a decision rather than a surprise.
//!
//! **These numbers were the reference implementation's and are not any more.**
//! Two deliberate departures, in this order:
//!
//! 1. *The legend went.* The reference draws a numbered key beneath the diagram
//!    and sizes the canvas to hold it. Hovering a badge or an arrowhead already
//!    raises a bubble carrying the same sentence, so the key restated on every
//!    diagram what the drawing says on demand, and charged height for it whether
//!    or not anyone read it.
//!
//! 2. *Text is measured, not estimated.* Both renderers used to size boxes from a
//!    character-class model — "a wide lowercase letter is 1.2 average glyphs" —
//!    which under-measured real strings by as much as 11%. Widths now come from
//!    Inter's own advances. Because a C4 box has a fixed width, the correction
//!    lands on **wrapping**: `Explores and edits a codebase` genuinely needs two
//!    lines and was being laid out as one, so boxes are 138 tall where they were
//!    122 and the drawing grows downward. The reference is still one line, and
//!    still wrong.
//!
//! The five real architecture diagrams the layout was originally verified against
//! live outside this repository, so nothing here can be re-checked against them.
//!
//! The fixture is chosen for interactions rather than size: a nested boundary
//! inside another, two rows of boxes, an author-numbered step that must survive,
//! and a route that has to turn twice to get around a box.

use ags_mermaid::c4;

const SOURCE: &str = r#"C4Container
title Layout fixture
UpdateLayoutConfig($c4ShapeInRow="2", $c4BoundaryInRow="1")
Person(dev, "Developer", "Explores and edits a codebase")
System_Ext(mail, "Mail", "SMTP relay")
Container_Boundary(app, "Application") {
  Container(api, "API", "Rust", "Serves the graph over HTTP")
  ContainerDb(store, "code.db", "SQLite")
  Container_Boundary(bg, "Background") {
    ContainerQueue(bus, "Bus", "Kafka")
    Container(worker, "Worker", "Rust")
  }
}
Rel(dev, api, "1. Queries", "HTTPS")
BiRel(api, store, "Reads and writes", "SQLite")
Rel(worker, bus, "Publishes to", "Kafka")
Rel(bus, api, "Notifies")
Rel(api, mail, "Sends through", "SMTP")
Rel(dev, mail, "Reads")
"#;

const EXPECTED: &str = "\
canvas 580 818
title Layout fixture
el dev 64 68 180 138 «Person» descr=2
el mail 336 68 180 138 «External System» descr=1
el api 64 342 180 138 «Container» descr=2
el store 336 342 180 138 «Container» descr=0
el bus 64 616 180 138 «Container» descr=0
el worker 336 616 180 138 «Container» descr=0
bnd app d0 28 302 524 488
bnd bg d1 46 576 488 196
rel dev->api step=1 badge=147.5,274 pts=147.5,206 147.5,342
rel api->store step=2 badge=290,411 pts=244,411 336,411
rel worker->bus step=3 badge=290,685 pts=336,685 244,685
rel bus->api step=4 badge=154,548 pts=154,616 154,480
rel api->mail step=5 badge=167,296 pts=167,342 167,274 426,274 426,206
rel dev->mail step=6 badge=290,137 pts=244,137 336,137";

/// Three decimal places, which is far finer than a coordinate is ever drawn at.
fn n(v: f64) -> String {
    format!("{}", ags_mermaid::round_half_up(v * 1000.0) / 1000.0)
}

fn canonical(placed: &c4::Placed) -> String {
    let mut out = vec![format!("canvas {} {}", n(placed.width), n(placed.height))];
    if let Some(title) = &placed.title {
        out.push(format!("title {title}"));
    }
    for e in &placed.elements {
        out.push(format!(
            "el {} {} {} {} {} {} descr={}",
            e.alias,
            n(e.rect.x),
            n(e.rect.y),
            n(e.rect.width),
            n(e.rect.height),
            e.tag,
            e.descr.len()
        ));
    }
    for b in &placed.boundaries {
        out.push(format!(
            "bnd {} d{} {} {} {} {}",
            b.alias,
            b.depth,
            n(b.rect.x),
            n(b.rect.y),
            n(b.rect.width),
            n(b.rect.height)
        ));
    }
    for r in &placed.relationships {
        let points: Vec<String> = r
            .points
            .iter()
            .map(|p| format!("{},{}", n(p.x), n(p.y)))
            .collect();
        out.push(format!(
            "rel {}->{} step={} badge={},{} pts={}",
            r.from,
            r.to,
            r.step,
            n(r.badge_center.x),
            n(r.badge_center.y),
            points.join(" ")
        ));
    }
    out.join("\n")
}

#[test]
fn matches_reference() {
    let placed = c4::layout(&c4::parse(SOURCE));
    assert_eq!(canonical(&placed), EXPECTED);
}

#[test]
fn the_same_source_always_draws_the_same_picture() {
    // The order search is seeded and the router's tie-breaks are ordered, so a
    // second pass must be identical — a layout that drifted between renders
    // would make every diff downstream of it meaningless.
    let once = canonical(&c4::layout(&c4::parse(SOURCE)));
    let twice = canonical(&c4::layout(&c4::parse(SOURCE)));
    assert_eq!(once, twice);
}

#[test]
fn a_diagram_with_no_content_still_yields_a_canvas() {
    let placed = c4::layout(&c4::parse("C4Context\ntitle Nothing here"));
    assert!(placed.width > 0.0);
    assert!(placed.height > 0.0);
    assert!(placed.elements.is_empty());
}

#[test]
fn a_relationship_naming_a_missing_element_is_dropped_rather_than_drawn() {
    let placed = c4::layout(&c4::parse(
        "C4Context\nSystem(a,\"A\")\nSystem(b,\"B\")\nRel(a,ghost,\"x\")\nRel(a,b,\"y\")",
    ));
    assert_eq!(placed.relationships.len(), 1);
    assert_eq!(placed.relationships[0].to, "b");
}
