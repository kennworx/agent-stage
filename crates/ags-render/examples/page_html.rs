//! Bake a markdown artifact into a page, for review.
//!
//! `cargo run -p agent-stage --example page_html -- doc.md > page.html`

fn main() {
    for path in std::env::args().skip(1) {
        match std::fs::read_to_string(&path) {
            Ok(source) => print!("{}", ags_render::bake(&source)),
            Err(err) => eprintln!("cannot read {path}: {err}"),
        }
    }
}
