//! Redraw the cross-machine raster reference.
//!
//! `tests/raster_determinism.rs` compares a fresh render against a committed PNG,
//! which is how "the same source rasterises to the same bytes on any machine" is
//! asserted rather than claimed. When the drawing legitimately changes — a label
//! moves, a stroke width is retuned — the reference has to be redrawn, and this is
//! what redraws it.
//!
//! ```sh
//! cargo run -p ags-mermaid --features raster --example raster_reference
//! ```
//!
//! Run it deliberately, never to make a red run green: a failure whose cause you
//! cannot name is the test doing its job, and overwriting the reference throws
//! that information away. The reference is drawn on macOS/aarch64 and checked on
//! Linux/x86-64 in CI, so committing one drawn on a third platform quietly changes
//! what the test proves.

#[cfg(feature = "raster")]
fn main() {
    use ags_mermaid::{png, Options};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reference");
    let source = std::fs::read_to_string(root.join("determinism.mmd"))
        .expect("the reference source is beside this example");
    let drawn = png(&source, &Options::default(), 1.0).expect("the reference source renders");
    let out = root.join("determinism.png");
    std::fs::write(&out, &drawn).expect("the reference is writable");
    println!("{} — {} bytes", out.display(), drawn.len());
}

#[cfg(not(feature = "raster"))]
fn main() {
    eprintln!("needs --features raster: there is no rasteriser in the default build");
    std::process::exit(1);
}
