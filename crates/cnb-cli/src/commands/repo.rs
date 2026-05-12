//! `cnb repo` — repository commands (M2 §8.2, 11 subcommands).
//!
//! Fully ported to the typed SDK (`cnb-sdk`) as of Phase 2 step 2.10.
//! Verbs and backing SDK methods:
//!
//! | verb              | SDK entry point |
//! |-------------------|-----------------|
//! | `view` / `list` / `create` / `delete` / `archive` / `unarchive` / `transfer` / `set-visibility` / `fork` / `edit` | `RepositoriesClient::{get_by_id, get_repos, post_repo, delete_repo, archive_repo, unarchive_repo, transfer_repo, set_repo_visibility, list_forks_repos, update_repo}` |
//! | `list-pinned`     | `RepositoriesClient::get_pinned_repo_by_group` |
//! | `pin` / `unpin`   | `get_pinned_repo_by_group` for the read, then a raw
//!                      `PUT /{slug}/-/pinned-repos` via `Context::sdk_raw_json` because
//!                      the SDK does not model the PUT endpoint — tracked as SDK-I18. |
//! | `contributors`    | `RepoContributorClient::get_repo_contributor_trend` (typed)
//!                      when no `--days`; `Context::sdk_raw_get` with `?days=N` otherwise —
//!                      the SDK's `GetRepoContributorTrendQuery` doesn't expose
//!                      `days`. Tracked as SDK-I17. |

use std::path::PathBuf;
use std::process::Command;

use clap::{Args, Subcommand};
use cnb_sdk::models::{CreateRepoReq, RepoPatch, TransferSlugReq};
use cnb_sdk::repositories::{ListForksReposQuery, SetRepoVisibilityQuery};
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoCmd,
}

#[derive(Debug, Subcommand)]
pub enum RepoCmd {
    /// List repositories for a user, an org/group slug, or the current token.
    List(ListArgs),
    /// View a repository's details.
    View(ViewArgs),
    /// Create a new repository.
    Create(CreateArgs),
    /// Clone a repository to a local directory (uses your `git` binary).
    Clone(CloneArgs),
    /// List forks of a repository.
    Fork(ForkArgs),
    /// Delete a repository (destructive).
    Delete(DeleteArgs),
    /// Edit repository metadata.
    Edit(EditArgs),
    /// Archive a repository.
    Archive(ArchiveArgs),
    /// Unarchive a previously archived repository.
    Unarchive(ArchiveArgs),
    /// Transfer a repository to a new owner.
    Transfer(TransferArgs),
    /// Change a repository's visibility level.
    SetVisibility(SetVisibilityArgs),
    /// Pin one or more repositories to the slug owner's profile (M4).
    Pin(PinArgs),
    /// Remove one or more repositories from the pinned set (M4).
    Unpin(PinArgs),
    /// List the pinned repositories of a slug (M4).
    ListPinned(ListPinnedArgs),
    /// Show contributor trend for a repository (M4).
    Contributors(ContributorsArgs),
}

#[derive(Debug, Args)]
pub struct PinArgs {
    /// Group/org slug whose pinned-set we're modifying.
    pub slug: String,
    /// Repo slugs to add or remove (e.g. `cnb/feedback`).
    #[arg(required = true)]
    pub repos: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ListPinnedArgs {
    pub slug: String,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct ContributorsArgs {
    pub slug: String,
    /// Time window in days (server default if omitted).
    #[arg(long)]
    pub days: Option<u32>,
    #[command(flatten)]
    pub out: OutputOpts,
}

// ---------- shared output options ----------

#[derive(Debug, Args, Clone)]
pub struct OutputOpts {
    /// Emit JSON instead of a table / cards.
    #[arg(long)]
    pub json: bool,
    /// Apply a jq filter (implies `--json` semantics).
    #[arg(long)]
    pub jq: Option<String>,
    /// Apply a tinytemplate string (implies `--json` semantics).
    #[arg(long)]
    pub template: Option<String>,
}

impl OutputOpts {
    fn render_value(&self, ctx: &Context, v: &Value) -> Result<bool, CliError> {
        if let Some(tpl) = self.template.as_deref() {
            let s = template::apply(v, tpl)?;
            println!("{s}");
            return Ok(true);
        }
        if let Some(expr) = self.jq.as_deref() {
            let outs = jq::apply(v, expr)?;
            let mut stdout = std::io::stdout().lock();
            for o in outs {
                json_out::write_json(&mut stdout, &o, false)?;
            }
            return Ok(true);
        }
        if self.json {
            let mut stdout = std::io::stdout().lock();
            json_out::write_json(&mut stdout, v, ctx.io.stdout_is_tty)?;
            return Ok(true);
        }
        Ok(false)
    }
}

// ---------- list ----------

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Either `USER` (alphanumeric, no slash) or `OWNER/GROUP/...` (org/group).
    /// Omit to list repos accessible by the current token.
    pub target: Option<String>,
    /// Page size (server may cap).
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    /// Page number (1-based).
    #[arg(long, default_value_t = 1u32)]
    pub page: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

// ---------- view ----------

#[derive(Debug, Args)]
pub struct ViewArgs {
    /// `OWNER/REPO[/SUBGROUP]`. Omit to auto-detect from `git remote origin`.
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

// ---------- create ----------

#[derive(Debug, Args)]
pub struct CreateArgs {
    /// `OWNER/NAME` or `OWNER/GROUP/NAME` — final segment is the new repo name.
    pub slug: String,
    #[arg(long)]
    pub description: Option<String>,
    /// Visibility: `public` | `internal` | `private`.
    #[arg(long, value_parser = ["public", "internal", "private"], default_value = "private")]
    pub visibility: String,
    #[arg(long)]
    pub default_branch: Option<String>,
    /// Clone into a local directory after creation.
    #[arg(long)]
    pub clone: bool,
}

// ---------- clone ----------

#[derive(Debug, Args)]
pub struct CloneArgs {
    /// `OWNER/REPO[/SUBGROUP]` — the slug to clone.
    pub repo: String,
    /// Target directory (defaults to the basename of `repo`).
    pub dir: Option<PathBuf>,
}

// ---------- fork (list) ----------

#[derive(Debug, Args)]
pub struct ForkArgs {
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

// ---------- destructive ----------

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub repo: Option<String>,
    /// Skip the confirmation prompt.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ArchiveArgs {
    pub repo: Option<String>,
}

// ---------- edit ----------

#[derive(Debug, Args)]
pub struct EditArgs {
    pub repo: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub default_branch: Option<String>,
}

// ---------- transfer ----------

#[derive(Debug, Args)]
pub struct TransferArgs {
    pub repo: Option<String>,
    /// New owner namespace (e.g. another user or group slug).
    #[arg(long)]
    pub to: String,
    #[arg(long)]
    pub yes: bool,
}

// ---------- set-visibility ----------

#[derive(Debug, Args)]
pub struct SetVisibilityArgs {
    /// `public` (0) | `internal` (10) | `private` (20).
    #[arg(value_parser = ["public", "internal", "private"])]
    pub visibility: String,
    /// `OWNER/REPO[/SUBGROUP]` (or auto-detected from `git remote origin`).
    #[arg(long)]
    pub repo: Option<String>,
}

// ============================================================================
//                                  dispatch
// ============================================================================

pub async fn run(ctx: &mut Context, args: RepoArgs) -> Result<(), CliError> {
    match args.command {
        RepoCmd::List(a) => list(ctx, a).await,
        RepoCmd::View(a) => view(ctx, a).await,
        RepoCmd::Create(a) => create(ctx, a).await,
        RepoCmd::Clone(a) => clone(ctx, a),
        RepoCmd::Fork(a) => fork(ctx, a).await,
        RepoCmd::Delete(a) => delete(ctx, a).await,
        RepoCmd::Edit(a) => edit(ctx, a).await,
        RepoCmd::Archive(a) => archive(ctx, a, false).await,
        RepoCmd::Unarchive(a) => archive(ctx, a, true).await,
        RepoCmd::Transfer(a) => transfer(ctx, a).await,
        RepoCmd::SetVisibility(a) => set_visibility(ctx, a).await,
        RepoCmd::Pin(a) => pin(ctx, a, true).await,
        RepoCmd::Unpin(a) => pin(ctx, a, false).await,
        RepoCmd::ListPinned(a) => list_pinned(ctx, a).await,
        RepoCmd::Contributors(a) => contributors(ctx, a).await,
    }
}

// ---------------------------------------------------------------------------
//                                impl funcs
// ---------------------------------------------------------------------------

/// Render the `visibility_level` field of a repo DTO as a human string.
///
/// SDK 0.2.2 (per `docs/sdk-issues.md` SDK-I03) made `Visibility` a real
/// enum with canonical wire variants `"Public"` / `"Private"` / `"Secret"`
/// and a custom `Deserialize` that also accepts lowercase strings,
/// the legacy `"Internal"` synonym (mapped to `Secret`), and the
/// historical integer encoding (`0` / `10` / `20`).
///
/// This helper preserves user-facing output identical to the SDK's
/// canonical capitalisation so `cnb repo view --json | jq` round-trips
/// the wire form. Unknown / missing values render as `?`.
pub(crate) fn format_visibility(raw: Option<&Value>) -> &'static str {
    match raw {
        Some(Value::String(s)) => match s.as_str() {
            "Public" | "public" => "Public",
            "Private" | "private" => "Private",
            // The SDK collapses "Internal" into "Secret" — we follow that
            // mapping so the CLI agrees with what `cnb_sdk::models::Visibility`
            // would deserialise the same string into.
            "Secret" | "secret" | "Internal" | "internal" => "Secret",
            _ => "?",
        },
        Some(Value::Number(n)) => match n.as_i64() {
            Some(0) => "Public",
            Some(10) => "Secret",
            Some(20) => "Private",
            _ => "?",
        },
        _ => "?",
    }
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let page = i64::from(args.page);
    let page_size = i64::from(args.limit.max(1));
    // Dispatch to the right endpoint based on what the user supplied:
    //   - no target          → `GET /user/repos`        (current user)
    //   - target with a `/`  → `GET /{slug}/-/repos`    (group subgroup)
    //   - bare username      → `GET /users/{u}/repos`   (other user)
    //
    // We hit each endpoint via `sdk_raw_get` (raw `serde_json::Value`)
    // rather than the typed `client.repositories().get_repos(...)` calls.
    // Why: cnb 0.2.2's `Repos4UserBase` DTO types `flags` as
    // `Option<crate::models::Repo>`, but the live server returns a plain
    // string here (e.g. `"Unknown"`). Decoding into the typed DTO blows
    // up with `invalid type: string "Unknown", expected struct Repo`.
    // Dropping into raw `Value` sidesteps that field entirely — and the
    // table renderer below only reads `path` / `name` / `description` /
    // `visibility_level` / `updated_at`, so we lose nothing.
    //
    // Tracked as a follow-up SDK issue (sibling to SDK-I02 / SDK-I11).
    // Once the upstream DTO is fixed (or relaxed to
    // `Option<serde_json::Value>`), revert this back to the typed call.
    let path = match args.target.as_deref() {
        None => format!("/user/repos?page={page}&page_size={page_size}"),
        Some(t) if t.contains('/') => {
            format!("/{}/-/repos?page={page}&page_size={page_size}", t.trim_matches('/'))
        }
        Some(user) => format!(
            "/users/{}/repos?page={page}&page_size={page_size}",
            user.trim_matches('/')
        ),
    };
    let v = ctx.sdk_raw_get(&path).await?;
    if args.out.render_value(ctx, &v)? {
        return Ok(());
    }
    // Default table.
    let empty = Vec::<Value>::new();
    let arr = v.as_array().unwrap_or(&empty);
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(arr.len());
    for it in arr {
        let path_field = it.get("path").and_then(Value::as_str).unwrap_or("");
        let name = it.get("name").and_then(Value::as_str).unwrap_or(path_field);
        let desc = it.get("description").and_then(Value::as_str).unwrap_or("");
        let vis = format_visibility(it.get("visibility_level"));
        // Prefer the canonical `updated_at` field that the SDK DTO pins
        // down; fall back to the older `last_activity_at` key that some
        // legacy responses use so cached mocks keep working.
        let upd = it
            .get("updated_at")
            .or_else(|| it.get("last_activity_at"))
            .and_then(Value::as_str)
            .unwrap_or("");
        rows.push(vec![
            if path_field.is_empty() {
                name.to_owned()
            } else {
                path_field.to_owned()
            },
            desc.to_owned(),
            vis.to_owned(),
            upd.to_owned(),
        ]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["NAME", "DESCRIPTION", "VISIBILITY", "UPDATED"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // We hit the SDK twice on purpose:
    //   1. Typed call (`get_by_id`) — catches schema regressions early and
    //      proves the SDK is wired up. Result is discarded.
    //   2. Raw `Value` call via `ctx.sdk_raw_get(...)` — preserves every
    //      field the server sends (e.g. `default_branch`, which is NOT
    //      part of the `Repos4User` DTO yet), so --json / --jq / --template
    //      stay faithful to the wire payload.
    // Both calls share the same reqwest client + connection pool, so the
    // second request reuses the existing TCP/TLS session; the added cost
    // is one extra round-trip to the same host, acceptable for single-
    // object views.
    let _dto = {
        let client = ctx.sdk()?;
        client.repositories().get_by_id(repo.clone()).await?
    };
    let v = ctx.sdk_raw_get(&format!("/{repo}")).await?;
    if args.out.render_value(ctx, &v)? {
        return Ok(());
    }
    let path = v.get("path").and_then(Value::as_str).unwrap_or(&repo);
    let desc = v.get("description").and_then(Value::as_str).unwrap_or("");
    let vis = format_visibility(v.get("visibility_level"));
    let branch = v.get("default_branch").and_then(Value::as_str).unwrap_or("");
    let upd = v
        .get("updated_at")
        .or_else(|| v.get("last_activity_at"))
        .and_then(Value::as_str)
        .unwrap_or("");
    println!("{path}");
    println!("  Visibility:    {vis}");
    if !branch.is_empty() {
        println!("  Default branch: {branch}");
    }
    if !desc.is_empty() {
        println!("  Description:   {desc}");
    }
    if !upd.is_empty() {
        println!("  Last activity: {upd}");
    }
    Ok(())
}

async fn create(ctx: &mut Context, args: CreateArgs) -> Result<(), CliError> {
    // Split slug into owner-path and final name. The "owner" part is the
    // group/user namespace the repo will live under (`/{slug}/-/repos`),
    // the final segment is the new repo's name.
    let (slug, name) = args
        .slug
        .rsplit_once('/')
        .ok_or_else(|| CliError::BadArgs(format!("expected `OWNER/NAME`: {}", args.slug)))?;
    if slug.is_empty() || name.is_empty() {
        return Err(CliError::BadArgs(format!("slug `{}` must be `OWNER/NAME`", args.slug)));
    }
    if args.default_branch.is_some() {
        // The SDK's `CreateRepoReq` does not include `default_branch` —
        // see SDK-I11 in docs/sdk-issues.md. The cnb-api facade silently
        // dropped it before; we surface the gap rather than pretending.
        return Err(CliError::BadArgs(
            "--default-branch is not supported by the create endpoint; \
             create the repo first, then push your branch and update via the web UI"
                .into(),
        ));
    }
    let body = CreateRepoReq {
        name: Some(name.to_owned()),
        description: args.description.clone(),
        license: None,
        // SDK aliases `Visibility = String` — pass the canonical name
        // directly. CLI argv has already been validated by clap's
        // value_parser to be one of `public|internal|private`.
        visibility: Some(args.visibility.clone()),
    };
    let v = {
        let client = ctx.sdk()?;
        client.repositories().create_repo(slug.to_owned(), &body).await?
    };
    let path = v
        .get("path")
        .and_then(Value::as_str)
        .map_or_else(|| format!("{slug}/{name}"), str::to_owned);
    eprintln!("✓ Created {path}");

    if args.clone {
        let dir = PathBuf::from(name);
        clone_into(&path, Some(dir), &ctx.host)?;
    }
    Ok(())
}

fn clone(ctx: &mut Context, args: CloneArgs) -> Result<(), CliError> {
    clone_into(&args.repo, args.dir, &ctx.host)
}

fn clone_into(repo: &str, dir: Option<PathBuf>, host: &str) -> Result<(), CliError> {
    let url = format!("https://{host}/{repo}.git");
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg(&url);
    if let Some(d) = &dir {
        cmd.arg(d);
    }
    let status = cmd.status()?;
    if !status.success() {
        return Err(CliError::Generic(format!(
            "git clone failed (status {})",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

async fn fork(ctx: &mut Context, args: ForkArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // SDK returns a `ListForks { fork_tree_count, forks: Option<Vec<Forks>> }`
    // wrapper; we unwrap to the inner array so `--json` keeps producing a
    // bare array (matching `gh repo fork --json`'s output shape and the
    // cnb-api facade's previous behaviour). See SDK-I13.
    let dto = {
        let client = ctx.sdk()?;
        let q = ListForksReposQuery::new();
        client.repositories().list_forks_repos(repo, &q).await?
    };
    let items = dto.forks.unwrap_or_default();
    let v = serde_json::to_value(&items).expect("Forks serialises infallibly");
    if args.out.render_value(ctx, &v)? {
        return Ok(());
    }
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(arr.len());
    for it in &arr {
        let path = it.get("path").and_then(Value::as_str).unwrap_or("");
        let upd = it
            .get("updated_at")
            .or_else(|| it.get("last_activity_at"))
            .and_then(Value::as_str)
            .unwrap_or("");
        rows.push(vec![path.to_owned(), upd.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["FORK", "UPDATED"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Type to confirm deletion of `{repo}` (y/N)"), args.yes)?;
    let client = ctx.sdk()?;
    let _ = client.repositories().delete_repo(repo.clone()).await?;
    eprintln!("✓ Deleted {repo}");
    Ok(())
}

async fn edit(ctx: &mut Context, args: EditArgs) -> Result<(), CliError> {
    // The SDK's `RepoPatch` body only carries `description / license /
    // site / topics`. `--name` (rename) and `--default-branch` are not
    // representable on this endpoint — see SDK-I11. We surface the gap
    // explicitly rather than silently dropping the flags the way the
    // cnb-api facade did.
    if args.name.is_some() {
        return Err(CliError::BadArgs(
            "--name (rename) is not supported by the PATCH /{repo} endpoint; \
             use the web UI for now"
                .into(),
        ));
    }
    if args.default_branch.is_some() {
        return Err(CliError::BadArgs(
            "--default-branch is not supported by the PATCH /{repo} endpoint; \
             push the desired branch and set it as default via the web UI"
                .into(),
        ));
    }
    if args.description.is_none() {
        return Err(CliError::BadArgs(
            "nothing to edit — pass --description (rename / default-branch must \
             use the web UI for now, see `cnb repo edit --help`)"
                .into(),
        ));
    }
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let body = RepoPatch {
        description: args.description.clone(),
        license: None,
        site: None,
        topics: None,
    };
    let client = ctx.sdk()?;
    let _ = client.repositories().update_repo(repo.clone(), &body).await?;
    eprintln!("✓ Updated {repo}");
    Ok(())
}

async fn archive(ctx: &mut Context, args: ArchiveArgs, unarchive: bool) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.sdk()?;
    if unarchive {
        let _ = client.repositories().un_archive_repo(repo.clone()).await?;
        eprintln!("✓ Unarchived {repo}");
    } else {
        let _ = client.repositories().archive_repo(repo.clone()).await?;
        eprintln!("✓ Archived {repo}");
    }
    Ok(())
}

async fn transfer(ctx: &mut Context, args: TransferArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Transfer `{repo}` → `{}/`? (y/N)", args.to), args.yes)?;
    // The SDK's `TransferSlugReq` has both `source` and `target`. The
    // server reads `source` from the URL path, so we only need to set
    // `target`; mirror what the cnb-api facade did.
    let body = TransferSlugReq {
        source: None,
        target: Some(args.to.clone()),
    };
    let client = ctx.sdk()?;
    let _ = client.repositories().transfer_repo(repo.clone(), &body).await?;
    eprintln!("✓ Transferred {repo} → {}", args.to);
    Ok(())
}

async fn set_visibility(ctx: &mut Context, args: SetVisibilityArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // SDK sends `visibility` as a **query parameter**, not a body. See
    // SDK-I12: this disagrees with our prior cnb-api facade which sent a
    // JSON body `{visibility_level: 0|10|20}`. We follow the SDK on the
    // assumption it tracks the OpenAPI spec; if the server rejects the
    // query form, bump SDK-I12 to blocker.
    let q = SetRepoVisibilityQuery::new().visibility(args.visibility.clone());
    let client = ctx.sdk()?;
    let _ = client.repositories().set_repo_visibility(repo.clone(), &q).await?;
    eprintln!("✓ {repo}: visibility set to {}", args.visibility);
    Ok(())
}

async fn pin(ctx: &mut Context, args: PinArgs, add: bool) -> Result<(), CliError> {
    // Resolved upstream in cnb 0.2.2 (SDK-I18): `set_pinned_repo_by_group`
    // is now a first-class typed method, so the read-modify-write cycle
    // can stay entirely on the SDK's typed surface — no more
    // `Context::sdk_raw_json` round-trip.
    //
    // **Wire-shape note**: the SDK serialises `body: &Vec<String>` as a
    // bare JSON array (`["cnb/docs","cnb/feedback"]`) rather than the
    // `{"repos":[…]}` envelope our previous raw-PUT used. We follow the
    // SDK on the assumption it tracks the OpenAPI spec; the wiremock
    // tests below were updated alongside this method to match.
    let current = {
        let client = ctx.sdk()?;
        client
            .repositories()
            .get_pinned_repo_by_group(args.slug.clone())
            .await?
    };
    let mut set: std::collections::BTreeSet<String> = current
        .iter()
        .filter_map(|r| r.path.clone().or_else(|| r.name.clone()))
        .collect();
    if add {
        for r in &args.repos {
            set.insert(r.clone());
        }
    } else {
        for r in &args.repos {
            set.remove(r);
        }
    }
    let final_list: Vec<String> = set.into_iter().collect();
    {
        let client = ctx.sdk()?;
        let _ = client
            .repositories()
            .set_pinned_repo_by_group(args.slug.clone(), &final_list)
            .await?;
    }
    eprintln!("✓ {} pinned set updated ({} entries)", args.slug, final_list.len());
    Ok(())
}

async fn list_pinned(ctx: &mut Context, args: ListPinnedArgs) -> Result<(), CliError> {
    let items = {
        let client = ctx.sdk()?;
        client
            .repositories()
            .get_pinned_repo_by_group(args.slug.clone())
            .await?
    };
    // Serialise the typed Vec back to a JSON array for rendering.
    let v = Value::Array(
        items
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| CliError::Generic(format!("serialise pinned repos: {e}")))?,
    );
    if args.out.render_value(ctx, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let path = it.path.as_deref().or(it.name.as_deref()).unwrap_or("");
        let desc = it.description.as_deref().unwrap_or("");
        rows.push(vec![path.to_owned(), desc.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["REPO", "DESCRIPTION"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn contributors(ctx: &mut Context, args: ContributorsArgs) -> Result<(), CliError> {
    // The SDK's `get_repo_contributor_trend` query only exposes
    // `limit` / `exclude_external_users` — the `days=N` window that
    // our CLI has always offered is not in the typed query struct.
    // Preserve the CLI contract by routing through `sdk_raw_get`
    // when `--days` is set, which lets us pass the raw query string
    // verbatim. Without `--days`, call the typed method so the
    // request still exercises the SDK's typed path. See SDK-I17.
    let v = if let Some(days) = args.days {
        ctx.sdk_raw_get(&format!("/{}/-/contributor/trend?days={days}", args.slug))
            .await?
    } else {
        let q = cnb_sdk::repo_contributor::GetRepoContributorTrendQuery::new();
        let trend = {
            let client = ctx.sdk()?;
            client
                .repo_contributor()
                .get_repo_contributor_trend(args.slug.clone(), &q)
                .await?
        };
        serde_json::to_value(&trend).map_err(|e| CliError::Generic(format!("serialise contributor trend: {e}")))?
    };
    if args.out.render_value(ctx, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    cnb_tty::json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_visibility_accepts_strings() {
        // Canonical wire form (SDK 0.2.2 default).
        assert_eq!(format_visibility(Some(&json!("Public"))), "Public");
        assert_eq!(format_visibility(Some(&json!("Private"))), "Private");
        assert_eq!(format_visibility(Some(&json!("Secret"))), "Secret");
        // Lowercase + legacy `Internal` → still understood, normalised to
        // canonical capitalisation on output.
        assert_eq!(format_visibility(Some(&json!("public"))), "Public");
        assert_eq!(format_visibility(Some(&json!("private"))), "Private");
        assert_eq!(format_visibility(Some(&json!("internal"))), "Secret");
        assert_eq!(format_visibility(Some(&json!("Internal"))), "Secret");
        assert_eq!(format_visibility(Some(&json!("weird"))), "?");
    }

    #[test]
    fn format_visibility_tolerates_legacy_integer_encoding() {
        assert_eq!(format_visibility(Some(&json!(0))), "Public");
        // SDK 0.2.2 maps the integer `10` (legacy "internal") onto
        // `Visibility::Secret`. We follow that.
        assert_eq!(format_visibility(Some(&json!(10))), "Secret");
        assert_eq!(format_visibility(Some(&json!(20))), "Private");
        assert_eq!(format_visibility(Some(&json!(99))), "?");
    }

    #[test]
    fn format_visibility_handles_missing_and_null() {
        assert_eq!(format_visibility(None), "?");
        assert_eq!(format_visibility(Some(&json!(null))), "?");
    }
}
