//! The `present` command — Gate 1.
//!
//! Validates an agent-authored artifact and, on failure, emits a TOON error
//! collection to stdout and exits non-zero **without serving**. On success it
//! exits zero. Serving the page + engine bundle and opening the browser are
//! deferred to the render slice (inbrowser §1/§3); this command is the
//! engine-free structural gate only. `run` is a thin shell over the pure [`gate`]
//! core, which stays free of any I/O.

use std::process::ExitCode;

use ags_render::{errors_to_toon, validate_source};

/// What the gate decided: the text to print and the process exit code.
pub struct Outcome {
    /// Text to write to stdout (TOON errors on failure; empty on success).
    pub stdout: String,
    /// Process exit code: `0` clean, `1` validation failed.
    pub code: u8,
}

/// The pure Gate-1 decision over an artifact's source text. On validation failure
/// it returns the TOON error collection and a non-zero code; on success, empty
/// output and `0`. No I/O — this is the unit-testable core of `present`.
#[must_use]
pub fn gate(source: &str) -> Outcome {
    let errors = validate_source(source);
    if errors.is_empty() {
        Outcome {
            stdout: String::new(),
            code: 0,
        }
    } else {
        Outcome {
            stdout: errors_to_toon(&errors),
            code: 1,
        }
    }
}

/// Run `present [--check] [--fresh] <file>`: validate; on failure emit TOON and
/// exit without serving. On success, serve the artifact for in-browser review —
/// unless `--check`, which validates only (the agent's fast pre-check / CI path).
/// `--fresh` discards any existing feedback log so the review starts from scratch.
pub fn run(path: &str, check: bool, port: Option<u16>, fresh: bool) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("ags: cannot read {path}: {err}");
            return ExitCode::from(2);
        }
    };
    let outcome = gate(&source);
    if !outcome.stdout.is_empty() {
        println!("{}", outcome.stdout);
    }
    if outcome.code != 0 {
        return ExitCode::from(outcome.code);
    }
    if check {
        return ExitCode::SUCCESS;
    }
    match crate::serve::serve(std::path::Path::new(path), &source, port, fresh) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("ags: serve failed: {err}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_artifact_gates_clean() {
        let src = "```code #c lang=rust\nfn main() {}\n```";
        let outcome = gate(src);
        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn invalid_artifact_emits_toon_and_nonzero() {
        // A mistyped block type: `mermiad` is prose as far as the renderer is
        // concerned, so the near-miss gate is what stops it reaching a reviewer.
        let outcome = gate("```mermiad\ngraph TD\n```");
        assert_eq!(outcome.code, 1);
        assert!(outcome.stdout.starts_with("errors[1]{id,kind,detail}:"));
        assert!(outcome.stdout.contains("near-miss-type"));
    }

    #[test]
    fn an_unrecognized_language_fence_gates_clean() {
        // The reclassification: a plain code fence is prose, not a failure.
        let outcome = gate("intro\n\n```rust\nfn main() {}\n```");
        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.is_empty());
    }

    #[test]
    fn unsafe_html_chunk_gates_nonzero() {
        let outcome = gate("```html #h\n<div><script>x()</script></div>\n```");
        assert_eq!(outcome.code, 1);
        assert!(outcome.stdout.contains("html-script"));
    }
}
