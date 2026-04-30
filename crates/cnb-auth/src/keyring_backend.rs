//! Pluggable system-keyring abstraction.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::AuthError;

pub trait KeyringBackend: Send + Sync + std::fmt::Debug {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, AuthError>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), AuthError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), AuthError>;
}

/// Production backend backed by the [`keyring`] crate (macOS Keychain / Win Cred / Linux Secret Service).
#[derive(Debug, Default)]
pub struct RealKeyring;

impl KeyringBackend for RealKeyring {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, AuthError> {
        let entry = keyring::Entry::new(service, account).map_err(|e| AuthError::Keyring(e.to_string()))?;
        match entry.get_password() {
            Ok(s) => Ok(Some(s)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AuthError::Keyring(e.to_string())),
        }
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), AuthError> {
        let entry = keyring::Entry::new(service, account).map_err(|e| AuthError::Keyring(e.to_string()))?;
        entry
            .set_password(secret)
            .map_err(|e| AuthError::Keyring(e.to_string()))
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), AuthError> {
        let entry = keyring::Entry::new(service, account).map_err(|e| AuthError::Keyring(e.to_string()))?;
        match entry.delete_password() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AuthError::Keyring(e.to_string())),
        }
    }
}

/// In-memory backend for tests and CI environments without a working keyring.
#[derive(Debug, Default)]
pub struct InMemoryKeyring {
    inner: Mutex<HashMap<String, String>>,
}

impl InMemoryKeyring {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(service: &str, account: &str) -> String {
        format!("{service}::{account}")
    }
}

impl KeyringBackend for InMemoryKeyring {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, AuthError> {
        Ok(self.inner.lock().unwrap().get(&Self::key(service, account)).cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), AuthError> {
        self.inner
            .lock()
            .unwrap()
            .insert(Self::key(service, account), secret.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), AuthError> {
        self.inner.lock().unwrap().remove(&Self::key(service, account));
        Ok(())
    }
}

/// Backend that always reports failure — useful to simulate keyring being unavailable.
#[derive(Debug, Default)]
pub struct FailingKeyring;

impl KeyringBackend for FailingKeyring {
    fn get(&self, _service: &str, _account: &str) -> Result<Option<String>, AuthError> {
        Err(AuthError::Keyring("simulated keyring failure".into()))
    }
    fn set(&self, _service: &str, _account: &str, _secret: &str) -> Result<(), AuthError> {
        Err(AuthError::Keyring("simulated keyring failure".into()))
    }
    fn delete(&self, _service: &str, _account: &str) -> Result<(), AuthError> {
        Err(AuthError::Keyring("simulated keyring failure".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_round_trip() {
        let kr = InMemoryKeyring::new();
        assert!(kr.get("svc", "acct").unwrap().is_none());
        kr.set("svc", "acct", "secret").unwrap();
        assert_eq!(kr.get("svc", "acct").unwrap().as_deref(), Some("secret"));
        kr.delete("svc", "acct").unwrap();
        assert!(kr.get("svc", "acct").unwrap().is_none());
    }
}
