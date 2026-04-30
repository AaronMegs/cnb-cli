#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("not inside a git repository")]
    NotARepo,
    #[error("no remote named `{0}`")]
    NoRemote(String),
    #[error("failed to parse remote url `{url}`: {reason}")]
    Parse { url: String, reason: String },
    #[error("failed to spawn git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git command exited with status {status}: {stderr}")]
    NonZero { status: i32, stderr: String },
}
