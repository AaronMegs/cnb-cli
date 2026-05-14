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
            .env("CNB_KEYRING_BACKEND", "none");
        // Windows needs SystemRoot for runtime DLLs (rustls, getaddrinfo,
        // etc.) and for the loader to resolve subsystem-32. Without it
        // the spawned cnb process can fail at TLS init or DNS resolve
        // even when the request itself targets 127.0.0.1. We forward
        // the host value through (assert_cmd's env_clear wipes it).
        // Same goes for TEMP/TMP — used by tempfile crate during
        // hosts.toml writes — and PATHEXT, used by Windows process
        // launcher to resolve `git`/`cnb` lookups.
        for key in [
            "SYSTEMROOT",
            "SystemRoot",
            "TEMP",
            "TMP",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "PATHEXT",
            "WINDIR",
            "COMSPEC",
        ] {
            if let Some(v) = std::env::var_os(key) {
                cmd.env(key, v);
            }
        }
        // **Do not** call `env_remove("CNB_TOKEN")` here. `env_clear()`
        // already wipes every variable, and a subsequent `env_remove`
        // followed by an outer `.env("CNB_TOKEN", value)` triggered a
        // Windows-specific quirk in std::process: the case-insensitive
        // env-key map kept the remove semantic and the outer set never
        // landed in the spawned process, leaving `cnb` to fall through
        // the env > keyring > file resolver and exit 4 (Unauthorized)
        // on the very tests that explicitly stage a fake token.
        cmd
    }
}
