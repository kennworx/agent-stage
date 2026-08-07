//! Colour arithmetic: hex, HSL, mixing, and the chart series palette.
//!
//! Chart types need more colours than a theme provides, so the palette derives
//! them from the accent rather than shipping a fixed list — a diagram then
//! matches whatever theme it is rendered under instead of fighting it.
//!
//! Series 1 and up alternate darker and lighter shades of the accent's hue, with
//! a small hue drift so the family stays recognisable. The direction flips on a
//! dark background: shades that read as "darker" against white disappear into a
//! dark page, so on dark backgrounds the odd series lighten instead.

use super::round::round_half_up;

/// Accent used when the theme supplies none, or supplies something that is not a
/// hex colour — a theme may legitimately hand over `var(--ags-accent)`, which cannot
/// be arithmetic on.
pub const CHART_ACCENT_FALLBACK: &str = "#3b82f6";

/// Whether a string is a six-digit hex colour.
pub fn is_valid_hex(color: &str) -> bool {
    let Some(body) = color.strip_prefix('#') else {
        return false;
    };
    body.len() == 6 && body.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
    let body = hex.strip_prefix('#').unwrap_or(hex);
    let byte = |i: usize| -> f64 {
        body.get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0)
            .into()
    };
    (byte(0), byte(2), byte(4))
}

fn rgb_to_hex(r: f64, g: f64, b: f64) -> String {
    let clamp = |v: f64| -> u8 {
        let c = round_half_up(v).clamp(0.0, 255.0);
        // Clamped to 0..=255 immediately above, so the cast cannot wrap.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "value is clamped to the u8 range on the line above"
        )]
        let out = c as u8;
        out
    };
    format!("#{:02x}{:02x}{:02x}", clamp(r), clamp(g), clamp(b))
}

/// Hue in degrees, saturation and lightness as percentages.
#[expect(
    clippy::many_single_char_names,
    reason = "r/g/b/h/s/l are the notation of the colour space; longer names would obscure the formula"
)]
fn hex_to_hsl(hex: &str) -> (f64, f64, f64) {
    let (r8, g8, b8) = hex_to_rgb(hex);
    let (r, g, b) = (r8 / 255.0, g8 / 255.0, b8 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = f64::midpoint(max, min);
    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, l * 100.0);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let hue = if (max - r).abs() < f64::EPSILON {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < f64::EPSILON {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (hue * 360.0, s * 100.0, l * 100.0)
}

#[expect(
    clippy::many_single_char_names,
    reason = "c/x/m/r/g/b are the notation of the HSL-to-RGB formula"
)]
fn hsl_to_hex(h: f64, s: f64, l: f64) -> String {
    let (si, li) = (s / 100.0, l / 100.0);
    let c = (1.0 - (2.0f64.mul_add(li, -1.0)).abs()) * si;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = c.mul_add(-0.5, li);
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    rgb_to_hex((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0)
}

/// Whether a background reads as dark, by lightness.
pub fn is_dark_background(bg_hex: &str) -> bool {
    hex_to_hsl(bg_hex).2 < 50.0
}

/// Composite `fg` over `bg` at `ratio` opacity, in RGB space.
pub fn mix_hex(bg_hex: &str, fg_hex: &str, ratio: f64) -> String {
    let (br, bg, bb) = hex_to_rgb(bg_hex);
    let (fr, fg, fb) = hex_to_rgb(fg_hex);
    let inv = 1.0 - ratio;
    rgb_to_hex(
        br.mul_add(inv, fr * ratio),
        bg.mul_add(inv, fg * ratio),
        bb.mul_add(inv, fb * ratio),
    )
}

/// Colour for chart series `index`, derived from the theme accent.
///
/// Index 0 is the accent itself. Later indices alternate darker and lighter
/// shades of the same hue, one tier further out every two series, with a small
/// hue drift so the family stays coherent.
pub fn series_color(index: usize, accent: &str, bg: Option<&str>) -> String {
    if index == 0 {
        return accent.to_string();
    }
    let safe_accent = if is_valid_hex(accent) {
        accent
    } else {
        CHART_ACCENT_FALLBACK
    };
    let dark_bg = bg
        .filter(|b| is_valid_hex(b))
        .is_some_and(is_dark_background);
    let (h, s, _) = hex_to_hsl(safe_accent);
    let (h_shift, l) = series_ramp(index, dark_bg);
    let new_h = ((h + h_shift) % 360.0 + 360.0) % 360.0;
    hsl_to_hex(new_h, s.clamp(SERIES_S.0, SERIES_S.1), l)
}

/// The saturation a series is drawn at, whatever the accent's own.
///
/// A grey accent would otherwise give a chart of greys, and a neon one a chart
/// that cannot be looked at.
pub const SERIES_S: (f64, f64) = (55.0, 85.0);

/// The hue shift and the lightness of series `index`.
///
/// **Lightness is assigned, not nudged.** A ramp that walks away from the
/// accent's own lightness runs off the end: with seven series and a mid-tone
/// accent the last one lands at black, which is what a Sankey's seventh node came
/// out as. Assigning from a fixed base and clamping both ends keeps every series
/// inside a band that reads on either page.
///
/// The pairs alternate either side of that base so adjacent indices are easy to
/// tell apart, and the direction flips on a dark page — where the "darker" half
/// of a pair is the one that disappears.
///
/// Shared with the token-mode CSS in [`crate::theme::series_css`], which writes
/// the same numbers into relative colour syntax. One source, so a diagram cannot
/// come out one set of colours in a page and another as a standalone image.
pub fn series_ramp(index: usize, dark_bg: bool) -> (f64, f64) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a chart with 2^53 series is not a chart"
    )]
    let tier = ((index as f64) / 2.0).ceil();
    let darker = (index % 2 == 1) != dark_bg;
    let l = if darker {
        (13.0f64.mul_add(-tier, 48.0)).max(25.0)
    } else {
        (11.0f64.mul_add(tier, 55.0)).min(78.0)
    };
    (if darker { -8.0 } else { 12.0 } * tier, l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sixth_of_the_wheel_converts_to_its_own_corner_of_the_cube() {
        // Hue is handled in six ranges, and a wrong bound in any one of them
        // shows up as a colour that is nearly right — the hardest kind to spot
        // by eye, and the reason this is asserted rather than looked at.
        let full = |h: f64| hsl_to_hex(h, 100.0, 50.0);
        assert_eq!(full(0.0), "#ff0000", "red");
        assert_eq!(full(60.0), "#ffff00", "yellow");
        assert_eq!(full(120.0), "#00ff00", "green");
        assert_eq!(full(180.0), "#00ffff", "cyan");
        assert_eq!(full(240.0), "#0000ff", "blue");
        assert_eq!(full(300.0), "#ff00ff", "magenta");
    }

    #[test]
    fn lightness_runs_from_black_through_the_hue_to_white() {
        assert_eq!(hsl_to_hex(0.0, 100.0, 0.0), "#000000");
        assert_eq!(hsl_to_hex(0.0, 100.0, 100.0), "#ffffff");
        // With no saturation the hue does not matter at all.
        assert_eq!(hsl_to_hex(210.0, 0.0, 50.0), hsl_to_hex(30.0, 0.0, 50.0));
    }

    #[test]
    fn recognises_hex_colours() {
        assert!(is_valid_hex("#3b82f6"));
        assert!(is_valid_hex("#FFFFFF"));
        assert!(!is_valid_hex("3b82f6"));
        assert!(!is_valid_hex("#3b82f"));
        assert!(!is_valid_hex("#3b82f6a"));
        assert!(!is_valid_hex("#gggggg"));
        // A theme may hand over a CSS variable reference; it must not be treated
        // as a colour to do arithmetic on.
        assert!(!is_valid_hex("var(--ags-accent)"));
        assert!(!is_valid_hex(""));
    }

    /// Values from the renderer this replaces, so a drift in the derivation
    /// shows up as a failure rather than as charts quietly changing colour.
    #[test]
    fn matches_the_reference_palette() {
        for (index, accent, bg, expected) in [
            (0, "#3b82f6", None, "#3b82f6"),
            (1, "#3b82f6", None, "#0d5ba5"),
            (2, "#3b82f6", None, "#5f79f2"),
            (3, "#3b82f6", None, "#0a5076"),
            (4, "#3b82f6", None, "#9592f6"),
            (5, "#f63b82", None, "#760a5e"),
            (1, "#3b82f6", Some("#0b0b0d"), "#5f79f2"),
            (2, "#3b82f6", Some("#0b0b0d"), "#0d5ba5"),
        ] {
            let got = series_color(index, accent, bg);
            assert_eq!(got, expected, "series {index} of {accent} on {bg:?}");
        }
    }

    #[test]
    fn a_bad_accent_falls_back_rather_than_failing() {
        // Series 0 is handed back untouched — a caller asking for the accent gets
        // whatever it supplied, even a CSS variable.
        assert_eq!(
            series_color(0, "var(--ags-accent)", None),
            "var(--ags-accent)"
        );
        // Later series need arithmetic, so they derive from the fallback.
        assert_eq!(
            series_color(1, "var(--ags-accent)", None),
            series_color(1, CHART_ACCENT_FALLBACK, None)
        );
    }

    #[test]
    fn shade_direction_flips_on_a_dark_background() {
        let light = series_color(1, "#3b82f6", Some("#ffffff"));
        let dark = series_color(1, "#3b82f6", Some("#0b0b0d"));
        assert_ne!(light, dark);
        // On a dark page the first derived shade lightens instead of darkening.
        assert_eq!(dark, series_color(2, "#3b82f6", Some("#ffffff")));
    }

    #[test]
    fn an_invalid_background_is_ignored() {
        assert_eq!(
            series_color(1, "#3b82f6", Some("nonsense")),
            series_color(1, "#3b82f6", None)
        );
    }

    #[test]
    fn greys_round_trip_through_hsl() {
        // A grey has no hue; the conversion must not invent one.
        for hex in ["#000000", "#808080", "#ffffff"] {
            let (h, s, _) = hex_to_hsl(hex);
            assert!((h - 0.0).abs() < 1e-9, "{hex} gained a hue");
            assert!((s - 0.0).abs() < 1e-9, "{hex} gained saturation");
        }
    }

    #[test]
    fn hue_survives_a_round_trip() {
        for hex in ["#3b82f6", "#f63b82", "#82f63b"] {
            let (h, s, l) = hex_to_hsl(hex);
            assert_eq!(hsl_to_hex(h, s, l), hex, "{hex} did not survive");
        }
    }

    #[test]
    fn mixing_interpolates_between_the_two() {
        assert_eq!(mix_hex("#000000", "#ffffff", 0.0), "#000000");
        assert_eq!(mix_hex("#000000", "#ffffff", 1.0), "#ffffff");
        assert_eq!(mix_hex("#000000", "#ffffff", 0.5), "#808080");
    }

    #[test]
    fn darkness_is_judged_by_lightness() {
        assert!(is_dark_background("#0b0b0d"));
        assert!(is_dark_background("#000000"));
        assert!(!is_dark_background("#ffffff"));
        assert!(!is_dark_background("#eff1f5"));
    }
}
