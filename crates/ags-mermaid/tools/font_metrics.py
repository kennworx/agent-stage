#!/usr/bin/env python3
"""Generate `src/metrics/table.rs` — advance widths read from the real fonts.

The renderer measures text to size every box, and the box sizes the canvas. That
measurement used to be a character-class estimate ("wide lowercase is 1.2 average
glyphs"), which is deterministic but wrong by enough to matter. This reads the
advance widths out of the fonts the diagrams actually declare, so the measurement
is the font's own answer.

Read at build time rather than at run time, and committed as a table, because
`ags-mermaid` is deliberately dependency-free and is compiled to WebAssembly: a
font parser plus 800 KB of font in every build is a high price for numbers that
never change between releases. The table is a few kilobytes and needs no parser.

    tmp/fontenv/bin/python crates/ags-mermaid/tools/font_metrics.py \
        --inter tmp/fonts/inter/extras/ttf \
        --mono  tmp/fonts/jb/fonts/ttf \
        --out   crates/ags-mermaid/src/metrics/table.rs

Requires fontTools (dev-time only; nothing at run time).
"""

import argparse
import pathlib
import sys

from fontTools.ttLib import TTFont

# The four weights the renderer draws at. Index order is load-bearing: the Rust
# side indexes this array by weight bucket.
WEIGHTS = [400, 500, 600, 700]
INTER_FACES = {400: "Regular", 500: "Medium", 600: "SemiBold", 700: "Bold"}
MONO_FACES = {400: "Regular", 500: "Medium", 600: "SemiBold", 700: "Bold"}


def covered() -> list[int]:
    """The codepoints the table carries.

    Latin and the punctuation a diagram actually uses. Everything outside this
    falls back to the character-class model, which is what already handles the
    ranges no Latin font could answer for anyway — CJK, Hangul, emoji.
    """
    points: list[int] = []
    points += range(0x20, 0x7F)  # ASCII printable
    points += range(0xA0, 0x100)  # Latin-1 supplement
    points += range(0x2010, 0x2028)  # dashes, quotes, ellipsis, dagger
    points += [0x2039, 0x203A, 0x2044, 0x20AC]  # single guillemets, fraction slash, euro
    points += range(0x2190, 0x21B0)  # arrows
    points += [0x2212, 0x2260, 0x2264, 0x2265, 0x2248]  # minus, comparisons
    points += range(0x2500, 0x2580)  # box drawing
    points += range(0x25A0, 0x25D0)  # geometric shapes
    points += [0x2605, 0x2606, 0x2610, 0x2611, 0x2713, 0x2714, 0x2717, 0x2718, 0x26A0]
    return sorted(set(points))


def advances(path: pathlib.Path) -> tuple[int, dict[int, int]]:
    """Every codepoint's advance width in font units, plus the units-per-em."""
    font = TTFont(str(path), lazy=True)
    upem = font["head"].unitsPerEm
    cmap = font.getBestCmap()
    hmtx = font["hmtx"]
    out: dict[int, int] = {}
    for code, name in cmap.items():
        if name in hmtx.metrics:
            out[code] = hmtx.metrics[name][0]
    font.close()
    return upem, out


def face(directory: pathlib.Path, family: str, style: str) -> pathlib.Path:
    path = directory / f"{family}-{style}.ttf"
    if not path.exists():
        sys.exit(f"missing {path}")
    return path


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--inter", required=True, type=pathlib.Path)
    ap.add_argument("--mono", required=True, type=pathlib.Path)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    ap.add_argument("--inter-version", default="unknown")
    ap.add_argument("--mono-version", default="unknown")
    args = ap.parse_args()

    inter_upem = None
    inter: dict[int, list[int]] = {}
    for i, weight in enumerate(WEIGHTS):
        upem, table = advances(face(args.inter, "Inter", INTER_FACES[weight]))
        if inter_upem is None:
            inter_upem = upem
        elif upem != inter_upem:
            sys.exit(f"Inter {weight} has upem {upem}, expected {inter_upem}")
        for code in covered():
            if code in table:
                inter.setdefault(code, [0] * len(WEIGHTS))[i] = table[code]

    # Drop any codepoint a weight is missing: a partial row would measure one
    # weight as zero-width, which is worse than falling back to the estimate.
    inter = {c: w for c, w in inter.items() if all(v > 0 for v in w)}

    mono_upem = None
    mono_advance: list[int] = []
    for weight in WEIGHTS:
        upem, table = advances(face(args.mono, "JetBrainsMono", MONO_FACES[weight]))
        if mono_upem is None:
            mono_upem = upem
        widths = {table[c] for c in covered() if c in table}
        if len(widths) != 1:
            sys.exit(f"JetBrains Mono {weight} is not monospaced over the covered set: {sorted(widths)}")
        mono_advance.append(widths.pop())

    rows = "\n".join(
        f"    ('\\u{{{c:04x}}}', [{', '.join(str(v) for v in inter[c])}]),"
        for c in sorted(inter)
    )
    args.out.write_text(f'''//! Advance widths read from the fonts the diagrams declare.
//!
//! **Generated — do not edit.** Regenerate with `tools/font_metrics.py`; see that
//! script for why the numbers are baked in rather than read from a font at run
//! time. Sources: Inter {args.inter_version} and `JetBrains Mono` {args.mono_version},
//! both SIL Open Font License 1.1.
//!
//! Widths are in font units and are divided by [`UPEM`] before use, so they scale
//! to any font size without carrying a float per glyph.

/// Weights the table carries, in the order each row's array is indexed.
pub(super) const WEIGHTS: [u32; {len(WEIGHTS)}] = {WEIGHTS};

/// Inter's units per em.
pub(super) const UPEM: f64 = {inter_upem}.0;

/// `JetBrains Mono`'s units per em.
pub(super) const MONO_UPEM: f64 = {mono_upem}.0;

/// Every glyph's advance in `JetBrains Mono`, by weight — one number per weight,
/// because the face is monospaced and the generator refuses to emit this if it
/// ever stops being.
pub(super) const MONO_ADVANCE: [u16; {len(WEIGHTS)}] = {mono_advance};

/// Advance width per codepoint, sorted by codepoint so lookup can bisect.
pub(super) const INTER: &[(char, [u16; {len(WEIGHTS)}])] = &[
{rows}
];
''')
    print(f"wrote {args.out}: {len(inter)} codepoints x {len(WEIGHTS)} weights, "
          f"inter upem={inter_upem}, mono upem={mono_upem}, mono advances={mono_advance}")


if __name__ == "__main__":
    main()
