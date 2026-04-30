//! Config and credential file management for cnb CLI.
//!
//! Two files are managed:
//!
//! - `config.toml` — user preferences (editor, pager, color, aliases, ...).
//! - `hosts.toml`  — auth state (default host, default user, optional fallback token).
//!
//! Both live under [`paths::config_dir`].

pub mod atomic_write;
pub mod config;
pub mod error;
pub mod hosts;
pub mod paths;

pub use config::{Config, CoreConfig, OutputConfig};
pub use error::ConfigError;
pub use hosts::{HostEntry, Hosts, UserEntry};

/// Schema version currently produced by this crate.
pub const SCHEMA_VERSION: u32 = 1;
