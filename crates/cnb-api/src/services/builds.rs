//! `Builds` service for `cnb build …` (M3 §8.6).

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

fn ensure_no_slash(c: &str, what: &str) -> Result<(), ApiError> {
    if c.contains('/') {
        return Err(ApiError::InvalidUrl(format!("{what} must not contain `/`: {c:?}")));
    }
    Ok(())
}

/// Body for `POST /{repo}/-/build/start`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct StartBuildBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    /// Inline pipeline yaml content (alternative to branch's default `.cnb.yml`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// Event name; defaults to `api_trigger` when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Free-form environment variables (key→value).
    #[serde(skip_serializing_if = "serde_json::Map::is_empty", default)]
    pub env: serde_json::Map<String, Value>,
    /// `true` to wait until the build is fully scheduled before returning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync: Option<String>,
}

/// `POST /{repo}/-/build/start`.
pub async fn start(client: &Client, repo: &str, body: &StartBuildBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/build/start"), Some(body))
        .await
}

/// `GET /{repo}/-/build/logs` — list pipeline builds.
pub async fn list(client: &Client, repo: &str, query: &str) -> Result<Value, ApiError> {
    let p = if query.is_empty() {
        format!("/{repo}/-/build/logs")
    } else {
        format!("/{repo}/-/build/logs?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{repo}/-/build/status/{sn}`.
pub async fn status(client: &Client, repo: &str, sn: &str) -> Result<Value, ApiError> {
    ensure_no_slash(sn, "build sn")?;
    client
        .request_json(Method::GET, &format!("/{repo}/-/build/status/{sn}"), None::<&()>)
        .await
}

/// `GET /{repo}/-/build/logs/stage/{sn}/{pipelineId}/{stageId}`.
pub async fn stage(
    client: &Client,
    repo: &str,
    sn: &str,
    pipeline_id: &str,
    stage_id: &str,
) -> Result<Value, ApiError> {
    ensure_no_slash(sn, "build sn")?;
    ensure_no_slash(pipeline_id, "pipeline id")?;
    ensure_no_slash(stage_id, "stage id")?;
    client
        .request_json(
            Method::GET,
            &format!("/{repo}/-/build/logs/stage/{sn}/{pipeline_id}/{stage_id}"),
            None::<&()>,
        )
        .await
}

/// `POST /{repo}/-/build/stop/{sn}`.
pub async fn cancel(client: &Client, repo: &str, sn: &str) -> Result<Value, ApiError> {
    ensure_no_slash(sn, "build sn")?;
    client
        .request_json(Method::POST, &format!("/{repo}/-/build/stop/{sn}"), None::<&()>)
        .await
}

/// `DELETE /{repo}/-/build/logs/{sn}`.
pub async fn delete_logs(client: &Client, repo: &str, sn: &str) -> Result<Value, ApiError> {
    ensure_no_slash(sn, "build sn")?;
    client
        .request_json(Method::DELETE, &format!("/{repo}/-/build/logs/{sn}"), None::<&()>)
        .await
}

/// `POST /{repo}/-/build/crontab/sync/{branch}`.
pub async fn crontab_sync(client: &Client, repo: &str, branch: &str) -> Result<Value, ApiError> {
    ensure_no_slash(branch, "branch")?;
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/build/crontab/sync/{branch}"),
            None::<&()>,
        )
        .await
}

/// `GET /{repo}/-/build/runner/download/log/{pipelineId}` — returns raw bytes
/// (the server replies with text/plain; we surface the body verbatim).
pub async fn download_log(client: &Client, repo: &str, pipeline_id: &str) -> Result<String, ApiError> {
    ensure_no_slash(pipeline_id, "pipeline id")?;
    let resp = client
        .request_passthrough(
            Method::GET,
            &format!("/{repo}/-/build/runner/download/log/{pipeline_id}"),
            None,
            &[],
        )
        .await?;
    if !resp.is_success() {
        return Err(resp.into_error());
    }
    Ok(resp.body)
}

/// Single-element pipeline status snapshot (best-effort projection).
#[derive(Debug, Clone, Deserialize)]
pub struct StatusSnapshot {
    #[serde(default)]
    pub status: String,
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
    async fn start_sends_branch() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/build/start"))
            .and(body_partial_json(json!({"branch":"main"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"sn":"abc"})))
            .mount(&server)
            .await;
        let v = start(
            &client(&server),
            "cnb/feedback",
            &StartBuildBody {
                branch: Some("main".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(v["sn"], "abc");
    }

    #[tokio::test]
    async fn status_rejects_slashed_sn() {
        let server = MockServer::start().await;
        let err = status(&client(&server), "cnb/feedback", "evil/path").await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }

    #[tokio::test]
    async fn cancel_uses_post() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/build/stop/abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
            .mount(&server)
            .await;
        let _ = cancel(&client(&server), "cnb/feedback", "abc").await.unwrap();
    }

    #[tokio::test]
    async fn download_log_returns_body_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback/-/build/runner/download/log/p1"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello logs"))
            .mount(&server)
            .await;
        let s = download_log(&client(&server), "cnb/feedback", "p1").await.unwrap();
        assert_eq!(s, "hello logs");
    }
}
