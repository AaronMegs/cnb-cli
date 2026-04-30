//! `config.toml` schema (user preferences).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::atomic_write::write_secure;
use crate::error::ConfigError;
use crate::paths::config_file;
use crate::SCHEMA_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub core: CoreConfig,
    #[serde(default)]
    pub output: OutputConfig,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            core: CoreConfig::default(),
            output: OutputConfig::default(),
            aliases: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// `vim`, `nano`, `code -w`, ...
    pub editor: Option<String>,
    /// `less -FRX`, `more`, ...
    pub pager: Option<String>,
    /// `https` | `ssh`
    #[serde(default = "default_git_protocol")]
    pub git_protocol: String,
    /// `enabled` | `disabled`
    #[serde(default = "default_prompt")]
    pub prompt: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            editor: None,
            pager: None,
            git_protocol: default_git_protocol(),
            prompt: default_prompt(),
        }
    }
}

fn default_git_protocol() -> String {
    "https".into()
}

fn default_prompt() -> String {
    "enabled".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// `auto` | `always` | `never`
    pub color: String,
    pub default_json_indent: u8,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            color: "auto".into(),
            default_json_indent: 2,
        }
    }
}

impl Config {
    /// Load config from default path; absent file → default values.
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from(&config_file()?)
    }

    pub fn load_from(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(s) => {
                let cfg: Self = toml::from_str(&s).map_err(|e| ConfigError::Parse {
                    path: path.to_path_buf(),
                    source: e,
                })?;
                if cfg.version != SCHEMA_VERSION {
                    return Err(ConfigError::UnsupportedSchema(cfg.version, SCHEMA_VERSION));
                }
                Ok(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io {
                path: path.to_path_buf(),
                source: e,
            }),
        }
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        self.save_to(&config_file()?)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), ConfigError> {
        let s = toml::to_string_pretty(self)?;
        write_secure(path, &s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_round_trips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config::default();
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.core.git_protocol, "https");
        assert_eq!(loaded.output.color, "auto");
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.toml");
        let cfg = Config::load_from(&path).unwrap();
        assert_eq!(cfg.version, SCHEMA_VERSION);
    }
}
