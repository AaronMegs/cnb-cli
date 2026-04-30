//! `cnb release` — releases (M3 §8.5, 9 subcommands).
//!
//! Note: `--repo OWNER/REPO` is consistently a flag (not a positional) for
//! all release subcommands because most of them already have multiple
//! mandatory positional arguments (id/asset_id/tag/filename). Mixing in a
//! trailing optional positional `repo` would make clap unable to disambiguate
//! values reliably (e.g. `release view cnb/feedback` could be tag-or-repo).
//! This matches `gh release` conventions.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_api::services::releases;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

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

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let q = format!("page={}&page_size={}", args.page, args.limit.max(1));
    let client = ctx.api()?;
    let items = releases::list(client, &repo, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let tag = it.get("tag_name").and_then(Value::as_str).unwrap_or("");
        let name = it.get("name").and_then(Value::as_str).unwrap_or("");
        let pub_at = it.get("published_at").and_then(Value::as_str).unwrap_or("");
        let draft = it.get("draft").and_then(Value::as_bool).unwrap_or(false);
        let pre = it.get("prerelease").and_then(Value::as_bool).unwrap_or(false);
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
    let client = ctx.api()?;
    let v = if args.latest {
        releases::latest(client, &repo).await?
    } else if let Some(id) = &args.id {
        releases::view_by_id(client, &repo, id).await?
    } else {
        releases::view_by_tag(client, &repo, args.tag.as_deref().expect("checked above")).await?
    };
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let tag = v.get("tag_name").and_then(Value::as_str).unwrap_or("");
    let name = v.get("name").and_then(Value::as_str).unwrap_or("");
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    let pub_at = v.get("published_at").and_then(Value::as_str).unwrap_or("");
    println!("{tag} — {name}");
    if !pub_at.is_empty() {
        println!("  Published: {pub_at}");
    }
    if let Some(assets) = v.get("assets").and_then(Value::as_array) {
        if !assets.is_empty() {
            println!("  Assets ({}):", assets.len());
            for a in assets {
                let n = a.get("name").and_then(Value::as_str).unwrap_or("");
                let s = a.get("size").and_then(Value::as_i64).unwrap_or(0);
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
    let body = releases::CreateReleaseBody {
        tag_name: args.tag.clone(),
        name: args.title,
        body: body_text,
        draft: if args.draft { Some(true) } else { None },
        prerelease: if args.prerelease { Some(true) } else { None },
        make_latest: None,
        target_commitish: args.target,
    };
    let client = ctx.api()?;
    let v = releases::create(client, &repo, &body).await?;
    let id = v.get("id").and_then(Value::as_str).unwrap_or("");
    eprintln!("✓ Created release `{}` (id={id})", args.tag);

    if !args.asset.is_empty() && !id.is_empty() {
        let client = ctx.api()?;
        for p in &args.asset {
            let _ = releases::upload_asset(client, &repo, id, p, false, None).await?;
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
    let body = releases::EditReleaseBody {
        name: args.title,
        body: body_text,
        draft: args.draft,
        prerelease: args.prerelease,
        make_latest: None,
    };
    let client = ctx.api()?;
    let _ = releases::edit(client, &repo, &args.id, &body).await?;
    eprintln!("✓ Updated release {}", args.id);
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    ctx.confirm(&format!("Delete release `{}` from `{repo}`? (y/N)", args.id), args.yes)?;
    let client = ctx.api()?;
    let _ = releases::delete(client, &repo, &args.id).await?;
    eprintln!("✓ Deleted release {}", args.id);
    Ok(())
}

async fn upload(ctx: &mut Context, args: UploadArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let client = ctx.api()?;
    for p in &args.files {
        let _ = releases::upload_asset(client, &repo, &args.id, p, args.clobber, args.ttl).await?;
        eprintln!("  ↑ uploaded {}", p.display());
    }
    eprintln!("✓ Uploaded {} file(s) to release {}", args.files.len(), args.id);
    Ok(())
}

async fn download(ctx: &mut Context, args: DownloadArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let client = ctx.api()?;
    let dest = releases::download_asset(client, &repo, &args.tag, &args.filename, &args.output).await?;
    eprintln!("✓ Downloaded {} → {}", args.filename, dest.display());
    Ok(())
}

async fn asset_view(ctx: &mut Context, args: AssetArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.repo.as_deref())?;
    let client = ctx.api()?;
    let v = releases::view_asset(client, &repo, &args.id, &args.asset_id).await?;
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
    let client = ctx.api()?;
    let _ = releases::delete_asset(client, &repo, &args.id, &args.asset_id).await?;
    eprintln!("✓ Deleted asset {}", args.asset_id);
    Ok(())
}
