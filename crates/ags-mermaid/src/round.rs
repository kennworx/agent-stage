//! Coordinate rounding, matching the renderer this port replaces.
//!
//! SVG coordinates are emitted at one decimal place. That rounding has to agree
//! with the previous renderer exactly, because every port is verified by diffing
//! its geometry — a tie broken the other way shows up as a spurious mismatch and
//! sends someone hunting a layout bug that does not exist.
//!
//! JavaScript's `Math.round` breaks ties toward positive infinity: `-0.5`
//! becomes `-0`. Rust's `f64::round` breaks them away from zero: `-0.5` becomes
//! `-1.0`. They agree everywhere else, which is exactly what makes the
//! difference easy to miss — it only appears on an exact negative half, and only
//! in a diagram with negative coordinates.

/// Round half toward positive infinity, as JavaScript's `Math.round` does.
pub fn round_half_up(v: f64) -> f64 {
    (v + 0.5).floor()
}

/// A coordinate at one decimal place, formatted for an SVG attribute.
///
/// Trailing `.0` is dropped so the output matches the previous renderer's
/// `String(Math.round(n * 10) / 10)` character for character.
pub fn coord(v: f64) -> String {
    let scaled = round_half_up(v * 10.0) / 10.0;
    // `-0` and `0` are the same coordinate; emitting the sign would be noise.
    if scaled == 0.0 {
        return "0".to_string();
    }
    let s = format!("{scaled}");
    s
}

/// A count as a coordinate.
///
/// Every count that reaches arithmetic in a layout is a number of boxes, lanes,
/// rows or slices in one diagram — orders of magnitude below the point where an
/// `f64` starts skipping integers.
#[expect(
    clippy::cast_precision_loss,
    reason = "counts of elements in a diagram, never near 2^53"
)]
pub fn count(n: usize) -> f64 {
    n as f64
}

/// A ratio at three decimal places, for a value that is not a coordinate.
///
/// A transform's scale factor is the case that forces this: an icon authored on
/// a 24-unit grid and drawn at 20px scales by 0.8333, and rounding that to a
/// coordinate's single decimal draws the glyph four percent small.
pub fn ratio(v: f64) -> String {
    let scaled = round_half_up(v * 1000.0) / 1000.0;
    if scaled == 0.0 {
        return "0".to_string();
    }
    format!("{scaled}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_positive_halves_up() {
        assert!((round_half_up(0.5) - 1.0).abs() < 1e-9);
        assert!((round_half_up(1.5) - 2.0).abs() < 1e-9);
        assert!((round_half_up(2.5) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn rounds_negative_halves_toward_positive_infinity() {
        // The whole reason this module exists: `f64::round` gives -1.0 here.
        assert!((round_half_up(-0.5) - 0.0).abs() < 1e-9);
        assert!((round_half_up(-1.5) + 1.0).abs() < 1e-9);
        assert!((round_half_up(-2.5) + 2.0).abs() < 1e-9);
    }

    #[test]
    fn agrees_with_rust_rounding_away_from_ties() {
        for v in [0.4, 0.6, -0.4, -0.6, 12.3, -12.3, 99.999] {
            assert!(
                (round_half_up(v) - v.round()).abs() < 1e-9,
                "diverged at {v}"
            );
        }
    }

    #[test]
    fn coordinates_keep_one_decimal() {
        assert_eq!(coord(174.0), "174");
        assert_eq!(coord(105.94), "105.9");
        assert_eq!(coord(105.95), "106");
        assert_eq!(coord(-70.42), "-70.4");
    }

    #[test]
    fn a_count_becomes_a_coordinate() {
        assert!((count(0) - 0.0).abs() < 1e-9);
        assert!((count(41) - 41.0).abs() < 1e-9);
    }

    #[test]
    fn a_ratio_keeps_enough_precision_to_scale_a_glyph() {
        // The case this exists for: a coordinate's single decimal would round
        // this to 0.8 and draw every icon four percent small.
        assert_eq!(ratio(20.0 / 24.0), "0.833");
        assert_eq!(ratio(1.0), "1");
        assert_eq!(ratio(0.0), "0");
        assert_eq!(ratio(-0.0004), "0");
    }

    #[test]
    fn zero_has_no_sign() {
        assert_eq!(coord(0.0), "0");
        assert_eq!(coord(-0.0), "0");
        // Rounds to zero from below — must not emit "-0".
        assert_eq!(coord(-0.01), "0");
    }
}
