//! `Releases` service for `cnb release …` (M3 §8.5).
//!
//! ## Asset upload (two-phase, pre-signed URL flow)
//!
//! 1. `POST /{repo}/-/releases/{release_id}/asset-upload-url` with metadata
//!    (`asset_name`, `size`, optional `overwrite`/`ttl`) → CNB returns
//!    `{upload_url, verify_url, expires_in_sec}`.
//! 2. `PUT <upload_url>` with the file bytes (streamed from disk).
//! 3. `POST <verify_url>` to commit the upload (verify_url already includes
//!    `{upload_token}/{asset_path}` segments).
//!
//! Steps 2 + 3 use absolute URLs returned by step 1 — bypassing
//! [`Client::request_*`] which are base-relative — so we go through
//! [`Client::http`] directly with auth headers stripped (the pre-signed URL
//! authorizes by token in its query string, not Authorization header; sending
//! an extra header is harmless but signing services dislike it).

use std::path::Path;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::client::Client;
use crate::error::ApiError;

fn ensure_no_slash(c: &str, what: &str) -> Result<(), ApiError> {
    if c.contains('/') {
        return Err(ApiError::InvalidUrl(format!("{what} must not contain `/`: {c:?}")));
    }
    Ok(())
}

/// `GET /{repo}/-/releases?page=...&page_size=...`.
pub async fn list(client: &Client, repo: &str, query: &str) -> Result<Vec<Value>, ApiError> {
    let p = if query.is_empty() {
        format!("/{repo}/-/releases")
    } else {
        format!("/{repo}/-/releases?{query}")
    };
    client.request_json(Method::GET, &p, None::<&()>).await
}

/// `GET /{repo}/-/releases/latest`.
pub async fn latest(client: &Client, repo: &str) -> Result<Value, ApiError> {
    client
        .request_json(Method::GET, &format!("/{repo}/-/releases/latest"), None::<&()>)
        .await
}

/// `GET /{repo}/-/releases/tags/{tag}`.
pub async fn view_by_tag(client: &Client, repo: &str, tag: &str) -> Result<Value, ApiError> {
    ensure_no_slash(tag, "release tag")?;
    client
        .request_json(Method::GET, &format!("/{repo}/-/releases/tags/{tag}"), None::<&()>)
        .await
}

/// `GET /{repo}/-/releases/{release_id}`.
pub async fn view_by_id(client: &Client, repo: &str, release_id: &str) -> Result<Value, ApiError> {
    ensure_no_slash(release_id, "release id")?;
    client
        .request_json(Method::GET, &format!("/{repo}/-/releases/{release_id}"), None::<&()>)
        .await
}

/// Body for `POST /{repo}/-/releases`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CreateReleaseBody {
    pub tag_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
    /// `"true"` | `"false"` | `"legacy"` (string per CNB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub make_latest: Option<String>,
    /// SHA or branch name to tag against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_commitish: Option<String>,
}

pub async fn create(client: &Client, repo: &str, body: &CreateReleaseBody) -> Result<Value, ApiError> {
    client
        .request_json(Method::POST, &format!("/{repo}/-/releases"), Some(body))
        .await
}

/// Body for `PATCH /{repo}/-/releases/{release_id}`.
#[derive(Debug, Clone, Serialize, Default)]
pub struct EditReleaseBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prerelease: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub make_latest: Option<String>,
}

pub async fn edit(client: &Client, repo: &str, release_id: &str, body: &EditReleaseBody) -> Result<Value, ApiError> {
    ensure_no_slash(release_id, "release id")?;
    client
        .request_json(Method::PATCH, &format!("/{repo}/-/releases/{release_id}"), Some(body))
        .await
}

pub async fn delete(client: &Client, repo: &str, release_id: &str) -> Result<Value, ApiError> {
    ensure_no_slash(release_id, "release id")?;
    client
        .request_json(Method::DELETE, &format!("/{repo}/-/releases/{release_id}"), None::<&()>)
        .await
}

/// `GET /{repo}/-/releases/{release_id}/assets/{asset_id}`.
pub async fn view_asset(client: &Client, repo: &str, release_id: &str, asset_id: &str) -> Result<Value, ApiError> {
    ensure_no_slash(release_id, "release id")?;
    ensure_no_slash(asset_id, "asset id")?;
    client
        .request_json(
            Method::GET,
            &format!("/{repo}/-/releases/{release_id}/assets/{asset_id}"),
            None::<&()>,
        )
        .await
}

/// `DELETE /{repo}/-/releases/{release_id}/assets/{asset_id}`.
pub async fn delete_asset(client: &Client, repo: &str, release_id: &str, asset_id: &str) -> Result<Value, ApiError> {
    ensure_no_slash(release_id, "release id")?;
    ensure_no_slash(asset_id, "asset id")?;
    client
        .request_json(
            Method::DELETE,
            &format!("/{repo}/-/releases/{release_id}/assets/{asset_id}"),
            None::<&()>,
        )
        .await
}

// ============================================================================
//                          Two-phase asset upload
// ============================================================================

#[derive(Debug, Clone, Serialize)]
struct AssetUploadUrlReqBody<'a> {
    asset_name: &'a str,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    overwrite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<u32>,
}

/// CNB's response for `POST .../asset-upload-url`.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetUploadUrl {
    pub upload_url: String,
    pub verify_url: String,
    #[serde(default)]
    pub expires_in_sec: i64,
}

/// Upload one local file to a release via the two-phase pre-signed URL flow.
///
/// Returns the parsed body of the verification call (typically `{}` on success).
pub async fn upload_asset(
    client: &Client,
    repo: &str,
    release_id: &str,
    path: &Path,
    overwrite: bool,
    ttl_days: Option<u32>,
) -> Result<Value, ApiError> {
    ensure_no_slash(release_id, "release id")?;

    let asset_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ApiError::InvalidUrl(format!("invalid file path: {path:?}")))?
        .to_owned();
    let meta = tokio::fs::metadata(path).await?;
    let size = meta.len();

    // Phase 1: ask for an upload URL.
    let req = AssetUploadUrlReqBody {
        asset_name: &asset_name,
        size,
        overwrite: if overwrite { Some(true) } else { None },
        ttl: ttl_days,
    };
    let url_info: AssetUploadUrl = client
        .request_json(
            Method::POST,
            &format!("/{repo}/-/releases/{release_id}/asset-upload-url"),
            Some(&req),
        )
        .await?;

    // Phase 2: stream-PUT the file bytes to the pre-signed URL.
    let file = File::open(path).await?;
    let stream = ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);
    let put_resp = client
        .http()
        .put(&url_info.upload_url)
        .header(reqwest::header::CONTENT_LENGTH, size)
        .body(body)
        .send()
        .await?;
    let put_status = put_resp.status();
    if !put_status.is_success() {
        let text = put_resp.text().await.unwrap_or_default();
        return Err(ApiError::from_http(put_status.as_u16(), &text, None));
    }

    // Phase 3: confirm. The verify_url is absolute; route via http() too.
    let verify_resp = client.http().post(&url_info.verify_url).send().await?;
    let verify_status = verify_resp.status();
    let verify_body = verify_resp.text().await.unwrap_or_default();
    if !verify_status.is_success() {
        return Err(ApiError::from_http(verify_status.as_u16(), &verify_body, None));
    }
    Ok(serde_json::from_str(&verify_body).unwrap_or(Value::Null))
}

/// `GET /{repo}/-/releases/download/{tag}/{filename}` — follows the 302
/// redirect emitted by CNB and streams the response body into `dest_dir`.
///
/// Returns the absolute path of the written file.
pub async fn download_asset(
    client: &Client,
    repo: &str,
    tag: &str,
    filename: &str,
    dest_dir: &Path,
) -> Result<std::path::PathBuf, ApiError> {
    ensure_no_slash(tag, "release tag")?;
    ensure_no_slash(filename, "filename")?;

    // Allow redirects (the default) — CNB returns 302 to a signed download URL.
    let resp = client
        .request_passthrough(
            Method::GET,
            &format!("/{repo}/-/releases/download/{tag}/{filename}"),
            None,
            &[],
        )
        .await?;
    if !resp.is_success() {
        return Err(resp.into_error());
    }

    tokio::fs::create_dir_all(dest_dir).await?;
    let dest = dest_dir.join(filename);
    tokio::fs::write(&dest, resp.body.as_bytes()).await?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;
    use url::Url;
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> Client {
        Client::builder()
            .base_url(Url::parse(&server.uri()).unwrap())
            .token("t")
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn list_uses_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback/-/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"id":"r1"}])))
            .mount(&server)
            .await;
        let v = list(&client(&server), "cnb/feedback", "page=1").await.unwrap();
        assert_eq!(v.len(), 1);
    }

    #[tokio::test]
    async fn create_sends_tag_name() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/releases"))
            .and(body_partial_json(json!({"tag_name":"v1.0.0"})))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id":"r1","tag_name":"v1.0.0"})))
            .mount(&server)
            .await;
        let _ = create(
            &client(&server),
            "cnb/feedback",
            &CreateReleaseBody {
                tag_name: "v1.0.0".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn view_by_tag_url_encoded() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cnb/feedback/-/releases/tags/v1.0.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id":"r1"})))
            .mount(&server)
            .await;
        let _ = view_by_tag(&client(&server), "cnb/feedback", "v1.0.0").await.unwrap();
    }

    #[tokio::test]
    async fn upload_asset_runs_three_phases() {
        let server = MockServer::start().await;
        let upload_path = "/upload/abc";
        let verify_path = "/verify/def";
        // Phase 1
        Mock::given(method("POST"))
            .and(path("/cnb/feedback/-/releases/r1/asset-upload-url"))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "upload_url": format!("{}{}", server.uri(), upload_path),
                "verify_url": format!("{}{}", server.uri(), verify_path),
                "expires_in_sec": 600
            })))
            .mount(&server)
            .await;
        // Phase 2 (PUT)
        Mock::given(method("PUT"))
            .and(path(upload_path))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        // Phase 3
        Mock::given(method("POST"))
            .and(path(verify_path))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
            .mount(&server)
            .await;

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"hello").unwrap();
        let v = upload_asset(&client(&server), "cnb/feedback", "r1", tmp.path(), false, None)
            .await
            .unwrap();
        assert_eq!(v["ok"], true);
    }

    #[tokio::test]
    async fn delete_rejects_slashed_id() {
        let server = MockServer::start().await;
        let err = delete(&client(&server), "cnb/feedback", "evil/path").await.unwrap_err();
        assert!(matches!(err, ApiError::InvalidUrl(_)));
    }
}
