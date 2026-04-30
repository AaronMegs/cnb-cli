//! IO streams, color management, and output post-processing (JSON / jq / template) for cnb CLI.
//!
//! This crate is intentionally light: it only carries what `cnb api` needs in M1.
//! Richer table rendering for `repo list` / `issue list` etc. lands in M2.

pub mod color;
pub mod io_streams;
pub mod jq;
pub mod json_out;
pub mod table;
pub mod template;

pub use color::ColorMode;
pub use io_streams::IoStreams;

/// Errors produced by the post-processing helpers.
#[derive(Debug, thiserror::Error)]
pub enum TtyError {
    #[error("invalid jq filter: {0}")]
    JqParse(String),
    #[error("jq runtime error: {0}")]
    JqRun(String),
    #[error("template error: {0}")]
    Template(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
