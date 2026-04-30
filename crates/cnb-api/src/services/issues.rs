//! `Issues` service for `cnb issue …` (M2 §8.3).

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

/// `GET /{repo}/-/issues`.
pub async fn list(client: &Client, repo: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{repo}/-/issues")
    } else {
        format!("/{repo}/-/issues?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /user/issues` — issues across repos for the current user.
pub async fn list_self(client: &Client, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        "/user/issues".to_owned()
    } else {
        format!("/user/issues?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{repo}/-/issues/{number}`.
pub async fn view(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/issues/{number}"), None::<&()>)
        .await
}

/// Create body for `POST /{repo}/-/issues`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateIssueBody {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub assignees: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub labels: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

/// `POST /{repo}/-/issues`.
pub async fn create(client: &Client, repo: &str, body: &CreateIssueBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/issues"), Some(body))
        .await
}

/// Edit body for `PATCH /{repo}/-/issues/{number}`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct EditIssueBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// `"open"` or `"closed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
}

/// `PATCH /{repo}/-/issues/{number}`.
pub async fn edit(client: &Client, repo: &str, number: u64, body: &EditIssueBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::PATCH, &format!("/{repo}/-/issues/{number}"), Some(body))
        .await
}

/// Convenience: `close` = PATCH state=closed.
pub async fn close(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    let body = EditIssueBody {
        state: Some("closed".into()),
        ..Default::default()
    };
    edit(client, repo, number, &body).await
}

/// Convenience: `reopen` = PATCH state=open.
pub async fn reopen(client: &Client, repo: &str, number: u64) -> Result<Value, ApiError> {
    let body = EditIssueBody {
        state: Some("open".into()),
        ..Default::default()
    };
    edit(client, repo, number, &body).await
}

/// `GET /{repo}/-/issues/{number}/comments`.
pub async fn list_comments(client: &Client, repo: &str, number: u64) -> Result<Vec<Value>, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/issues/{number}/comments"), None::<&()>)
        .await
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentBody<'a> {
    pub body: &'a str,
}

/// `POST /{repo}/-/issues/{number}/comments`.
pub async fn comment(client: &Client, repo: &str, number: u64, body: &str) -> Result<Value, ApiError> {
    let payload = CommentBody { body };
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/issues/{number}/comments"),
            Some(&payload),
        )
        .await
}

/// `PATCH /{repo}/-/issues/{number}/comments/{comment_id}`.
pub async fn edit_comment(
    client: &Client,
    repo: &str,
    number: u64,
    comment_id: u64,
    body: &str,
) -> Result<Value, ApiError> {
    let payload = CommentBody { body };
    client
        .request_json(
            Method::PATCH,
            &format!("/{repo}/-/issues/{number}/comments/{comment_id}"),
            Some(&payload),
        )
        .await
}

#[derive(Debug, Clone, Serialize)]
pub struct AssigneesBody<'a> {
    pub assignees: &'a [String],
}

/// `POST /{repo}/-/issues/{number}/assignees`.
pub async fn add_assignees(client: &Client, repo: &str, number: u64, assignees: &[String]) -> Result<Value, ApiError> {
    for a in assignees {
        ensure_no_slash(a, "assignee")?;
    }
    let payload = AssigneesBody { assignees };
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/issues/{number}/assignees"),
            Some(&payload),
        )
        .await
}

/// `DELETE /{repo}/-/issues/{number}/assignees`.
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
            &format!("/{repo}/-/issues/{number}/assignees"),
            Some(&payload),
        )
        .await
}

#[derive(Debug, Clone, Serialize)]
pub struct LabelsBody<'a> {
    pub labels: &'a [String],
}

/// `POST /{repo}/-/issues/{number}/labels` — append labels.
pub async fn add_labels(client: &Client, repo: &str, number: u64, labels: &[String]) -> Result<Value, ApiError> {
    let payload = LabelsBody { labels };
    client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/issues/{number}/labels"),
            Some(&payload),
        )
        .await
}

/// `DELETE /{repo}/-/issues/{number}/labels/{name}` — remove single label.
pub async fn remove_label(client: &Client, repo: &str, number: u64, name: &str) -> Result<Value, ApiError> {
    ensure_no_slash(name, "label name")?;
    client
        .request_json(
            Method::DELETE,
            &format!("/{repo}/-/issues/{number}/labels/{name}"),
            None::<&()>,
        )
        .await
}

// ---------------------------------------------------------------------------
//                          M3: activity / property
// ---------------------------------------------------------------------------

/// `GET /{repo}/-/issues/{number}/activities`.
pub async fn list_activities(client: &Client, repo: &str, number: u64, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{repo}/-/issues/{number}/activities")
    } else {
        format!("/{repo}/-/issues/{number}/activities?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{repo}/-/issues/{number}/property`.
pub async fn list_properties(client: &Client, repo: &str, number: u64) -> Result<Vec<Value>, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/issues/{number}/property"), None::<&()>)
        .await
}

/// One property update entry.
#[derive(Debug, Clone, Serialize)]
pub struct PropertyUpdate {
    pub key: String,
    pub value: String,
}

/// Body for `PATCH /{repo}/-/issues/{number}/property`.
#[derive(Debug, Clone, Serialize)]
pub struct SetPropertiesBody {
    pub properties: Vec<PropertyUpdate>,
}

/// `PATCH /{repo}/-/issues/{number}/property`.
pub async fn set_properties(
    client: &Client,
    repo: &str,
    number: u64,
    properties: Vec<PropertyUpdate>,
) -> Result<Value, ApiError> {
    let body = SetPropertiesBody { properties };
    client
        .request_json(
            Method::PATCH,
            &format!("/{repo}/-/issues/{number}/property"),
            Some(&body),
        )
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
    async fn list_passes_query_string() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback/-/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"number":1}])))
            .mount(&server)
            .await;
        let v = list(&client(&server), "cnb/feedback", "state=open").await.unwrap();
        assert_eq!(v.len(), 1);
    }

    #[tokio::test]
    async fn create_sends_title() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/issues"))
            .and(body_partial_json(json!({"title":"bug"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"number":42,"title":"bug"})))
            .mount(&server)
            .await;
        let v = create(
            &client(&server),
            "cnb/feedback",
            &CreateIssueBody {
                title: "bug".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(v["number"], 42);
    }

    #[tokio::test]
    async fn close_sends_state_closed() {
        let server = MockServer::start().await;
        Mock::given(method("PATCH"))
            .and(path("/cnb/feedback/-/issues/42"))
            .and(body_partial_json(json!({"state":"closed"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"number":42,"state":"closed"})))
            .mount(&server)
            .await;
        let _ = close(&client(&server), "cnb/feedback", 42).await.unwrap();
    }

    #[tokio::test]
    async fn add_assignees_rejects_slashed() {
        let server = MockServer::start().await;
        let err = add_assignees(&client(&server), "cnb/feedback", 1, &["evil/path".into()])
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }
}
