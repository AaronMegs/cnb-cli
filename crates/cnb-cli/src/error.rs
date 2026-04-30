//! Top-level CLI error type. Maps to process exit codes per DESIGN §12.

use cnb_api::ApiError;
use cnb_auth::AuthError;
use cnb_config::ConfigError;
use cnb_git::GitError;
use cnb_tty::TtyError;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] ApiError),

    #[error(transparent)]
    Auth(#[from] AuthError),

    #[error(transparent)]
    Config(#[from] ConfigError),

    #[error(transparent)]
    Tty(#[from] TtyError),

    #[error(transparent)]
    Git(#[from] GitError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("invalid argument: {0}")]
    BadArgs(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("operation cancelled")]
    Interrupted,

    /// User declined a destructive confirmation prompt (e.g. `repo delete`).
    /// Distinct from [`Interrupted`] so scripts can react to "user said no"
    /// without conflating it with Ctrl-C.
    #[error("cancelled by user")]
    Cancelled,

    #[error("{0}")]
    Generic(String),
}

impl CliError {
    /// Process exit code, per DESIGN §12.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Api(ApiError::Unauthorized) | Self::Auth(AuthError::NotLoggedIn | AuthError::NoUser(_)) => 4,
            Self::Api(ApiError::NotFound) => 2,
            // Code 8 covers both server-side rate limiting and user-declined
            // destructive prompts: in both cases the operation did not run and
            // a reasonable retry strategy is "wait and try again".
            Self::Api(ApiError::RateLimited { .. }) | Self::Cancelled => 8,
            Self::Api(ApiError::Api { http_status, .. }) if (500..600).contains(&u32::from(*http_status)) => 9,
            Self::BadArgs(_) | Self::NotImplemented(_) => 3,
            Self::Interrupted => 5,
            Self::Config(_) => 10,
            _ => 1,
        }
    }
}
