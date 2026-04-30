//! Thin wrapper over the system `git` binary.

use std::path::Path;
use std::process::Command;

use crate::error::GitError;

/// Run `git remote get-url <remote>` in `cwd` and return the URL.
pub fn remote_url(cwd: &Path, remote: &str) -> Result<String, GitError> {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(["remote", "get-url", remote])
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        // Best-effort classification.
        if stderr.contains("No such remote") {
            return Err(GitError::NoRemote(remote.to_owned()));
        }
        if stderr.contains("not a git repository") {
            return Err(GitError::NotARepo);
        }
        return Err(GitError::NonZero {
            status: out.status.code().unwrap_or(-1),
            stderr,
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
