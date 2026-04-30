//! M2: end-to-end coverage for `cnb label …` and `cnb pr …`.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn label_list_tsv_default() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/labels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name": "bug", "color": "ff0000", "description": "Something broken"},
            {"name": "enhancement", "color": "00ff00"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["label", "list", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "bug\tff0000\tSomething broken");
    assert_eq!(lines[1], "enhancement\t00ff00\t");
}

#[tokio::test]
async fn label_create_with_color_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/labels"))
        .and(body_partial_json(json!({"name":"needs-triage","color":"ff8800"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"name":"needs-triage"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["label", "create", "needs-triage", "cnb/feedback", "--color", "ff8800"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Created label `needs-triage`"));
}

#[tokio::test]
async fn pr_view_renders_branch_arrow() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/pulls/7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "number": 7,
            "title": "feat: shiny",
            "state": "open",
            "source_branch": "feat/shiny",
            "target_branch": "main",
            "body": "Adds the shiny widget."
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["pr", "view", "7", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#7 feat: shiny"))
        .stdout(predicate::str::contains("feat/shiny → main"))
        .stdout(predicate::str::contains("Adds the shiny widget."));
}

#[tokio::test]
async fn pr_merge_with_yes_uses_put() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/cnb/feedback/-/pulls/7/merge"))
        .and(body_partial_json(json!({"merge_method":"squash"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"merged":true})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["pr", "merge", "7", "cnb/feedback", "--method", "squash", "--yes"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Merged PR #7"));
}

#[tokio::test]
async fn mr_alias_resolves_to_pr() {
    // `cnb mr` should be equivalent to `cnb pr`.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/pulls/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "number": 1, "title": "hi", "state": "open",
            "source_branch": "f", "target_branch": "main"
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["mr", "view", "1", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#1 hi"));
}
