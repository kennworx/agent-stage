//! Agent-authored palettes, resolved to CSS ahead of serving.
//!
//! A `theme` block names a palette the reviewer can pick. The viewer this
//! replaces resolved one in JavaScript and wrote the tokens onto `<html>`; a page
//! that renders without a script has to resolve it ahead of time instead, so the
//! whole thing lands as CSS custom properties and one radio per theme.
//!
//! A theme need not spell out every token. Three ways to fill the gaps, in the
//! order they win:
//!
//! 1. **An explicit `token: #hex`** always wins.
//! 2. **A `seed: #hex`** expands to the whole palette through an OKLCH lightness
//!    ramp — hue and chroma from the seed, lightness assigned by role and flipped
//!    between modes.
//! 3. **`background`/`foreground` seeds** fill the middle tokens by `color-mix()`.
//!
//! A token none of those reach is omitted, so it falls through to the base
//! `:root` cascade rather than being pinned to a default that fights the theme.
//!
//! The OKLCH maths is done here rather than left to CSS relative colour syntax
//! (`oklch(from <seed> L C H)`) for the same reason the viewer did it in JS: where
//! that syntax is unsupported, a seed theme silently falls back to the base look
//! instead of the palette it asked for. Plain hex renders everywhere. The colour
//! space is Björn Ottosson's `OKLab`.

use crate::validate::{is_hex_color, THEME_TOKENS};

/// Which of a theme's two palettes is being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

/// One `theme` block: the tokens it sets, split by the section they appeared in.
///
/// Pairs rather than a map, because the page is emitted deterministically and a
/// hash map's iteration order is not. The lists are tiny — seven tokens at most —
/// so a linear lookup costs nothing worth naming.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Theme {
    /// The block's `#id`, which is what the picker shows and what a recorded
    /// choice names.
    pub name: String,
    /// Set before any `dark:`/`light:` header, and so applying to both.
    shared: Vec<(String, String)>,
    dark: Vec<(String, String)>,
    light: Vec<(String, String)>,
}

/// A colour as OKLCH: perceptual lightness 0..1, chroma, hue in degrees.
#[derive(Debug, Clone, Copy)]
struct Oklch {
    l: f64,
    c: f64,
    h: f64,
}

/// The lightness ramp, as `(token, lightness, chroma scale)`.
///
/// Hue comes from the seed; lightness is assigned by role; chroma is scaled down
/// for the neutrals so a background reads as a faint tint rather than a grey wash.
/// Light mode uses a *larger* elevation delta than dark — on a near-white page,
/// background, card and border collapse into one flat sheet unless the page drops
/// to a soft tint and cards float near white.
const DARK_RAMP: &[(&str, f64, f64)] = &[
    ("background", 0.16, 0.30),
    ("primary-foreground", 0.16, 0.30),
    ("card", 0.20, 0.45),
    ("border", 0.32, 0.60),
    ("muted-foreground", 0.70, 0.35),
    ("foreground", 0.94, 0.18),
];

const LIGHT_RAMP: &[(&str, f64, f64)] = &[
    ("background", 0.94, 0.14),
    ("primary-foreground", 0.99, 0.05),
    ("card", 0.985, 0.09),
    ("border", 0.78, 0.30),
    ("muted-foreground", 0.48, 0.35),
    ("foreground", 0.24, 0.20),
];

/// The band the accent's lightness is clamped into, per mode.
///
/// The accent keeps the seed's full chroma; only lightness moves, and only enough
/// to stay legible against the background derived from that same seed — lifted on
/// dark, held down on light so it still reads against white.
const DARK_PRIMARY_L: (f64, f64) = (0.55, 0.72);
const LIGHT_PRIMARY_L: (f64, f64) = (0.45, 0.60);

/// The middle tokens `color-mix()` fills from the background/foreground seeds, as
/// `(token, inputs, expression)`.
///
/// A token is only mixed when the theme actually moves one of its inputs, so a
/// primary-only theme leaves these alone and inherits the base look rather than
/// drifting away from it.
const MIX: &[(&str, &[&str], &str)] = &[
    (
        "card",
        &["background", "foreground"],
        "color-mix(in srgb, var(--background), var(--foreground) 8%)",
    ),
    // The border has to read against both the page and the card, which is itself
    // 8% toward the foreground. 18% left it nearly invisible; 32% gives a clear
    // separation in either mode.
    (
        "border",
        &["background", "foreground"],
        "color-mix(in srgb, var(--background), var(--foreground) 32%)",
    ),
    (
        "muted-foreground",
        &["background", "foreground"],
        "color-mix(in srgb, var(--foreground), var(--background) 35%)",
    ),
    ("primary-foreground", &["background"], "var(--background)"),
];

/// Parse a `theme` block body into the palette it declares.
///
/// Mirrors the viewer's parser exactly, including its tolerance: a line that is
/// not a recognised token, or whose value is not a hex colour, is skipped rather
/// than rejected. Gate 1 is what reports those — this runs on an artifact that
/// already passed it, so being strict a second time would only mean two places
/// that can disagree about what a theme says.
pub fn parse(name: &str, body: &str) -> Theme {
    let mut theme = Theme {
        name: name.to_string(),
        ..Theme::default()
    };
    let mut section = Section::Shared;
    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match line.trim_end_matches(':').trim() {
            "dark" => {
                section = Section::Dark;
                continue;
            }
            "light" => {
                section = Section::Light;
                continue;
            }
            _ => {}
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if !(key == "seed" || THEME_TOKENS.contains(&key)) || !is_hex_color(value) {
            continue;
        }
        theme
            .section_mut(section)
            .push((key.to_string(), value.to_string()));
    }
    theme
}

/// Which part of a theme block subsequent lines belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Shared,
    Dark,
    Light,
}

impl Theme {
    fn section_mut(&mut self, section: Section) -> &mut Vec<(String, String)> {
        match section {
            Section::Shared => &mut self.shared,
            Section::Dark => &mut self.dark,
            Section::Light => &mut self.light,
        }
    }

    /// The value this theme sets for `key` in `mode`: the mode's own section
    /// first, then the shared one it overrides.
    ///
    /// Searched backwards within a section, so a key written twice takes its last
    /// value — which is what the object-spread the viewer used to do would give,
    /// and the only reading under which repeating a line means anything.
    fn get<'a>(&'a self, key: &str, mode: Mode) -> Option<&'a str> {
        let specific = match mode {
            Mode::Dark => &self.dark,
            Mode::Light => &self.light,
        };
        let last = |list: &'a [(String, String)]| {
            list.iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        last(specific).or_else(|| last(&self.shared))
    }

    /// Whether this theme sets nothing at all, and so would emit an empty rule.
    pub fn is_empty(&self) -> bool {
        self.shared.is_empty() && self.dark.is_empty() && self.light.is_empty()
    }
}

/// The `token → value` pairs a theme applies in `mode`, in [`THEME_TOKENS`] order.
///
/// A value is either a concrete hex colour or a `color-mix()`/`var()` expression;
/// both are legal on the right of a custom property, and the distinction stops
/// mattering once it reaches CSS.
pub fn resolve(theme: &Theme, mode: Mode) -> Vec<(&'static str, String)> {
    let ramp = theme.get("seed", mode).map(|seed| derive(seed, mode));
    THEME_TOKENS
        .iter()
        .filter_map(|&token| {
            let value = if let Some(set) = theme.get(token, mode) {
                Some(set.to_string())
            } else if let Some(ramp) = ramp.as_ref() {
                ramp.iter()
                    .find(|(t, _)| *t == token)
                    .map(|(_, v)| v.clone())
            } else {
                mixed(theme, token, mode)
            };
            value.map(|v| (token, v))
        })
        .collect()
}

/// The `color-mix()` expression for `token`, when the theme moved an input of it.
fn mixed(theme: &Theme, token: &str, mode: Mode) -> Option<String> {
    let (_, inputs, expr) = MIX.iter().find(|(t, _, _)| *t == token)?;
    inputs
        .iter()
        .any(|input| theme.get(input, mode).is_some())
        .then(|| (*expr).to_string())
}

/// A theme's tokens as the body of a CSS rule, e.g. `--background:#111;--card:#222`.
pub fn css(theme: &Theme, mode: Mode) -> String {
    resolve(theme, mode)
        .iter()
        .map(|(token, value)| format!("--{token}:{value}"))
        .collect::<Vec<_>>()
        .join(";")
}

/// One accent seed expanded into the whole palette for `mode`.
fn derive(seed_hex: &str, mode: Mode) -> Vec<(&'static str, String)> {
    let seed = hex_to_oklch(seed_hex);
    let (ramp, (min_l, max_l)) = match mode {
        Mode::Dark => (DARK_RAMP, DARK_PRIMARY_L),
        Mode::Light => (LIGHT_RAMP, LIGHT_PRIMARY_L),
    };
    let mut out: Vec<(&'static str, String)> = ramp
        .iter()
        .map(|&(token, l, chroma_scale)| {
            (
                token,
                oklch_to_hex(Oklch {
                    l,
                    c: seed.c * chroma_scale,
                    h: seed.h,
                }),
            )
        })
        .collect();
    out.push((
        "primary",
        oklch_to_hex(Oklch {
            l: seed.l.clamp(min_l, max_l),
            c: seed.c,
            h: seed.h,
        }),
    ));
    out
}

/// `#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa` to linear-light RGB, alpha dropped.
fn parse_hex(hex: &str) -> (f64, f64, f64) {
    let digits = hex.trim_start_matches('#');
    let six: String = if digits.len() < 6 {
        // `#rgb` and `#rgba` double each digit; the alpha nibble is simply not
        // among the three taken.
        digits.chars().take(3).flat_map(|c| [c, c]).collect()
    } else {
        digits.chars().take(6).collect()
    };
    // Gate 1 has already accepted this as a hex colour, so the parse holds; nought
    // is a total answer rather than a panic if it ever does not.
    let packed = u32::from_str_radix(&six, 16).unwrap_or(0);
    let channel = |shift: u32| f64::from((packed >> shift) & 255) / 255.0;
    (
        srgb_to_linear(channel(16)),
        srgb_to_linear(channel(8)),
        srgb_to_linear(channel(0)),
    )
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[expect(
    clippy::many_single_char_names,
    reason = "r/g/b and L/a/b are the names the OKLab reference uses; renaming them \
              would make this harder to check against the source it was ported from"
)]
fn hex_to_oklch(hex: &str) -> Oklch {
    let (r, g, b) = parse_hex(hex);
    let l_ = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m_ = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s_ = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    let l = 0.210_454_255_3 * l_ + 0.793_617_785 * m_ - 0.004_072_046_8 * s_;
    let a = 1.977_998_495_1 * l_ - 2.428_592_205 * m_ + 0.450_593_709_9 * s_;
    let b_axis = 0.025_904_037_1 * l_ + 0.782_771_766_2 * m_ - 0.808_675_766 * s_;
    let mut h = b_axis.atan2(a).to_degrees();
    if h < 0.0 {
        h += 360.0;
    }
    Oklch {
        l,
        c: a.hypot(b_axis),
        h,
    }
}

fn oklch_to_hex(color: Oklch) -> String {
    let hr = color.h.to_radians();
    let a = color.c * hr.cos();
    let b_axis = color.c * hr.sin();
    let l_ = (color.l + 0.396_337_777_4 * a + 0.215_803_757_3 * b_axis).powi(3);
    let m_ = (color.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b_axis).powi(3);
    let s_ = (color.l - 0.089_484_177_5 * a - 1.291_485_548 * b_axis).powi(3);
    let r = linear_to_srgb(4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_);
    let g = linear_to_srgb(-1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_);
    let b = linear_to_srgb(-0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_);
    format!("#{}{}{}", channel(r), channel(g), channel(b))
}

/// One channel of linear-light output as two lowercase hex digits.
///
/// Clamped per channel rather than scaled toward the gamut: at the chroma these
/// ramps ask for, a component lands outside 0..1 only marginally, and clamping
/// matches what the renderer this replaces did — a smarter mapping would move
/// every derived colour and turn a port into a redesign.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0..=1 then scaled by 255, so the rounded value is a whole \
              number in 0..=255 and the cast to u8 is exact"
)]
fn channel(value: f64) -> String {
    let scaled = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{scaled:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_splits_its_lines_by_section() {
        let theme = parse(
            "t",
            "primary: #ff0000\ndark:\nbackground: #000000\nlight:\nbackground: #ffffff\n",
        );
        assert_eq!(theme.get("primary", Mode::Dark), Some("#ff0000"));
        assert_eq!(theme.get("primary", Mode::Light), Some("#ff0000"));
        assert_eq!(theme.get("background", Mode::Dark), Some("#000000"));
        assert_eq!(theme.get("background", Mode::Light), Some("#ffffff"));
    }

    #[test]
    fn a_mode_section_overrides_the_shared_one() {
        let theme = parse("t", "background: #111111\ndark:\nbackground: #222222\n");
        assert_eq!(theme.get("background", Mode::Dark), Some("#222222"));
        assert_eq!(theme.get("background", Mode::Light), Some("#111111"));
    }

    #[test]
    fn a_line_that_is_not_a_token_or_not_a_colour_is_skipped() {
        // Gate 1 reports these; resolving them a second way would be a second
        // opinion about what the artifact says.
        let theme = parse(
            "t",
            "\nnonsense\nbogus-token: #ff0000\nprimary: notacolour\nprimary: #00ff00\n",
        );
        assert_eq!(theme.get("primary", Mode::Dark), Some("#00ff00"));
        assert_eq!(theme.get("bogus-token", Mode::Dark), None);
        assert_eq!(theme.get("nonsense", Mode::Dark), None);
    }

    #[test]
    fn a_theme_that_sets_nothing_is_empty() {
        assert!(parse("t", "").is_empty());
        assert!(parse("t", "just words\n").is_empty());
        assert!(!parse("t", "primary: #ff0000\n").is_empty());
    }

    #[test]
    fn a_header_is_recognised_with_or_without_its_colon() {
        let with = parse("t", "dark:\nprimary: #ff0000\n");
        let without = parse("t", "dark\nprimary: #ff0000\n");
        assert_eq!(with.get("primary", Mode::Dark), Some("#ff0000"));
        assert_eq!(without.get("primary", Mode::Dark), Some("#ff0000"));
        assert_eq!(with.get("primary", Mode::Light), None);
    }

    #[test]
    fn an_explicit_token_beats_the_seed_ramp() {
        let theme = parse("t", "seed: #6a5acd\nbackground: #010203\n");
        let resolved = resolve(&theme, Mode::Dark);
        let background = resolved
            .iter()
            .find(|(t, _)| *t == "background")
            .map(|(_, v)| v.as_str());
        assert_eq!(background, Some("#010203"));
        // The rest still comes from the ramp.
        assert!(resolved.iter().any(|(t, _)| *t == "foreground"));
    }

    #[test]
    fn a_seed_fills_every_token() {
        let theme = parse("t", "seed: #6a5acd\n");
        for mode in [Mode::Dark, Mode::Light] {
            let resolved = resolve(&theme, mode);
            assert_eq!(resolved.len(), THEME_TOKENS.len(), "{mode:?}");
            for (_, value) in &resolved {
                assert!(value.starts_with('#') && value.len() == 7, "{value}");
            }
        }
    }

    #[test]
    fn background_and_foreground_seeds_mix_the_middle_tokens() {
        let theme = parse("t", "background: #111111\nforeground: #eeeeee\n");
        let resolved = resolve(&theme, Mode::Dark);
        let get = |token: &str| {
            resolved
                .iter()
                .find(|(t, _)| *t == token)
                .map(|(_, v)| v.clone())
        };
        assert_eq!(get("background").as_deref(), Some("#111111"));
        assert!(get("card").is_some_and(|v| v.starts_with("color-mix(")));
        assert!(get("border").is_some_and(|v| v.starts_with("color-mix(")));
        assert_eq!(
            get("primary-foreground").as_deref(),
            Some("var(--background)")
        );
        // `primary` has no mix rule and was not set, so it falls through.
        assert_eq!(get("primary"), None);
    }

    #[test]
    fn a_theme_that_moves_no_mix_input_leaves_the_middle_tokens_alone() {
        // A primary-only theme must not drag card/border away from the base look.
        let theme = parse("t", "primary: #ff0000\n");
        let resolved = resolve(&theme, Mode::Dark);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved.first().map(|(t, _)| *t), Some("primary"));
    }

    #[test]
    fn the_tokens_come_out_in_a_fixed_order() {
        // The page is emitted deterministically, so two renders of one artifact
        // must not differ by the order of a rule's declarations.
        let theme = parse("t", "seed: #6a5acd\n");
        let once = css(&theme, Mode::Dark);
        assert_eq!(once, css(&theme, Mode::Dark));
        assert!(once.starts_with("--background:#"), "{once}");
        assert!(once.contains(";--foreground:#"), "{once}");
    }

    #[test]
    fn css_is_empty_when_a_theme_resolves_to_nothing() {
        assert_eq!(css(&parse("t", ""), Mode::Dark), "");
    }

    #[test]
    fn a_short_hex_expands_the_way_css_does() {
        // #abc is #aabbcc, and the alpha nibble of #abcd is dropped.
        assert_eq!(oklch_to_hex(hex_to_oklch("#abc")), "#aabbcc");
        assert_eq!(oklch_to_hex(hex_to_oklch("#abcd")), "#aabbcc");
        assert_eq!(oklch_to_hex(hex_to_oklch("#aabbccdd")), "#aabbcc");
    }

    #[test]
    fn a_colour_survives_the_round_trip_through_oklch() {
        for hex in ["#000000", "#ffffff", "#6a5acd", "#3b82f6", "#fe8019"] {
            assert_eq!(oklch_to_hex(hex_to_oklch(hex)), hex, "{hex}");
        }
    }

    #[test]
    fn an_unparsable_hex_is_black_rather_than_a_panic() {
        // Unreachable through Gate 1, which is exactly why it is asserted: the
        // total answer is what makes that true rather than lucky.
        assert_eq!(oklch_to_hex(hex_to_oklch("#zzzzzz")), "#000000");
    }

    #[test]
    fn a_channel_outside_the_gamut_is_clamped_not_wrapped() {
        assert_eq!(channel(-1.0), "00");
        assert_eq!(channel(0.0), "00");
        assert_eq!(channel(1.0), "ff");
        assert_eq!(channel(2.0), "ff");
        assert_eq!(channel(0.5), "80");
    }

    #[test]
    fn the_two_modes_of_one_seed_differ() {
        // The ramp flips lightness between modes; if they matched, the light
        // palette would be the dark one and the toggle would do nothing.
        let theme = parse("t", "seed: #6a5acd\n");
        assert_ne!(css(&theme, Mode::Dark), css(&theme, Mode::Light));
    }
}
