//! Path resolution for config and credential files.

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::ConfigError;

const ENV_CONFIG_DIR: &str = "CNB_CONFIG_DIR";

/// Resolve the directory that holds `config.toml` and `hosts.toml`.
///
/// Order:
/// 1. `$CNB_CONFIG_DIR` if set.
/// 2. Platform-default via [`ProjectDirs::from("cool", "cnb", "cnb")`].
pub fn config_dir() -> Result<PathBuf, ConfigError> {
    if let Some(v) = std::env::var_os(ENV_CONFIG_DIR) {
        if !v.is_empty() {
            return Ok(PathBuf::from(v));
        }
    }
    ProjectDirs::from("cool", "cnb", "cnb")
        .map(|p| p.config_dir().to_path_buf())
        .ok_or(ConfigError::NoConfigDir)
}

pub fn config_file() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn hosts_file() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join("hosts.toml"))
}
