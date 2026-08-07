//! The plumbing under a reply: bodies in, headers out.
//!
//! The host never runs JavaScript, so the page's own script rides under a nonce
//! the CSP names here and nothing else is allowed to run at all.

use std::io;
use std::process::Command;

use tiny_http::{Header, Request, Response};

pub(super) fn read_body(request: &mut Request) -> String {
    let mut body = String::new();
    request
        .as_reader()
        .read_to_string(&mut body)
        .unwrap_or_default();
    body
}

/// Answer with a status code + short body under `content_type`.
pub(super) fn reply(
    request: Request,
    (status, body): (u16, &'static str),
    content_type: &'static str,
) -> io::Result<()> {
    request.respond(
        Response::from_string(body)
            .with_status_code(status)
            .with_header(ctype(content_type)),
    )
}

/// Answer 200 with borrowed body text under `content_type`.
pub(super) fn reply_str(
    request: Request,
    body: &str,
    content_type: &'static str,
) -> io::Result<()> {
    request.respond(
        Response::from_string(body)
            .with_header(ctype(content_type))
            .with_header(no_cache()),
    )
}

/// A `Content-Type` header from a static, well-formed value.
pub(super) fn ctype(value: &'static str) -> Header {
    #[expect(
        clippy::unwrap_used,
        reason = "static, well-formed Content-Type header never fails to parse"
    )]
    Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}

/// The Content-Security-Policy served with the review document — the *real*
/// HTML-safety boundary (the Gate-1 sanitizer is a fast-fail hint, not the boundary).
/// `script-src 'nonce-…'` runs only the page's own nonce-tagged script and blocks
/// every other script, inline or `javascript:`, even one that slipped past Gate 1 —
/// tighter than the `'self'` it replaces, which would have run any same-origin file.
/// `form-action 'self'` is what lets a comment and an answer post without a script; `object-src`,
/// `base-uri`, `form-action`, and `frame-ancestors 'none'` close the plugin, `<base>`,
/// form-submission, and framing vectors. `style-src` keeps `'unsafe-inline'` because
/// themed content and the mermaid SVG rely on inline `style` attributes (which cannot
/// execute code); `img-src` allows same-origin, `data:`, and `https:` images. The
/// Gate-1 sanitizer permits remote image URLs (rejecting only `javascript:`/
/// `vbscript:`/`data:text/html`); plaintext `http:` is intentionally excluded here,
/// so an https image renders but an http one does not.
pub(super) const CSP: &str = "default-src 'self'; script-src 'nonce-ags-review'; \
     style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; object-src 'none'; \
     base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

/// A `Location` header for a redirect.
///
/// The target is built by [`ags_render::anchor_for`] from a block id the artifact
/// declared, so it is well-formed; a header that somehow will not parse falls back
/// to one that does rather than panicking mid-response.
pub(super) fn location(to: &str) -> Header {
    Header::from_bytes(&b"Location"[..], to.as_bytes())
        .unwrap_or_else(|()| ctype("text/html; charset=utf-8"))
}

/// The `Content-Security-Policy` header (see [`CSP`]).
pub(super) fn csp_header() -> Header {
    #[expect(
        clippy::unwrap_used,
        reason = "static, well-formed Content-Security-Policy header never fails to parse"
    )]
    Header::from_bytes(&b"Content-Security-Policy"[..], CSP.as_bytes()).unwrap()
}

/// A `Cache-Control: no-store` header. Every `present` rebuilds and re-serves the
/// viewer bundle, and the asset URLs carry no version, so heuristic caching would let
/// a reviewer's browser mix a stale `viewer.js`/`viewer.css` with a newer artifact —
/// which renders as broken styling. `no-store` forces a fresh fetch each load.
pub(super) fn no_cache() -> Header {
    #[expect(
        clippy::unwrap_used,
        reason = "static, well-formed Cache-Control header never fails to parse"
    )]
    Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..]).unwrap()
}

/// Open `url` in a browser via `$AGS_OPEN_CMD` (default `open`). Best-effort.
pub(super) fn open_browser(url: &str) {
    let cmd = std::env::var("AGS_OPEN_CMD").unwrap_or_else(|_| "open".to_string());
    open_browser_cmd(&cmd, url);
}

/// Best-effort browser open via `cmd`; a spawn failure is reported, not fatal.
pub(super) fn open_browser_cmd(cmd: &str, url: &str) {
    if let Err(err) = Command::new(cmd).arg(url).spawn() {
        eprintln!("ags: could not open a browser ({err}); open {url} manually");
    }
}
