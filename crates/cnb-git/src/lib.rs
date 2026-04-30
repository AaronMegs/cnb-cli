//! Local git operations for cnb CLI.
//!
//! M1 only exposes URL parsing and a thin `git remote get-url` wrapper.
//! Higher-level commands (clone / push / fetch) land in M2.

pub mod error;
pub mod git_cmd;
pub mod remote;

pub use error::GitError;
pub use remote::{parse_remote_url, RepoSlug};
