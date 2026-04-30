//! Shared helpers for CLI integration tests.

#![allow(dead_code, unreachable_pub)]

use std::path::PathBuf;

use assert_cmd::Command;
use tempfile::TempDir;

/// Lightweight per-test environment: isolated config dir + Null keyring backend.
pub struct TestEnv {
    pub _dir: TempDir,
    pub config_dir: PathBuf,
}

impl TestEnv {
    pub fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let config_dir = dir.path().to_path_buf();
        Self { _dir: dir, config_dir }
    }

    /// Build an `assert_cmd::Command` invoking the `cnb` binary with this env's
    /// config dir, the `none` keyring backend, and any extra base URL override.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("cnb").unwrap();
        cmd.env_clear()
            .env("HOME", &self.config_dir)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("CNB_CONFIG_DIR", &self.config_dir)
            .env("CNB_KEYRING_BACKEND", "none")
            .env_remove("CNB_TOKEN");
        cmd
    }
}
