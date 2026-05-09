# No non-JSON transport — bytes endpoints need a side-car `reqwest::Client`

> SDK ref: `cnb 0.2.x`
> Tracking id (consumer side): SDK-I14 — see
> [`cnb-cli` sdk-issues.md](https://…)

## TL;DR

The SDK's shared HTTP layer (`cnb::http::HttpInner`) is
**JSON-only on both legs**:

- `execute::<T>(method, url)` — no body, decodes response as
  `T` via `serde_json::from_slice`.
- `execute_with_body::<T, B: Serialize>(method, url, body)` —
  attaches body with `req.json(b)` (always sets
  `Content-Type: application/json`), decodes response as `T`.

There's no way to attach a raw `reqwest::Body` (file stream,
multipart, plain bytes) and no way to retrieve a raw byte body
(file download, plain-text log). The underlying
`reqwest::Client` is also not exposed (`HttpInner` has no
`fn client(&self)` accessor).

This forces consumers to spin up a **second** `reqwest::Client`
for any non-JSON flow. That client doesn't share the SDK's:

- connection pool
- retry policy
- auth header (consumer reads `CNB_TOKEN` from env again)
- tracing layer
- base-URL resolution (consumer re-implements
  `CNB_API_BASE` > default precedence)

We have **three** legitimate cnb.cool flows that all hit this
gap and all share the exact same workaround.

## The three affected flows

### 1. Two-phase release asset upload — phase 2 (`PUT <pre-signed url>` with file bytes)

Phase 1 (`POST /{repo}/-/releases/{tag}/asset-upload-url`) is a
typed JSON call → SDK handles it.
**Phase 2** (`PUT <upload_url>` with file bytes) cannot be
expressed by the SDK because `execute_with_body` forces a JSON
body.

Our workaround in `crates/cnb-cli/src/commands/release.rs:479`
(commit [`b785d35`](https://…)):

```rust
use tokio::fs::File;
use tokio_util::io::ReaderStream;

let file = File::open(path).await?;
let stream = ReaderStream::new(file);
let body = reqwest::Body::wrap_stream(stream);

// Side-car client. No auth header (pre-signed URL).
let put_resp = reqwest::Client::new()
    .put(&upload_url)
    .header(reqwest::header::CONTENT_LENGTH, size)
    .body(body)
    .send()
    .await?;
```

### 2. Release asset download (`GET /{repo}/-/releases/download/{tag}/{filename}`)

The endpoint returns a 302 to a signed URL whose body is file
bytes. The SDK's typed method
`ReleasesClient::get_releases_asset` types the response as
`serde_json::Value` and decodes via `serde_json::from_slice` —
which always fails on the first non-JSON byte:

```text
expected value at line 1 column 1
```

Our workaround in `crates/cnb-cli/src/commands/release.rs:539`:

```rust
let token = std::env::var("CNB_TOKEN").unwrap_or_default();
let mut req = reqwest::Client::new().get(full); // side-car
if !token.is_empty() {
    req = req.bearer_auth(token);
}
let resp = req.send().await?;
// Stream resp.bytes() to disk.
```

### 3. Pipeline runner log download (`GET /{repo}/-/build/runner/download/log/{pipelineId}`)

Returns plain text (the runner log,
`text/plain; charset=utf-8`). The typed method
`BuildClient::build_runner_download_log` types the response as
`serde_json::Value` and again fails on the first non-JSON byte.

Our workaround in `crates/cnb-cli/src/commands/build.rs:421`:

```rust
let token = std::env::var("CNB_TOKEN").unwrap_or_default();
let mut req = reqwest::Client::new().get(full); // side-car
if !token.is_empty() {
    req = req.bearer_auth(token);
}
let resp = req.send().await?;
let text = resp.text().await?;
```

## Why this matters

All three workarounds:

- Re-implement base-URL resolution (`CNB_API_BASE` > default).
- Re-read `CNB_TOKEN` from env. The SDK's resolved token (from
  keyring, hosts file, etc.) is not reachable.
- Use a fresh `reqwest::Client::new()`, getting **none** of the
  SDK's pool / retry / tracing.
- For the `--attach` two-phase upload (issue and PR comment
  attachments), the cnb-cli consumer cannot eliminate the
  `cnb-api` facade crate at all; it remains in the dependency
  tree purely for `services::uploads`.

Three unrelated verbs share the same workaround. That makes the
case for one generic fix stronger than any single one.

## Suggested fix (any one of these unblocks all three flows)

**Option A — expose the underlying `reqwest::Client`** (smallest
patch):

```rust
impl HttpInner {
    pub fn reqwest_client(&self) -> &reqwest::Client { &self.client }
    pub fn base_url(&self) -> &Url { &self.base_url }
    pub fn url(&self, path: &str) -> Result<Url, …> {
        self.base_url.join(path).map_err(|e| …)
    }
}
```

Consumers can build any request shape they want and still share
the SDK's client / pool / base-URL. This pairs naturally with
**SDK-I01** (which asks for the same `url()` and `base_url()`
visibility).

**Option B — extend `execute_with_body` to take an arbitrary `reqwest::Body`**:

```rust
pub async fn execute_raw_body(
    &self,
    method: Method,
    url: Url,
    content_type: HeaderValue,
    body: reqwest::Body,
) -> Result<reqwest::Response, …>;

pub async fn execute_bytes(
    &self,
    method: Method,
    url: Url,
) -> Result<bytes::Bytes, …>;
```

Plus a streaming-response variant for download flows. Slightly
larger surface, but keeps everything inside `HttpInner`.

**Option C — model the three bytes endpoints as first-class typed methods**:

```rust
impl ReleasesClient {
    pub async fn upload_release_asset(&self, repo: &str, tag: &str, path: &Path) -> Result<…>;
    pub async fn download_release_asset(&self, repo: &str, tag: &str, name: &str) -> Result<bytes::Bytes>;
}

impl BuildClient {
    pub async fn download_runner_log(&self, repo: &str, pipeline_id: i64) -> Result<String>;
}
```

The consumer never has to think about transport at all. Best
DX, biggest patch.

Our preference: **Option A first**, then C as endpoints get
attention. A is the smallest diff and unblocks consumers
immediately; C makes it a non-issue for the average user.

## Severity

Annoyance, but with three unrelated invocations sharing the
same workaround:

- Two HTTP clients per `cnb release upload` invocation
- Two HTTP clients per `cnb release download` invocation
- Two HTTP clients per `cnb build logs` invocation

…and one residual dependency on the `cnb-api` facade crate the
consumer would otherwise drop entirely.
