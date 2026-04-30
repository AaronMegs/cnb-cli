//! `Pulls` service for `cnb pr …` (M2 §8.4). `mr` is a CLI-level alias.

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

/// `GET /{repo}/-/pulls`.
pub async fn list(client: &Client, repo: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{repo}/-/pulls")
    } else {
        format!("/{repo}/-/pulls?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{repo}/-/pulls/{number}`.
pub async fn view(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/pulls/{number}"), None::<&()>)
        .await
}

/// Body for `POST /{repo}/-/pulls`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreatePullBody {
    pub title: String,
    pub source_branch: String,
    pub target_branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_repo: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub assignees: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub labels: Vec<String>,
}

/// `POST /{repo}/-/pulls`.
pub async fn create(client: &Client, repo: &str, body: &CreatePullBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/pulls"), Some(body))
        .await
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct EditPullBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// `"open"` or `"closed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_branch: Option<String>,
}

/// `PATCH /{repo}/-/pulls/{number}`.
pub async fn edit(client: &Client, repo: &str, number: u64, body: &EditPullBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::PATCH, &format!("/{repo}/-/pulls/{number}"), Some(body))
        .await
}

pub async fn close(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    let body = EditPullBody {
        state: Some("closed".into()),
        ..Default::default()
    };
    edit(client, repo, number, &body).await
}

pub async fn reopen(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    let body = EditPullBody {
        state: Some("open".into()),
        ..Default::default()
    };
    edit(client, repo, number, &body).await
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentBody<'a> {
    pub body: &'a str,
}

/// `POST /{repo}/-/pulls/{number}/comments`.
pub async fn comment(client: &Client, repo: &str, number: u64, body: &str) -> Result<Value, ApiError> {
    let payload = CommentBody { body };
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/pulls/{number}/comments"),
            Some(&payload),
        )
        .await
}

/// `GET /{repo}/-/pulls/{number}/files` — files changed in a PR.
pub async fn files(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/pulls/{number}/files"), None::<&()>)
        .await
}

/// `GET /{repo}/-/pulls/{number}/commits` — commits in a PR.
pub async fn commits(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/pulls/{number}/commits"), None::<&()>)
        .await
}

#[derive(Debug, Clone, Serialize)]
pub struct AssigneesBody<'a> {
    pub assignees: &'a [String],
}

pub async fn add_assignees(client: &Client, repo: &str, number: u64, assignees: &[String]) -> Result<Value, ApiError> {
    for a in assignees {
        ensure_no_slash(a, "assignee")?;
    }
    let payload = AssigneesBody { assignees };
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/pulls/{number}/assignees"),
            Some(&payload),
        )
        .await
}

pub async fn remove_assignees(
    client: &Client,
    repo: &str,
    number: u64,
    assignees: &[String],
) -> Result<Value, ApiError> {
    for a in assignees {
        ensure_no_slash(a, "assignee")?;
    }
    let payload = AssigneesBody { assignees };
    client
        .request_json(
            Method::DELETE,
            &format!("/{repo}/-/pulls/{number}/assignees"),
            Some(&payload),
        )
        .await
}

#[derive(Debug, Clone, Serialize)]
pub struct LabelsBody<'a> {
    pub labels: &'a [String],
}

pub async fn add_labels(client: &Client, repo: &str, number: u64, labels: &[String]) -> Result<Value, ApiError> {
    let payload = LabelsBody { labels };
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/pulls/{number}/labels"),
            Some(&payload),
        )
        .await
}

pub async fn remove_label(client: &Client, repo: &str, number: u64, name: &str) -> Result<Value, ApiError> {
    ensure_no_slash(name, "label name")?;
    client
        .request_json(
            Method::DELETE,
            &format!("/{repo}/-/pulls/{number}/labels/{name}"),
            None::<&()>,
        )
        .await
}

/// Body for `PUT /{repo}/-/pulls/{number}/merge`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct MergeBody {
    /// `"merge"` (default), `"squash"`, or `"rebase"`. Server validates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
    /// Delete source branch after merge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_source_branch: Option<bool>,
}

/// `PUT /{repo}/-/pulls/{number}/merge`.
pub async fn merge(client: &Client, repo: &str, number: u64, body: &MergeBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::PUT, &format!("/{repo}/-/pulls/{number}/merge"), Some(body))
        .await
}

// ---------------------------------------------------------------------------
//                        M3: reviews / checks / batch
// ---------------------------------------------------------------------------

/// `GET /{repo}/-/pulls/{number}/reviews`.
pub async fn list_reviews(client: &Client, repo: &str, number: u64, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{repo}/-/pulls/{number}/reviews")
    } else {
        format!("/{repo}/-/pulls/{number}/reviews?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// Body for `POST /{repo}/-/pulls/{number}/reviews`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateReviewBody {
    /// `"approve"` | `"comment"` | `"request_changes"` | `"pending"`.
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

pub async fn create_review(
    client: &Client,
    repo: &str,
    number: u64,
    body: &CreateReviewBody,
) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/pulls/{number}/reviews"), Some(body))
        .await
}

/// `GET /{repo}/-/pulls/{number}/commit-statuses`.
pub async fn checks(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    client
        .request_json(
            Method::GET,
            &format!("/{repo}/-/pulls/{number}/commit-statuses"),
            None::<&()>,
        )
        .await
}

/// `GET /{repo}/-/pull-in-batch?n=N1&n=N2&...`.
pub async fn batch(client: &Client, repo: &str, numbers: &[u64]) -> Result<Value, ApiError> {
    use std::fmt::Write;
    if numbers.is_empty() {
        return Err(ApiError::InvalidUrl("batch: numbers must not be empty".into()));
    }
    let mut q = String::new();
    for (i, n) in numbers.iter().enumerate() {
        if i > 0 {
            q.push('&');
        }
        write!(&mut q, "n={n}").expect("write to String");
    }
    client
        .request_json(Method::GET, &format!("/{repo}/-/pull-in-batch?{q}"), None::<&()>)
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
    async fn create_includes_branches() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/pulls"))
            .and(body_partial_json(
                json!({"source_branch":"feat/x","target_branch":"main"}),
            ))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"number":7})))
            .mount(&server)
            .await;
        let _ = create(
            &client(&server),
            "cnb/feedback",
            &CreatePullBody {
                title: "feat".into(),
                source_branch: "feat/x".into(),
                target_branch: "main".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn merge_uses_put() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/cnb/feedback/-/pulls/7/merge"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"merged":true})))
            .mount(&server)
            .await;
        let _ = merge(
            &client(&server),
            "cnb/feedback",
            7,
            &MergeBody {
                merge_method: Some("squash".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }
}
