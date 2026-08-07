//! Detecting box-drawing art inside a code block.
//!
//! ASCII and Unicode box art only reads correctly when consecutive lines *tile*
//! — the vertical strokes of `│` must touch across the line boundary. That needs
//! tighter leading than ordinary code wants, so the two cases are told apart by
//! content rather than by asking the author to declare it.

/// Whether `source` contains Unicode box-drawing or block-element characters.
///
/// `U+2500–U+257F` is Box Drawing (`│ ─ ┌ ┐ └ ┘ ├ ┼ …`) and `U+2580–U+259F` is
/// Block Elements (`█ ▀ ▄ ░ ▒ …`); between them they cover every glyph that has
/// to meet its neighbour above or below. Arrows and dashes are deliberately
/// excluded: they sit inside the line box and need no special leading.
pub fn has_box_drawing(source: &str) -> bool {
    source
        .chars()
        .any(|c| matches!(c, '\u{2500}'..='\u{257f}' | '\u{2580}'..='\u{259f}'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drawn_box_is_recognised() {
        assert!(has_box_drawing("┌───┐\n│ a │\n└───┘"));
        assert!(has_box_drawing("progress ████░░░░"));
    }

    #[test]
    fn ordinary_code_is_not() {
        assert!(!has_box_drawing("fn main() { println!(\"hi\"); }"));
        assert!(!has_box_drawing(""));
    }

    #[test]
    fn glyphs_that_sit_inside_the_line_box_do_not_count() {
        // An arrow or a dash needs no special leading, so it must not tighten
        // the whole block.
        assert!(!has_box_drawing("a → b — c ✓"));
        assert!(!has_box_drawing("+---+\n| a |\n+---+"));
    }
}
