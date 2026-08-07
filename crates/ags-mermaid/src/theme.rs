//! The token vocabulary a diagram draws with, and the CSS that derives it.
//!
//! A diagram names two colours — a background and a foreground — and everything
//! else is a blend of the two, so a drawing stays coherent under any theme
//! rather than only under the one it was designed against. Five further tokens
//! let a page override a blend it does not like.
//!
//! **In token mode the blending is done by CSS, not here.** That is the whole
//! point: `color-mix()` re-evaluates when a token changes, so a page restyles
//! every diagram by setting one variable and nothing is re-rendered. Computing
//! the blends in Rust would freeze them at whatever the theme was when the SVG
//! was written, and a page overriding the accent would get boxes in the new
//! colour and derived shades still based on the old one.
//!
//! In fixed mode there is no cascade to re-evaluate, so the same blends are
//! computed here and written as literals.

use crate::api::ColorMode;
use crate::color::{mix_hex, CHART_ACCENT_FALLBACK};

/// Tokens a page may set to steer a diagram.
///
/// `bg` and `fg` carry everything; the rest are overrides for a specific blend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub bg: String,
    pub fg: String,
    pub accent: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: "#ffffff".to_string(),
            fg: "#1e2430".to_string(),
            accent: CHART_ACCENT_FALLBACK.to_string(),
        }
    }
}

/// The tokens a page may set, and therefore the ones that need a namespace.
///
/// A custom property inherits, so a page defining `--border` for its own chrome
/// would have that value win inside every embedded diagram, over the engine's own
/// fallback. Prefixing these is what makes the two vocabularies independent; the
/// derived tokens (`--_text`, `--_arrow`) need no prefix because nothing outside
/// the drawing writes them.
const PUBLIC: [&str; 7] = ["bg", "fg", "accent", "muted", "line", "surface", "border"];

/// A derived token: its name, the public token that overrides it, and how much
/// foreground is mixed into the background when nothing overrides it.
///
/// The percentages are the existing renderer's, kept rather than re-tuned — this
/// change moves rendering, it does not restyle it.
const DERIVED: [(&str, Option<&str>, u8); 12] = [
    ("text", None, 100),
    ("text-sec", Some("muted"), 60),
    ("text-muted", Some("muted"), 40),
    ("text-faint", None, 25),
    ("line", Some("line"), 50),
    ("arrow", Some("accent"), 85),
    ("node-fill", Some("surface"), 3),
    ("node-stroke", Some("border"), 20),
    ("group-fill", None, 0),
    ("group-hdr", None, 5),
    ("inner-stroke", None, 12),
    ("key-badge", None, 10),
];

/// The `<style>` content for a diagram.
///
/// In token mode each derived value is a `color-mix()` the browser evaluates, so
/// it tracks the tokens. In fixed mode the same mixes are resolved to literals.
pub fn style_block(theme: &Theme, mode: &ColorMode) -> String {
    // The public tokens a page would otherwise supply. In token mode they are
    // the page's to set — writing them here would override the very thing a
    // theme change is meant to move. In fixed mode there is no page, and every
    // rule that reads `var(--ags-bg)` would resolve to nothing without them.
    let base = match mode {
        ColorMode::Tokens => String::new(),
        ColorMode::Fixed => format!(
            "--ags-bg:{};--ags-fg:{};--ags-accent:{};",
            theme.bg, theme.fg, theme.accent
        ),
    };
    let body = DERIVED
        .into_iter()
        .map(|(name, override_token, weight)| {
            let value = match mode {
                ColorMode::Tokens => derived_css(override_token, weight),
                ColorMode::Fixed => mix_hex(&theme.bg, &theme.fg, f64::from(weight) / 100.0),
            };
            format!("--_{name}:{value};")
        })
        .collect::<Vec<String>>()
        .concat();
    format!("svg{{{base}{body}}}")
}

/// How each colour is spelled for the target being drawn.
///
/// A renderer says *which* colour it wants — the arrow colour, the node fill — and
/// this says how to write it. Under [`ColorMode::Tokens`] that is a `var()`
/// reference a page can restyle; under [`ColorMode::Fixed`] it is the literal the
/// blend resolves to.
///
/// The alternative, which this replaces, was for every renderer to write
/// `var(--_arrow)` itself and for a later pass to rewrite the finished document
/// when that turned out to be wrong for the target. Two things were wrong with
/// that: the renderer was deciding something it has no view on — whether a cascade
/// will exist — and the correction happened after the fact, so `Fixed` shipped for
/// a long time meaning "literal values bound to properties nothing resolves".
/// Deciding once, here, is what makes `Fixed` mean what it says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Colors {
    /// Whether a colour is spelled as a reference or as a value.
    references: bool,
    /// The token prelude, which only a referencing target needs.
    block: String,
    /// Every token, and how it is written here.
    values: Vec<(String, String)>,
}

impl Colors {
    /// Work out how each colour is written, for `theme` on `mode`.
    #[must_use]
    pub fn new(theme: &Theme, mode: &ColorMode) -> Self {
        let references = matches!(mode, ColorMode::Tokens);
        let mut values = Vec::with_capacity(DERIVED.len() + 3);
        for (token, value) in [
            ("bg", &theme.bg),
            ("fg", &theme.fg),
            ("accent", &theme.accent),
        ] {
            let written = if references {
                format!("var(--ags-{token}, {value})")
            } else {
                value.clone()
            };
            // Keyed as it is spelled in the CSS, so the Fixed-mode resolver finds it.
            values.push((format!("ags-{token}"), written));
        }
        for (name, _, weight) in DERIVED {
            let written = if references {
                format!("var(--_{name})")
            } else {
                mix_hex(&theme.bg, &theme.fg, f64::from(weight) / 100.0)
            };
            values.push((format!("_{name}"), written));
        }
        Self {
            references,
            block: style_block(theme, mode),
            values,
        }
    }

    /// How one token is written, given the fallback its author supplied.
    ///
    /// The fallback only survives where references do: a literal has nothing to
    /// fall back from.
    #[must_use]
    pub fn token(&self, name: &str, fallback: &str) -> String {
        if self.references {
            // The fallback rides along, always. A reference without one renders
            // unstyled the moment the drawing is lifted out of the page that
            // defines the token — off-theme is recoverable, unstyled is not.
            // Only a public token takes the namespace. Those are the names a
            // page might also define — and did, which is what made a page's own
            // `--border` leak into every diagram and win. Everything else is
            // internal to the drawing and nothing outside writes it.
            return if PUBLIC.contains(&name) {
                format!("var(--ags-{name}, {fallback})")
            } else {
                format!("var(--{name}, {fallback})")
            };
        }
        self.values
            .iter()
            .find(|(token, _)| token == name)
            .map_or_else(|| fallback.to_string(), |(_, written)| written.clone())
    }

    /// Write every colour in a stylesheet the way this target needs it.
    ///
    /// Rules are authored in the token vocabulary — `fill:var(--_text)` — because
    /// that *is* the colour's name, and it is what a page reading the SVG expects
    /// to find. Whether the name survives into the output or is answered here is
    /// this config's decision, not each renderer's.
    #[must_use]
    pub fn resolve_css(&self, css: &str) -> String {
        if self.references {
            return css.to_string();
        }
        crate::tokens::resolve_all(css, &self.values)
    }
}

/// One derived value as CSS: the page's override if it set one, else a blend.
fn derived_css(override_token: Option<&str>, weight: u8) -> String {
    let blend = if weight == 100 {
        "var(--ags-fg)".to_string()
    } else if weight == 0 {
        "var(--ags-bg)".to_string()
    } else {
        format!("color-mix(in srgb, var(--ags-fg) {weight}%, var(--ags-bg))")
    };
    match override_token {
        Some(token) => format!("var(--ags-{token}, {blend})"),
        None => blend,
    }
}

/// Ink that reads on `over` — the colour it is written on top of.
///
/// A label on a filled area cannot take its colour from the page: the area's
/// lightness is the engine's to assign, so a token that happens to be light on a
/// light page (or the reverse) leaves the label invisible. This is what put white
/// `9%` on a pale wedge.
///
/// In token mode the area is a `hsl(from …)` expression whose lightness the page
/// can still move, so the choice is deferred to CSS: multiplying the distance
/// from mid-lightness by a large number and clamping turns it into a step — black
/// above the midpoint, white below — evaluated by the browser whenever the theme
/// changes. Saturation goes to nought so the ink is neutral rather than a tint of
/// the area it sits on.
pub fn ink_css(over: &str, mode: &ColorMode) -> String {
    match mode {
        ColorMode::Tokens => format!("hsl(from {over} h 0 clamp(0, calc((49 - l) * 100), 100))"),
        ColorMode::Fixed => if crate::color::is_dark_background(over) {
            "#ffffff"
        } else {
            "#141414"
        }
        .to_string(),
    }
}

/// Chart series shades, which need hue rotation rather than a blend.
///
/// `color-mix()` cannot rotate a hue, so this is the one derivation that needs
/// relative colour syntax. In fixed mode it falls back to the computed palette.
pub fn series_css(index: usize, mode: &ColorMode, theme: &Theme) -> String {
    if index == 0 {
        return match mode {
            ColorMode::Tokens => format!("var(--ags-accent, {CHART_ACCENT_FALLBACK})"),
            ColorMode::Fixed => theme.accent.clone(),
        };
    }
    match mode {
        ColorMode::Tokens => {
            // The same ramp the fixed path walks. Only the hue is taken from the
            // accent — the lightness is assigned, so a page that swaps the accent
            // for a very dark or very pale one still gets a legible chart, and the
            // seventh series cannot land on black.
            let (hue, l) =
                crate::color::series_ramp(index, crate::color::is_dark_background(&theme.bg));
            let (min_s, max_s) = crate::color::SERIES_S;
            format!(
                "hsl(from var(--ags-accent, {CHART_ACCENT_FALLBACK}) \
                 calc(h + {hue}) clamp({min_s}, s, {max_s}) {l})"
            )
        }
        ColorMode::Fixed => crate::color::series_color(index, &theme.accent, Some(&theme.bg)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_mode_derives_in_css_so_a_theme_change_is_enough() {
        let css = style_block(&Theme::default(), &ColorMode::Tokens);
        // The mixes must survive into the page: a token change re-evaluates them.
        assert!(
            css.contains("color-mix(in srgb, var(--ags-fg) 50%, var(--ags-bg))"),
            "{css}"
        );
        assert!(!css.contains('#'), "token mode must emit no literal: {css}");
    }

    #[test]
    fn a_page_override_wins_over_the_blend() {
        let css = style_block(&Theme::default(), &ColorMode::Tokens);
        // `--line` overrides the line blend; the blend stays as its fallback.
        assert!(
            css.contains(
                "--_line:var(--ags-line, color-mix(in srgb, var(--ags-fg) 50%, var(--ags-bg)))"
            ),
            "{css}"
        );
    }

    #[test]
    fn primary_text_is_the_foreground_rather_than_a_mix_with_itself() {
        let css = style_block(&Theme::default(), &ColorMode::Tokens);
        assert!(css.contains("--_text:var(--ags-fg);"), "{css}");
    }

    #[test]
    fn fixed_mode_emits_literals_and_no_variables() {
        let css = style_block(&Theme::default(), &ColorMode::Fixed);
        assert!(
            !css.contains("var("),
            "fixed mode must not reference tokens: {css}"
        );
        assert!(!css.contains("color-mix"), "{css}");
        assert!(css.contains("--_text:#1e2430"), "{css}");
    }

    #[test]
    fn every_derived_token_appears_in_both_modes() {
        for mode in [ColorMode::Tokens, ColorMode::Fixed] {
            let css = style_block(&Theme::default(), &mode);
            for (name, _, _) in DERIVED {
                assert!(
                    css.contains(&format!("--_{name}:")),
                    "{name} missing in {mode:?}"
                );
            }
        }
    }

    #[test]
    fn series_zero_is_the_accent_itself() {
        assert_eq!(
            series_css(0, &ColorMode::Tokens, &Theme::default()),
            format!("var(--ags-accent, {CHART_ACCENT_FALLBACK})")
        );
        let theme = Theme {
            accent: "#abcdef".into(),
            ..Theme::default()
        };
        assert_eq!(series_css(0, &ColorMode::Fixed, &theme), "#abcdef");
    }

    #[test]
    fn series_shades_rotate_hue_in_css_because_a_mix_cannot() {
        // The one derivation `color-mix()` cannot express, and the reason
        // relative colour syntax is needed at all.
        let css = series_css(1, &ColorMode::Tokens, &Theme::default());
        assert!(css.starts_with("hsl(from var(--ags-accent"), "{css}");
        assert!(css.contains("calc(h"), "{css}");
        // The saturation is the accent's, held inside a band; the lightness is
        // not the accent's at all — see `series_ramp`.
        assert!(css.contains("clamp(55, s, 85)"), "{css}");
    }

    #[test]
    fn a_series_lightness_is_assigned_rather_than_nudged() {
        // A shift relative to the accent runs off the end: seven series on a
        // mid-tone accent used to put the last one at black. Every index has to
        // land inside the band, however deep the ramp goes.
        let theme = Theme::default();
        for index in 0..24 {
            let css = series_css(index, &ColorMode::Tokens, &theme);
            if index == 0 {
                continue;
            }
            let lightness: f64 = css
                .trim_end_matches(')')
                .rsplit(' ')
                .next()
                .and_then(|l| l.parse().ok())
                .unwrap_or_else(|| panic!("a lightness to read: {css}"));
            assert!(
                (25.0..=78.0).contains(&lightness),
                "series {index} is at {lightness}: {css}"
            );
            assert!(
                !css.contains("calc(l"),
                "nothing relative to the accent's own"
            );
        }
    }

    #[test]
    fn a_page_and_a_standalone_image_agree_on_a_series() {
        // The two modes write the same ramp two ways. If they drift, a diagram
        // changes colour when it leaves the page it was drawn for.
        let theme = Theme::default();
        for index in 1..8 {
            let tokens = series_css(index, &ColorMode::Tokens, &theme);
            let fixed = series_css(index, &ColorMode::Fixed, &theme);
            let (hue, l) = crate::color::series_ramp(index, false);
            assert!(tokens.contains(&format!("calc(h + {hue})")), "{tokens}");
            assert!(tokens.ends_with(&format!(" {l})")), "{tokens}");
            // And the literal one is a colour, derived from the same numbers.
            assert!(fixed.starts_with('#'), "{fixed}");
        }
    }

    #[test]
    fn ink_is_chosen_against_what_it_is_written_on() {
        // Fixed mode knows the area's colour, so it picks now.
        assert_eq!(ink_css("#0b0e14", &ColorMode::Fixed), "#ffffff");
        assert_eq!(ink_css("#e8e2c4", &ColorMode::Fixed), "#141414");
    }

    #[test]
    fn token_ink_defers_the_choice_to_the_page() {
        // The area's lightness can still move under a theme change, so the step
        // has to be evaluated by the browser rather than baked in here.
        let css = ink_css("var(--ags-accent)", &ColorMode::Tokens);
        assert!(css.starts_with("hsl(from var(--ags-accent)"), "{css}");
        assert!(css.contains("clamp(0, calc((49 - l) * 100), 100)"), "{css}");
        // Neutral ink: a tint of the area it sits on reads as a smudge.
        assert!(css.contains(" h 0 "), "{css}");
    }

    #[test]
    fn fixed_series_shades_are_literals() {
        let css = series_css(1, &ColorMode::Fixed, &Theme::default());
        assert!(css.starts_with('#'), "{css}");
        assert!(!css.contains("var("), "{css}");
    }

    #[test]
    fn odd_and_even_series_move_in_opposite_directions() {
        let theme = Theme::default();
        let odd = series_css(1, &ColorMode::Tokens, &theme);
        let even = series_css(2, &ColorMode::Tokens, &theme);
        assert_ne!(odd, even);
        // One below the base, one above it — so adjacent series are told apart
        // by lightness as well as by hue.
        assert!(odd.ends_with(" 35)"), "{odd}");
        assert!(even.ends_with(" 66)"), "{even}");
        assert!(odd.contains("calc(h + -8)"), "{odd}");
        assert!(even.contains("calc(h + 12)"), "{even}");
    }

    #[test]
    fn a_dark_page_flips_which_half_of_a_pair_is_darkened() {
        // The darker shades are the ones that vanish on a dark background, so the
        // pair swaps rather than both ends being pushed the same way.
        let light = series_css(1, &ColorMode::Tokens, &Theme::default());
        let dark = series_css(
            1,
            &ColorMode::Tokens,
            &Theme {
                bg: "#0b0e14".into(),
                ..Theme::default()
            },
        );
        assert_ne!(light, dark, "the background has to reach the ramp");
    }
}
