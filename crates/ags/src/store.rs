//! Where a review's feedback log lives — the location both legs of the loop agree on.
//!
//! The log sits **beside the artifact** as `<artifact>.ags.jsonl`, so the replies are
//! a readable file next to the thing being reviewed. The name is derived from the
//! artifact, so `present` (writing) and `poll` (reading) resolve the same log for the
//! same artifact with no coordination. Gitignore `*.ags.jsonl` to keep them untracked.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use ags_feedback::Session;

/// The log path for `artifact_path`: `<artifact>.ags.jsonl` beside it.
#[must_use]
pub fn log_path(artifact_path: &Path) -> PathBuf {
    let mut name = artifact_path
        .file_name()
        .unwrap_or_else(|| OsStr::new("artifact"))
        .to_os_string();
    name.push(".ags.jsonl");
    artifact_path.with_file_name(name)
}

/// Open (creating parent dirs) the feedback session for `artifact_path`.
///
/// # Errors
/// Propagates I/O errors from creating the log's parent directory.
pub fn open_session(artifact_path: &Path) -> io::Result<Session> {
    Session::open_at(log_path(artifact_path))
}
