//! M4: end-to-end coverage for `cnb registry …`, `cnb mission …`, and
//! `cnb org …`. Phase 2 step 2.9 — all three command groups now run
//! through the typed SDK.

mod common;

use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ----------------------------------------------------------------------------
// registry
// ----------------------------------------------------------------------------

#[tokio::test]
async fn registry_list_emits_tsv_rows() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/-/registries"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"path":"cnb/releases","name":"releases"},
            {"path":"cnb/internal","name":"internal"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["registry", "list", "cnb"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "cnb/releases\treleases");
    assert_eq!(lines[1], "cnb/internal\tinternal");
}

#[tokio::test]
async fn registry_set_visibility_uses_query_param() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/releases/-/settings/set_visibility"))
        .and(query_param("visibility", "public"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["registry", "set-visibility", "cnb/releases", "public"])
        .assert()
        .success()
        .stderr(predicate::str::contains("visibility set to public"));
}

#[tokio::test]
async fn registry_set_visibility_rejects_invalid_value() {
    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        // clap value_parser rejects before any HTTP call
        .args(["registry", "set-visibility", "cnb/releases", "weird"])
        .assert()
        .failure();
}

#[tokio::test]
async fn registry_package_list_with_type_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/releases/-/packages"))
        .and(query_param("type", "npm"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name":"my-lib","package_type":"npm"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["registry", "package", "list", "cnb/releases", "--kind", "npm"])
        .assert()
        .success()
        .stdout(predicate::str::contains("npm\tmy-lib"));
}

#[tokio::test]
async fn registry_package_view_emits_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/releases/-/packages/npm/my-lib"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "npm": {"desc": "the lib", "package": "my-lib"}
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "registry",
            "package",
            "view",
            "cnb/releases",
            "--type",
            "npm",
            "--name",
            "my-lib",
            "--json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // `PackageDetail.npm` is typed as `CommonRegistryPackageDetail`, so
    // only its known fields survive the round-trip. `desc` is one of
    // them; `description` would be dropped silently (documented wart).
    assert_eq!(v["npm"]["desc"], "the lib");
    assert_eq!(v["npm"]["package"], "my-lib");
}

#[tokio::test]
async fn registry_tag_list_uses_raw_passthrough() {
    // The SDK's typed `list_package_tags` returns `models::Tag`
    // (single-object, wrong shape). The CLI deliberately issues the
    // typed call first (response discarded) and then reads the real
    // array shape via `sdk_raw_get`. We mount the response once —
    // `Mock::respond_with` serves every matching request, so both
    // the discarded typed call and the raw re-read succeed.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/releases/-/packages/npm/my-lib/-/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name":"1.0.0","updated_at":"2026-04-01"},
            {"name":"1.1.0","updated_at":"2026-04-15"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args([
            "registry",
            "tag",
            "list",
            "cnb/releases",
            "--type",
            "npm",
            "--name",
            "my-lib",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "1.0.0\t2026-04-01");
    assert_eq!(lines[1], "1.1.0\t2026-04-15");
}

// ----------------------------------------------------------------------------
// mission
// ----------------------------------------------------------------------------

#[tokio::test]
async fn mission_delete_with_yes() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cnb/missions/alpha"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["mission", "delete", "cnb/missions/alpha", "--yes"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Deleted mission cnb/missions/alpha"));
}

#[tokio::test]
async fn mission_view_sort_posts_ids_list() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/missions/alpha/-/mission/view-list"))
        .and(body_partial_json(json!({"ids": ["v1", "v2", "v3"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["mission", "view-sort", "cnb/missions/alpha", "--ids", "v1,v2,v3"])
        .assert()
        .success();
}

#[tokio::test]
async fn mission_view_edit_rejects_malformed_json_file() {
    // File exists but content is not a valid MissionView — the CLI's
    // typed `read_typed_json` should surface a BadArgs (exit 3) before
    // making any HTTP call.
    let tmpdir = tempfile::tempdir().unwrap();
    let bad = tmpdir.path().join("view.json");
    std::fs::write(&bad, b"not even close").unwrap();

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", "http://127.0.0.1:1") // unreachable — exit 3 first
        .args([
            "mission",
            "view-edit",
            "cnb/missions/alpha",
            "--config-file",
            bad.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3);
}

#[tokio::test]
async fn mission_view_list_emits_typed_array() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/missions/alpha/-/mission/view-list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id":"v1","name":"All","type":"table"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["mission", "view-list", "cnb/missions/alpha"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v[0]["id"], "v1");
    assert_eq!(v[0]["name"], "All");
}

// ----------------------------------------------------------------------------
// org
// ----------------------------------------------------------------------------

#[tokio::test]
async fn org_list_emits_slug_from_path_field() {
    // `OrganizationAccess` DTO uses `path` rather than `slug`; the
    // CLI reads it via to_value(..).get("path") for the TSV column.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/user/groups"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"path":"cnb","name":"CNB"},
            {"path":"myorg","name":"My Org"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "cnb\tCNB");
    assert_eq!(lines[1], "myorg\tMy Org");
}

#[tokio::test]
async fn org_view_prints_name_and_description() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "CNB",
            "description": "Cloud-native build"
        })))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "view", "cnb"])
        .assert()
        .success()
        .stdout(predicate::str::contains("CNB"))
        .stdout(predicate::str::contains("Cloud-native build"));
}

#[tokio::test]
async fn org_member_add_posts_access_level_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cnb/-/members/alice"))
        .and(body_partial_json(json!({"access_level": "admin"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "member", "add", "cnb", "alice", "--role", "admin"])
        .assert()
        .success()
        .stderr(predicate::str::contains("✓ Added alice to cnb as admin"));
}

#[tokio::test]
async fn org_member_edit_puts_access_level_body() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/cnb/-/members/alice"))
        .and(body_partial_json(json!({"access_level": "write"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "member", "edit", "cnb", "alice", "--role", "write"])
        .assert()
        .success();
}

#[tokio::test]
async fn org_member_list_uses_typed_access_level() {
    // After the SDK port, the CLI reads `access_level` (the canonical
    // DTO field). The legacy `role` key that the cnb-api facade
    // previously tolerated is NOT a fallback anymore — a deliberate
    // simplification that matches every other typed-first port.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cnb/-/members"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"username":"alice","access_level":"admin"},
            {"username":"bob","access_level":"read"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "member", "list", "cnb"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "alice\tadmin");
    assert_eq!(lines[1], "bob\tread");
}

#[tokio::test]
async fn org_follower_falls_back_to_current_user() {
    let server = MockServer::start().await;
    // Step 1: the CLI probes `/user` to resolve the current username.
    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"username":"me"})))
        .mount(&server)
        .await;
    // Step 2: followers of `me`.
    Mock::given(method("GET"))
        .and(path("/users/me/followers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"username":"alice","nickname":"Alice A."}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    env.cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "follower"])
        .assert()
        .success()
        .stdout(predicate::str::contains("alice\tAlice A."));
}

#[tokio::test]
async fn org_following_with_explicit_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/alice/following"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"username":"bob"},
            {"username":"carol","nickname":"Carol"}
        ])))
        .mount(&server)
        .await;

    let env = common::TestEnv::new();
    let assert = env
        .cmd()
        .env("CNB_TOKEN", "fake")
        .env("CNB_API_BASE", server.uri())
        .args(["org", "following", "alice"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "bob\t");
    assert_eq!(lines[1], "carol\tCarol");
}
