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
        // SDK 0.2.2's `Visibility` is a real enum with canonical wire form
        // `"Public"` / `"Private"` / `"Secret"`. `format_visibility` now
        // normalises lowercase / legacy "Internal" / integer encodings onto
        // those capitalised strings so the CLI agrees with the SDK.
        .stdout(predicate::str::contains("Visibility:    Public"))
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
    // SDK 0.2.2 canonical Visibility capitalisation; see m2_repo.rs comment
    // on the view test for context.
    assert!(lines[1].contains("alice/second\tstuff\tPrivate\t2026-01-03"));
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

// ============================================================================
// Phase 2 step 2.10 — pin / unpin / list-pinned / contributors
// ============================================================================

#[tokio::test]
async fn repo_list_pinned_renders_path_and_description() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/-/pinned-repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"path": "cnb/feedback", "description": "Feedback repo"},
            {"path": "cnb/docs",     "description": ""}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "list-pinned", "cnb"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "cnb/feedback\tFeedback repo");
    assert_eq!(lines[1], "cnb/docs\t");
}

#[tokio::test]
async fn repo_pin_adds_to_existing_set_via_put() {
    // `pin` first does a GET of the current set, then PUTs the merged
    // set back. Wiremock mounts both mocks; the CLI reads via SDK and
    // writes via `Context::sdk_raw_json` because the SDK does not model
    // the PUT endpoint (SDK-I18).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/-/pinned-repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"path": "cnb/feedback", "description": "Existing"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/cnb/-/pinned-repos"))
        // Keys BTreeSet-sorted, so the expected order is stable:
        // cnb/docs < cnb/feedback lexicographically.
        .and(body_partial_json(json!({"repos": ["cnb/docs", "cnb/feedback"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "pin", "cnb", "cnb/docs"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ cnb pinned set updated (2 entries)"));
}

#[tokio::test]
async fn repo_unpin_removes_from_existing_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/-/pinned-repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"path": "cnb/feedback"},
            {"path": "cnb/docs"}
        ])))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/cnb/-/pinned-repos"))
        .and(body_partial_json(json!({"repos": ["cnb/feedback"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "unpin", "cnb", "cnb/docs"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ cnb pinned set updated (1 entries)"));
}

#[tokio::test]
async fn repo_contributors_typed_call_without_days() {
    // Without --days, CLI calls the typed
    // `RepoContributorClient::get_repo_contributor_trend` path.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/contributor/trend"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user_total": 3,
            "week_total": 4
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "contributors", "cnb/feedback"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["user_total"], 3);
    assert_eq!(v["week_total"], 4);
}

#[tokio::test]
async fn repo_contributors_raw_passthrough_with_days() {
    // With --days, CLI routes through `Context::sdk_raw_get` so the
    // query string is forwarded verbatim — the SDK's typed query
    // struct does not expose `days`. See SDK-I17.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/feedback/-/contributor/trend"))
        .and(wiremock::matchers::query_param("days", "30"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user_total": 7
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["repo", "contributors", "cnb/feedback", "--days", "30"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["user_total"], 7);
}
