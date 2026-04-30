//! Auxiliary `repo` endpoints introduced in M4 (§8.2 deferred items).
//!
//! Two endpoints are exposed:
//! - `GET / PUT /{slug}/-/pinned-repos` — list/replace the slug owner's pinned repo set.
//! - `GET /{slug}/-/contributor/trend` — contributor trend per slug.
//!
//! `repo collaborator` and `repo activity` from DESIGN §8.2 are not currently
//! exposed by the OpenAPI spec; the CLI falls back to `cnb api` as documented
//! in DESIGN §16 risk #5.

use reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

/// `GET /{slug}/-/pinned-repos`.
pub async fn list_pinned(client: &Client, slug: &str) -> Result<Vec<Value>, ApiError> {
    client
        .request_json(Method::GET, &format!("/{slug}/-/pinned-repos"), None::<&()>)
        .await
}

#[derive(Debug, Serialize)]
pub struct SetPinnedBody {
    pub repos: Vec<String>,
}

/// `PUT /{slug}/-/pinned-repos` — replace the pinned set.
pub async fn set_pinned(client: &Client, slug: &str, repos: Vec<String>) -> Result<Value, ApiError> {
    let body = SetPinnedBody { repos };
    client
        .request_json(Method::PUT, &format!("/{slug}/-/pinned-repos"), Some(&body))
        .await
}

/// `GET /{slug}/-/contributor/trend`.
pub async fn contributor_trend(client: &Client, slug: &str, query: &str) -> Result<Value, ApiError> {
    let p = if query.is_empty() {
        format!("/{slug}/-/contributor/trend")
    } else {
        format!("/{slug}/-/contributor/trend?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;
    use url::Url;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> Client {
        Client::builder()
            .base_url(Url::parse(&server.uri()).unwrap())
            .token("t")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn set_pinned_sends_repos() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/cnb/-/pinned-repos"))
            .and(body_partial_json(json!({"repos": ["cnb/a", "cnb/b"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let _ = set_pinned(&client(&server), "cnb", vec!["cnb/a".into(), "cnb/b".into()])
            .await
            .unwrap();
    }
}
