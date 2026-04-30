//! `Users` service. M1 only exposes `GET /user` (used by `cnb auth login`).

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::ApiError;

/// Minimal user view as exposed by `GET /user`. CNB returns more fields; we only
/// promise to decode these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// Fetch the user owning the current Bearer token.
pub async fn get_self(client: &Client) -> Result<User, ApiError> {
    client.request_json(reqwest::Method::GET, "/user", None::<&()>).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Client;
    use serde_json::json;
    use std::time::Duration;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn parses_minimum_user() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"username":"alice","email":"a@x"})))
            .mount(&server)
            .await;

        let c = Client::builder()
            .base_url(Url::parse(&server.uri()).unwrap())
            .token("t")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let u = get_self(&c).await.unwrap();
        assert_eq!(u.username, "alice");
        assert_eq!(u.email.as_deref(), Some("a@x"));
    }
}
