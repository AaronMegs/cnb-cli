//! M2: end-to-end coverage for `cnb repo …` against a wiremock backend.
//!
//! `view` / `list` are SDK-backed as of Phase 2; their mock payloads
//! therefore model `Repos4User` faithfully — notably `visibility_level`
//! is a **string** (the upstream spec aliases `Visibility = String`) and
//! timestamps sit in `updated_at`, not `last_activity_at`.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
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
            "visibility_level": "public",
            "default_branch": "main",
            "updated_at": "2026-01-01T00:00:00Z"
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
            "visibility_level": "private"
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
        .stdout(predicate::str::contains("\"private\""));
}

#[tokio::test]
async fn repo_list_user_emits_tsv_when_piped() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/alice/repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name": "first",  "path": "alice/first",  "visibility_level": "public",
             "updated_at": "2026-01-02"},
            {"name": "second", "path": "alice/second", "description": "stuff",
             "visibility_level": "private", "updated_at": "2026-01-03"}
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
// ---------------------------------------------------------------------------
// Phase 2 step 2.6 — write-path coverage backed by the typed SDK.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn repo_create_sends_post_with_visibility_string() {
    // SDK's `CreateRepoReq` types `visibility` as `Option<String>` (the
    // upstream alias `Visibility = String`). The CLI takes one of
    // `public|internal|private` and forwards verbatim — verify the body
    // shape.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/alice/-/repos"))
        .and(body_partial_json(json!({
            "name": "widget",
            "description": "shiny new widget",
            "visibility": "private"
        })))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({"path": "alice/widget", "visibility_level": "private"})),
        )
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "repo",
            "create",
            "alice/widget",
            "--description",
            "shiny new widget",
            "--visibility",
            "private",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Created alice/widget"));
}

#[tokio::test]
async fn repo_create_with_default_branch_is_rejected() {
    // SDK-I11: `CreateRepoReq` does not include `default_branch`. The
    // CLI surfaces the gap explicitly rather than silently dropping the
    // flag.
    let server = MockServer::start().await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "create", "alice/widget", "--default-branch", "main"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn repo_edit_sends_patch_description() {
    // `RepoPatch` only models description / license / site / topics
    // (SDK-I11). The CLI accepts only --description; rename and
    // default-branch are rejected with exit 3.
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cnb/feedback"))
        .and(body_partial_json(json!({"description": "freshly polished"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "edit", "cnb/feedback", "--description", "freshly polished"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Updated cnb/feedback"));
}

#[tokio::test]
async fn repo_edit_rejects_rename() {
    let server = MockServer::start().await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "edit", "cnb/feedback", "--name", "newname"])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn repo_archive_posts_to_settings_archive() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/settings/archive"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "archive", "cnb/feedback"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Archived cnb/feedback"));
}

#[tokio::test]
async fn repo_unarchive_posts_to_settings_unarchive() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/settings/unarchive"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "unarchive", "cnb/feedback"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Unarchived cnb/feedback"));
}

#[tokio::test]
async fn repo_transfer_with_yes_sends_target_in_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/transfer"))
        .and(body_partial_json(json!({"target": "newowner"})))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "transfer", "cnb/feedback", "--to", "newowner", "--yes"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Transferred cnb/feedback → newowner"));
}

#[tokio::test]
async fn repo_set_visibility_uses_query_string() {
    // SDK-I12: SDK's set_repo_visibility sends `visibility` as a query
    // parameter (NOT a body). Wiremock will only match the request if
    // the query param is present with the right value.
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/feedback/-/settings/set_visibility"))
        .and(wiremock::matchers::query_param("visibility", "internal"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "set-visibility", "internal", "--repo", "cnb/feedback"])
        .assert()
        .success()
        .stderr(predicate::str::contains("visibility set to internal"));
}

#[tokio::test]
async fn repo_fork_unwraps_listforks_envelope() {
    // SDK-I13: list_forks_repos returns
    // `ListForks { fork_tree_count, forks: Option<Vec<Forks>> }` rather
    // than a plain `Vec`. The CLI unwraps `.forks` so --json output
    // remains a bare array (matching gh CLI's shape).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/forks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "fork_tree_count": 2,
            "forks": [
                {"path": "alice/feedback-fork", "updated_at": "2026-04-01"},
                {"path": "bob/feedback-fork",   "updated_at": "2026-04-02"}
            ]
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "fork", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "expected 2 fork rows, got: {stdout:?}");
    assert!(lines[0].contains("alice/feedback-fork"));
    assert!(lines[1].contains("bob/feedback-fork"));
}
