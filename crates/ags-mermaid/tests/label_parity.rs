//! Label normalisation, pinned against the renderer this replaces.
//!
//! The transformations are hand-rolled — one of the rules uses a lookbehind that
//! Rust's `regex` cannot express, and a regex engine is a heavy passenger in a
//! WebAssembly build that needs it for four substitutions. Hand-rolling means
//! the parity has to be demonstrated rather than assumed, so these are the
//! reference implementation's own outputs.
//!
//! `*a**b*` is here because it is the case that actually diverged: without the
//! lookbehind, the `*` at index 3 reads as an opener and the label becomes
//! `*a*<i>b</i>`. Every unit test in the module passed while that was wrong.
use ags_mermaid::{normalize_label, strip_formatting_tags};
#[test]
fn matches_reference() {
    let cases: &[(&str, &str)] = &[
        ("\"quoted\"", "quoted"),
        ("\"say \"hi\"\"", "say \"hi\""),
        ("\"unbalanced", "\"unbalanced"),
        ("a<br>b", "a\nb"),
        ("a<br/>b", "a\nb"),
        ("a<br />b", "a\nb"),
        ("a<BR>b", "a\nb"),
        ("a\\nb", "a\nb"),
        ("H<sub>2</sub>O", "H2O"),
        ("<mark>hot</mark>", "hot"),
        ("**bold**", "<b>bold</b>"),
        ("*italic*", "<i>italic</i>"),
        ("~~gone~~", "<s>gone</s>"),
        ("a **b** c", "a <b>b</b> c"),
        ("2 * 3", "2 * 3"),
        ("* item", "* item"),
        ("a * b * c", "a * b * c"),
        ("**x**", "<b>x</b>"),
        ("plain", "plain"),
        ("", ""),
        ("a<b>b</b>c", "a<b>b</b>c"),
        ("*a**b*", "*a**b*"),
        ("kenn-indexer::workflow", "kenn-indexer::workflow"),
        (
            "**bold** and *italic* and ~~struck~~",
            "<b>bold</b> and <i>italic</i> and <s>struck</s>",
        ),
        ("a*b", "a*b"),
        ("*", "*"),
        ("**", "**"),
        ("x <small>y</small> z", "x y z"),
    ];
    let mut bad = vec![];
    for (input, expected) in cases {
        let got = normalize_label(input);
        if got != *expected {
            bad.push(format!("{input:?}: got {got:?}, want {expected:?}"));
        }
    }
    for (input, expected) in [
        ("<b>bold</b> text", "bold text"),
        ("<i>a</i><s>b</s>", "ab"),
        ("<span>x</span>", "<span>x</span>"),
        ("<STRONG>y</STRONG>", "y"),
    ] {
        let got = strip_formatting_tags(input);
        if got != expected {
            bad.push(format!("strip {input:?}: got {got:?}, want {expected:?}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} divergences:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
