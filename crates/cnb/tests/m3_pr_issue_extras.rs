//! M3: end-to-end coverage for `cnb pr review/checks/batch` and
//! `cnb issue activity/properties`.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn pr_review_approve() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/pulls/7/reviews"))
        .and(body_partial_json(json!({"event":"approve","body":"LGTM"})))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["pr", "review", "7", "cnb/feedback", "--approve", "--body", "LGTM"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Submitted `approve` review on PR #7"));
}

#[tokio::test]
async fn pr_review_requires_one_event() {
    let server = MockServer::start().await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["pr", "review", "7", "cnb/feedback"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn pr_checks_returns_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/pulls/7/commit-statuses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "sha":"abc","state":"success","statuses":[{"name":"ci","state":"success"}]
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["pr", "checks", "7", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"state\""))
        .stdout(predicate::str::contains("\"success\""));
}

#[tokio::test]
async fn pr_batch_emits_query_params() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/pull-in-batch"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"number":1},{"number":2}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["pr", "batch", "1", "2", "--repo", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"number\""));
}

#[tokio::test]
async fn issue_activity_emits_table() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/issues/42/activities"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"type":"comment","actor":{"username":"alice"},"submitted_at":"2026-04-01"},
            {"type":"label_added","actor":{"username":"bob"},"submitted_at":"2026-04-02"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["issue", "activity", "42", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "comment\talice\t2026-04-01");
    assert_eq!(lines[1], "label_added\tbob\t2026-04-02");
}

#[tokio::test]
async fn issue_properties_get_lists_table() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/issues/42/property"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"key":"sev","name":"Severity","value":"high"},
            {"key":"area","name":"Area","value":"backend"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["issue", "properties", "42", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "sev\tSeverity\thigh");
    assert_eq!(lines[1], "area\tArea\tbackend");
}

#[tokio::test]
async fn issue_properties_set_writes_patch() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cnb/feedback/-/issues/42/property"))
        .and(body_partial_json(json!({
            "properties":[{"key":"sev","value":"low"},{"key":"area","value":"frontend"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "issue",
            "properties",
            "42",
            "cnb/feedback",
            "--set",
            "sev=low",
            "--set",
            "area=frontend",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Updated 2 property/properties on #42"));
}
