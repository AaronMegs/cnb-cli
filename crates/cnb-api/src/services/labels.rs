//! `Labels` service for `cnb label …` (M2 §8.3).
//!
//! Note on path safety: [`crate::url_safe::resolve`] splits its `path` argument
//! on `/` to produce URL segments. Any user-controlled component embedded into
//! a path **must not contain `/`** or it would leak across segments. We
//! validate this defensively here for label `name`s; callers receive a clear
//! [`ApiError::InvalidUrl`] instead of a silent path injection.

use reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

fn ensure_no_slash(component: &str, what: &str) -> Result<(), ApiError> {
    if component.contains('/') {
        return Err(ApiError::InvalidUrl(format!(
            "{what} must not contain `/`: {component:?}"
        )));
    }
    Ok(())
}

/// `GET /{repo}/-/labels`.
pub async fn list(client: &Client, repo: &str) -> Result<Vec<Value>, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/labels"), None::<&()>)
        .await
}

/// `POST /{repo}/-/labels`.
#[derive(Debug, Clone, Serialize)]
pub struct CreateLabelBody<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
}

pub async fn create(client: &Client, repo: &str, body: &CreateLabelBody<'_>) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/labels"), Some(body))
        .await
}

/// `PATCH /{repo}/-/labels/{name}`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct EditLabelBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
}

pub async fn edit(client: &Client, repo: &str, name: &str, body: &EditLabelBody<'_>) -> Result<Value, ApiError> {
    ensure_no_slash(name, "label name")?;
    client
        .request_json(Method::PATCH, &format!("/{repo}/-/labels/{name}"), Some(body))
        .await
}

/// `DELETE /{repo}/-/labels/{name}`.
pub async fn delete(client: &Client, repo: &str, name: &str) -> Result<Value, ApiError> {
    ensure_no_slash(name, "label name")?;
    client
        .request_json(Method::DELETE, &format!("/{repo}/-/labels/{name}"), None::<&()>)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;
    use url::Url;
    use wiremock::matchers::{method, path};
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
    async fn list_returns_array() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback/-/labels"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"name":"bug"}])))
            .mount(&server)
            .await;
        let v = list(&client(&server), "cnb/feedback").await.unwrap();
        assert_eq!(v.len(), 1);
    }

    #[tokio::test]
    async fn delete_works_for_url_safe_names() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/cnb/feedback/-/labels/needs%20triage"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let _ = delete(&client(&server), "cnb/feedback", "needs triage").await.unwrap();
    }

    #[tokio::test]
    async fn delete_rejects_slashed_name() {
        let server = MockServer::start().await;
        // Even though no mock is set, the call should fail before any HTTP.
        let err = delete(&client(&server), "cnb/feedback", "evil/path").await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }
}
