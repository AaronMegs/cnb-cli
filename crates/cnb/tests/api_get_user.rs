//! `CNB_TOKEN=... cnb api /user` → success path with JSON output, --silent, -i.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount_user_ok(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", "Bearer fake"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-request-id", "rid-test")
                .set_body_json(json!({"username":"alice","email":"a@x"})),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn api_get_user_prints_json() {
    let server = MockServer::start().await;
    mount_user_ok(&server).await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["api", "/user"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"username\""))
        .stdout(predicate::str::contains("alice"));
}

#[tokio::test]
async fn api_silent_suppresses_body() {
    let server = MockServer::start().await;
    mount_user_ok(&server).await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["api", "/user", "--silent"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.is_empty(), "expected empty stdout, got: {stdout}");
}

#[tokio::test]
async fn api_include_prints_response_headers() {
    let server = MockServer::start().await;
    mount_user_ok(&server).await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["api", "/user", "-i"])
        .assert()
        .success()
        .stdout(predicate::str::contains("HTTP/1.1 200"))
        .stdout(predicate::str::contains("x-request-id: rid-test"));
}

#[tokio::test]
async fn api_jq_filter_applied() {
    let server = MockServer::start().await;
    mount_user_ok(&server).await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["api", "/user", "--jq", ".username"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert_eq!(stdout.trim(), "\"alice\"");
}
