//! `cnb auth login --with-token` → validates against /user, persists, status reports user.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn login_with_token_persists_then_status_shows_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(header("authorization", "Bearer test-pat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"username":"alice","email":"a@x"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let base = server.uri();

    // 1) login --with-token
    env.cmd()
        .env("CNB_API_BASE", &base)
        .args(["auth", "login", "--with-token"])
        .write_stdin("test-pat\n")
        .assert()
        .success()
        .stderr(predicate::str::contains("Logged in to cnb.cool as alice"));

    // 2) status
    env.cmd()
        .env("CNB_API_BASE", &base)
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Logged in to cnb.cool as alice"))
        .stdout(predicate::str::contains("Token: ✓ valid"));

    // 3) token (just prints)
    env.cmd()
        .env("CNB_API_BASE", &base)
        .args(["auth", "token"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("test-pat"));
}
