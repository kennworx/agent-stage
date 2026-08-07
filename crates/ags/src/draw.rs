//! `ags draw` — a diagram on its own, outside any artifact.
//!
//! The third render target, after the served page and the baked page. This output
//! has no document to supply theme tokens, so it carries literal colours by
//! default — which is what makes it embeddable somewhere that is not our own page.
//!
//! This was a second binary, `ags-mermaid`. It is a subcommand now: it shared the
//! renderer, the theme vocabulary and the colour modes with `ags` and differed only
//! in what it wrote at the end, so shipping two executables asked a user to know
//! which one drew diagrams.

use std::io::Read as _;
use std::process::ExitCode;

/// What `ags draw` was asked for.
pub struct Args {
    /// Diagram source file; standard input when absent.
    pub input: Option<String>,
    /// Write here instead of standard output.
    pub out: Option<String>,
    /// Reference theme tokens instead of writing literal colours, for an SVG that
    /// will be embedded in a page and themed by it.
    pub tokens: bool,
    /// Report what the drawing gets wrong and draw nothing.
    pub check: bool,
}

/// Choose between a named file and the stream, and report either failure with the
/// name of what it was reading.
///
/// The stream is injected so both arms can be exercised. Reading real standard
/// input in a test either blocks on a terminal or depends on how the harness was
/// launched, so the branch would go untested exactly where a mistake is invisible
/// — the pipe case is the one nobody runs by hand.
fn source_from(
    input: Option<&str>,
    stream: impl FnOnce() -> std::io::Result<String>,
) -> Result<String, String> {
    match input {
        Some(path) => std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}")),
        None => stream().map_err(|e| format!("stdin: {e}")),
    }
}

/// Read the source from a file, or from standard input when none is named.
fn read_source(input: Option<&str>) -> Result<String, String> {
    source_from(input, || {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    })
}

/// Choose between a named file and the stream, as [`source_from`] does and for the
/// same reason.
fn write_to(out: Option<&str>, body: &str, stream: impl FnOnce(&str)) -> Result<(), String> {
    if let Some(path) = out {
        return std::fs::write(path, body).map_err(|e| format!("{path}: {e}"));
    }
    stream(body);
    Ok(())
}

/// Write rendered output to a file, or to standard output when none is named.
fn write_output(out: Option<&str>, body: &str) -> Result<(), String> {
    write_to(out, body, |body| print!("{body}"))
}

/// Report what a drawing gets wrong, one violation to a line.
///
/// The layered types have no reference to be diffed against — a layout has no
/// single right answer — so this is what stands in its place: not "the same as
/// before" but "nothing a reader would trip over".
fn check(source: &str) -> Result<Vec<String>, String> {
    if source.trim().is_empty() {
        return Err("empty diagram source".to_string());
    }
    let rendered = ags_mermaid::render_svg(source, &ags_mermaid::Options::default())
        .map_err(|err| err.to_string())?;
    Ok(rendered
        .violations
        .iter()
        .map(ToString::to_string)
        .collect())
}

/// Render `source`, or explain why it cannot be rendered.
///
/// Literal colours by default, because a standalone image has no page behind
/// it and a token reference would resolve to nothing. `--tokens` is for the
/// other case: an SVG going into a page that will theme it, where baked colours
/// are exactly what stops the theme from reaching it.
fn render(source: &str, colors: ags_mermaid::ColorMode) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("empty diagram source".to_string());
    }
    let options = ags_mermaid::Options {
        colors,
        ..ags_mermaid::Options::default()
    };
    ags_mermaid::render_svg(source, &options)
        .map(|rendered| rendered.svg)
        .map_err(|err| err.to_string())
}

/// What the arguments asked for, once the source has been read.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// A drawing, ready to be written wherever it was asked for.
    Drawn(String),
    /// What the check found, empty when there was nothing to report.
    Checked(Vec<String>),
}

/// Decide what to make of a source. Pure, so the decision is testable and only
/// the reading and writing around it are not.
fn produce(source: &str, args: &Args) -> Result<Outcome, String> {
    if args.check {
        return check(source).map(Outcome::Checked);
    }
    let colors = if args.tokens {
        ags_mermaid::ColorMode::Tokens
    } else {
        ags_mermaid::ColorMode::Fixed
    };
    render(source, colors).map(Outcome::Drawn)
}

/// Do what the arguments asked for.
///
/// `None` means a drawing was written; `Some` is what the check found.
fn run(args: &Args) -> Result<Option<Vec<String>>, String> {
    let source = read_source(args.input.as_deref())?;
    match produce(&source, args)? {
        Outcome::Checked(found) => Ok(Some(found)),
        Outcome::Drawn(svg) => write_output(args.out.as_deref(), &svg).map(|()| None),
    }
}

pub fn main(args: &Args) -> ExitCode {
    match run(args) {
        Ok(found) if found.as_ref().is_none_or(Vec::is_empty) => ExitCode::SUCCESS,
        Ok(found) => {
            for violation in found.unwrap_or_default() {
                eprintln!("{violation}");
            }
            ExitCode::FAILURE
        }
        Err(message) => {
            eprintln!("ags draw: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_reported_with_its_name() {
        let err = read_source(Some("/nonexistent/diagram.mmd")).unwrap_err();
        assert!(err.contains("/nonexistent/diagram.mmd"), "{err}");
    }

    #[test]
    fn a_readable_file_is_returned_verbatim() {
        let dir = std::env::temp_dir().join("ags-draw-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("d.mmd");
        std::fs::write(&path, "pie title X").unwrap();
        let got = read_source(Some(path.to_str().unwrap())).unwrap();
        assert_eq!(got, "pie title X");
    }

    #[test]
    fn writing_to_a_named_file_puts_the_body_there() {
        let dir = std::env::temp_dir().join("ags-draw-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("out.svg");
        write_output(Some(path.to_str().unwrap()), "<svg/>").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<svg/>");
    }

    #[test]
    fn writing_to_an_unwritable_path_is_reported() {
        let err = write_output(Some("/nonexistent/dir/out.svg"), "<svg/>").unwrap_err();
        assert!(err.contains("out.svg"), "{err}");
    }

    #[test]
    fn the_stream_is_used_only_when_no_file_is_named() {
        // Both arms, without touching real standard input — which in a test either
        // blocks on a terminal or depends on how the harness was launched.
        assert_eq!(
            source_from(None, || Ok("from the pipe".to_string())).unwrap(),
            "from the pipe"
        );
        let err = source_from(None, || Err(std::io::Error::other("pipe closed"))).unwrap_err();
        assert!(err.starts_with("stdin: "), "{err}");

        let mut written = String::new();
        write_to(None, "drawn", |body| written.push_str(body)).unwrap();
        assert_eq!(written, "drawn");
    }

    #[test]
    fn a_named_file_wins_over_the_stream() {
        let dir = std::env::temp_dir().join("ags-draw-test-seams");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("in.mmd");
        std::fs::write(&path, "pie title X").unwrap();
        let name = path.to_string_lossy().to_string();

        let untouched = source_from(Some(&name), || panic!("the stream must not be read")).unwrap();
        assert_eq!(untouched, "pie title X");

        let out = dir.join("out.svg");
        let out_name = out.to_string_lossy().to_string();
        write_to(Some(&out_name), "body", |_| {
            panic!("the stream must not be written")
        })
        .unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "body");
    }

    #[test]
    fn a_drawing_is_written_and_a_clean_check_reports_nothing() {
        let dir = std::env::temp_dir().join("ags-draw-test-run");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("d.mmd");
        std::fs::write(&src, "graph TD\n  A-->B\n").unwrap();
        let out = dir.join("d.svg");

        let drawn = Args {
            input: Some(src.to_string_lossy().to_string()),
            out: Some(out.to_string_lossy().to_string()),
            tokens: false,
            check: false,
        };
        assert_eq!(run(&drawn).unwrap(), None, "a drawing reports nothing");
        assert!(std::fs::read_to_string(&out).unwrap().starts_with("<svg"));
        assert_eq!(main(&drawn), ExitCode::SUCCESS);

        let checked = Args {
            check: true,
            ..drawn
        };
        assert_eq!(
            run(&checked).unwrap(),
            Some(Vec::new()),
            "nothing to report"
        );
        assert_eq!(main(&checked), ExitCode::SUCCESS);
    }

    #[test]
    fn a_source_that_cannot_be_read_fails_rather_than_drawing_nothing() {
        let args = Args {
            input: Some("/no/such/diagram.mmd".to_string()),
            out: None,
            tokens: false,
            check: false,
        };
        let err = run(&args).unwrap_err();
        assert!(err.contains("/no/such/diagram.mmd"), "{err}");
        assert_eq!(main(&args), ExitCode::FAILURE);
    }

    #[test]
    fn empty_input_is_an_error_not_a_panic() {
        render("", ags_mermaid::ColorMode::Fixed).unwrap_err();
        render("  \n ", ags_mermaid::ColorMode::Fixed).unwrap_err();
    }

    #[test]
    fn a_header_nobody_recognises_reports_rather_than_panicking() {
        let err = render("sunburstChart", ags_mermaid::ColorMode::Fixed).unwrap_err();
        assert!(err.contains("unknown diagram type"), "{err}");
    }

    fn asked(check: bool, tokens: bool) -> Args {
        Args {
            input: None,
            out: None,
            tokens,
            check,
        }
    }

    #[test]
    fn asking_for_a_drawing_gets_one() {
        let out = produce("graph TD\n  A --> B", &asked(false, false));
        assert!(matches!(out, Ok(Outcome::Drawn(svg)) if svg.starts_with("<svg")));
    }

    #[test]
    fn asking_for_tokens_gets_a_drawing_that_reads_its_page() {
        let Ok(Outcome::Drawn(svg)) = produce("graph TD\n  A --> B", &asked(false, true)) else {
            panic!("a drawing")
        };
        assert!(
            svg.contains("color-mix"),
            "the blends are left for the page"
        );
        let Ok(Outcome::Drawn(fixed)) = produce("graph TD\n  A --> B", &asked(false, false)) else {
            panic!("a drawing")
        };
        assert!(!fixed.contains("color-mix"));
    }

    #[test]
    fn asking_for_a_check_gets_what_it_found() {
        assert_eq!(
            produce("graph TD\n  A --> B", &asked(true, false)),
            Ok(Outcome::Checked(Vec::new()))
        );
    }

    #[test]
    fn a_source_that_cannot_be_drawn_cannot_be_checked_either() {
        let empty = Err("empty diagram source".to_string());
        assert_eq!(produce("", &asked(true, false)), empty);
        assert_eq!(produce("", &asked(false, false)), empty);
    }

    #[test]
    fn a_clean_drawing_has_nothing_to_report() {
        assert_eq!(check("graph TD\n  A --> B"), Ok(Vec::new()));
    }

    #[test]
    fn a_check_of_nothing_is_an_error_rather_than_a_pass() {
        assert_eq!(check("   "), Err("empty diagram source".to_string()));
        assert!(
            matches!(check("sunburstChart"), Err(message) if message.contains("unknown diagram type"))
        );
    }

    #[test]
    fn a_typo_is_reported_as_one_rather_than_drawn_as_something_else() {
        let err = render("pae title X", ags_mermaid::ColorMode::Fixed).unwrap_err();
        assert!(err.contains("did you mean"), "{err}");
    }

    #[test]
    fn an_implemented_type_becomes_a_standalone_image() {
        let svg = render(
            "pie title Shares\n\"a\" : 10\n\"b\" : 5",
            ags_mermaid::ColorMode::Fixed,
        )
        .unwrap();
        assert!(svg.starts_with("<svg"), "{svg}");
        // Standalone means no page behind it, so nothing may be left for a
        // cascade to resolve.
        assert!(svg.contains("--ags-bg:#"), "{svg}");
        assert!(!svg.contains("color-mix"), "{svg}");
    }

    #[test]
    fn an_svg_meant_for_a_page_leaves_its_colours_to_the_page() {
        // The opposite case, and the one a baked colour silently breaks: the
        // page sets `--bg` and `--fg`, and the drawing has to read them.
        let svg = render(
            "pie title Shares\n\"a\" : 10\n\"b\" : 5",
            ags_mermaid::ColorMode::Tokens,
        )
        .unwrap();
        assert!(!svg.contains("--ags-bg:#"), "{svg}");
        assert!(svg.contains("var(--ags-fg)"), "{svg}");
    }
}
