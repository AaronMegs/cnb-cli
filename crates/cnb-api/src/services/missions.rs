//! `Missions` service for `cnb mission …` (M4 §8.9).

use reqwest::Method;
use serde_json::Value;

use crate::client::Client;
use crate::error::ApiError;

/// `GET /{slug}/-/missions`.
pub async fn list(client: &Client, slug: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{slug}/-/missions")
    } else {
        format!("/{slug}/-/missions?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `DELETE /{mission}`.
pub async fn delete(client: &Client, mission: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::DELETE, &format!("/{mission}"), None::<&()>)
        .await
}

/// `GET /{mission}/-/mission/view-list`.
pub async fn view_list(client: &Client, mission: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{mission}/-/mission/view-list"), None::<&()>)
        .await
}

/// `PUT /{mission}/-/mission/view-list` — add or edit a view.
pub async fn put_view_list(client: &Client, mission: &str, body: &Value) -> Result<Value, ApiError> {
    client
        .request_json(Method::PUT, &format!("/{mission}/-/mission/view-list"), Some(body))
        .await
}

/// `POST /{mission}/-/mission/view-list` — sort the view list.
#[derive(Debug, serde::Serialize)]
pub struct SortViewListBody {
    pub ids: Vec<String>,
}

pub async fn sort_view_list(client: &Client, mission: &str, ids: Vec<String>) -> Result<Value, ApiError> {
    let body = SortViewListBody { ids };
    client
        .request_json(Method::POST, &format!("/{mission}/-/mission/view-list"), Some(&body))
        .await
}

/// `GET /{mission}/-/mission/view`.
pub async fn get_view(client: &Client, mission: &str, query: &str) -> Result<Value, ApiError> {
    let p = if query.is_empty() {
        format!("/{mission}/-/mission/view")
    } else {
        format!("/{mission}/-/mission/view?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `POST /{mission}/-/mission/view`.
pub async fn set_view(client: &Client, mission: &str, body: &Value) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{mission}/-/mission/view"), Some(body))
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
    async fn view_list_returns_value() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/m1/-/mission/view-list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"views": []})))
            .mount(&server)
            .await;
        let _ = view_list(&client(&server), "cnb/m1").await.unwrap();
    }

    #[tokio::test]
    async fn sort_sends_ids() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/m1/-/mission/view-list"))
            .and(body_partial_json(json!({"ids": ["a", "b"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let _ = sort_view_list(&client(&server), "cnb/m1", vec!["a".into(), "b".into()])
            .await
            .unwrap();
    }
}
