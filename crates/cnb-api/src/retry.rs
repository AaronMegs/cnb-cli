//! Tiny retry helper: exponential backoff for idempotent requests on 5xx/429.

use std::time::Duration;

/// Whether an HTTP method is safe to retry without risk of duplicate side effects.
pub fn is_idempotent(method: &reqwest::Method) -> bool {
    matches!(
        *method,
        reqwest::Method::GET
            | reqwest::Method::HEAD
            | reqwest::Method::PUT
            | reqwest::Method::DELETE
            | reqwest::Method::OPTIONS
    )
}

/// Whether a response status is retry-eligible.
pub fn is_retriable_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

/// Exponential backoff: 200ms, 400ms, 800ms, ... capped at 5s.
pub fn backoff_for_attempt(attempt: u32) -> Duration {
    let base_ms = 200u64;
    let cap_ms = 5_000u64;
    let ms = base_ms.saturating_mul(2u64.saturating_pow(attempt)).min(cap_ms);
    Duration::from_millis(ms)
}

/// Parse a `Retry-After` header value (seconds-only form) into a [`Duration`].
pub fn parse_retry_after(value: Option<&str>) -> Option<Duration> {
    value
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Default maximum retry attempts.
pub const MAX_RETRIES: u32 = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotent_methods() {
        assert!(is_idempotent(&reqwest::Method::GET));
        assert!(is_idempotent(&reqwest::Method::DELETE));
        assert!(!is_idempotent(&reqwest::Method::POST));
        assert!(!is_idempotent(&reqwest::Method::PATCH));
    }

    #[test]
    fn retriable_statuses() {
        assert!(is_retriable_status(429));
        assert!(is_retriable_status(500));
        assert!(is_retriable_status(503));
        assert!(!is_retriable_status(404));
        assert!(!is_retriable_status(400));
        assert!(!is_retriable_status(200));
    }

    #[test]
    fn backoff_grows_then_caps() {
        assert_eq!(backoff_for_attempt(0).as_millis(), 200);
        assert_eq!(backoff_for_attempt(1).as_millis(), 400);
        assert_eq!(backoff_for_attempt(2).as_millis(), 800);
        assert_eq!(backoff_for_attempt(10).as_millis(), 5_000);
    }

    #[test]
    fn retry_after_seconds() {
        assert_eq!(parse_retry_after(Some("3")), Some(Duration::from_secs(3)));
        assert_eq!(parse_retry_after(Some("garbage")), None);
        assert_eq!(parse_retry_after(None), None);
    }
}
