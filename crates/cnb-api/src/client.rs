//! Single-flight HTTP client wrapper.

use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::{Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tracing::{debug, instrument, warn};
use url::Url;

use crate::error::ApiError;
use crate::retry::{backoff_for_attempt, is_idempotent, is_retriable_status, parse_retry_after, MAX_RETRIES};
use crate::tracing_layer::redact_header;
use crate::url_safe;

/// Default base URL when neither builder nor env override is provided.
pub const DEFAULT_BASE_URL: &str = "https://api.cnb.cool";

/// Override env var for base URL (testing only — see README).
const ENV_API_BASE: &str = "CNB_API_BASE";

const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Clone)]
pub struct Client {
    inner: Arc<reqwest::Client>,
    base: Url,
    token: Option<String>,
    user_agent: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base.as_str())
            .field("token", &self.token.as_ref().map(|_| "***"))
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub fn base_url(&self) -> &Url {
        &self.base
    }

    /// Issue a request and return the raw [`reqwest::Response`].
    /// Performs token injection, UA injection, retry on 5xx/429 (idempotent only).
    #[instrument(level = "debug", skip(self, body), fields(method = %method, path))]
    pub async fn request_raw(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
        extra_headers: &[(String, String)],
    ) -> Result<Response, ApiError> {
        let url = url_safe::resolve(&self.base, path)?;
        let max = if is_idempotent(&method) { MAX_RETRIES } else { 0 };

        for attempt in 0..=max {
            let mut req = self.inner.request(method.clone(), url.clone());
            req = self.inject_auth_and_ua(req);

            for (k, v) in extra_headers {
                if let (Ok(name), Ok(value)) = (HeaderName::try_from(k.as_str()), HeaderValue::try_from(v.as_str())) {
                    req = req.header(name, value);
                }
            }

            if let Some(b) = &body {
                req = req.json(b);
            }

            debug!(attempt, url = %url, "sending request");
            let resp = req.send().await?;
            let status = resp.status().as_u16();

            if is_retriable_status(status) && attempt < max {
                let retry_after = parse_retry_after(resp.headers().get("retry-after").and_then(|v| v.to_str().ok()));
                let delay = retry_after.unwrap_or_else(|| backoff_for_attempt(attempt));
                warn!(
                    status,
                    attempt,
                    delay_ms = delay.as_millis() as u64,
                    "retriable failure, backing off"
                );
                tokio::time::sleep(delay).await;
                continue;
            }

            return Ok(resp);
        }
        unreachable!("retry loop always returns or continues");
    }

    fn inject_auth_and_ua(&self, mut req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req = req.header(USER_AGENT, &self.user_agent);
        if let Some(token) = &self.token {
            // Construct via HeaderValue so we can mark it sensitive (no debug print).
            let mut hv = HeaderValue::try_from(format!("Bearer {token}"))
                .expect("token contains invalid header bytes; should have been validated upstream");
            hv.set_sensitive(true);
            req = req.header(AUTHORIZATION, hv);
            // Keep the redact helper warm in tests.
            let _ = redact_header("Authorization", "***");
        }
        req
    }

    /// Issue a request and parse the JSON body into `T` (on 2xx) or normalize to an [`ApiError`].
    pub async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T, ApiError> {
        let body_value = match body {
            Some(b) => Some(serde_json::to_value(b)?),
            None => None,
        };
        let resp = self.request_raw(method, path, body_value, &[]).await?;
        let status = resp.status();
        let request_id = extract_request_id(resp.headers());
        let text = resp.text().await?;

        if status.is_success() {
            if text.is_empty() {
                // For empty bodies, attempt to deserialize as Null.
                return Ok(serde_json::from_value(serde_json::Value::Null)?);
            }
            return Ok(serde_json::from_str::<T>(&text)?);
        }
        Err(map_status(status, text, request_id))
    }

    /// Issue a request and return the response as `(status, headers, body_text)` for `cnb api`.
    pub async fn request_passthrough(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
        extra_headers: &[(String, String)],
    ) -> Result<PassthroughResponse, ApiError> {
        let resp = self.request_raw(method, path, body, extra_headers).await?;
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
        let body = resp.text().await?;
        Ok(PassthroughResponse {
            status,
            headers,
            body,
            request_id,
        })
    }

    /// Build a [`reqwest::RequestBuilder`] pre-configured with auth + UA and
    /// a multipart body. Used by [`crate::services::uploads`] for streaming
    /// file uploads where `request_raw` (which only handles JSON) is not
    /// sufficient.
    ///
    /// The returned builder is **not** retried automatically — multipart
    /// streams aren't safely retriable.
    pub fn multipart_request(
        &self,
        method: Method,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<reqwest::RequestBuilder, ApiError> {
        let url = url_safe::resolve(&self.base, path)?;
        let mut req = self.inner.request(method, url);
        req = self.inject_auth_and_ua(req);
        req = req.multipart(form);
        Ok(req)
    }

    /// Borrow the underlying [`reqwest::Client`] for absolute-URL requests
    /// (e.g. release pre-signed asset uploads where the URL is returned by
    /// the server and is **not** relative to [`Self::base_url`]).
    ///
    /// Callers are responsible for any auth — this client does **not**
    /// auto-inject `Authorization` or `User-Agent` headers. Pre-signed URLs
    /// authorize via query string and are the primary use case.
    pub fn http(&self) -> &reqwest::Client {
        &self.inner
    }
}

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

    pub fn into_error(self) -> ApiError {
        map_status(
            StatusCode::from_u16(self.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            self.body,
            self.request_id,
        )
    }
}

fn extract_request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
}

fn map_status(status: StatusCode, body: String, request_id: Option<String>) -> ApiError {
    let code = status.as_u16();
    if code == 429 {
        return ApiError::RateLimited { retry_after_sec: 0 };
    }
    ApiError::from_http(code, &body, request_id)
}

#[derive(Default)]
pub struct ClientBuilder {
    base: Option<Url>,
    token: Option<String>,
    timeout: Option<Duration>,
    user_agent: Option<String>,
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("base", &self.base.as_ref().map(Url::as_str))
            .field("token", &self.token.as_ref().map(|_| "***"))
            .field("timeout", &self.timeout)
            .field("user_agent", &self.user_agent)
            .finish()
    }
}

impl ClientBuilder {
    pub fn base_url(mut self, url: Url) -> Self {
        self.base = Some(url);
        self
    }

    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = Some(t);
        self
    }

    pub fn build(self) -> Result<Client, ApiError> {
        let base = match self.base {
            Some(u) => u,
            None => match std::env::var(ENV_API_BASE) {
                Ok(v) if !v.is_empty() => {
                    Url::parse(&v).map_err(|e| ApiError::InvalidUrl(format!("CNB_API_BASE={v}: {e}")))?
                }
                _ => Url::parse(DEFAULT_BASE_URL).expect("hardcoded URL is valid"),
            },
        };
        let user_agent = self.user_agent.unwrap_or_else(default_user_agent);

        let inner = reqwest::Client::builder()
            .https_only(false)
            .timeout(self.timeout.unwrap_or(Duration::from_secs(60)))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Client {
            inner: Arc::new(inner),
            base,
            token: self.token,
            user_agent,
        })
    }
}

fn default_user_agent() -> String {
    format!(
        "cnb-cli/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn fixture_client(server: &MockServer) -> Client {
        Client::builder()
            .base_url(Url::parse(&server.uri()).unwrap())
            .token("fake-token")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn happy_get_with_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", "Bearer fake-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"username":"alice"})))
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let v: serde_json::Value = c.request_json(Method::GET, "/user", None::<&()>).await.unwrap();
        assert_eq!(v["username"], "alice");
    }

    #[tokio::test]
    async fn unauthorized_401_maps_to_unauthorized() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({"errcode":16,"errmsg":"not logged in"})))
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let err = c
            .request_json::<serde_json::Value>(Method::GET, "/user", None::<&()>)
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Unauthorized), "got {err:?}");
    }

    #[tokio::test]
    async fn not_found_404_maps_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({"errcode":5})))
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let err = c
            .request_json::<serde_json::Value>(Method::GET, "/missing", None::<&()>)
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::NotFound));
    }

    #[tokio::test]
    async fn rate_limited_429_after_retries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/limited"))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let err = c
            .request_json::<serde_json::Value>(Method::GET, "/limited", None::<&()>)
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::RateLimited { .. }));
    }

    #[tokio::test]
    async fn server_error_5xx_after_retries() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/oops"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({"errcode":99,"errmsg":"oops"})))
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let err = c
            .request_json::<serde_json::Value>(Method::GET, "/oops", None::<&()>)
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Api { http_status: 500, .. }));
    }

    #[tokio::test]
    async fn decode_failure_propagates() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let err = c
            .request_json::<serde_json::Value>(Method::GET, "/user", None::<&()>)
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Decode(_)));
    }

    #[tokio::test]
    async fn passthrough_returns_status_and_request_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-request-id", "rid-42")
                    .set_body_json(json!({"ok":true})),
            )
            .mount(&server)
            .await;

        let c = fixture_client(&server);
        let r = c.request_passthrough(Method::GET, "/user", None, &[]).await.unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.request_id.as_deref(), Some("rid-42"));
        assert!(r.body.contains("\"ok\""));
    }
}
