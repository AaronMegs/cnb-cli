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

#[tokio::test]
async fn build_logs_downloads_plain_text_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/build/runner/download/log/p-1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("hello logs"))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["build", "logs", "p-1", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello logs"));
}

#[tokio::test]
async fn build_logs_rejects_slashed_pipeline_id() {
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", "http://127.0.0.1:1") // unreachable — exit 3 first
        .args(["build", "logs", "evil/path", "cnb/feedback"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn build_list_reads_typed_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/build/logs"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {"sn":"sn-a","status":"running","sourceRef":"main","createTime":"2026-05-01"},
                {"sn":"sn-b","status":"success","targetRef":"dev","createTime":"2026-05-02"}
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
        .args(["build", "list", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "sn-a\trunning\tmain\t2026-05-01");
    assert_eq!(lines[1], "sn-b\tsuccess\tdev\t2026-05-02");
}

#[tokio::test]
async fn build_cancel_with_yes_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/build/stop/sn-9"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"sn":"sn-9","success":true})))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["build", "cancel", "sn-9", "cnb/feedback", "--yes"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Cancelled build sn-9"));
}

#[tokio::test]
async fn build_crontab_sync_hits_post_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/build/crontab/sync/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"code":0,"message":"ok"})))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["build", "crontab-sync", "main", "cnb/feedback"])
        .assert()
        .success();
}

#[tokio::test]
async fn workspace_delete_with_yes_and_pipeline_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/workspace/delete"))
        .and(body_partial_json(json!({"pipelineId":"p-77"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"code":0})))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["workspace", "delete", "--pipeline-id", "p-77", "--yes"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Deleted workspace (p-77)"));
}

#[tokio::test]
async fn workspace_view_card_lists_channels() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/workspace/detail/sn-7"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "webide": "https://w.cnb/webide",
            "remoteSsh": "vscode://remote-ssh/connect?host=w-sn-7",
            "ssh": "ssh user@cnb:12345",
            "jumpUrl": "https://cnb/jump"
        })))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["workspace", "view", "--sn", "sn-7", "cnb/feedback"])
        .assert()
        .success()
        .stdout(predicate::str::contains("webide"))
        .stdout(predicate::str::contains("https://w.cnb/webide"))
        .stdout(predicate::str::contains("remoteSsh"))
        .stdout(predicate::str::contains("jumpUrl"));
}
