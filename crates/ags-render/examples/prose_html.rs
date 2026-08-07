//! Render a markdown file's prose to HTML, for diffing against another renderer.
//!
//! `cargo run -p agent-stage --example prose_html -- doc.md`

fn main() {
    let mut prose = ags_render::Prose::new();
    for path in std::env::args().skip(1) {
        match std::fs::read_to_string(&path) {
            Ok(source) => print!("{}", prose.render(&source)),
            Err(err) => eprintln!("cannot read {path}: {err}"),
        }
    }
}
