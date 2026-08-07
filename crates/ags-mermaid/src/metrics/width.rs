//! Text width, from the fonts the diagrams declare — without a font file or a DOM.
//!
//! Every diagram sizes its boxes from this, and the canvas from the boxes, so a
//! divergence here reflows an entire drawing.
//!
//! A run's width is the sum of its glyphs' advances, read out of Inter and
//! `JetBrains Mono` and baked into [`table`] at build time. Reading them at run time
//! would mean a font parser and 800 KB of font in a crate that is deliberately
//! dependency-free and compiled to WebAssembly; the numbers never change between
//! releases, so the table is a few kilobytes and needs no parser.
//!
//! **A fallback remains, much smaller than it was.** The table covers Latin and
//! the punctuation diagrams use; a Latin font has no answer for CJK, Hangul or
//! emoji, so those are still measured by class — double width, as before. The
//! per-character width classes that used to sit alongside them (`W` at 1.5, `i` at
//! 0.4) are gone: every character they named is in the table, so no input could
//! reach them. The guess has narrowed from "every character" to "the characters no
//! bundled Latin font could measure".
//!
//! What this does *not* account for is kerning. A browser applies the font's pair
//! adjustments and shrinks `AV` or `To` slightly; summing advances does not. The
//! error is therefore in the safe direction — a box is a shade wider than the text
//! needs rather than a shade too narrow.

use super::table;

/// Zero-width overlays: combining diacritical marks.
fn is_combining_mark(code: u32) -> bool {
    matches!(code,
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// Roughly double-width: CJK, Hangul, fullwidth forms.
fn is_fullwidth(code: u32) -> bool {
    matches!(code,
        0x1100..=0x115F   // Hangul Jamo
        | 0x2E80..=0x2EFF // CJK Radicals Supplement
        | 0x2F00..=0x2FDF // Kangxi Radicals
        | 0x3000..=0x303F // CJK Symbols and Punctuation
        | 0x3040..=0x309F // Hiragana
        | 0x30A0..=0x30FF // Katakana
        | 0x3100..=0x312F // Bopomofo
        | 0x3130..=0x318F // Hangul Compatibility Jamo
        | 0x3190..=0x31FF // Kanbun and extensions
        | 0x3200..=0x33FF // Enclosed CJK and Compatibility
        | 0x3400..=0x4DBF // CJK Extension A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xAC00..=0xD7AF // Hangul Syllables
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0xFF00..=0xFF60 // Fullwidth ASCII
        | 0xFFE0..=0xFFE6 // Fullwidth symbols
    ) || code >= 0x2_0000 // CJK Extension B and beyond
}

/// Pictographic characters, which render at roughly double width.
///
/// The original uses `\p{Emoji_Presentation}` and `\p{Extended_Pictographic}`.
/// Rust's standard library has no Unicode property tables, so this covers the
/// pictographic blocks by range. Characters outside them that the property
/// escapes would catch are measured as ordinary glyphs — narrower than they
/// render, so a label containing one is sized slightly small.
fn is_pictographic(code: u32) -> bool {
    matches!(code,
        0x2190..=0x21FF   // Arrows
        | 0x2300..=0x23FF // Miscellaneous Technical
        | 0x2600..=0x27BF // Misc Symbols and Dingbats
        | 0x2B00..=0x2BFF // Misc Symbols and Arrows
        | 0x1F000..=0x1FAFF // Emoji planes
    )
}

/// Relative width of one character, in "average glyph" units.
///
/// This used to carry a width class per character — `W` at 1.5, `i` at 0.4, and so
/// on, transcribed from the renderer being replaced. Every one of those characters
/// is in the table now, so none of those arms could ever run again; they were
/// answering a question the font answers better, and a `match` nothing reaches is
/// worse than no `match`.
///
/// What is left is what a Latin font genuinely cannot be asked: a combining mark
/// that overlays the glyph before it, and the scripts and symbols that render at
/// roughly double width. Anything else outside the table — Greek, Cyrillic, a
/// codepoint Inter lacks in one weight — takes the average, exactly as before.
fn char_width(c: char) -> f64 {
    let code = c as u32;
    if is_combining_mark(code) {
        return 0.0;
    }
    if is_fullwidth(code) || is_pictographic(code) {
        return 2.0;
    }
    1.0
}

/// Which column of the table a font weight reads.
///
/// The renderer draws at 400, 500, 600 and 700 and the table carries exactly
/// those. A weight between two of them takes the lighter, which is the same
/// direction the class model rounded and keeps a hypothetical 550 from measuring
/// as bold.
fn weight_index(font_weight: u32) -> usize {
    table::WEIGHTS
        .iter()
        .rposition(|w| font_weight >= *w)
        .unwrap_or(0)
}

/// One glyph's advance in em units, from the font, or `None` when the table has
/// no answer for it.
fn advance_em(c: char, weight: usize) -> Option<f64> {
    let row = table::INTER
        .binary_search_by_key(&c, |(glyph, _)| *glyph)
        .ok()?;
    let widths = table::INTER.get(row)?.1;
    Some(f64::from(*widths.get(weight)?) / table::UPEM)
}

/// One glyph's width in em units when the font cannot be asked.
///
/// The class model, scaled by the same weight ratio it always used, so a
/// character outside the table measures exactly as it did before the table
/// existed.
fn estimated_em(c: char, font_weight: u32) -> f64 {
    let base = if font_weight >= 600 {
        0.60
    } else if font_weight >= 500 {
        0.57
    } else {
        0.54
    };
    char_width(c) * base
}

/// Rendered width of `text`, in pixels.
///
/// The trailing padding is a margin rather than a correction: an advance already
/// includes its glyph's side bearings, so the sum is the width. It stays because
/// a few glyphs overhang their advance, and because a box a hair too wide is
/// invisible where one a hair too narrow clips.
pub fn text_width(text: &str, font_size: f64, font_weight: u32) -> f64 {
    let weight = weight_index(font_weight);
    let total: f64 = text
        .chars()
        .map(|c| advance_em(c, weight).unwrap_or_else(|| estimated_em(c, font_weight)))
        .sum();
    total * font_size + font_size * 0.15
}

/// Width of a monospace run, where every glyph is one advance wide.
///
/// Read from the table like everything else. `JetBrains Mono` advances 600/1000 of
/// an em at every weight — which is exactly the ratio this used to hard-code, so
/// no monospace text moved when the measurement stopped being a guess.
pub fn mono_text_width(text: &str, font_size: f64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a label long enough to lose precision here would not fit on any canvas"
    )]
    let count = text.chars().count() as f64;
    let advance = f64::from(table::MONO_ADVANCE[0]) / table::MONO_UPEM;
    count * font_size * advance
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Widths on strings out of the real reference diagrams, pinned so a drift
    /// shows up as a failing assertion rather than as a mysterious reflow.
    ///
    /// These are the font's own numbers, not this code's: each was checked by
    /// summing Inter 4.1's `hmtx` advances for the string independently, outside
    /// Rust. `kenn-store` is 10624 units over a 2048 em, which is 5.1875 em, which
    /// at 14px is 72.625 plus the 2.1 margin.
    ///
    /// They replace the character-class model's answers, which were as much as 11%
    /// narrower — `Orchestrates pipeline plus store lifecycle` was measured 192.918
    /// and is really 214.012, so that label had been getting a box a tenth too
    /// small.
    #[test]
    fn matches_the_fonts_own_advances() {
        for (text, size, weight, expected) in [
            ("kenn-store", 14.0, 400, 74.725),
            ("IndexerDriver", 14.0, 600, 94.741),
            ("", 14.0, 400, 2.1),
            ("Structural code graph", 14.0, 400, 146.161),
            ("«Container»", 11.0, 600, 67.817),
            (
                "Orchestrates pipeline plus store lifecycle",
                11.0,
                400,
                214.012,
            ),
        ] {
            let got = text_width(text, size, weight);
            assert!(
                (got - expected).abs() < 0.001,
                "{text:?} at {size}/{weight}: got {got}, font says {expected}"
            );
        }
    }

    #[test]
    fn a_glyph_measures_its_advance_over_the_em() {
        // Inter Regular advances `o` 1228 units over a 2048 em. One `o` at 100px
        // is therefore 59.961px, plus the 15px margin.
        let one = text_width("o", 100.0, 400);
        assert!(
            (one - (1228.0 / 2048.0 * 100.0 + 15.0)).abs() < 1e-9,
            "{one}"
        );
    }

    #[test]
    fn the_weight_buckets_pick_their_own_column() {
        // 400/500/600/700 are the columns; anything between takes the lighter.
        assert_eq!(weight_index(400), 0);
        assert_eq!(weight_index(500), 1);
        assert_eq!(weight_index(600), 2);
        assert_eq!(weight_index(700), 3);
        assert_eq!(
            weight_index(550),
            1,
            "between two weights takes the lighter"
        );
        assert_eq!(weight_index(900), 3, "beyond the table takes the heaviest");
        assert_eq!(weight_index(100), 0, "below the table takes the lightest");
    }

    #[test]
    fn weight_widens_the_run() {
        let light = text_width("Structural code graph", 14.0, 400);
        let medium = text_width("Structural code graph", 14.0, 500);
        let bold = text_width("Structural code graph", 14.0, 600);
        assert!(light < medium);
        assert!(medium < bold);
    }

    #[test]
    fn narrow_and_wide_glyphs_differ() {
        assert!(text_width("lll", 14.0, 400) < text_width("MMM", 14.0, 400));
        assert!(text_width("iii", 14.0, 400) < text_width("ooo", 14.0, 400));
    }

    #[test]
    fn a_space_is_narrower_than_a_letter() {
        assert!(text_width("   ", 14.0, 400) < text_width("ooo", 14.0, 400));
    }

    #[test]
    fn combining_marks_add_nothing() {
        // U+0301 COMBINING ACUTE ACCENT overlays the previous glyph.
        assert!((text_width("e\u{0301}", 14.0, 400) - text_width("e", 14.0, 400)).abs() < 1e-9);
    }

    #[test]
    fn what_the_font_cannot_answer_falls_back_to_the_class_model() {
        // Inter has no CJK, so `中` is measured by class as it always was — wider
        // than a Latin letter, which is the property that matters. It is no longer
        // *exactly* four average glyphs, because the Latin ones it used to be
        // compared against are now the font's own widths rather than the model's.
        let latin = text_width("oo", 14.0, 400);
        assert!(text_width("中中", 14.0, 400) > latin);
        assert!(text_width("한한", 14.0, 400) > latin);
        assert!(text_width("😀😀", 14.0, 400) > latin, "emoji planes too");
        // And the fallback is the class model exactly: fullwidth is 2.0 units at
        // the 0.54 ratio for weight 400.
        let cjk = text_width("中", 14.0, 400) - text_width("", 14.0, 400);
        assert!((cjk - 2.0 * 0.54 * 14.0).abs() < 1e-9, "{cjk}");
    }

    #[test]
    fn a_script_the_table_skips_takes_the_average_at_its_own_weight() {
        // Cyrillic is neither in the table nor double-width, so it lands on the
        // average — and the weight ratio still applies to it, which is the only
        // part of the old model that still does any work.
        let at = |w| text_width("д", 100.0, w) - text_width("", 100.0, w);
        assert!((at(400) - 54.0).abs() < 1e-9, "{}", at(400));
        assert!((at(500) - 57.0).abs() < 1e-9, "{}", at(500));
        assert!((at(600) - 60.0).abs() < 1e-9, "{}", at(600));
        assert!((at(700) - 60.0).abs() < 1e-9, "700 shares 600's ratio");
    }

    #[test]
    fn a_symbol_the_font_does_have_comes_from_the_font() {
        // U+2605 BLACK STAR is in Inter at 2140/2048 em, so it is measured rather
        // than guessed at the pictographic double width.
        let star = text_width("★", 100.0, 400) - text_width("", 100.0, 400);
        assert!((star - 2140.0 / 2048.0 * 100.0).abs() < 1e-9, "{star}");
    }

    #[test]
    fn digits_and_uppercase_take_the_fonts_widths() {
        // In Inter a digit is *wider* than an `o` (1292 vs 1228 units) — the class
        // model called them equal, which is the kind of error this table removes.
        assert!(text_width("000", 14.0, 400) > text_width("ooo", 14.0, 400));
        assert!(text_width("AAA", 14.0, 400) > text_width("aaa", 14.0, 400));
    }

    #[test]
    fn punctuation_sits_between_narrow_and_average() {
        let narrow = text_width("...", 14.0, 400);
        let punct = text_width("(((", 14.0, 400);
        let average = text_width("ooo", 14.0, 400);
        assert!(narrow < punct);
        assert!(punct < average);
    }

    #[test]
    fn r_is_semi_narrow() {
        assert!(text_width("rrr", 14.0, 400) < text_width("ooo", 14.0, 400));
        assert!(text_width("rrr", 14.0, 400) > text_width("iii", 14.0, 400));
    }

    #[test]
    fn monospace_is_uniform() {
        assert!((mono_text_width("iiii", 11.0) - mono_text_width("MMMM", 11.0)).abs() < 1e-9);
        assert!((mono_text_width("abc", 10.0) - 18.0).abs() < 1e-9);
        assert!((mono_text_width("", 10.0) - 0.0).abs() < 1e-9);
    }
}
