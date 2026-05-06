//! `cnb search` — first command backed by the typed SDK (`cnb` crate /
//! `cnb-sdk`).
//!
//! These tests intentionally exercise the **end-to-end wire path** through
//! the SDK:
//!
//! 1. CLI parses argv and dispatches to `commands::search`.
//! 2. `Context::sdk()` resolves the token (file backend, since
//!    `CNB_KEYRING_BACKEND=none`) and the base URL (from `CNB_API_BASE`).
//! 3. `cnb_sdk::ApiClient::search().list_public_repos(...)` builds and sends
//!    `GET /search/public-repos`.
//! 4. Wiremock returns a typed body that the SDK deserialises into
//!    `Vec<Repos4UserBase>`, which the CLI then renders.
//!
//! If any of those wiring decisions break (token plumbing, base URL
//! override, query-string mapping, DTO deserialisation), one of these tests
//! will fail before we let the migration touch a second command.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_hits() -> serde_json::Value {
    // Shape mirrors the generated `Repos4UserBase` DTO — only fields the
    // CLI actually surfaces are required. Notes:
    //   * `id` is `Option<String>`, not integer (schema choice)
    //   * full repo path lives in `path`, not `full_path`
    //   * visibility lives in `visibility_level`, not `visibility`
    json!([
        {
            "id": "1",
            "name": "feedback",
            "path": "cnb/feedback",
            "visibility_level": "public",
            "updated_at": "2026-04-30T10:00:00Z"
        },
        {
            "id": "2",
            "name": "cli",
            "path": "cnb/cli",
            "visibility_level": "public",
            "updated_at": "2026-05-01T08:30:00Z"
        }
    ])
}

#[tokio::test]
async fn search_default_renders_table() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/public-repos"))
        .and(header("authorization", "Bearer fake"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_hits()))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["search", "rust"])
        .assert()
        .success()
        // In a non-TTY assert_cmd run the table renderer emits TSV without
        // a header row (project convention, shared with every other list
        // command). Assert on the data rows instead.
        .stdout(predicate::str::contains("cnb/feedback"))
        .stdout(predicate::str::contains("cnb/cli"))
        .stdout(predicate::str::contains("public"));
}

#[tokio::test]
async fn search_forwards_query_parameters() {
    // Verifies that `--flags`, `--order-by`, `--desc`, `--top-n` and the
    // positional KEY all reach the wire as the SDK's `ListPublicReposQuery`
    // expects. Wiremock will only match if every expected query param
    // shows up with the right value.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/public-repos"))
        .and(query_param("key", "rust"))
        .and(query_param("flags", "build,ci"))
        .and(query_param("order_by", "stars"))
        .and(query_param("desc", "true"))
        .and(query_param("topN", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "search",
            "rust",
            "--flags",
            "build,ci",
            "--order-by",
            "stars",
            "--desc",
            "--top-n",
            "5",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn search_json_emits_full_payload() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/public-repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_hits()))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["search", "rust", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"path\""))
        .stdout(predicate::str::contains("\"cnb/feedback\""));
}

#[tokio::test]
async fn search_jq_filter_applied() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/public-repos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_hits()))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["search", "rust", "--jq", ".[].path"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // jq emits each element on its own line as a JSON string literal.
    assert!(
        stdout.contains("\"cnb/feedback\"") && stdout.contains("\"cnb/cli\""),
        "expected both paths in jq output, got: {stdout}"
    );
}

#[tokio::test]
async fn search_unauthorized_maps_to_exit_4() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/public-repos"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"code":"E_AUTH","message":"bad token"})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "stale")
        .env("CNB_API_BASE", server.uri())
        .args(["search", "rust"])
        .assert()
        .failure()
        .code(4);
}
