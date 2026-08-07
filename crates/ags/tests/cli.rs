//! End-to-end tests for the `ags` CLI. These drive the real binary so the
//! argv/file-I/O paths are exercised: `present` covers inbrowser 5.1/5.2, and
//! `poll` covers the feedback-transport return leg.

use std::path::PathBuf;
use std::process::Command;

/// Write `contents` to a uniquely-named file under the per-run target tmp dir.
fn artifact(name: &str, contents: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, contents).expect("write artifact");
    path
}

/// Run `ags <args...>`, returning `(exit_code, stdout)`.
fn run(args: &[&str]) -> (i32, String) {
    run_env(args, &[])
}

/// Run `ags <args...>` with extra environment variables set on the child.
fn run_env(args: &[&str], envs: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ags"));
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let output = cmd.output().expect("spawn ags");
    let code = output.status.code().expect("exit code");
    (code, String::from_utf8(output.stdout).expect("utf8 stdout"))
}

#[test]
fn valid_artifact_passes_gate_and_stays_silent() {
    let path = artifact(
        "valid.md",
        "Some prose.\n\n```mermaid #flow feedback=annotate\ngraph TD\n  A-->B\n```\n",
    );
    let (code, stdout) = run(&["present", "--check", path.to_str().unwrap()]);
    assert_eq!(
        code, 0,
        "valid artifact should pass Gate 1 (--check = no serve)"
    );
    assert!(
        stdout.trim().is_empty(),
        "clean gate prints nothing, got: {stdout}"
    );
}

#[test]
fn invalid_artifact_is_reported_not_served() {
    let path = artifact("invalid.md", "```mermiad\ngraph TD\n```\n");
    let (code, stdout) = run(&["present", path.to_str().unwrap()]);
    assert_ne!(code, 0, "invalid artifact must not serve");
    assert!(
        stdout.starts_with("errors["),
        "expected TOON errors, got: {stdout}"
    );
    assert!(stdout.contains("near-miss-type"));
}

#[test]
fn an_unrecognized_language_fence_is_prose_and_serves() {
    // Reclassification end-to-end: ```rust is ordinary markdown, not a block, so
    // Gate 1 has nothing to say about it.
    let path = artifact("langfence.md", "intro\n\n```rust\nfn main() {}\n```\n");
    let (code, stdout) = run(&["present", "--check", path.to_str().unwrap()]);
    assert_eq!(
        code, 0,
        "a plain code fence should gate clean, got: {stdout}"
    );
    assert!(stdout.trim().is_empty());
}

#[test]
fn unsafe_html_chunk_is_rejected() {
    let path = artifact(
        "unsafe.md",
        "```html #h\n<div><script>steal()</script></div>\n```\n",
    );
    let (code, stdout) = run(&["present", path.to_str().unwrap()]);
    assert_ne!(code, 0);
    assert!(
        stdout.contains("html-script"),
        "expected html-script error, got: {stdout}"
    );
}

#[test]
fn themed_html_hardcoded_color_is_rejected() {
    // An `html` block is themed content: a hardcoded color must fail Gate 1, so
    // the artifact is never served (validate → emit TOON → exit non-zero).
    let path = artifact(
        "themed.md",
        "```html #card\n<div style=\"color:#fff;background:#111\">hi</div>\n```\n",
    );
    let (code, stdout) = run(&["present", path.to_str().unwrap()]);
    assert_ne!(
        code, 0,
        "themed content with a hardcoded color must not serve"
    );
    assert!(
        stdout.contains("html-hardcoded-color"),
        "expected html-hardcoded-color error, got: {stdout}"
    );
}

#[test]
fn catalog_prints_the_block_vocabulary() {
    // `ags catalog` prints the closed vocabulary the agent authors against, straight
    // from the validator's schema (so it can't drift from Gate 1).
    let (code, stdout) = run(&["catalog"]);
    assert_eq!(code, 0, "catalog succeeds");
    assert!(stdout.contains("block catalog"), "has a header: {stdout}");
    // every closed type, and a validator-derived enum + required marker.
    for t in [
        "mermaid", "question", "table", "code", "html", "note", "theme",
    ] {
        assert!(stdout.contains(t), "lists '{t}': {stdout}");
    }
    assert!(
        stdout.contains("*type=radio|checkbox|text|select"),
        "{stdout}"
    );
    // the affordance line uses the `human can:` label (not the `feedback=` attr name).
    assert!(stdout.contains("human can: annotate"), "{stdout}");
}

#[test]
fn bake_emits_a_standalone_page_and_refuses_an_invalid_artifact() {
    let path = artifact(
        "bake.md",
        "# Title\n\n```code #c lang=rust\nfn main() {}\n```\n",
    );
    let (code, stdout) = run(&["bake", path.to_str().unwrap()]);
    assert_eq!(code, 0, "valid artifact bakes: {stdout}");
    // Finished markup, not a mount point for a bundle to fill in.
    assert!(stdout.contains("fn main()"), "artifact rendered: {stdout}");
    assert!(stdout.contains("<title>ags · bake.md</title>"), "{stdout}");
    assert!(
        !stdout.contains("window.__ARTIFACT__"),
        "the artifact is drawn, not handed to a script: {stdout}"
    );
    assert!(!stdout.contains("viewer.js"), "no shell: {stdout}");
    assert!(!stdout.contains("<script"), "no script at all: {stdout}");
    assert!(
        stdout.contains("Content-Security-Policy"),
        "baked page carries a CSP: {stdout}"
    );
    // --out writes the page to a file instead of stdout.
    let out_path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("baked.html");
    let (code, _) = run(&[
        "bake",
        "--out",
        out_path.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "bake --out succeeds");
    assert!(std::fs::read_to_string(&out_path)
        .expect("baked file")
        .contains("<!doctype html>"));
    // The flags that chose a shell are gone with the shell.
    let (code, _) = run(&["bake", "--inline", path.to_str().unwrap()]);
    assert_ne!(code, 0, "--inline is no longer a flag");
    // a nonexistent artifact is an I/O error, not a panic.
    let (code, _) = run(&["bake", "/no/such/artifact/xyzzy.md"]);
    assert_eq!(code, 2);
    // an --out under a missing directory is a write error.
    let (code, _) = run(&[
        "bake",
        "--out",
        "/no/such/dir/baked.html",
        path.to_str().unwrap(),
    ]);
    assert_eq!(code, 2);
    // an invalid artifact is refused (non-zero, nothing baked to stdout).
    let bad = artifact("bake-bad.md", "```mermiad\ngraph TD\n```\n");
    let (code, stdout) = run(&["bake", bad.to_str().unwrap()]);
    assert_ne!(code, 0, "invalid artifact must not bake");
    assert!(
        !stdout.contains("<!doctype html>"),
        "no page on failure: {stdout}"
    );
}

#[test]
fn version_flag_prints_and_succeeds() {
    let (code, stdout) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.starts_with("ags "), "got: {stdout}");
}

#[test]
fn missing_file_argument_is_a_usage_error() {
    let (code, _) = run(&["present"]);
    assert_eq!(code, 2);
}

#[test]
fn nonexistent_file_is_an_io_error() {
    let (code, _) = run(&["present", "/no/such/artifact/xyzzy.md"]);
    assert_eq!(code, 2);
}

#[test]
fn unknown_command_is_a_usage_error() {
    let (code, _) = run(&["frobnicate"]);
    assert_eq!(code, 2);
}

#[test]
fn help_flag_prints_usage_and_succeeds() {
    let (code, stdout) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage: ags"), "got: {stdout}");
    assert!(stdout.contains("present"));
}

#[test]
fn no_command_is_a_usage_error() {
    let (code, _) = run(&[]);
    assert_eq!(code, 2);
}

#[test]
fn poll_empty_session_returns_empty_after_the_bound() {
    let path = artifact("poll-empty.md", "# artifact\n");
    let (code, stdout) = run_env(
        &["poll", path.to_str().unwrap()],
        &[("AGS_POLL_MAX_TICKS", "2"), ("AGS_POLL_INTERVAL_MS", "1")],
    );
    assert_eq!(code, 0, "an empty poll should still succeed");
    assert!(stdout.starts_with("feedback[0]:"), "got: {stdout}");
    assert!(stdout.contains("ended: false"), "got: {stdout}");
    assert!(
        stdout.trim_end().ends_with("closed: false"),
        "got: {stdout}"
    );
}

#[test]
fn poll_nonexistent_artifact_is_an_error() {
    let (code, _) = run(&["poll", "/no/such/artifact/xyzzy.md"]);
    assert_eq!(code, 2);
}

/// The page a reviewer is served: rendered markup, forms, and the policy that lets
/// them post without a script.
///
/// Nothing is fetched alongside it. `/viewer.js` and `/viewer.css` are gone with
/// the bundle, so the binary embeds no web assets at all and `web/dist` is no
/// longer needed to compile it.
fn assert_page_is_rendered(get: &impl Fn(&str, bool) -> String) {
    let page = get("/", false);
    assert!(page.contains("200 OK"), "page should 200");
    // The document arrives rendered: the prose and the code block are in the
    // markup, not fetched and drawn by a script afterwards.
    assert!(
        page.contains("fn main()"),
        "the artifact is rendered into the page"
    );
    assert!(
        !page.contains("/viewer.js"),
        "the page no longer loads a bundle: {}",
        page.chars().take(400).collect::<String>()
    );
    // A reviewer can act with no script: a comment and an answer are form posts.
    assert!(
        page.contains("action=\"/feedback\""),
        "the page carries a composer"
    );
    assert!(
        page.contains("action=\"/finish\""),
        "the review can be finished"
    );
    // The document carries the CSP — the real HTML-safety boundary. A nonce is
    // tighter than `'self'`: only the page's own script runs (Gate 1 is a
    // fast-fail hint, not the boundary).
    let page_lc = page.to_lowercase();
    assert!(
        page_lc.contains("content-security-policy: default-src 'self'")
            && page_lc.contains("script-src 'nonce-ags-review'")
            && page_lc.contains("form-action 'self'"),
        "page carries a script-locking CSP that still permits its forms: {}",
        page.chars().take(400).collect::<String>()
    );
    assert!(
        get("/artifact.md", false).contains("fn main()"),
        "artifact served"
    );
}

#[test]
fn present_serves_the_artifact_over_http() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::process::Stdio;
    use std::time::Duration;

    let path = artifact(
        "serve.md",
        "# Title\n\n```code #c lang=rust\nfn main() {}\n```\n",
    );
    // serve on a random port; stub the browser open; learn the port from stdout.
    let mut child = Command::new(env!("CARGO_BIN_EXE_ags"))
        .args(["present", path.to_str().unwrap()])
        .env("AGS_OPEN_CMD", "true")
        .env("AGS_SERVE_MAX_REQUESTS", "3") // exit gracefully after the 3 requests below
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn serve");

    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("startup line");
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .expect("addr in startup line")
        .to_string();

    // GET `p`, optionally offering brotli.
    let get = |p: &str, accept_br: bool| -> String {
        let mut s = TcpStream::connect(&addr).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let ae = if accept_br {
            "Accept-Encoding: br\r\n"
        } else {
            ""
        };
        write!(
            s,
            "GET {p} HTTP/1.1\r\nHost: localhost\r\n{ae}Connection: close\r\n\r\n"
        )
        .expect("write");
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).expect("read");
        String::from_utf8_lossy(&buf).into_owned()
    };

    assert_page_is_rendered(&get);
    assert!(get("/nope", false).contains("404"), "unknown path 404s");

    // The server handled its 3 requests and exits on its own. It was 8 before the
    // page stopped being a bundle: five of them fetched assets that no longer exist.
    child.wait().expect("wait for graceful exit");
}

#[test]
fn annotate_loop_delivers_feedback_to_poll() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::process::Stdio;
    use std::time::Duration;

    // A valid artifact with an id'd block the browser can anchor feedback on.
    let path = artifact(
        "annotate.md",
        "```mermaid #flow feedback=annotate\ngraph TD\n  A-->B\n```\n",
    );
    // The serving process and the poll below resolve the same log beside `path`.
    let mut child = Command::new(env!("CARGO_BIN_EXE_ags"))
        .args(["present", path.to_str().unwrap()])
        .env("AGS_OPEN_CMD", "true")
        .env("AGS_SERVE_MAX_REQUESTS", "5") // the 5 requests below, then graceful exit
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn serve");

    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("startup line");
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .expect("addr in startup line")
        .to_string();

    let http = |method: &str, target: &str, body: &str| -> String {
        let mut s = TcpStream::connect(&addr).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        write!(
            s,
            "{method} {target} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write");
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).expect("read");
        String::from_utf8_lossy(&buf).into_owned()
    };

    // 1: a well-formed node-anchored annotation is accepted.
    let good = r#"{"block_id":"flow","sub_target":{"Node":"Auth"},"kind":"annotation","body":"arrow points the wrong way"}"#;
    assert!(
        http("POST", "/feedback", good).contains("200 OK"),
        "good feedback accepted"
    );
    // 2: /state exposes the settled feedback for the viewer's recovery on load.
    let state = http("GET", "/state", "");
    assert!(
        state.contains("200 OK") && state.contains("arrow points the wrong way"),
        "state reflects the annotation: {state}"
    );
    // 3: a malformed body is a 400.
    assert!(
        http("POST", "/feedback", "not json").contains("400"),
        "bad feedback rejected"
    );
    // 4: the feedback route is POST-only.
    assert!(
        http("GET", "/feedback", "").contains("405"),
        "GET /feedback is 405"
    );
    // 5: finishing the review finalizes the session.
    assert!(
        http("POST", "/finish", "").contains("200 OK"),
        "finish accepted"
    );

    child.wait().expect("wait for graceful exit");

    // The agent's return leg sees exactly the accepted annotation, then ended.
    let (code, out) = run_env(
        &["poll", path.to_str().unwrap()],
        &[("AGS_POLL_MAX_TICKS", "2"), ("AGS_POLL_INTERVAL_MS", "1")],
    );
    assert_eq!(code, 0, "poll succeeds");
    assert!(
        out.contains("#flow/node:Auth"),
        "node anchor delivered: {out}"
    );
    assert!(out.contains("annotation"), "kind delivered: {out}");
    assert!(
        out.contains("arrow points the wrong way"),
        "body delivered: {out}"
    );
    assert!(out.contains("ended: true"), "review ended: {out}");
    assert!(
        out.contains("closed: false"),
        "finished, not abandoned: {out}"
    );
}

#[test]
fn present_serve_bind_failure_exits_nonzero() {
    // occupy a port, then ask present to serve on it → bind fails (run's error arm).
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe");
    let port_str = listener.local_addr().expect("addr").port().to_string();
    let path = artifact("bindfail.md", "# just prose, valid and block-free\n");
    let (code, _) = run_env(
        &["present", "--port", &port_str, path.to_str().unwrap()],
        &[("AGS_OPEN_CMD", "true")],
    );
    assert_eq!(code, 2, "serving on an occupied port must fail");
}

#[test]
fn present_fresh_discards_the_prior_log() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::process::Stdio;
    use std::time::Duration;

    let path = artifact("fresh.md", "# just prose, valid\n");
    // Seed the log beside the artifact with a stale reply that `--fresh` must discard.
    let log = path.with_file_name("fresh.md.ags.jsonl");
    std::fs::write(
        &log,
        "{\"block_id\":\"x\",\"kind\":\"annotation\",\"body\":\"stale\",\"status\":\"new\"}\n",
    )
    .expect("seed log");

    let mut child = Command::new(env!("CARGO_BIN_EXE_ags"))
        .args(["present", "--fresh", path.to_str().unwrap()])
        .env("AGS_OPEN_CMD", "true")
        .env("AGS_SERVE_MAX_REQUESTS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn serve");

    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("startup line");
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .expect("addr")
        .to_string();

    let mut s = TcpStream::connect(&addr).expect("connect");
    s.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    write!(
        s,
        "GET /state HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .expect("write");
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).expect("read");
    let resp = String::from_utf8_lossy(&buf);
    assert!(
        !resp.contains("stale"),
        "--fresh discards the prior log: {resp}"
    );
    assert!(
        resp.contains("\"feedback\":[]"),
        "state starts empty: {resp}"
    );

    child.wait().expect("graceful exit");
}

#[test]
fn present_shutdown_beacon_stops_the_server_and_polls_closed() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpStream;
    use std::process::Stdio;
    use std::time::Duration;

    let path = artifact("shutdown.md", "# just prose, valid\n");
    // Short grace so the abandoned-close exits quickly; no request bound, so the server
    // must exit via the shutdown grace window, not `AGS_SERVE_MAX_REQUESTS`.
    let mut child = Command::new(env!("CARGO_BIN_EXE_ags"))
        .args(["present", path.to_str().unwrap()])
        .env("AGS_OPEN_CMD", "true")
        .env("AGS_SHUTDOWN_GRACE_MS", "120")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn serve");

    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("startup line");
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .expect("addr")
        .to_string();

    let http = |method: &str, target: &str| -> String {
        let mut s = TcpStream::connect(&addr).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        write!(
            s,
            "{method} {target} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .expect("write");
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).expect("read");
        String::from_utf8_lossy(&buf).into_owned()
    };

    // A page load (exercises the reconnect/cancel arm); /shutdown is POST-only.
    assert!(http("GET", "/").contains("200 OK"), "page served");
    assert!(
        http("GET", "/shutdown").contains("405"),
        "shutdown is POST-only"
    );
    // The close beacon: arms the grace window; with no reconnect the server exits.
    assert!(
        http("POST", "/shutdown").contains("200 OK"),
        "shutdown beacon accepted"
    );
    child.wait().expect("server exits after the grace window");

    // The review was never finished, so the agent's poll reports it closed, not ended.
    let (code, out) = run_env(
        &["poll", path.to_str().unwrap()],
        &[("AGS_POLL_MAX_TICKS", "2"), ("AGS_POLL_INTERVAL_MS", "1")],
    );
    assert_eq!(code, 0, "poll succeeds");
    assert!(
        out.contains("closed: true"),
        "abandoned review is closed: {out}"
    );
    assert!(out.contains("ended: false"), "and not finished: {out}");
}
