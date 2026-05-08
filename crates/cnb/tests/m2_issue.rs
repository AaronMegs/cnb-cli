//! M2: end-to-end coverage for `cnb issue …` against a wiremock backend.
//!
//! `list` and `view` are SDK-backed as of Phase 2 step 2.3; mock payloads
//! follow the generated DTO shape — notably `number` is a **string**, not
//! an integer, because the upstream OpenAPI spec aliases issue numbers to
//! strings. The CLI display path tolerates both forms via
//! `format_issue_number`.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn issue_list_jq_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/issues"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"number": "1", "title": "first",  "labels": [{"name":"bug"}], "updated_at": "2026-01-02"},
            {"number": "2", "title": "second", "labels": [],               "updated_at": "2026-01-03"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["issue", "list", "cnb/feedback", "--jq", ".[].number"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    // jq emits each element as a quoted JSON string literal (SDK DTO pins
    // numbers to strings).
    assert_eq!(lines, vec!["\"1\"", "\"2\""], "got: {stdout:?}");
}

#[tokio::test]
async fn issue_view_renders_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/issues/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "number": "42",
            "title": "the answer",
            "state": "open",
            "body": "details here",
            "author": {"username": "alice"}
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["issue", "view", "42", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#42 the answer"))
        .stdout(predicate::str::contains("Author: alice"))
        .stdout(predicate::str::contains("details here"));
}

#[tokio::test]
async fn issue_create_sends_title_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/issues"))
        .and(body_partial_json(json!({"title":"new bug","body":"something broke"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"number":"99"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "issue",
            "create",
            "--title",
            "new bug",
            "--body",
            "something broke",
            "cnb/feedback",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Created #99"));
}

#[tokio::test]
async fn issue_close_sends_state_closed() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cnb/feedback/-/issues/7"))
        .and(body_partial_json(json!({"state":"closed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"number":"7","state":"closed"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["issue", "close", "7", "cnb/feedback"])
        .assert()
        .success();
}
