//! Raw HTTP passthrough for `cnb api`.
//!
//! `cnb api` is the gh-style escape hatch: it lets users hit any path
//! with any method / body / extra headers and get back the verbatim
//! `(status, headers, body)` triple. The typed SDK only exposes JSON-
//! decoding entry points, so we layer this thin wrapper on top of its
//! shared `reqwest::Client` (which already has the `Authorization`
//! header, base URL, and connection pool wired up).
//!
//! Differences from the previous `cnb-api::Client::request_passthrough`:
//!
//! - No retry loop. Retries are easy to compose externally (`cnb api`
//!   is a debugging tool); duplicating the SDK's `HttpInner::execute`
//!   retry machinery here would add code without buying the user
//!   anything they can't already script. If we ever need it back, it
//!   should move into the SDK and be exposed through a future
//!   `HttpInner::execute_raw` style method.
//! - Auth / UA injection is delegated to the SDK's reqwest client
//!   (`default_headers`), so we never see the bearer token in this
//!   module — meaning we cannot accidentally log it.
//! - URL construction is delegated to `client.http().url(path)`, which
//!   handles the same `CNB_API_BASE` env override, trailing-slash
//!   normalisation, and path percent-encoding the cnb-api crate did.

use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Method;
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

/// Verbatim response surface exposed to `cnb api` callers.
#[derive(Debug, Clone)]
pub struct PassthroughResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub request_id: Option<String>,
}

impl PassthroughResponse {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

const REQUEST_ID_HEADER: &str = "x-request-id";

/// Issue a raw request through the SDK's shared reqwest client.
///
/// `path` is relative to the SDK's base URL (the same precedence used
/// by every typed call: explicit override > `CNB_API_BASE` env > SDK
/// default). `body` is sent as JSON when present; `extra_headers` are
/// appended on top of the SDK's defaults.
pub async fn request(
    ctx: &mut Context,
    method: Method,
    path: &str,
    body: Option<Value>,
    extra_headers: &[(String, String)],
) -> Result<PassthroughResponse, CliError> {
    let client = ctx.sdk()?;
    // Delegate URL construction to the SDK so this module stays free of
    // any base-URL parsing logic — see SDK-I01 (resolved in cnb 0.2.2).
    let url = client
        .http()
        .url(path)
        .map_err(|e| CliError::Generic(format!("invalid SDK path `{path}`: {e}")))?;

    let mut req = client.http().reqwest_client().request(method, url);
    for (k, v) in extra_headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(k.as_str()), HeaderValue::try_from(v.as_str())) {
            req = req.header(name, value);
        }
    }
    if let Some(b) = &body {
        req = req.json(b);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| CliError::Generic(format!("`cnb api` request failed: {e}")))?;

    let status = resp.status().as_u16();
    let mut headers = Vec::new();
    for (k, v) in resp.headers() {
        if let Ok(s) = v.to_str() {
            headers.push((k.as_str().to_owned(), s.to_owned()));
        }
    }
    let request_id = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(REQUEST_ID_HEADER))
        .map(|(_, v)| v.clone());
    let body_text = resp
        .text()
        .await
        .map_err(|e| CliError::Generic(format!("`cnb api` body read failed: {e}")))?;

    Ok(PassthroughResponse {
        status,
        headers,
        body: body_text,
        request_id,
    })
}

/// Convert a non-2xx [`PassthroughResponse`] into a [`CliError`] that
/// produces the right exit code per DESIGN §12 (4 / 2 / 8 / 9).
///
/// The CNB error envelope is `{"errcode": <i64>, "errmsg": "<…>"}`. We
/// special-case `errcode=16` → unauthenticated and `errcode=5` → not
/// found, matching the legacy `cnb-api::ApiError::from_http` mapping
/// that callers (and integration tests) depend on.
pub fn into_error(resp: PassthroughResponse) -> CliError {
    #[derive(serde::Deserialize)]
    struct Envelope {
        #[serde(default)]
        errcode: i64,
        #[serde(default)]
        errmsg: String,
    }
    let env: Option<Envelope> = serde_json::from_str(&resp.body).ok();
    let (errcode, errmsg) = match env {
        Some(e) => (e.errcode, e.errmsg),
        None => (-1, resp.body.clone()),
    };
    let request_id_suffix = resp
        .request_id
        .as_deref()
        .map(|id| format!(", request_id={id}"))
        .unwrap_or_default();

    match (resp.status, errcode) {
        (401, _) | (_, 16) => CliError::Unauthorized,
        (404, _) | (_, 5) => CliError::NotFound,
        (429, _) => CliError::RateLimited,
        (s, _) if (500..600).contains(&s) => CliError::ServerError {
            http_status: s,
            errcode,
            errmsg,
            request_id: resp.request_id,
        },
        (s, _) => CliError::Generic(format!("API error [{errcode}] {errmsg} (HTTP {s}{request_id_suffix})")),
    }
}
