//! Two-phase asset upload for `cnb issue create --attach` /
//! `cnb issue comment --attach` (DESIGN §8.3).
//!
//! Migrated from the retired `cnb-api::services::uploads` module.
//! Differences:
//!
//! - Uses the SDK's shared `reqwest::Client` for the multipart POST,
//!   which already carries the `Authorization` and `User-Agent`
//!   default headers — so the call site never sees the bearer token.
//! - Uses `client.http().url(path)` for URL construction, getting the
//!   same `CNB_API_BASE` env precedence and percent-encoding as every
//!   other SDK call.
//! - Errors flow back through [`CliError`] directly (the previous
//!   wrapper bounced through `cnb-api::ApiError`, which has now been
//!   removed).
//!
//! CNB exposes four attachment endpoints; the `Scope`/`Kind` matrix
//! routes per `(scope, kind)`:
//!
//! | scope         | files                                                     | images                                                     |
//! |---------------|-----------------------------------------------------------|------------------------------------------------------------|
//! | repository    | `POST /{repo}/-/upload/files`                             | `POST /{repo}/-/upload/imgs`                               |
//! | issue comment | `POST /{repo}/-/issues/{n}/comment-file-asset-upload-url` | `POST /{repo}/-/issues/{n}/comment-image-asset-upload-url` |
//!
//! Both endpoint families accept `multipart/form-data` with one or
//! more `file` parts. We stream from disk to avoid buffering large
//! attachments.

use std::path::Path;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use serde_json::Value;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::context::Context;
use crate::error::CliError;

/// What kind of asset is being uploaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Image,
}

/// Where to upload the asset.
#[derive(Debug, Clone)]
pub enum Scope<'a> {
    /// Repository-scoped attachment (e.g. attached when creating an issue).
    Repo(&'a str),
    /// Comment on an issue.
    IssueComment { repo: &'a str, number: u64 },
}

impl Scope<'_> {
    fn endpoint(&self, kind: Kind) -> String {
        match (self, kind) {
            (Scope::Repo(r), Kind::File) => format!("/{r}/-/upload/files"),
            (Scope::Repo(r), Kind::Image) => format!("/{r}/-/upload/imgs"),
            (Scope::IssueComment { repo, number }, Kind::File) => {
                format!("/{repo}/-/issues/{number}/comment-file-asset-upload-url")
            }
            (Scope::IssueComment { repo, number }, Kind::Image) => {
                format!("/{repo}/-/issues/{number}/comment-image-asset-upload-url")
            }
        }
    }
}

/// Result of one upload — the URL or markdown reference returned by CNB.
///
/// CNB is inconsistent across endpoints: some return `{"url": "..."}`,
/// others `{"data": {"url": "..."}}`, others a plain string. We
/// normalise to the first non-empty string we can find under the
/// common keys.
#[derive(Debug, Clone)]
pub struct Uploaded {
    pub kind: Kind,
    pub original_name: String,
    pub url: String,
    pub raw: Value,
}

#[derive(Debug, Deserialize)]
struct UrlEnvelope {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    data: Option<Value>,
}

fn extract_url(v: &Value) -> Option<String> {
    if let Ok(env) = serde_json::from_value::<UrlEnvelope>(v.clone()) {
        if let Some(u) = env.url.or(env.download_url) {
            if !u.is_empty() {
                return Some(u);
            }
        }
        if let Some(d) = env.data {
            if let Some(u) = d.get("url").and_then(|x| x.as_str()) {
                if !u.is_empty() {
                    return Some(u.to_owned());
                }
            }
            if let Some(u) = d.get("download_url").and_then(|x| x.as_str()) {
                if !u.is_empty() {
                    return Some(u.to_owned());
                }
            }
        }
    }
    if let Some(s) = v.as_str() {
        return Some(s.to_owned());
    }
    None
}

/// Auto-pick a [`Kind`] for `path` based on its file extension.
pub fn detect_kind(path: &Path) -> Kind {
    if let Some(mime) = mime_guess::from_path(path).first() {
        if mime.type_() == mime_guess::mime::IMAGE {
            return Kind::Image;
        }
    }
    Kind::File
}

/// Stream-upload one local file. The kind is auto-detected unless overridden.
///
/// Errors: file IO, network, or non-2xx responses surface as [`CliError`].
pub async fn upload_one(
    ctx: &mut Context,
    scope: Scope<'_>,
    path: &Path,
    forced_kind: Option<Kind>,
) -> Result<Uploaded, CliError> {
    let kind = forced_kind.unwrap_or_else(|| detect_kind(path));
    let endpoint = scope.endpoint(kind);

    let original_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::Generic(format!("invalid file path: {path:?}")))?
        .to_owned();
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_owned();

    let file = File::open(path).await?;
    let stream = ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);

    let part = Part::stream(body)
        .file_name(original_name.clone())
        .mime_str(&mime_type)
        .map_err(|e| CliError::Generic(format!("invalid mime `{mime_type}`: {e}")))?;
    let form = Form::new().part("file", part);

    let client = ctx.sdk()?;
    // Same base-URL precedence as every other SDK call thanks to the
    // SDK-owned URL builder (cnb 0.2.2 made `HttpInner::url` public).
    let url = client
        .http()
        .url(&endpoint)
        .map_err(|e| CliError::Generic(format!("invalid upload endpoint `{endpoint}`: {e}")))?;
    let resp = client
        .http()
        .reqwest_client()
        .post(url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| CliError::Generic(format!("upload POST failed: {e}")))?;

    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| CliError::Generic(format!("upload body read failed: {e}")))?;

    if !status.is_success() {
        return Err(CliError::Generic(format!(
            "upload {} {endpoint}: {text}",
            status.as_u16()
        )));
    }

    let v: Value = serde_json::from_str(&text).unwrap_or(Value::String(text.clone()));
    let url = extract_url(&v)
        .ok_or_else(|| CliError::Generic(format!("upload succeeded but response missing url field: {text}")))?;

    Ok(Uploaded {
        kind,
        original_name,
        url,
        raw: v,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn endpoint_routing_repo_file() {
        let s = Scope::Repo("cnb/feedback");
        assert_eq!(s.endpoint(Kind::File), "/cnb/feedback/-/upload/files");
        assert_eq!(s.endpoint(Kind::Image), "/cnb/feedback/-/upload/imgs");
    }

    #[test]
    fn endpoint_routing_issue_comment() {
        let s = Scope::IssueComment {
            repo: "cnb/feedback",
            number: 42,
        };
        assert_eq!(
            s.endpoint(Kind::File),
            "/cnb/feedback/-/issues/42/comment-file-asset-upload-url"
        );
        assert_eq!(
            s.endpoint(Kind::Image),
            "/cnb/feedback/-/issues/42/comment-image-asset-upload-url"
        );
    }

    #[test]
    fn detect_kind_recognises_images() {
        assert_eq!(detect_kind(&PathBuf::from("foo.png")), Kind::Image);
        assert_eq!(detect_kind(&PathBuf::from("foo.jpg")), Kind::Image);
        assert_eq!(detect_kind(&PathBuf::from("foo.txt")), Kind::File);
        assert_eq!(detect_kind(&PathBuf::from("foo")), Kind::File);
    }

    #[test]
    fn extract_url_handles_envelope_shapes() {
        assert_eq!(
            extract_url(&serde_json::json!({"url":"https://x/y.png"})).as_deref(),
            Some("https://x/y.png")
        );
        assert_eq!(
            extract_url(&serde_json::json!({"data":{"url":"https://x/y"}})).as_deref(),
            Some("https://x/y")
        );
        assert_eq!(
            extract_url(&serde_json::json!({"data":{"download_url":"https://x/z"}})).as_deref(),
            Some("https://x/z")
        );
        assert_eq!(extract_url(&serde_json::json!({})), None);
    }
}
