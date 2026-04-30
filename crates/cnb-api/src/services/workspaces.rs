//! `Workspaces` service for `cnb workspace …` (M3 §8.7).

use reqwest::Method;
use serde::Serialize;
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

fn ensure_no_slash(c: &str, what: &str) -> Result<(), ApiError> {
    if c.contains('/') {
        return Err(ApiError::InvalidUrl(format!("{what} must not contain `/`: {c:?}")));
    }
    Ok(())
}

/// `GET /workspace/list`.
pub async fn list(client: &Client, query: &str) -> Result<Value, ApiError> {
    let p = if query.is_empty() {
        "/workspace/list".to_owned()
    } else {
        format!("/workspace/list?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StartWorkspaceBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Full git ref (`refs/heads/main` or `refs/tags/v1`). Mutually exclusive
    /// with `branch` — if both are set, the server prefers `ref`.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub r#ref: Option<String>,
}

/// `POST /{repo}/-/workspace/start`.
pub async fn start(client: &Client, repo: &str, body: &StartWorkspaceBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/workspace/start"), Some(body))
        .await
}

/// `GET /{repo}/-/workspace/detail/{sn}`.
pub async fn view(client: &Client, repo: &str, sn: &str) -> Result<Value, ApiError> {
    ensure_no_slash(sn, "workspace sn")?;
    client
        .request_json(Method::GET, &format!("/{repo}/-/workspace/detail/{sn}"), None::<&()>)
        .await
}

/// Body for both `/workspace/stop` and (server-shared payload) `/workspace/delete`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct WorkspaceTargetBody {
    /// Server prefers `pipelineId` when both are present.
    #[serde(rename = "pipelineId", skip_serializing_if = "Option::is_none")]
    pub pipeline_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sn: Option<String>,
}

/// `POST /workspace/stop`.
pub async fn stop(client: &Client, body: &WorkspaceTargetBody) -> Result<Value, ApiError> {
    client.request_json(Method::POST, "/workspace/stop", Some(body)).await
}

/// `POST /workspace/delete`.
pub async fn delete(client: &Client, body: &WorkspaceTargetBody) -> Result<Value, ApiError> {
    client.request_json(Method::POST, "/workspace/delete", Some(body)).await
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
    async fn list_passes_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workspace/list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"list":[],"total":0})))
            .mount(&server)
            .await;
        let _ = list(&client(&server), "page=1").await.unwrap();
    }

    #[tokio::test]
    async fn start_sends_branch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/workspace/start"))
            .and(body_partial_json(json!({"branch":"main"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"url":"https://w/123","sn":"sn1"})))
            .mount(&server)
            .await;
        let _ = start(
            &client(&server),
            "cnb/feedback",
            &StartWorkspaceBody {
                branch: Some("main".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn view_rejects_slashed_sn() {
        let server = MockServer::start().await;
        let err = view(&client(&server), "cnb/feedback", "x/y").await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn stop_includes_pipeline_id_with_correct_camelcase() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/workspace/stop"))
            .and(body_partial_json(json!({"pipelineId":"p1"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"sn":"sn1"})))
            .mount(&server)
            .await;
        let _ = stop(
            &client(&server),
            &WorkspaceTargetBody {
                pipeline_id: Some("p1".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
}
