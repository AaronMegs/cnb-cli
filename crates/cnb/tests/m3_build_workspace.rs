//! M3: end-to-end coverage for `cnb build …` and `cnb workspace …`.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn build_run_emits_sn() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/build/start"))
        .and(body_partial_json(json!({"branch":"main"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"sn":"sn-123","buildLogUrl":"https://cnb/log/123"})),
        )
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["build", "run", "cnb/feedback", "--branch", "main"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Build triggered: sn=sn-123"))
        .stderr(predicate::str::contains("Logs: https://cnb/log/123"));
}

#[tokio::test]
async fn build_status_single_shot_prints_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/build/status/sn-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status":"running"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["build", "status", "sn-1", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("status: running"));
}

#[tokio::test]
async fn build_cancel_without_yes_aborts_off_tty() {
    let server = MockServer::start().await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["build", "cancel", "sn-1", "cnb/feedback"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn workspace_list_emits_tsv_when_piped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspace/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "list": [
                {"sn":"sn-1","slug":"cnb/feedback","branch":"main","status":"running"},
                {"sn":"sn-2","slug":"cnb/other","branch":"dev","status":"stopped"}
            ],
            "total": 2
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["workspace", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "sn-1\tcnb/feedback\tmain\trunning");
    assert_eq!(lines[1], "sn-2\tcnb/other\tdev\tstopped");
}

#[tokio::test]
async fn workspace_start_no_open_prints_url_to_stdout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/workspace/start"))
        .and(body_partial_json(json!({"branch":"main"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"url":"https://w.cnb/abc","sn":"sn-x"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["workspace", "start", "cnb/feedback", "--branch", "main", "--no-open"])
        .assert()
        .success()
        .stdout(predicate::str::contains("https://w.cnb/abc"));
}

#[tokio::test]
async fn workspace_stop_requires_sn_or_pipeline_id() {
    let server = MockServer::start().await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["workspace", "stop"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn ws_alias_resolves_to_workspace() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/workspace/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"list":[],"total":0})))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["ws", "list"])
        .assert()
        .success();
}
