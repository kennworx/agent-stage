//! Catching a fence language that was nearly right.
//!
//! A prose fence tagged `mermiad` is a typo, not a new block type, and saying so
//! is worth more than refusing it as unknown. One edit apart is the whole test:
//! anything looser starts renaming blocks the author meant.

use crate::block::{ValidationError, ValidationKind, BLOCK_TYPES};
use crate::parse::ProseFence;

/// maintain against no benefit. Re-run the sweep when [`BLOCK_TYPES`] changes.
pub(super) const FENCE_LANGUAGES: &[&str] = &["haml", "htm", "html5", "node", "none", "xhtml"];

/// Flag a prose fence whose type is one edit away from a block type.
///
/// An unrecognized fence is legitimate prose, so this is the only thing standing
/// between a mistyped ` ```mermiad ` and a diagram silently rendering as a grey
/// code block. Distance 1 catches the realistic slips — a transposition, a
/// dropped character, a doubled one — with [`FENCE_LANGUAGES`] excluded, because
/// a real language tag that happens to land within one edit is not a typo and
/// refusing it would reject a valid artifact.
pub(super) fn near_miss_error(fence: &ProseFence) -> Option<ValidationError> {
    if FENCE_LANGUAGES.contains(&fence.type_token.as_str()) {
        return None;
    }
    let suggestion = BLOCK_TYPES
        .iter()
        .find(|known| is_one_edit_apart(&fence.type_token, known))?;
    Some(ValidationError::new(
        format!("{}@{}", fence.type_token, fence.line),
        ValidationKind::NearMissType,
        format!(
            "'{}' is not a block type — did you mean '{}'? (an unrecognized fence \
             renders as plain code, with no id or review affordance)",
            fence.type_token, suggestion
        ),
    ))
}

/// Whether `a` and `b` are one edit apart: a single insertion, deletion,
/// substitution, or transposition of two adjacent characters.
///
/// The transposition case is **load-bearing**, not a refinement. `mermiad` — the
/// canonical typo — is two plain-Levenshtein edits away from `mermaid` (two
/// substitutions), so a plain edit-distance check would miss the single most
/// likely mistake this whole rule exists to catch. This is Damerau-Levenshtein.
///
/// A bounded check rather than a full distance matrix: the length gap decides
/// which shapes are possible, so it stays linear.
pub(super) fn is_one_edit_apart(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    match a.len().abs_diff(b.len()) {
        0 => differs_by_one_substitution(&a, &b) || is_one_transposition(&a, &b),
        1 if a.len() > b.len() => is_one_insertion(&b, &a),
        1 => is_one_insertion(&a, &b),
        _ => false,
    }
}

/// Whether two equal-length sequences differ at exactly one position.
pub(super) fn differs_by_one_substitution(a: &[char], b: &[char]) -> bool {
    a.iter().zip(b).filter(|(x, y)| x != y).count() == 1
}

/// Whether two equal-length sequences differ only by swapping one adjacent pair
/// (`mermiad` ↔ `mermaid`).
pub(super) fn is_one_transposition(a: &[char], b: &[char]) -> bool {
    let mut diffs = a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y);
    // Exactly two disagreements and no third.
    let (Some((i, (ai, bi))), Some((j, (aj, bj))), None) =
        (diffs.next(), diffs.next(), diffs.next())
    else {
        return false;
    };
    j == i + 1 && ai == bj && aj == bi
}

/// Whether `long` is `short` with exactly one character inserted (`long` is one
/// longer than `short`).
pub(super) fn is_one_insertion(short: &[char], long: &[char]) -> bool {
    // Find where they first diverge; if they never do, `long` is `short` plus a
    // trailing character, which is an insertion.
    let Some(at) = long.iter().zip(short).position(|(l, s)| l != s) else {
        return true;
    };
    // Drop the inserted character and the remainder must match exactly.
    long.get(at + 1..) == short.get(at..)
}
