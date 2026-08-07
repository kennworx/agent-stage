//! Serialize a Gate-1 error collection as TOON via the `toon-rs` encoder.
//!
//! [`ValidationError`] carries the output columns directly (`anchor` serialized as
//! `id`, `kind` as a kebab token, `detail`), so wrapping the slice under an
//! `errors` key produces the tabular form the agent reads:
//!
//! ```text
//! errors[2]{id,kind,detail}:
//!   #flow,unknown-attr,"'type' is not a valid attribute for a 'mermaid' block"
//!   #t,table-arity,row 3 has 2 cells but the header has 3
//! ```

use serde::Serialize;

use crate::block::ValidationError;

/// A TOON document wrapping the error rows under the `errors` key.
#[derive(Serialize)]
struct ErrorsDoc<'a> {
    errors: &'a [ValidationError],
}

/// Encode validation errors as a TOON `errors[N]{id,kind,detail}:` table. Only
/// called with at least one error (a clean gate prints nothing).
#[must_use]
pub fn errors_to_toon(errors: &[ValidationError]) -> String {
    toon_rs::encode_to_string(&ErrorsDoc { errors }, &toon_rs::Options::default())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::ValidationKind;

    #[test]
    fn rows_carry_id_kind_detail_with_quoting() {
        let errs = [
            ValidationError::new(
                "#c",
                ValidationKind::EmptyBody,
                "a 'code' block needs a non-empty body",
            ),
            ValidationError::new(
                "#m",
                ValidationKind::BadAttrValue,
                "'direction=XX' is not one of [TD, LR, BT, RL]",
            ),
        ];
        let toon = errors_to_toon(&errs);
        assert_eq!(
            toon,
            "errors[2]{id,kind,detail}:\n  \
             #c,empty-body,a 'code' block needs a non-empty body\n  \
             #m,bad-attr-value,\"'direction=XX' is not one of [TD, LR, BT, RL]\""
        );
    }

    #[test]
    fn single_error_encodes() {
        let toon = errors_to_toon(&[ValidationError::new(
            "#x",
            ValidationKind::UnknownType,
            "nope",
        )]);
        assert!(toon.starts_with("errors[1]{id,kind,detail}:"));
        assert!(toon.contains("#x,unknown-type,nope"));
    }
}
