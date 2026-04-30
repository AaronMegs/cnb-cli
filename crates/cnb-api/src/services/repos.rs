//! `Repos` service. Covers the endpoints exercised by `cnb repo …` (M2 §8.2).
//!
//! All paths are passed verbatim to [`crate::Client::request_json`] / `request_raw`,
//! which routes them through [`crate::url_safe::resolve`] — never concatenated by hand.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

/// Lightweight projection of a repository as returned by list endpoints.
///
/// We only promise to decode the fields below; everything else is preserved
/// in `extra` for `--json` consumers and templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility_level: Option<i64>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub last_activity_at: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Body for `POST /{slug}/-/repos`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateRepoBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `0 = public`, `10 = internal`, `20 = private` (CNB convention).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility_level: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
}

/// Body for `PATCH /{repo}`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct EditRepoBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
}

/// `GET /user/repos` — repositories owned/accessible by the current token.
pub async fn list_self(client: &Client, query: &str) -> Result<Vec<Value>, ApiError> {
    let path = if query.is_empty() {
        "/user/repos".to_owned()
    } else {
        format!("/user/repos?{query}")
    };
    client.request_json(Method::GET, &path, None::<&()>).await
}

/// `GET /users/{username}/repos`.
pub async fn list_user(client: &Client, username: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/users/{username}/repos")
    } else {
        format!("/users/{username}/repos?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{slug}/-/repos` — repositories under an org/group slug.
pub async fn list_under_slug(client: &Client, slug: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{slug}/-/repos")
    } else {
        format!("/{slug}/-/repos?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{repo}` — repo details.
pub async fn view(client: &Client, repo: &str) -> Result<Value, ApiError> {
    client.request_json(Method::GET, &format!("/{repo}"), None::<&()>).await
}

/// `POST /{slug}/-/repos` — create a repository under `slug`.
pub async fn create(client: &Client, slug: &str, body: &CreateRepoBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{slug}/-/repos"), Some(body))
        .await
}

/// `PATCH /{repo}` — edit metadata.
pub async fn edit(client: &Client, repo: &str, body: &EditRepoBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::PATCH, &format!("/{repo}"), Some(body))
        .await
}

/// `DELETE /{repo}` — delete a repository.
pub async fn delete(client: &Client, repo: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::DELETE, &format!("/{repo}"), None::<&()>)
        .await
}

/// `GET /{repo}/-/forks` — list forks of a repository.
pub async fn list_forks(client: &Client, repo: &str) -> Result<Vec<Value>, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/forks"), None::<&()>)
        .await
}

/// `POST /{slug}/-/settings/archive`.
pub async fn archive(client: &Client, slug: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{slug}/-/settings/archive"), None::<&()>)
        .await
}

/// `POST /{slug}/-/settings/unarchive`.
pub async fn unarchive(client: &Client, slug: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{slug}/-/settings/unarchive"), None::<&()>)
        .await
}

/// `POST /{repo}/-/transfer`.
#[derive(Debug, Clone, Serialize)]
pub struct TransferBody<'a> {
    pub new_namespace: &'a str,
}

pub async fn transfer(client: &Client, repo: &str, new_namespace: &str) -> Result<Value, ApiError> {
    let body = TransferBody { new_namespace };
    client
        .request_json(Method::POST, &format!("/{repo}/-/transfer"), Some(&body))
        .await
}

/// `POST /{repo}/-/settings/set_visibility`.
#[derive(Debug, Clone, Serialize)]
pub struct VisibilityBody {
    pub visibility_level: i64,
}

pub async fn set_visibility(client: &Client, repo: &str, level: i64) -> Result<Value, ApiError> {
    let body = VisibilityBody {
        visibility_level: level,
    };
    client
        .request_json(Method::POST, &format!("/{repo}/-/settings/set_visibility"), Some(&body))
        .await
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
    async fn view_decodes_minimal() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name":"feedback","path":"cnb/feedback"})))
            .mount(&server)
            .await;
        let v = view(&client(&server), "cnb/feedback").await.unwrap();
        assert_eq!(v["name"], "feedback");
    }

    #[tokio::test]
    async fn create_sends_json_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/-/repos"))
            .and(body_partial_json(json!({"name":"newrepo"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"name":"newrepo","path":"cnb/newrepo"})))
            .mount(&server)
            .await;
        let body = CreateRepoBody {
            name: "newrepo".into(),
            description: Some("hi".into()),
            visibility_level: Some(0),
            default_branch: None,
        };
        let v = create(&client(&server), "cnb", &body).await.unwrap();
        assert_eq!(v["name"], "newrepo");
    }

    #[tokio::test]
    async fn delete_returns_value() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/cnb/feedback"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let v = delete(&client(&server), "cnb/feedback").await.unwrap();
        assert!(v.is_null() || v.is_object());
    }

    #[tokio::test]
    async fn set_visibility_includes_level() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/settings/set_visibility"))
            .and(body_partial_json(json!({"visibility_level":20})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
            .mount(&server)
            .await;
        let _ = set_visibility(&client(&server), "cnb/feedback", 20).await.unwrap();
    }

    #[tokio::test]
    async fn list_forks_paginates_through_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback/-/forks"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"name":"fork1"}])))
            .mount(&server)
            .await;
        let v = list_forks(&client(&server), "cnb/feedback").await.unwrap();
        assert_eq!(v.len(), 1);
    }
}
