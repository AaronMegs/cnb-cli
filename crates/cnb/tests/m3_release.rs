//! M3: end-to-end coverage for `cnb release …` (incl. two-phase asset upload).

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn release_list_default_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"tag_name":"v1.0.0","name":"first","draft":false,"prerelease":false,"published_at":"2026-04-01"},
            {"tag_name":"v1.1.0-beta","name":"beta","draft":false,"prerelease":true,"published_at":"2026-04-15"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["release", "list", "--repo", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "v1.0.0\tfirst\t\t2026-04-01");
    assert_eq!(lines[1], "v1.1.0-beta\tbeta\tpre\t2026-04-15");
}

#[tokio::test]
async fn release_view_latest_renders_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/releases/latest"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tag_name": "v2.0.0",
            "name": "Big release",
            "body": "Notes here.",
            "published_at": "2026-04-30",
            "assets": [{"name":"foo.tar.gz","size":1024}]
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["release", "view", "--repo", "cnb/feedback", "--latest"])
        .assert()
        .success()
        .stdout(predicate::str::contains("v2.0.0 — Big release"))
        .stdout(predicate::str::contains("Published: 2026-04-30"))
        .stdout(predicate::str::contains("Assets (1):"))
        .stdout(predicate::str::contains("foo.tar.gz (1024 bytes)"))
        .stdout(predicate::str::contains("Notes here."));
}

#[tokio::test]
async fn release_view_requires_exactly_one_specifier() {
    let server = MockServer::start().await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["release", "view", "--repo", "cnb/feedback"])
        .assert()
        .failure()
        .code(3);
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["release", "view", "v1", "--repo", "cnb/feedback", "--latest"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn release_create_with_tag_and_notes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/releases"))
        .and(body_partial_json(json!({"tag_name":"v3.0.0","body":"line1\nline2"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"r-3","tag_name":"v3.0.0"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "release",
            "create",
            "v3.0.0",
            "--repo",
            "cnb/feedback",
            "--notes",
            "line1\nline2",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Created release `v3.0.0`"));
}

#[tokio::test]
async fn release_upload_runs_two_phase_chain() {
    let server = MockServer::start().await;
    let upload_path = "/upload/abc";
    let verify_path = "/verify/def";

    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/releases/r-1/asset-upload-url"))
        .and(body_partial_json(json!({"asset_name":"hello.txt"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "upload_url": format!("{}{}", server.uri(), upload_path),
            "verify_url": format!("{}{}", server.uri(), verify_path),
            "expires_in_sec": 600
        })))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path(upload_path))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(verify_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
        .mount(&server)
        .await;

    // Need a file named hello.txt for the asset_name match.
    let tmpdir = tempfile::tempdir().unwrap();
    let file_path = tmpdir.path().join("hello.txt");
    std::fs::write(&file_path, b"hello world").unwrap();
    let path_str = file_path.to_str().unwrap().to_owned();

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["release", "upload", "r-1", &path_str, "--repo", "cnb/feedback"])
        .assert()
        .success()
        .stderr(predicate::str::contains("↑ uploaded"))
        .stderr(predicate::str::contains("✓ Uploaded 1 file(s) to release r-1"));
}

#[tokio::test]
async fn release_delete_with_yes_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cnb/feedback/-/releases/r-7"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["release", "delete", "r-7", "--repo", "cnb/feedback", "--yes"])
        .assert()
        .success();
}
