//! Token resolution and `cnb auth` business logic.
//!
//! Token resolution order (DESIGN §5):
//!   1. `CNB_TOKEN` env var
//!   2. System keyring (via [`KeyringBackend`])
//!   3. `~/.config/cnb/hosts.toml`
//!
//! All persistence operations go through [`AuthService`], which holds a
//! pluggable [`KeyringBackend`]. Tests inject [`InMemoryKeyring`].

pub mod error;
pub mod keyring_backend;
pub mod resolver;
pub mod service;

#[cfg(test)]
mod test_util;

pub use error::AuthError;
pub use keyring_backend::{InMemoryKeyring, KeyringBackend, RealKeyring};
pub use resolver::{resolve_token, TokenSource};
pub use service::AuthService;

/// Service name used for the system keyring entry.
pub const KEYRING_SERVICE: &str = "cnb-cli";

/// Env var checked first by the resolver.
pub const ENV_TOKEN: &str = "CNB_TOKEN";
