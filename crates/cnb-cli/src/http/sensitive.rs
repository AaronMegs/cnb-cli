//! Header-name redaction used by `cnb api -i` so we never echo a
//! `Authorization` / token / cookie header verbatim back to the user.
//!
//! Migrated verbatim from `cnb-api::tracing_layer` (which had a single
//! consumer in cnb-cli, namely `cnb api`).

const SENSITIVE: &[&str] = &["authorization", "token", "password", "x-auth", "secret"];

/// True if a header / field name should be redacted.
pub fn is_sensitive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SENSITIVE.iter().any(|s| lower.contains(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_authorization() {
        assert!(is_sensitive("Authorization"));
        assert!(is_sensitive("X-Auth-Token"));
        assert!(is_sensitive("Cnb-Token"));
        assert!(!is_sensitive("Content-Type"));
        assert!(!is_sensitive("X-Trace-Id"));
    }

    #[test]
    fn case_insensitive_match() {
        assert!(is_sensitive("AUTHORIZATION"));
        assert!(is_sensitive("authorization"));
        assert!(is_sensitive("set-cookie-secret"));
    }
}
