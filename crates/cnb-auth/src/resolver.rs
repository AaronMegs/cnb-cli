//! Three-tier token resolution: env > keyring > hosts.toml.

use std::path::PathBuf;

use cnb_config::Hosts;
use tracing::debug;

use crate::error::AuthError;
use crate::keyring_backend::KeyringBackend;
use crate::{ENV_TOKEN, KEYRING_SERVICE};

/// Where a resolved token came from. Useful for `cnb auth status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    Env,
    Keyring { host: String, user: String },
    File { host: String, user: String },
}

/// Resolve the active token for `(host, user)`.
///
/// `hosts_path` lets tests redirect away from `~/.config/cnb/hosts.toml`.
/// In production, pass `None` and the canonical XDG path is used.
pub fn resolve_token(
    host: &str,
    user: Option<&str>,
    keyring: &dyn KeyringBackend,
    hosts_path: Option<&PathBuf>,
) -> Result<(String, TokenSource), AuthError> {
    // 1. Env.
    if let Ok(t) = std::env::var(ENV_TOKEN) {
        if !t.is_empty() {
            debug!("resolve_token: env hit");
            return Ok((t, TokenSource::Env));
        }
    }

    // Load hosts.toml (default-empty when missing).
    let hosts = match hosts_path {
        Some(p) => Hosts::load_from(p)?,
        None => Hosts::load()?,
    };
    let user_owned = user
        .map(str::to_owned)
        .or_else(|| hosts.default_user(host).map(str::to_owned))
        .ok_or_else(|| AuthError::NoUser(host.to_owned()))?;

    let account = format!("{host}:{user_owned}");

    // 2. Keyring.
    match keyring.get(KEYRING_SERVICE, &account) {
        Ok(Some(t)) if !t.is_empty() => {
            debug!("resolve_token: keyring hit");
            return Ok((
                t,
                TokenSource::Keyring {
                    host: host.to_owned(),
                    user: user_owned.clone(),
                },
            ));
        }
        Ok(_) => {}
        Err(e) => {
            debug!("keyring read failed (continuing to file): {e}");
        }
    }

    // 3. File.
    if let Some(t) = hosts.file_token(host, &user_owned) {
        debug!("resolve_token: file hit");
        return Ok((
            t.to_owned(),
            TokenSource::File {
                host: host.to_owned(),
                user: user_owned,
            },
        ));
    }

    Err(AuthError::NotLoggedIn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyring_backend::InMemoryKeyring;
    use crate::test_util::lock_env;
    use cnb_config::Hosts;
    use tempfile::TempDir;

    fn write_hosts(dir: &TempDir, mutate: impl FnOnce(&mut Hosts)) -> PathBuf {
        let path = dir.path().join("hosts.toml");
        let mut h = Hosts::default();
        mutate(&mut h);
        h.save_to(&path).unwrap();
        path
    }

    #[test]
    fn env_wins() {
        let _g = lock_env();
        std::env::set_var(ENV_TOKEN, "from-env");

        let dir = TempDir::new().unwrap();
        let path = write_hosts(&dir, |h| {
            h.upsert_user("cnb.cool", "alice", "https", Some("from-file".into()), false);
        });
        let kr = InMemoryKeyring::new();
        kr.set(KEYRING_SERVICE, "cnb.cool:alice", "from-keyring").unwrap();

        let (t, src) = resolve_token("cnb.cool", Some("alice"), &kr, Some(&path)).unwrap();
        assert_eq!(t, "from-env");
        assert_eq!(src, TokenSource::Env);

        std::env::remove_var(ENV_TOKEN);
    }

    #[test]
    fn keyring_used_when_no_env() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let path = write_hosts(&dir, |h| {
            h.upsert_user("cnb.cool", "alice", "https", None, true);
        });
        let kr = InMemoryKeyring::new();
        kr.set(KEYRING_SERVICE, "cnb.cool:alice", "from-keyring").unwrap();

        let (t, src) = resolve_token("cnb.cool", None, &kr, Some(&path)).unwrap();
        assert_eq!(t, "from-keyring");
        assert!(matches!(src, TokenSource::Keyring { .. }));
    }

    #[test]
    fn file_used_when_no_env_and_no_keyring() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let path = write_hosts(&dir, |h| {
            h.upsert_user("cnb.cool", "alice", "https", Some("from-file".into()), false);
        });
        let kr = InMemoryKeyring::new();

        let (t, src) = resolve_token("cnb.cool", None, &kr, Some(&path)).unwrap();
        assert_eq!(t, "from-file");
        assert!(matches!(src, TokenSource::File { .. }));
    }

    #[test]
    fn errors_when_no_user_known() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.toml");
        let kr = InMemoryKeyring::new();
        let err = resolve_token("cnb.cool", None, &kr, Some(&path)).unwrap_err();
        assert!(matches!(err, AuthError::NoUser(_)));
    }

    #[test]
    fn errors_when_no_token_anywhere() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let path = write_hosts(&dir, |h| {
            h.upsert_user("cnb.cool", "alice", "https", None, true);
        });
        let kr = InMemoryKeyring::new();
        let err = resolve_token("cnb.cool", None, &kr, Some(&path)).unwrap_err();
        assert!(matches!(err, AuthError::NotLoggedIn));
    }
}
