//! Helpers shared by tests across the crate.
//!
//! All tests in this crate touch `CNB_TOKEN`, so they must serialize via
//! the *same* mutex.

use std::sync::{Mutex, MutexGuard};

use crate::ENV_TOKEN;

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the cross-test env lock and ensure `CNB_TOKEN` is unset on entry.
/// Recovers automatically from a poisoned mutex (left over from a previous panic).
pub(crate) fn lock_env() -> MutexGuard<'static, ()> {
    let g = match ENV_LOCK.lock() {
        Ok(g) => g,
        Err(poison) => poison.into_inner(),
    };
    std::env::remove_var(ENV_TOKEN);
    g
}
