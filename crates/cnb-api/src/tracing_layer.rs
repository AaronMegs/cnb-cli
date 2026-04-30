//! Token-redacting helpers used when constructing tracing fields.
//!
//! We do not run a custom `tracing_subscriber::Layer` here — instead, the call
//! sites use [`redact`] to scrub headers/values *before* logging.
//! This is simpler than a Layer and impossible to bypass at log site.

const SENSITIVE: &[&str] = &["authorization", "token", "password", "x-auth", "secret"];

/// True if a header/field name should be redacted.
pub fn is_sensitive(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SENSITIVE.iter().any(|s| lower.contains(s))
}

/// Redact a header value if its name is sensitive; otherwise return the original.
pub fn redact_header(name: &str, value: &str) -> String {
    if is_sensitive(name) {
        "***".to_owned()
    } else {
        value.to_owned()
    }
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
    fn redacts_value() {
        assert_eq!(redact_header("Authorization", "Bearer abc"), "***");
        assert_eq!(redact_header("Content-Type", "application/json"), "application/json");
    }
}
