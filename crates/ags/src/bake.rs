//! The `bake` command — write the artifact out as one standalone page.
//!
//! Validates the artifact (Gate 1), then renders it with the same code `serve`
//! uses. What comes out is finished markup: no bundle, no script, nothing to
//! fetch, so it opens from a file with no server and no network.
//!
//! There is no shell to choose any more. `--asset-base` referenced a ~450 KB
//! viewer from a stable URL so it could be cached across artifacts, and
//! `--inline` embedded it; with a self-contained page of a few kilobytes there is
//! nothing left to amortise and nothing to embed. Both flags are gone, and with
//! them the URL validation, the second CSP shape and the mutual exclusion between
//! them.
//!
//! Safety: a served page gets its boundary from an HTTP CSP header (see
//! [`crate::serve`]); a baked file has no server, so the boundary rides in a
//! `<meta http-equiv="Content-Security-Policy">` that the renderer emits. It is
//! stricter than the one it replaces, because a page carrying no script can
//! forbid script outright.

use std::process::ExitCode;

/// Run `bake <file> [--out <file>]`: validate (Gate 1), then write the page to
/// `--out` or stdout. Gate-1 errors go to **stderr**, so a redirected stdout holds
/// the page or nothing.
pub fn run(file: &str, out: Option<&str>) -> ExitCode {
    let source = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("ags: cannot read {file}: {err}");
            return ExitCode::from(2);
        }
    };
    let outcome = crate::present::gate(&source);
    if outcome.code != 0 {
        eprintln!("{}", outcome.stdout);
        return ExitCode::from(outcome.code);
    }
    let html = ags_render::bake_named(
        &source,
        &crate::serve::display_name(std::path::Path::new(file)),
    );
    match out {
        Some(path) => {
            if let Err(err) = std::fs::write(path, html) {
                eprintln!("ags: cannot write {path}: {err}");
                return ExitCode::from(2);
            }
        }
        None => print!("{html}"),
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_reported_rather_than_baked() {
        assert_eq!(run("/no/such/artifact/xyzzy.md", None), ExitCode::from(2));
    }

    #[test]
    fn an_artifact_that_fails_gate_one_is_not_written() {
        let dir = std::env::temp_dir().join("ags-bake-test");
        std::fs::create_dir_all(&dir).expect("a temp dir");
        let bad = dir.join("bad.md");
        // Two blocks sharing an id — a Gate-1 failure, not a parse error.
        std::fs::write(&bad, "```note #a\nx\n```\n\n```note #a\ny\n```\n")
            .expect("write the fixture");
        let out = dir.join("out.html");
        let code = run(&bad.display().to_string(), Some(&out.display().to_string()));
        assert_eq!(code, ExitCode::from(1));
        assert!(!out.exists(), "a rejected artifact was written anyway");
    }

    #[test]
    fn a_valid_artifact_is_written_where_it_was_asked_for() {
        let dir = std::env::temp_dir().join("ags-bake-test");
        std::fs::create_dir_all(&dir).expect("a temp dir");
        let good = dir.join("good.md");
        std::fs::write(&good, "# Title\n\nWords.\n").expect("write the fixture");
        let out = dir.join("good.html");
        let code = run(
            &good.display().to_string(),
            Some(&out.display().to_string()),
        );
        assert_eq!(code, ExitCode::SUCCESS);
        let html = std::fs::read_to_string(&out).expect("the baked page");
        // Rendered, not a mount point for a bundle to fill in.
        assert!(html.contains("<title>ags · good.md</title>"), "{html}");
        assert!(html.contains("Words."), "{html}");
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("viewer.js"), "{html}");
    }
}
