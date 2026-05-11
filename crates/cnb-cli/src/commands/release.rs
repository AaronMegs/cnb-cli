//! `cnb release` — releases (M3 §8.5, 9 subcommands).
//!
//! Note: `--repo OWNER/REPO` is consistently a flag (not a positional) for
//! all release subcommands because most of them already have multiple
//! mandatory positional arguments (id/asset_id/tag/filename). Mixing in a
//! trailing optional positional `repo` would make clap unable to disambiguate
//! values reliably (e.g. `release view cnb/feedback` could be tag-or-repo).
//! This matches `gh release` conventions.
//!
//! ## SDK backing (Phase 2, Step 2.7)
//!
//! All nine subcommands are ported to the typed `cnb_sdk::releases` API.
//! The one place the typed SDK cannot cover on its own is the file-bytes
//! hop: phase-2 of `upload` (raw `PUT <upload_url>`) and the body-stream
//! of `download` both need bytes-level access, while `HttpInner::execute`
//! only sends / decodes JSON. We therefore keep a small ad-hoc
//! `reqwest::Client` inside this module for those two calls. The rest of
//! the flow (phase-1 `asset-upload-url`, phase-3 verify POST, list / view /
//! create / edit / delete / asset-view / asset-delete) goes through the
//! SDK's shared HTTP layer so auth / retry / tracing stay consistent.
//! Tracked under `docs/sdk-issues.md` · SDK-I14.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_sdk::models::{PatchReleaseForm, PostReleaseAssetUploadUrlForm, PostReleaseForm};
use cnb_sdk::releases::ListReleasesQuery;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct ReleaseArgs {
    #[command(subcommand)]
    pub command: ReleaseCmd,
}

#[derive(Debug, Subcommand)]
pub enum ReleaseCmd {
    /// List releases.
    List(ListArgs),
    /// View a release by tag, --id, or --latest.
    View(ViewArgs),
    /// Create a new release.
    Create(CreateArgs),
    /// Edit an existing release (by id).
    Edit(EditArgs),
    /// Delete a release by id (destructive).
    Delete(DeleteArgs),
    /// Upload one or more asset files to an existing release (by id).
    Upload(UploadArgs),
    /// Download a release asset (by tag + filename).
    Download(DownloadArgs),
    /// View a single asset's metadata.
    AssetView(AssetArgs),
    /// Delete a single asset (destructive).
    AssetDelete(AssetDeleteArgs),
}

#[derive(Debug, Args, Clone)]
pub struct OutputOpts {
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub jq: Option<String>,
    #[arg(long)]
    pub template: Option<String>,
}

/// Shared `--repo` flag across all release subcommands.
#[derive(Debug, Args, Clone)]
pub struct RepoOpt {
    /// `OWNER/REPO[/SUBGROUP]` (or auto-detected from `git remote origin`).
    #[arg(long, global = false)]
    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub repo: RepoOpt,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[arg(long, default_value_t = 1u32)]
    pub page: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    /// Tag name (positional). Mutually exclusive with --id and --latest.
    pub tag: Option<String>,
    #[command(flatten)]
    pub repo: RepoOpt,
    /// Look up by release id instead of tag.
    #[arg(long)]
    pub id: Option<String>,
    /// Fetch the latest release.
    #[arg(long)]
    pub latest: bool,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    pub tag: String,
    #[command(flatten)]
    pub repo: RepoOpt,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "notes_file")]
    pub notes: Option<String>,
    #[arg(long = "notes-file", value_name = "PATH")]
    pub notes_file: Option<String>,
    #[arg(long)]
    pub draft: bool,
    #[arg(long)]
    pub prerelease: bool,
    /// Branch or SHA to tag against.
    #[arg(long, value_name = "REF")]
    pub target: Option<String>,
    /// Asset paths to upload after creation.
    #[arg(long, value_name = "PATH")]
    pub asset: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    pub id: String,
    #[command(flatten)]
    pub repo: RepoOpt,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, conflicts_with = "notes_file")]
    pub notes: Option<String>,
    #[arg(long = "notes-file", value_name = "PATH")]
    pub notes_file: Option<String>,
    #[arg(long)]
    pub draft: Option<bool>,
    #[arg(long)]
    pub prerelease: Option<bool>,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub id: String,
    #[command(flatten)]
    pub repo: RepoOpt,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct UploadArgs {
    pub id: String,
    /// One or more local files to upload.
    #[arg(required = true, value_name = "FILE")]
    pub files: Vec<PathBuf>,
    #[command(flatten)]
    pub repo: RepoOpt,
    /// Allow overwriting same-named assets.
    #[arg(long)]
    pub clobber: bool,
    /// TTL in days (0 = forever).
    #[arg(long)]
    pub ttl: Option<u32>,
}

#[derive(Debug, Args)]
pub struct DownloadArgs {
    pub tag: String,
    pub filename: String,
    #[command(flatten)]
    pub repo: RepoOpt,
    /// Destination directory (default: current dir).
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct AssetArgs {
    pub id: String,
    pub asset_id: String,
    #[command(flatten)]
    pub repo: RepoOpt,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct AssetDeleteArgs {
    pub id: String,
    pub asset_id: String,
    #[command(flatten)]
    pub repo: RepoOpt,
    #[arg(long)]
    pub yes: bool,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: ReleaseArgs) -> Result<(), CliError> {
    match args.command {
        ReleaseCmd::List(a) => list(ctx, a).await,
        ReleaseCmd::View(a) => view(ctx, a).await,
        ReleaseCmd::Create(a) => create(ctx, a).await,
        ReleaseCmd::Edit(a) => edit(ctx, a).await,
        ReleaseCmd::Delete(a) => delete(ctx, a).await,
        ReleaseCmd::Upload(a) => upload(ctx, a).await,
        ReleaseCmd::Download(a) => download(ctx, a).await,
        ReleaseCmd::AssetView(a) => asset_view(ctx, a).await,
        ReleaseCmd::AssetDelete(a) => asset_delete(ctx, a).await,
    }
}

fn render(ctx: &Context, opts: &OutputOpts, v: &Value) -> Result<bool, CliError> {
    if let Some(tpl) = opts.template.as_deref() {
        println!("{}", template::apply(v, tpl)?);
        return Ok(true);
    }
    if let Some(expr) = opts.jq.as_deref() {
        let outs = jq::apply(v, expr)?;
        let mut stdout = std::io::stdout().lock();
        for o in outs {
            json_out::write_json(&mut stdout, &o, false)?;
        }
        return Ok(true);
    }
    if opts.json {
        let mut stdout = std::io::stdout().lock();
        json_out::write_json(&mut stdout, v, ctx.io.stdout_is_tty)?;
        return Ok(true);
    }
    Ok(false)
}

/// Serialise a typed SDK model back to `serde_json::Value` for rendering.
///
/// The render path expects dynamic access via `Value::get`, matching the
/// behaviour when `cnb-api` returned raw `Value`s. Typed-first + convert is
/// preferable to raw-only because the typed deserialisation already caught
/// any schema regressions earlier in the pipeline.
fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, CliError> {
    serde_json::to_value(t).map_err(|e| CliError::Generic(format!("serialise response: {e}")))
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let q = ListReleasesQuery::new()
        .page(i64::from(args.page))
        .page_size(i64::from(args.limit.max(1)));
    let items = {
        let client = ctx.sdk()?;
        client.releases().list_releases(repo.clone(), &q).await?
    };
    let v = Value::Array(items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?);
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let tag = it.tag_name.as_deref().unwrap_or("");
        let name = it.name.as_deref().unwrap_or("");
        let pub_at = it.published_at.as_deref().unwrap_or("");
        let draft = it.draft.unwrap_or(false);
        let pre = it.prerelease.unwrap_or(false);
        let label = if draft {
            "draft"
        } else if pre {
            "pre"
        } else {
            ""
        };
        rows.push(vec![
            tag.to_owned(),
            name.to_owned(),
            label.to_owned(),
            pub_at.to_owned(),
        ]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["TAG", "TITLE", "TYPE", "PUBLISHED"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let n_specifiers = usize::from(args.tag.is_some()) + usize::from(args.id.is_some()) + usize::from(args.latest);
    if n_specifiers != 1 {
        return Err(CliError::BadArgs(
            "specify exactly one of: TAG (positional) | --id ID | --latest".into(),
        ));
    }
    let rel = {
        let client = ctx.sdk()?;
        if args.latest {
            client.releases().get_latest_release(repo.clone()).await?
        } else if let Some(id) = &args.id {
            client.releases().get_release_by_id(repo.clone(), id.clone()).await?
        } else {
            let tag = args.tag.as_deref().expect("checked above").to_owned();
            client.releases().get_release_by_tag(repo.clone(), tag).await?
        }
    };
    let v = to_value(&rel)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let tag = rel.tag_name.as_deref().unwrap_or("");
    let name = rel.name.as_deref().unwrap_or("");
    let body = rel.body.as_deref().unwrap_or("");
    let pub_at = rel.published_at.as_deref().unwrap_or("");
    println!("{tag} — {name}");
    if !pub_at.is_empty() {
        println!("  Published: {pub_at}");
    }
    if let Some(assets) = rel.assets.as_ref() {
        if !assets.is_empty() {
            println!("  Assets ({}):", assets.len());
            for a in assets {
                let n = a.name.as_deref().unwrap_or("");
                let s = a.size.unwrap_or(0);
                println!("    - {n} ({s} bytes)");
            }
        }
    }
    if !body.is_empty() {
        println!();
        println!("{body}");
    }
    Ok(())
}

fn read_notes(notes: Option<String>, notes_file: Option<String>) -> Result<Option<String>, CliError> {
    match (notes, notes_file) {
        (Some(s), _) => Ok(Some(s)),
        (None, Some(p)) if p == "-" => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok(Some(s))
        }
        (None, Some(p)) => Ok(Some(std::fs::read_to_string(p)?)),
        (None, None) => Ok(None),
    }
}

async fn create(ctx: &mut Context, args: CreateArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let body_text = read_notes(args.notes, args.notes_file)?;
    let body = PostReleaseForm {
        tag_name: Some(args.tag.clone()),
        name: args.title,
        body: body_text,
        draft: if args.draft { Some(true) } else { None },
        prerelease: if args.prerelease { Some(true) } else { None },
        make_latest: None,
        target_commitish: args.target,
    };
    let rel = {
        let client = ctx.sdk()?;
        client.releases().post_release(repo.clone(), &body).await?
    };
    let id = rel.id.as_deref().unwrap_or("");
    eprintln!("✓ Created release `{}` (id={id})", args.tag);

    if !args.asset.is_empty() && !id.is_empty() {
        let assets = args.asset.clone();
        for p in &assets {
            upload_one(ctx, &repo, id, p, false, None).await?;
            eprintln!("  ↑ uploaded {}", p.display());
        }
    }
    Ok(())
}

async fn edit(ctx: &mut Context, args: EditArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let body_text = read_notes(args.notes, args.notes_file)?;
    if args.title.is_none() && body_text.is_none() && args.draft.is_none() && args.prerelease.is_none() {
        return Err(CliError::BadArgs(
            "nothing to edit — pass --title/--notes/--notes-file/--draft/--prerelease".into(),
        ));
    }
    let body = PatchReleaseForm {
        name: args.title,
        body: body_text,
        draft: args.draft,
        prerelease: args.prerelease,
        make_latest: None,
    };
    let client = ctx.sdk()?;
    let _ = client
        .releases()
        .patch_release(repo.clone(), args.id.clone(), &body)
        .await?;
    eprintln!("✓ Updated release {}", args.id);
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    ctx.confirm(&format!("Delete release `{}` from `{repo}`? (y/N)", args.id), args.yes)?;
    let client = ctx.sdk()?;
    let _ = client.releases().delete_release(repo.clone(), args.id.clone()).await?;
    eprintln!("✓ Deleted release {}", args.id);
    Ok(())
}

async fn upload(ctx: &mut Context, args: UploadArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    for p in &args.files {
        upload_one(ctx, &repo, &args.id, p, args.clobber, args.ttl).await?;
        eprintln!("  ↑ uploaded {}", p.display());
    }
    eprintln!("✓ Uploaded {} file(s) to release {}", args.files.len(), args.id);
    Ok(())
}

/// Two-phase asset upload.
///
/// Phase 1 (POST asset-upload-url) and phase 3 (POST verify_url) both go
/// through the SDK's shared HTTP layer. Phase 2 (PUT file bytes to the
/// pre-signed URL) bypasses the SDK because `HttpInner::execute*` only
/// supports JSON bodies — see SDK-I14. We use a fresh `reqwest::Client`
/// for that hop, which also matches the legacy `cnb-api` behaviour of
/// keeping the pre-signed URL's auth out of the Authorization header.
async fn upload_one(
    ctx: &mut Context,
    repo: &str,
    release_id: &str,
    path: &std::path::Path,
    overwrite: bool,
    ttl_days: Option<u32>,
) -> Result<(), CliError> {
    let asset_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CliError::BadArgs(format!("invalid file path: {path:?}")))?
        .to_owned();
    let meta = tokio::fs::metadata(path).await?;
    let size = meta.len();

    // Phase 1: ask for a pre-signed URL via the typed SDK.
    let form = PostReleaseAssetUploadUrlForm {
        asset_name: Some(asset_name.clone()),
        overwrite: if overwrite { Some(true) } else { None },
        size: Some(i64::try_from(size).unwrap_or(i64::MAX)),
        ttl: ttl_days.map(i64::from),
    };
    let url_info = {
        let client = ctx.sdk()?;
        client
            .releases()
            .post_release_asset_upload_url(repo.to_owned(), release_id.to_owned(), &form)
            .await?
    };
    let upload_url = url_info
        .upload_url
        .ok_or_else(|| CliError::Generic("server omitted upload_url".into()))?;
    let verify_url = url_info
        .verify_url
        .ok_or_else(|| CliError::Generic("server omitted verify_url".into()))?;

    // Phase 2: stream-PUT the file bytes to the pre-signed URL.
    // Must use a standalone reqwest client — the SDK's execute path only
    // accepts JSON bodies (see SDK-I14).
    let file = File::open(path).await?;
    let stream = ReaderStream::new(file);
    let body = reqwest::Body::wrap_stream(stream);
    let put_resp = reqwest::Client::new()
        .put(&upload_url)
        .header(reqwest::header::CONTENT_LENGTH, size)
        .body(body)
        .send()
        .await
        .map_err(|e| CliError::Generic(format!("upload PUT failed: {e}")))?;
    let put_status = put_resp.status();
    if !put_status.is_success() {
        let text = put_resp.text().await.unwrap_or_default();
        return Err(CliError::Generic(format!("upload PUT {}: {text}", put_status.as_u16())));
    }

    // Phase 3: confirm. `verify_url` is absolute; hand it to the SDK's
    // shared `HttpInner::execute` which accepts an arbitrary `Url` and
    // carries the SDK's retry + auth config for free.
    let verify_parsed = url::Url::parse(&verify_url)
        .map_err(|e| CliError::Generic(format!("invalid verify_url `{verify_url}`: {e}")))?;
    let client = ctx.sdk()?;
    let _: Value = client.http().execute(reqwest::Method::POST, verify_parsed).await?;
    Ok(())
}

/// Download a release asset. The SDK's typed `get_releases_asset` decodes
/// the body as JSON, which is wrong for this endpoint (raw bytes after a
/// 302 to a presigned URL). cnb 0.2.2 made `HttpInner::reqwest_client()`
/// public (SDK-I14 resolved), so we now drive the GET through the SDK's
/// shared reqwest client via `Context::sdk_raw_get_bytes` — same
/// connection pool, same auth header, same base-URL precedence.
async fn download(ctx: &mut Context, args: DownloadArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let dest = download_bytes(ctx, &repo, &args.tag, &args.filename, &args.output).await?;
    eprintln!("✓ Downloaded {} → {}", args.filename, dest.display());
    Ok(())
}

async fn download_bytes(
    ctx: &mut Context,
    repo: &str,
    tag: &str,
    filename: &str,
    dest_dir: &std::path::Path,
) -> Result<std::path::PathBuf, CliError> {
    let path = format!("/{repo}/-/releases/download/{tag}/{filename}");
    let bytes = ctx.sdk_raw_get_bytes(&path).await?;
    tokio::fs::create_dir_all(dest_dir).await?;
    let dest = dest_dir.join(filename);
    tokio::fs::write(&dest, &bytes).await?;
    Ok(dest)
}

async fn asset_view(ctx: &mut Context, args: AssetArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let asset = {
        let client = ctx.sdk()?;
        client
            .releases()
            .get_release_asset(repo.clone(), args.id.clone(), args.asset_id.clone())
            .await?
    };
    let v = to_value(&asset)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn asset_delete(ctx: &mut Context, args: AssetDeleteArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    ctx.confirm(
        &format!(
            "Delete asset `{}` of release `{}` in `{repo}`? (y/N)",
            args.asset_id, args.id
        ),
        args.yes,
    )?;
    let client = ctx.sdk()?;
    let _ = client
        .releases()
        .delete_release_asset(repo.clone(), args.id.clone(), args.asset_id.clone())
        .await?;
    eprintln!("✓ Deleted asset {}", args.asset_id);
    Ok(())
}
