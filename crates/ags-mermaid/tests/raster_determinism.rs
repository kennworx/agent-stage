//! The same source must rasterise to the same bytes — here, and on any machine.
//!
//! Determinism is the property the whole raster feature rests on. An image that
//! differs between runs cannot be compared against anything, including its own
//! previous self, so every use of it — a regression check, a diff in review, an
//! artifact attached to a build — quietly stops meaning anything.
//!
//! Within one machine the library's own tests assert it. **This asserts it across
//! machines**, and it does so by being a file: the reference was produced on
//! macOS/aarch64 and CI runs Linux/x86-64, so a green run is two architectures
//! agreeing on every pixel and every byte of the encoding.
//!
//! That is the strongest form available without a second machine to hand, and it
//! is a real test rather than a claim — if `tiny-skia` reassociates a float
//! differently under NEON than under SSE, or the encoder's compression varies by
//! platform, this is what says so. A failure here is information, not noise: it
//! means "byte-identical across machines" is false and the promise has to be
//! narrowed to what is actually true.
//!
//! Regenerate deliberately, never to make a red run green:
//! `cargo run -p ags-mermaid --features raster --example raster_reference` — and
//! only once you know why the bytes moved.

#![cfg(feature = "raster")]

use ags_mermaid::{png, Options};

/// The diagram the reference was drawn from.
///
/// Chosen to exercise what a rasteriser can disagree about rather than to be
/// small: curved and straight strokes, a filled polygon arrowhead, a cylinder's
/// arcs, dashes-free but multi-segment routing, and text at two weights.
const SOURCE: &str = include_str!("reference/determinism.mmd");

/// The image, drawn on macOS/aarch64.
const REFERENCE: &[u8] = include_bytes!("reference/determinism.png");

#[test]
fn the_reference_diagram_rasterises_to_the_committed_bytes() {
    let drawn = png(SOURCE, &Options::default(), 1.0).expect("the reference source renders");
    assert_eq!(
        drawn.len(),
        REFERENCE.len(),
        "the image is {} bytes and the reference is {} — see this file's header \
         before regenerating",
        drawn.len(),
        REFERENCE.len()
    );
    let differing = drawn.iter().zip(REFERENCE).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing,
        0,
        "{differing} of {} bytes differ from the reference. If this is a Linux or \
         x86-64 run and macOS/aarch64 is green, then rasterisation is not \
         byte-identical across architectures and the claim needs narrowing.",
        REFERENCE.len()
    );
}

#[test]
fn repeated_renders_agree_with_each_other() {
    // Cheap, and it separates the two failures: if this passes while the
    // reference test fails, the drawing is stable and the machine is the variable.
    let once = png(SOURCE, &Options::default(), 1.0).unwrap();
    let twice = png(SOURCE, &Options::default(), 1.0).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn a_scaled_render_is_also_repeatable() {
    // Scaling multiplies every coordinate, so it is where a rounding difference
    // would show first.
    let once = png(SOURCE, &Options::default(), 3.0).unwrap();
    let twice = png(SOURCE, &Options::default(), 3.0).unwrap();
    assert_eq!(once, twice);
}
