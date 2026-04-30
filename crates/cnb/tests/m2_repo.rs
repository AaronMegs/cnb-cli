//! M2: end-to-end coverage for `cnb repo …` against a wiremock backend.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn repo_view_default_card_output() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "feedback",
            "path": "cnb/feedback",
            "description": "the official feedback repo",
            "visibility_level": 0,
            "default_branch": "main",
            "last_activity_at": "2026-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "view", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("cnb/feedback"))
        .stdout(predicate::str::contains("Visibility:    public"))
        .stdout(predicate::str::contains("Default branch: main"))
        .stdout(predicate::str::contains("the official feedback repo"));
}

#[tokio::test]
async fn repo_view_json_passthrough() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "feedback",
            "path": "cnb/feedback",
            "visibility_level": 20
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "view", "cnb/feedback", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"visibility_level\""))
        .stdout(predicate::str::contains("20"));
}

#[tokio::test]
async fn repo_list_user_emits_tsv_when_piped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/alice/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name": "first", "path": "alice/first", "visibility_level": 0, "last_activity_at": "2026-01-02"},
            {"name": "second", "path": "alice/second", "description": "stuff", "visibility_level": 20, "last_activity_at": "2026-01-03"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "list", "alice"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // Piped (assert_cmd is non-TTY): each row is TSV with no header.
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "got: {stdout:?}");
    assert!(lines[0].starts_with("alice/first\t"));
    assert!(lines[1].contains("alice/second\tstuff\tprivate\t2026-01-03"));
}

#[tokio::test]
async fn repo_delete_without_yes_and_no_tty_aborts() {
    // `--yes` is missing AND stdin is not a TTY (assert_cmd default) → must
    // refuse with `BadArgs` (exit 3), not silently delete.
    let server = MockServer::start().await;
    // Mount nothing — if we accidentally hit the API the request would 404
    // and the test would still fail, so this is a safety net.

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "delete", "cnb/feedback"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn repo_delete_with_yes_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cnb/feedback"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "delete", "cnb/feedback", "--yes"])
        .assert()
        .success();
}
