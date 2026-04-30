//! `AuthService` — pure business logic for `cnb auth login/logout/status/token`.
//!
//! IO (terminal prompts, network validation) lives in `cnb-cli`; this layer
//! only handles persistence + bookkeeping so it can be exhaustively unit tested.

use std::path::PathBuf;

use cnb_config::Hosts;
use tracing::warn;

use crate::error::AuthError;
use crate::keyring_backend::KeyringBackend;
use crate::resolver::{resolve_token, TokenSource};
use crate::KEYRING_SERVICE;

pub struct AuthService<'a> {
    pub keyring: &'a dyn KeyringBackend,
    pub hosts_path: PathBuf,
}

impl std::fmt::Debug for AuthService<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthService")
            .field("hosts_path", &self.hosts_path)
            .finish()
    }
}

/// Outcome of a successful `login`.
#[derive(Debug, Clone)]
pub struct LoginRecord {
    pub host: String,
    pub user: String,
    pub stored_in_keyring: bool,
}

/// Snapshot returned by `status`.
#[derive(Debug, Clone)]
pub struct AuthStatus {
    pub host: String,
    pub user: String,
    pub source: TokenSource,
}

impl<'a> AuthService<'a> {
    pub fn new(keyring: &'a dyn KeyringBackend, hosts_path: PathBuf) -> Self {
        Self { keyring, hosts_path }
    }

    /// Persist a freshly-validated token. Tries keyring first; falls back to file.
    pub fn login(&self, host: &str, user: &str, token: &str, git_protocol: &str) -> Result<LoginRecord, AuthError> {
        let account = format!("{host}:{user}");
        let stored_in_keyring = match self.keyring.set(KEYRING_SERVICE, &account, token) {
            Ok(()) => true,
            Err(e) => {
                warn!("keyring write failed, falling back to file: {e}");
                false
            }
        };

        let mut hosts = Hosts::load_from(&self.hosts_path)?;
        let token_for_file = if stored_in_keyring {
            None
        } else {
            Some(token.to_owned())
        };
        hosts.upsert_user(host, user, git_protocol, token_for_file, stored_in_keyring);
        // Set as default user if none yet.
        hosts.default_host = host.to_owned();
        hosts.save_to(&self.hosts_path)?;

        Ok(LoginRecord {
            host: host.to_owned(),
            user: user.to_owned(),
            stored_in_keyring,
        })
    }

    /// Remove credentials for `(host, user)` from both keyring and hosts.toml.
    pub fn logout(&self, host: &str, user: &str) -> Result<(), AuthError> {
        let account = format!("{host}:{user}");
        if let Err(e) = self.keyring.delete(KEYRING_SERVICE, &account) {
            // best-effort
            warn!("keyring delete failed: {e}");
        }
        let mut hosts = Hosts::load_from(&self.hosts_path)?;
        hosts.remove_user(host, user);
        hosts.save_to(&self.hosts_path)?;
        Ok(())
    }

    /// Resolve current login (returns Err::NotLoggedIn if none).
    pub fn status(&self, host: &str, user: Option<&str>) -> Result<AuthStatus, AuthError> {
        let (_, src) = resolve_token(host, user, self.keyring, Some(&self.hosts_path))?;
        let (host_owned, user_owned) = match &src {
            TokenSource::Env => {
                let hosts = Hosts::load_from(&self.hosts_path)?;
                let u = user
                    .map(str::to_owned)
                    .or_else(|| hosts.default_user(host).map(str::to_owned))
                    .unwrap_or_else(|| "(unknown)".to_owned());
                (host.to_owned(), u)
            }
            TokenSource::Keyring { host, user } | TokenSource::File { host, user } => (host.clone(), user.clone()),
        };
        Ok(AuthStatus {
            host: host_owned,
            user: user_owned,
            source: src,
        })
    }

    /// Resolve current token (returns Err::NotLoggedIn if none).
    pub fn token(&self, host: &str, user: Option<&str>) -> Result<String, AuthError> {
        let (t, _) = resolve_token(host, user, self.keyring, Some(&self.hosts_path))?;
        Ok(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyring_backend::{FailingKeyring, InMemoryKeyring};
    use crate::test_util::lock_env;
    use cnb_config::Hosts;
    use tempfile::TempDir;

    fn fresh(dir: &TempDir) -> PathBuf {
        dir.path().join("hosts.toml")
    }

    #[test]
    fn login_writes_to_keyring_when_available() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let kr = InMemoryKeyring::new();
        let svc = AuthService::new(&kr, fresh(&dir));

        let r = svc.login("cnb.cool", "alice", "tok-123", "https").unwrap();
        assert!(r.stored_in_keyring);

        let h = Hosts::load_from(&svc.hosts_path).unwrap();
        assert_eq!(h.default_user("cnb.cool"), Some("alice"));
        assert!(h.file_token("cnb.cool", "alice").is_none());

        let st = svc.status("cnb.cool", None).unwrap();
        assert!(matches!(st.source, TokenSource::Keyring { .. }));
        assert_eq!(svc.token("cnb.cool", None).unwrap(), "tok-123");
    }

    #[test]
    fn login_falls_back_to_file_when_keyring_unavailable() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let kr = FailingKeyring;
        let svc = AuthService::new(&kr, fresh(&dir));

        let r = svc.login("cnb.cool", "bob", "tok-999", "ssh").unwrap();
        assert!(!r.stored_in_keyring);

        let h = Hosts::load_from(&svc.hosts_path).unwrap();
        assert_eq!(h.file_token("cnb.cool", "bob"), Some("tok-999"));

        let st = svc.status("cnb.cool", None).unwrap();
        assert!(matches!(st.source, TokenSource::File { .. }));
    }

    #[test]
    fn logout_clears_both_stores() {
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let kr = InMemoryKeyring::new();
        let svc = AuthService::new(&kr, fresh(&dir));
        svc.login("cnb.cool", "alice", "t", "https").unwrap();

        svc.logout("cnb.cool", "alice").unwrap();
        assert!(matches!(svc.token("cnb.cool", None).unwrap_err(), AuthError::NoUser(_)));
    }

    #[cfg(unix)]
    #[test]
    fn hosts_toml_written_with_0600() {
        use std::os::unix::fs::PermissionsExt;
        let _g = lock_env();

        let dir = TempDir::new().unwrap();
        let kr = FailingKeyring;
        let svc = AuthService::new(&kr, fresh(&dir));
        svc.login("cnb.cool", "x", "tok", "https").unwrap();

        let mode = std::fs::metadata(&svc.hosts_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
    }
}
