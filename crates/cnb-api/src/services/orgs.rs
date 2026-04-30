//! `Orgs / Members / Followers` service for `cnb org …` (M4 §8.10).

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

/// `GET /user/groups` — orgs/groups the current user belongs to.
pub async fn list(client: &Client, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        "/user/groups".to_owned()
    } else {
        format!("/user/groups?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{group}` — group details.
pub async fn view(client: &Client, group: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{group}"), None::<&()>)
        .await
}

/// `GET /{group}/-/members?role=...`.
pub async fn list_members(client: &Client, group: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{group}/-/members")
    } else {
        format!("/{group}/-/members?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

#[derive(Debug, Serialize)]
pub struct AddMemberBody<'a> {
    pub username: &'a str,
    pub role: &'a str,
}

/// `POST /{group}/-/members/{username}` — add member with role.
///
/// Server treats this as upsert when called with PUT; for clarity we POST a
/// payload that includes the role.
pub async fn add_member(client: &Client, group: &str, username: &str, role: &str) -> Result<Value, ApiError> {
    ensure_no_slash(username, "username")?;
    let body = AddMemberBody { username, role };
    client
        .request_json(Method::POST, &format!("/{group}/-/members/{username}"), Some(&body))
        .await
}

#[derive(Debug, Serialize)]
pub struct EditMemberBody<'a> {
    pub role: &'a str,
}

/// `PUT /{group}/-/members/{username}` — change role.
pub async fn edit_member(client: &Client, group: &str, username: &str, role: &str) -> Result<Value, ApiError> {
    ensure_no_slash(username, "username")?;
    let body = EditMemberBody { role };
    client
        .request_json(Method::PUT, &format!("/{group}/-/members/{username}"), Some(&body))
        .await
}

/// `DELETE /{group}/-/members/{username}`.
pub async fn remove_member(client: &Client, group: &str, username: &str) -> Result<Value, ApiError> {
    ensure_no_slash(username, "username")?;
    client
        .request_json(Method::DELETE, &format!("/{group}/-/members/{username}"), None::<&()>)
        .await
}

/// `GET /users/{username}/followers`.
pub async fn followers(client: &Client, username: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    ensure_no_slash(username, "username")?;
    let p = if query.is_empty() {
        format!("/users/{username}/followers")
    } else {
        format!("/users/{username}/followers?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /users/{username}/following`.
pub async fn following(client: &Client, username: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    ensure_no_slash(username, "username")?;
    let p = if query.is_empty() {
        format!("/users/{username}/following")
    } else {
        format!("/users/{username}/following?{query}")
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
    async fn list_my_groups() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user/groups"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"slug": "cnb"}])))
            .mount(&server)
            .await;
        let _ = list(&client(&server), "").await.unwrap();
    }

    #[tokio::test]
    async fn add_member_includes_role() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/-/members/alice"))
            .and(body_partial_json(json!({"role": "admin"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
            .mount(&server)
            .await;
        let _ = add_member(&client(&server), "cnb", "alice", "admin").await.unwrap();
    }

    #[tokio::test]
    async fn add_member_rejects_slashed_username() {
        let server = MockServer::start().await;
        let err = add_member(&client(&server), "cnb", "evil/x", "read").await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }
}
