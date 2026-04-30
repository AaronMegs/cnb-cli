//! `Registries` service for `cnb registry …` (M4 §8.8).
//!
//! Covers two server families: the registry container (`/{slug}/-/registries`,
//! `DELETE /{registry}`, etc.) and the packages within (`/{slug}/-/packages`).
//!
//! Per the OpenAPI spec, the package `type` path segment is restricted to a
//! whitelist (`docker`, `helm`, `dockermodel`, `maven`, `npm`, `ohpm`, `pypi`,
//! `nuget`, `composer`, `conan`, `cargo`). The CLI layer enforces this; the
//! facade itself only does path-segment safety.

use reqwest::Method;
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

/// Server-recognised package types.
pub const PACKAGE_TYPES: &[&str] = &[
    "docker",
    "helm",
    "dockermodel",
    "maven",
    "npm",
    "ohpm",
    "pypi",
    "nuget",
    "composer",
    "conan",
    "cargo",
];

fn ensure_no_slash(c: &str, what: &str) -> Result<(), ApiError> {
    if c.contains('/') {
        return Err(ApiError::InvalidUrl(format!("{what} must not contain `/`: {c:?}")));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
//                              Registries
// ---------------------------------------------------------------------------

/// `GET /{slug}/-/registries`.
pub async fn list(client: &Client, slug: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{slug}/-/registries")
    } else {
        format!("/{slug}/-/registries?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `DELETE /{registry}`.
pub async fn delete(client: &Client, registry: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::DELETE, &format!("/{registry}"), None::<&()>)
        .await
}

/// `POST /{registry}/-/settings/set_visibility`.
#[derive(Debug, serde::Serialize)]
struct VisibilityBody {
    visibility_level: i64,
}

pub async fn set_visibility(client: &Client, registry: &str, level: i64) -> Result<Value, ApiError> {
    let body = VisibilityBody {
        visibility_level: level,
    };
    client
        .request_json(
            Method::POST,
            &format!("/{registry}/-/settings/set_visibility"),
            Some(&body),
        )
        .await
}

// ---------------------------------------------------------------------------
//                                 Packages
// ---------------------------------------------------------------------------

/// `GET /{slug}/-/packages?type=...`.
pub async fn list_packages(client: &Client, slug: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{slug}/-/packages")
    } else {
        format!("/{slug}/-/packages?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{slug}/-/packages/{type}/{name}`.
pub async fn view_package(client: &Client, slug: &str, kind: &str, name: &str) -> Result<Value, ApiError> {
    ensure_no_slash(kind, "package type")?;
    ensure_no_slash(name, "package name")?;
    client
        .request_json(Method::GET, &format!("/{slug}/-/packages/{kind}/{name}"), None::<&()>)
        .await
}

/// `DELETE /{slug}/-/packages/{type}/{name}`.
pub async fn delete_package(client: &Client, slug: &str, kind: &str, name: &str) -> Result<Value, ApiError> {
    ensure_no_slash(kind, "package type")?;
    ensure_no_slash(name, "package name")?;
    client
        .request_json(
            Method::DELETE,
            &format!("/{slug}/-/packages/{kind}/{name}"),
            None::<&()>,
        )
        .await
}

// ---------------------------------------------------------------------------
//                                  Tags
// ---------------------------------------------------------------------------

/// `GET /{slug}/-/packages/{type}/{name}/-/tags`.
pub async fn list_tags(client: &Client, slug: &str, kind: &str, name: &str) -> Result<Vec<Value>, ApiError> {
    ensure_no_slash(kind, "package type")?;
    ensure_no_slash(name, "package name")?;
    client
        .request_json(
            Method::GET,
            &format!("/{slug}/-/packages/{kind}/{name}/-/tags"),
            None::<&()>,
        )
        .await
}

/// `GET /{slug}/-/packages/{type}/{name}/-/tag/{tag}`.
pub async fn view_tag(client: &Client, slug: &str, kind: &str, name: &str, tag: &str) -> Result<Value, ApiError> {
    ensure_no_slash(kind, "package type")?;
    ensure_no_slash(name, "package name")?;
    ensure_no_slash(tag, "tag")?;
    client
        .request_json(
            Method::GET,
            &format!("/{slug}/-/packages/{kind}/{name}/-/tag/{tag}"),
            None::<&()>,
        )
        .await
}

/// `DELETE /{slug}/-/packages/{type}/{name}/-/tag/{tag}`.
pub async fn delete_tag(client: &Client, slug: &str, kind: &str, name: &str, tag: &str) -> Result<Value, ApiError> {
    ensure_no_slash(kind, "package type")?;
    ensure_no_slash(name, "package name")?;
    ensure_no_slash(tag, "tag")?;
    client
        .request_json(
            Method::DELETE,
            &format!("/{slug}/-/packages/{kind}/{name}/-/tag/{tag}"),
            None::<&()>,
        )
        .await
}

/// `GET /{slug}/-/packages/{type}/{name}/-/tag/{tag}/provenance`.
pub async fn provenance(client: &Client, slug: &str, kind: &str, name: &str, tag: &str) -> Result<Value, ApiError> {
    ensure_no_slash(kind, "package type")?;
    ensure_no_slash(name, "package name")?;
    ensure_no_slash(tag, "tag")?;
    client
        .request_json(
            Method::GET,
            &format!("/{slug}/-/packages/{kind}/{name}/-/tag/{tag}/provenance"),
            None::<&()>,
        )
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

    #[test]
    fn package_type_whitelist_includes_docker_and_npm() {
        assert!(PACKAGE_TYPES.contains(&"docker"));
        assert!(PACKAGE_TYPES.contains(&"npm"));
        assert!(PACKAGE_TYPES.contains(&"cargo"));
    }

    #[tokio::test]
    async fn list_uses_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/-/registries"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"name": "main"}])))
            .mount(&server)
            .await;
        let v = list(&client(&server), "cnb", "page=1").await.unwrap();
        assert_eq!(v.len(), 1);
    }

    #[tokio::test]
    async fn view_package_rejects_slash() {
        let server = MockServer::start().await;
        let err = view_package(&client(&server), "cnb", "evil/x", "n").await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn provenance_path_is_correct() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/-/packages/npm/foo/-/tag/v1/provenance"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"slsa": "v1"})))
            .mount(&server)
            .await;
        let _ = provenance(&client(&server), "cnb", "npm", "foo", "v1").await.unwrap();
    }
}
