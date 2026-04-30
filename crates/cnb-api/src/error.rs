//! Unified error model for the cnb HTTP client.

use serde::Deserialize;

/// Body shape returned by CNB on error: `{"errcode": <i64>, "errmsg": "<...>"}`.
///
/// Some endpoints omit `errmsg` (e.g. plain `{"errcode":5}`); we tolerate that.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponseError {
    pub errcode: i64,
    #[serde(default)]
    pub errmsg: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error [{errcode}] {errmsg} (HTTP {http_status}{})",
        request_id.as_deref().map(|id| format!(", request_id={id}")).unwrap_or_default())]
    Api {
        errcode: i64,
        errmsg: String,
        http_status: u16,
        request_id: Option<String>,
        raw: serde_json::Value,
    },

    #[error("unauthorized: please run `cnb auth login`")]
    Unauthorized,

    #[error("not found")]
    NotFound,

    #[error("rate limited (retry after {retry_after_sec}s)")]
    RateLimited { retry_after_sec: u64 },

    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),

    #[error("auth error: {0}")]
    Auth(String),

    #[error("invalid url: {0}")]
    InvalidUrl(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl ApiError {
    /// Build the appropriate variant from a non-2xx HTTP response body.
    ///
    /// `body_text` is the verbatim response body (already drained); `request_id` is best-effort.
    pub fn from_http(http_status: u16, body_text: &str, request_id: Option<String>) -> Self {
        // Try to parse as CNB error envelope; fall back to a synthetic one.
        let parsed: Option<ApiResponseError> = serde_json::from_str(body_text).ok();
        let raw: serde_json::Value =
            serde_json::from_str(body_text).unwrap_or(serde_json::Value::String(body_text.to_owned()));

        let (errcode, errmsg) = match parsed {
            Some(e) => (e.errcode, e.errmsg),
            None => (-1, body_text.to_owned()),
        };

        // CNB-specific code mapping.
        match (http_status, errcode) {
            (401, _) | (_, 16) => Self::Unauthorized,
            (404, _) | (_, 5) => Self::NotFound,
            _ => Self::Api {
                errcode,
                errmsg,
                http_status,
                request_id,
                raw,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_errcode_16_to_unauthorized() {
        let e = ApiError::from_http(401, r#"{"errcode":16,"errmsg":"not logged in"}"#, None);
        assert!(matches!(e, ApiError::Unauthorized));
    }

    #[test]
    fn maps_errcode_5_to_not_found() {
        let e = ApiError::from_http(404, r#"{"errcode":5}"#, None);
        assert!(matches!(e, ApiError::NotFound));
    }

    #[test]
    fn falls_back_to_api_variant() {
        let e = ApiError::from_http(500, r#"{"errcode":99,"errmsg":"oops"}"#, Some("rid-1".into()));
        match e {
            ApiError::Api {
                errcode,
                http_status,
                request_id,
                ..
            } => {
                assert_eq!(errcode, 99);
                assert_eq!(http_status, 500);
                assert_eq!(request_id.as_deref(), Some("rid-1"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn handles_non_json_body() {
        let e = ApiError::from_http(502, "Bad Gateway", None);
        match e {
            ApiError::Api { errcode, errmsg, .. } => {
                assert_eq!(errcode, -1);
                assert_eq!(errmsg, "Bad Gateway");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }
}
