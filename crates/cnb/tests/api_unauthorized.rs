//! `cnb api /user` against a 401 response → exit code 4 + login hint.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn api_returns_exit_4_on_401() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"errcode":16,"errmsg":"not logged in"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "stale")
        .env("CNB_API_BASE", server.uri())
        .args(["api", "/user"])
        .assert()
        .failure()
        .code(4)
        .stderr(predicate::str::contains("cnb auth login"));
}
