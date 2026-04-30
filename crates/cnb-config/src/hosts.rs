//! `hosts.toml` schema (per-host auth state).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::atomic_write::write_secure;
use crate::error::ConfigError;
use crate::paths::hosts_file;
use crate::SCHEMA_VERSION;

pub const DEFAULT_HOST: &str = "cnb.cool";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hosts {
    pub version: u32,
    #[serde(default = "default_host")]
    pub default_host: String,
    #[serde(default)]
    pub hosts: BTreeMap<String, HostEntry>,
}

fn default_host() -> String {
    DEFAULT_HOST.into()
}

impl Default for Hosts {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            default_host: DEFAULT_HOST.into(),
            hosts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostEntry {
    pub default_user: Option<String>,
    #[serde(default = "default_git_protocol")]
    pub git_protocol: String,
    #[serde(default)]
    pub users: BTreeMap<String, UserEntry>,
}

impl Default for HostEntry {
    fn default() -> Self {
        Self {
            default_user: None,
            git_protocol: default_git_protocol(),
            users: BTreeMap::new(),
        }
    }
}

fn default_git_protocol() -> String {
    "https".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserEntry {
    /// Token stored in file (only when keyring is unavailable). Empty when keyring is used.
    #[serde(default)]
    pub token: String,
    /// Whether the token is stored in the system keyring.
    #[serde(default)]
    pub keyring: bool,
}

impl Hosts {
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(&hosts_file()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let h: Self = toml::from_str(&s).map_err(|e| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source: e,
                })?;
                if h.version != SCHEMA_VERSION {
                    return Err(ConfigError::UnsupportedSchema(h.version, SCHEMA_VERSION));
                }
                Ok(h)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io {
                path: path.to_path_buf(),
                source: e,
            }),
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&hosts_file()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        let s = toml::to_string_pretty(self)?;
        write_secure(path, &s)
    }

    /// Convenience: get the default user for `host`.
    pub fn default_user(&self, host: &str) -> Option<&str> {
        self.hosts.get(host)?.default_user.as_deref()
    }

    /// Convenience: get the file-stored token for `(host, user)`, if any.
    pub fn file_token(&self, host: &str, user: &str) -> Option<&str> {
        let entry = self.hosts.get(host)?.users.get(user)?;
        if entry.keyring || entry.token.is_empty() {
            None
        } else {
            Some(entry.token.as_str())
        }
    }

    /// Insert/replace a user record. Does not save to disk.
    pub fn upsert_user(&mut self, host: &str, user: &str, git_protocol: &str, token: Option<String>, in_keyring: bool) {
        let entry = self.hosts.entry(host.to_owned()).or_default();
        if entry.default_user.is_none() {
            entry.default_user = Some(user.to_owned());
        }
        entry.git_protocol = git_protocol.to_owned();
        entry.users.insert(
            user.to_owned(),
            UserEntry {
                token: token.unwrap_or_default(),
                keyring: in_keyring,
            },
        );
    }

    /// Remove a user. If they were the default user, clear it.
    pub fn remove_user(&mut self, host: &str, user: &str) {
        if let Some(entry) = self.hosts.get_mut(host) {
            entry.users.remove(user);
            if entry.default_user.as_deref() == Some(user) {
                entry.default_user = entry.users.keys().next().cloned();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hosts.toml");
        let mut h = Hosts::default();
        h.upsert_user("cnb.cool", "alice", "https", None, true);
        h.save_to(&path).unwrap();

        let loaded = Hosts::load_from(&path).unwrap();
        assert_eq!(loaded.default_host, "cnb.cool");
        assert_eq!(loaded.default_user("cnb.cool"), Some("alice"));
        assert!(loaded.file_token("cnb.cool", "alice").is_none());
    }

    #[test]
    fn file_token_is_returned_when_keyring_false() {
        let mut h = Hosts::default();
        h.upsert_user("cnb.cool", "bob", "https", Some("tok-bob".into()), false);
        assert_eq!(h.file_token("cnb.cool", "bob"), Some("tok-bob"));
    }

    #[test]
    fn remove_default_user_promotes_another() {
        let mut h = Hosts::default();
        h.upsert_user("cnb.cool", "alice", "https", None, true);
        h.upsert_user("cnb.cool", "bob", "https", None, true);
        h.remove_user("cnb.cool", "alice");
        assert_eq!(h.default_user("cnb.cool"), Some("bob"));
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("none.toml");
        let h = Hosts::load_from(&path).unwrap();
        assert_eq!(h.default_host, "cnb.cool");
    }
}
