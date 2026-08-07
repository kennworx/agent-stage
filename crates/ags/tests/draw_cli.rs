//! `ags draw` as a process, because some of it only exists as one.
//!
//! Reading standard input and writing standard output cannot be exercised by
//! calling a function: the stream has to be real, attached to something, and
//! closed by someone. So this spawns the built binary and pipes to it.
//!
//! `CARGO_BIN_EXE_ags` is the binary this test was compiled alongside, so there is
//! no path to guess and no chance of testing yesterday's build.

use std::io::Write as _;
use std::process::{Command, Stdio};

const AGS: &str = env!("CARGO_BIN_EXE_ags");

/// Run `ags draw` with `args`, feeding `stdin`, and return (status, stdout, stderr).
fn draw(args: &[&str], stdin: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(AGS)
        .arg("draw")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary this test was built with is runnable");
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(stdin.as_bytes())
        .expect("the child accepts its input");
    let done = child.wait_with_output().expect("the child finishes");
    (
        done.status.code(),
        String::from_utf8_lossy(&done.stdout).into_owned(),
        String::from_utf8_lossy(&done.stderr).into_owned(),
    )
}

#[test]
fn a_diagram_piped_in_comes_back_as_an_svg_on_stdout() {
    let (code, out, err) = draw(&[], "graph TD\n  A[Start] --> B[Finish]\n");
    assert_eq!(code, Some(0), "stderr: {err}");
    assert!(out.starts_with("<svg"), "{out}");
    assert!(out.contains("Start") && out.contains("Finish"), "{out}");
}

#[test]
fn a_piped_diagram_carries_literal_colours_because_it_has_no_page() {
    // The default is `Fixed`: there is no document behind a piped drawing, so a
    // token reference would resolve to nothing and the SVG would render black.
    let (code, out, _) = draw(&[], "pie title Shares\n\"a\" : 10\n\"b\" : 5\n");
    assert_eq!(code, Some(0));
    assert!(!out.contains("var(--"), "no reference may survive: {out}");
}

#[test]
fn tokens_asks_for_the_opposite_and_gets_it() {
    let (code, out, _) = draw(&["--tokens"], "pie title Shares\n\"a\" : 10\n\"b\" : 5\n");
    assert_eq!(code, Some(0));
    assert!(
        out.contains("var(--"),
        "a page-bound drawing keeps its references"
    );
}

#[test]
fn a_check_that_finds_something_reports_it_and_fails() {
    // Every node on one row joined to every node on the next. In a layered
    // drawing every pair of edges whose endpoints invert has to cross, and with
    // three by three that is nine, in any order the rows are put in. A drawing
    // that renders perfectly well and reads wrong, which is the point of the
    // check.
    //
    // Two real diagrams held this role before it and the engine fixed them both:
    // the `ci` subgraph enclosing a node not in it, then a state machine whose
    // transitions crossed.
    let source = "graph TD\n  A1 --> B1\n  A1 --> B2\n  A1 --> B3\n  A2 --> B1\n  A2 --> B2\n  A2 --> B3\n  A3 --> B1\n  A3 --> B2\n  A3 --> B3\n";
    let (code, out, err) = draw(&["--check"], source);
    assert_eq!(code, Some(1), "a finding must fail the command");
    assert!(out.is_empty(), "a check draws nothing: {out}");
    assert!(err.contains("cross"), "{err}");
}

#[test]
fn a_clean_check_says_nothing_and_succeeds() {
    let (code, out, err) = draw(&["--check"], "graph TD\n  A-->B\n");
    assert_eq!(code, Some(0), "stderr: {err}");
    assert!(out.is_empty() && err.is_empty(), "out={out} err={err}");
}

#[test]
fn a_source_that_names_no_diagram_fails_with_the_reason() {
    let (code, _, err) = draw(&[], "sunburstChart\n  a: 1\n");
    assert_eq!(code, Some(1));
    assert!(err.contains("ags draw:"), "prefixed by the command: {err}");
    assert!(err.contains("unknown diagram type"), "{err}");
}

#[test]
fn an_empty_pipe_is_an_error_rather_than_an_empty_drawing() {
    let (code, out, err) = draw(&[], "");
    assert_eq!(code, Some(1));
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("empty diagram source"), "{err}");
}
