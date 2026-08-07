//! A drawing as a PNG, for the places an SVG cannot go.
//!
//! Behind the `raster` feature, and off by default. A page build has a browser to
//! draw with; carrying an SVG renderer, a font database and a rasteriser into
//! WebAssembly to duplicate it would be paying for the one thing the target
//! already does best.
//!
//! **The image is rendered from our own SVG**, not from a second drawing routine.
//! Two renderers would be two answers to every geometry question, and the one that
//! nobody looks at would be the one that rots. So this is the same markup a page
//! gets, drawn.
//!
//! Two things are deliberately different from the served page:
//!
//! *Colours are literal.* [`ColorMode::Tokens`] writes `var(--_text)` and expects a
//! page to define it; a standalone image has no page, and a token reference would
//! resolve to nothing. [`ColorMode::Fixed`] exists for precisely this, so raster
//! forces it rather than asking the caller to remember.
//!
//! *Fonts are the ones we ship.* A host's fonts differ per machine, and the same
//! diagram would rasterise differently on each — which would make an image
//! comparison compare the machines rather than the drawings. The faces embedded
//! here are the two every diagram's own CSS names, subset to the same characters
//! [`crate::metrics`] can measure: what we draw is what we measured.

use crate::api::{ColorMode, Options, RenderError};

/// The faces a drawing is rendered with, subset to the characters the metrics
/// table covers.
///
/// Deliberately the same coverage as the measurement: a glyph we cannot measure
/// would be laid out by the fallback and drawn as tofu, and the two failures are
/// better kept to the same set of characters than spread across two.
const FACES: [&[u8]; 8] = [
    include_bytes!("../fonts/Inter-Regular.ttf"),
    include_bytes!("../fonts/Inter-Medium.ttf"),
    include_bytes!("../fonts/Inter-SemiBold.ttf"),
    include_bytes!("../fonts/Inter-Bold.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../fonts/JetBrainsMono-SemiBold.ttf"),
    include_bytes!("../fonts/JetBrainsMono-Bold.ttf"),
];

/// Why a drawing could not be turned into an image.
#[derive(Debug, Clone, PartialEq)]
pub enum RasterError {
    /// The diagram itself could not be drawn; nothing was rasterised.
    Render(RenderError),
    /// The SVG was drawn but could not be re-read. Only reachable if this crate
    /// emits markup it cannot itself parse, which is a bug here rather than in
    /// the artifact.
    Svg(String),
    /// The requested image has no area, or is larger than a pixel buffer can hold.
    ///
    /// Carries what was asked for rather than what it would have been rounded to:
    /// a request for 0.4 pixels is more useful reported as 0.4 than as 0.
    Size { width: f64, height: f64 },
    /// The pixels were produced but could not be encoded.
    Encode(String),
}

impl core::fmt::Display for RasterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Render(err) => write!(f, "{err}"),
            Self::Svg(msg) => write!(f, "the drawing could not be re-read: {msg}"),
            Self::Size { width, height } => {
                write!(f, "an image of {width:.0}x{height:.0} cannot be allocated")
            }
            Self::Encode(msg) => write!(f, "the image could not be encoded: {msg}"),
        }
    }
}

impl From<RenderError> for RasterError {
    fn from(err: RenderError) -> Self {
        Self::Render(err)
    }
}

/// The font database, built from the embedded faces alone.
///
/// System fonts are never loaded. Doing so would let a machine's own Inter — a
/// different version, or a different font wearing the name — win over the one that
/// was measured, and the same source would rasterise differently on two machines
/// that both looked correctly configured.
fn fonts() -> usvg::fontdb::Database {
    let mut db = usvg::fontdb::Database::new();
    for face in FACES {
        db.load_font_data(face.to_vec());
    }
    db
}

/// Pixel dimensions for a drawing at `scale`, or `None` when the result has no
/// area or overflows.
fn size(width: f32, height: f32, scale: f32) -> Option<(u32, u32)> {
    let w = (width * scale).round();
    let h = (height * scale).round();
    if !w.is_finite() || !h.is_finite() || w < 1.0 || h < 1.0 || w > 32_768.0 || h > 32_768.0 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "bounds checked directly above: finite, >= 1, <= 32768"
    )]
    Some((w as u32, h as u32))
}

/// Draw `source` as a PNG at `scale` times its natural size.
///
/// `options.colors` is ignored and forced to [`ColorMode::Fixed`]: an image has no
/// document behind it to resolve a token against.
///
/// # Errors
/// When the diagram cannot be drawn, when the drawing cannot be re-read, when the
/// requested size has no area or is too large, or when encoding fails.
pub fn png(source: &str, options: &Options, scale: f32) -> Result<Vec<u8>, RasterError> {
    let fixed = Options {
        colors: ColorMode::Fixed,
        ..options.clone()
    };
    let drawing = crate::api::render_svg(source, &fixed)?;
    let tree = usvg::Tree::from_str(
        &drawing.svg,
        &usvg::Options {
            fontdb: std::sync::Arc::new(fonts()),
            ..usvg::Options::default()
        },
    )
    .map_err(|e| RasterError::Svg(e.to_string()))?;

    let natural = tree.size();
    let asked = RasterError::Size {
        width: f64::from(natural.width() * scale),
        height: f64::from(natural.height() * scale),
    };
    let (width, height) = size(natural.width(), natural.height(), scale).ok_or(asked.clone())?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(asked)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|e| RasterError::Encode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOW: &str = "graph TD\n  Auth[Sign in] --> Home[Home page]\n";

    fn png_of(source: &str) -> Vec<u8> {
        png(source, &Options::default(), 1.0).expect("renders")
    }

    #[test]
    fn a_drawing_becomes_a_png() {
        let bytes = png_of(FLOW);
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "PNG magic");
        assert!(bytes.len() > 1000, "got {} bytes", bytes.len());
    }

    #[test]
    fn the_same_source_rasterises_byte_identically() {
        // The determinism the whole feature rests on: an image that differed
        // between two runs could not be compared against anything, including its
        // own previous self.
        assert_eq!(png_of(FLOW), png_of(FLOW));
    }

    #[test]
    fn nothing_is_read_from_the_machine() {
        // The font database carries the embedded faces and only those. A system
        // font winning here is how the same source starts rasterising differently
        // on two machines that both look correctly set up.
        let db = fonts();
        assert_eq!(db.len(), FACES.len());
        // Each static face names its own family — "Inter", "Inter Medium",
        // "Inter SemiBold" — so the check is the prefix, not equality.
        assert!(
            db.faces().all(|f| {
                f.families.iter().any(|(name, _)| {
                    name.starts_with("Inter") || name.starts_with("JetBrains Mono")
                })
            }),
            "{:?}",
            db.faces().map(|f| f.families.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn scale_multiplies_the_pixels_not_the_geometry() {
        let one = png(FLOW, &Options::default(), 1.0).unwrap();
        let two = png(FLOW, &Options::default(), 2.0).unwrap();
        assert!(two.len() > one.len(), "a 2x image carries more pixels");
    }

    #[test]
    fn a_source_that_will_not_draw_is_reported_as_such() {
        let err = png("sunburstChart", &Options::default(), 1.0).unwrap_err();
        assert!(matches!(err, RasterError::Render(_)), "{err:?}");
        assert!(err.to_string().contains("sunburstchart"), "{err}");
    }

    #[test]
    fn a_size_with_no_area_or_beyond_a_buffer_is_refused() {
        assert_eq!(size(100.0, 50.0, 2.0), Some((200, 100)));
        assert_eq!(size(100.0, 50.0, 0.0), None, "no area");
        assert_eq!(size(0.4, 50.0, 1.0), None, "rounds to nothing");
        assert_eq!(size(100.0, 50.0, 1e9), None, "beyond a pixel buffer");
        assert_eq!(size(f32::NAN, 50.0, 1.0), None);
        assert_eq!(size(f32::INFINITY, 50.0, 1.0), None);
    }

    #[test]
    fn every_error_reads_as_a_sentence() {
        assert!(RasterError::Svg("bad".into()).to_string().contains("bad"));
        assert!(RasterError::Encode("full".into())
            .to_string()
            .contains("full"));
        assert!(RasterError::Size {
            width: 0.0,
            height: 9.0
        }
        .to_string()
        .contains("0x9"));
        assert!(RasterError::from(RenderError::UnknownType {
            found: "nope".into(),
            suggestion: None,
        })
        .to_string()
        .contains("nope"));
    }
}
