//! `cnb issue` — issues, comments, assignees, labels (M2 §8.3, 11 subcommands).
//!
//! Fully SDK-backed as of Phase 2 step 2.11; the `--attach` flow on
//! `create` / `comment` (multipart upload) is handled by the local
//! [`crate::http::uploads`] module, which itself rides on top of the
//! SDK's shared `reqwest::Client` (so attachment requests carry the
//! same `Authorization`, `User-Agent`, base URL, and connection pool
//! as every typed call). Every other verb — `list`, `view`, `create`,
//! `edit`, `close`, `reopen`, `comment`, `comment-edit`, `assign`,
//! `label`, `comments` (list), `activity`, `properties` (read and
//! write) — runs through `cnb_sdk::issues::IssuesClient`.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_sdk::issues::{ListIssueActivitiesQuery, ListIssuesQuery, ListUserIssuesQuery};
use cnb_sdk::models::{
    DeleteIssueAssigneesForm, IssuePropertiesForm, PatchIssueCommentForm, PatchIssueForm, PostIssueAssigneesForm,
    PostIssueCommentForm, PostIssueForm, PostIssueLabelsForm, PropertyForm,
};
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;
use crate::http::uploads;

#[derive(Debug, Args)]
pub struct IssueArgs {
    #[command(subcommand)]
    pub command: IssueCmd,
}

#[derive(Debug, Subcommand)]
pub enum IssueCmd {
    /// List issues in a repository (or across the current user).
    List(ListArgs),
    /// View an issue.
    View(ViewArgs),
    /// Create a new issue.
    Create(CreateArgs),
    /// Edit an issue's title/body/state.
    Edit(EditArgs),
    /// Close an issue.
    Close(NumberArgs),
    /// Reopen an issue.
    Reopen(NumberArgs),
    /// Add a comment to an issue.
    Comment(CommentArgs),
    /// Edit an existing comment.
    CommentEdit(CommentEditArgs),
    /// Add or remove assignees.
    Assign(AssignArgs),
    /// Add or remove labels.
    Label(LabelArgs),
    /// List comments for an issue.
    Comments(NumberArgs),
    /// Show an issue's timeline activities (M3).
    Activity(ActivityArgs),
    /// View or set an issue's custom properties (M3).
    Properties(PropertiesArgs),
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

#[derive(Debug, Args)]
pub struct ListArgs {
    pub repo: Option<String>,
    /// `open` (default) | `closed` | `all`.
    #[arg(long, default_value = "open")]
    pub state: String,
    /// Restrict to issues authored by the current user.
    #[arg(long)]
    pub mine: bool,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[arg(long, default_value_t = 1u32)]
    pub page: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub title: String,
    #[arg(long)]
    pub body: Option<String>,
    pub repo: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub assignee: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    pub label: Vec<String>,
    #[arg(long)]
    pub priority: Option<String>,
    /// Attach one or more files (auto-detected as image vs file).
    #[arg(long, value_name = "PATH")]
    pub attach: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    /// `open` | `closed`.
    #[arg(long)]
    pub state: Option<String>,
    #[arg(long)]
    pub priority: Option<String>,
}

#[derive(Debug, Args)]
pub struct NumberArgs {
    pub number: u64,
    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct ActivityArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[arg(long, default_value_t = 1u32)]
    pub page: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct PropertiesArgs {
    pub number: u64,
    pub repo: Option<String>,
    /// Set one or more properties as `KEY=VALUE` (multiple `--set` allowed).
    /// When omitted, lists current properties.
    #[arg(long, value_name = "KEY=VALUE")]
    pub set: Vec<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct CommentArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[arg(long, conflicts_with = "body_file")]
    pub body: Option<String>,
    /// Read body from a file (`-` for stdin).
    #[arg(long = "body-file", value_name = "PATH")]
    pub body_file: Option<String>,
    #[arg(long, value_name = "PATH")]
    pub attach: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CommentEditArgs {
    pub number: u64,
    pub comment_id: u64,
    pub repo: Option<String>,
    #[arg(long)]
    pub body: String,
}

#[derive(Debug, Args)]
pub struct AssignArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub add: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    pub remove: Vec<String>,
}

#[derive(Debug, Args)]
pub struct LabelArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub add: Vec<String>,
    /// Remove a single label (server limitation: deletes one at a time).
    #[arg(long)]
    pub remove: Option<String>,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: IssueArgs) -> Result<(), CliError> {
    match args.command {
        IssueCmd::List(a) => list(ctx, a).await,
        IssueCmd::View(a) => view(ctx, a).await,
        IssueCmd::Create(a) => create(ctx, a).await,
        IssueCmd::Edit(a) => edit(ctx, a).await,
        IssueCmd::Close(a) => close_or_reopen(ctx, a, true).await,
        IssueCmd::Reopen(a) => close_or_reopen(ctx, a, false).await,
        IssueCmd::Comment(a) => comment(ctx, a).await,
        IssueCmd::CommentEdit(a) => comment_edit(ctx, a).await,
        IssueCmd::Assign(a) => assign(ctx, a).await,
        IssueCmd::Label(a) => label(ctx, a).await,
        IssueCmd::Comments(a) => comments(ctx, a).await,
        IssueCmd::Activity(a) => activity(ctx, a).await,
        IssueCmd::Properties(a) => properties(ctx, a).await,
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
    let page = i64::from(args.page);
    let page_size = i64::from(args.limit.max(1));
    // Upstream treats `all` specially (omit the query param); `open` and
    // `closed` pass through verbatim.
    let state = if args.state == "all" {
        None
    } else {
        Some(args.state.clone())
    };

    let items: Vec<Value> = if args.mine {
        // `GET /user/issues` — issues assigned to / authored by the current
        // token. The upstream query schema is a superset of the repo-scoped
        // one; we only wire the fields our CLI currently exposes.
        let mut q = ListUserIssuesQuery::new().page(page).page_size(page_size);
        if let Some(s) = state {
            q = q.state(s);
        }
        let client = ctx.sdk()?;
        let dto = client.issues().list_user_issues(&q).await?;
        // `UserIssue` -> generic Value for uniform rendering. Same
        // Serialize guarantee as elsewhere.
        serde_json::to_value(&dto)
            .expect("UserIssue serialises infallibly")
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        // `GET /{repo}/-/issues` — repo-scoped listing. Typed response is
        // `Vec<Issue>`.
        let repo = ctx.resolve_repo(args.repo.as_deref())?;
        let mut q = ListIssuesQuery::new().page(page).page_size(page_size);
        if let Some(s) = state {
            q = q.state(s);
        }
        let client = ctx.sdk()?;
        let dto = client.issues().list_issues(repo, &q).await?;
        serde_json::to_value(&dto)
            .expect("Issue serialises infallibly")
            .as_array()
            .cloned()
            .unwrap_or_default()
    };

    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }

    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        // The typed DTO pins `number` to a string, but older servers (and
        // our legacy mocks) sometimes emit integers. Accept both on the
        // display path so real-world responses keep rendering.
        let n_display = format_issue_number(it.get("number"));
        let title = it.get("title").and_then(Value::as_str).unwrap_or("");
        let labels = it
            .get("labels")
            .and_then(|l| l.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let upd = it.get("updated_at").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![n_display, title.to_owned(), labels, upd.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["#", "TITLE", "LABELS", "UPDATED"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // SDK pins issue numbers to `i64`. Our CLI accepts `u64` for API
    // ergonomics; the cast is safe in practice (issue numbers never
    // approach `i64::MAX`) but we guard it anyway.
    let number_i64 = i64::try_from(args.number)
        .map_err(|_| CliError::BadArgs(format!("issue number out of range: {}", args.number)))?;
    // The `IssueDetail` DTO covers everything the CLI needs to render
    // (title / state / body / author / labels / assignees), so a single
    // typed call is sufficient — no raw-Value double-fetch required.
    let dto = {
        let client = ctx.sdk()?;
        client.issues().get_issue(repo, number_i64).await?
    };
    let v = serde_json::to_value(&dto).expect("IssueDetail serialises infallibly");
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let title = v.get("title").and_then(Value::as_str).unwrap_or("");
    let state = v.get("state").and_then(Value::as_str).unwrap_or("?");
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    let author = v
        .get("author")
        .and_then(|a| a.get("username").or_else(|| a.get("nickname")))
        .and_then(Value::as_str)
        .unwrap_or("?");
    println!("#{} {title}  [{state}]", args.number);
    println!("  Author: {author}");
    if !body.is_empty() {
        println!();
        println!("{body}");
    }
    Ok(())
}

/// Display formatter for an issue's `number` field.
///
/// The upstream OpenAPI spec models issue numbers as strings, but several
/// cnb.cool deployments still return integers. We accept either on the
/// display path and always render as `#<n>` so the UX is stable regardless
/// of how the server encodes the value.
fn format_issue_number(raw: Option<&Value>) -> String {
    match raw {
        Some(Value::String(s)) => format!("#{s}"),
        Some(Value::Number(n)) => format!("#{n}"),
        _ => String::from("#?"),
    }
}

async fn create(ctx: &mut Context, args: CreateArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut body = args.body.unwrap_or_default();
    if !args.attach.is_empty() {
        // The attachment endpoints are multipart/form-data, which the
        // typed SDK does not model directly — but `crate::http::uploads`
        // rides on top of `client.http().reqwest_client()`, so the same
        // bearer auth + base URL + connection pool the typed calls use
        // are reused for the attachment POSTs. The write of the issue
        // itself stays on the typed SDK.
        body = append_attachments(ctx, &repo, None, body, &args.attach).await?;
    }
    let payload = PostIssueForm {
        title: Some(args.title),
        body: if body.is_empty() { None } else { Some(body) },
        assignees: if args.assignee.is_empty() {
            None
        } else {
            Some(args.assignee)
        },
        labels: if args.label.is_empty() { None } else { Some(args.label) },
        priority: args.priority,
        ..Default::default()
    };
    let dto = {
        let client = ctx.sdk()?;
        client.issues().create_issue(repo.clone(), &payload).await?
    };
    // `IssueDetail.number` is `Option<String>` — parse to integer for
    // the log line when possible, fall back to the raw string.
    let n = dto
        .number
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .map(|n| n.to_string())
        .or(dto.number)
        .unwrap_or_else(|| "?".into());
    eprintln!("✓ Created #{n} in {repo}");
    Ok(())
}

async fn edit(ctx: &mut Context, args: EditArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    if args.title.is_none() && args.body.is_none() && args.state.is_none() && args.priority.is_none() {
        return Err(CliError::BadArgs(
            "nothing to edit — pass --title/--body/--state/--priority".into(),
        ));
    }
    if let Some(s) = &args.state {
        if !matches!(s.as_str(), "open" | "closed") {
            return Err(CliError::BadArgs(format!("--state must be open|closed, got {s}")));
        }
    }
    let body = PatchIssueForm {
        title: args.title,
        body: args.body,
        state: args.state,
        priority: args.priority,
        ..Default::default()
    };
    let number_i64 = issue_number_i64(args.number)?;
    let client = ctx.sdk()?;
    let _ = client.issues().update_issue(repo.clone(), number_i64, &body).await?;
    eprintln!("✓ Updated #{} in {repo}", args.number);
    Ok(())
}

async fn close_or_reopen(ctx: &mut Context, args: NumberArgs, close: bool) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // Close / reopen are thin wrappers around PATCH state={closed,open}
    // — the SDK has no dedicated close_issue verb. Keep the CLI
    // contract (`cnb issue close N`) by delegating to `update_issue`.
    let body = PatchIssueForm {
        state: Some(if close { "closed".into() } else { "open".into() }),
        ..Default::default()
    };
    let number_i64 = issue_number_i64(args.number)?;
    let client = ctx.sdk()?;
    let _ = client.issues().update_issue(repo.clone(), number_i64, &body).await?;
    if close {
        eprintln!("✓ Closed #{} in {repo}", args.number);
    } else {
        eprintln!("✓ Reopened #{} in {repo}", args.number);
    }
    Ok(())
}

async fn comment(ctx: &mut Context, args: CommentArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut body = match (&args.body, &args.body_file) {
        (Some(b), _) => b.clone(),
        (None, Some(p)) if p == "-" => {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
        (None, Some(p)) => std::fs::read_to_string(p)?,
        (None, None) => String::new(),
    };
    if !args.attach.is_empty() {
        body = append_attachments(ctx, &repo, Some(args.number), body, &args.attach).await?;
    }
    if body.trim().is_empty() {
        return Err(CliError::BadArgs(
            "comment body is empty — pass --body, --body-file, or --attach".into(),
        ));
    }
    let payload = PostIssueCommentForm {
        body: Some(body),
        ..Default::default()
    };
    let number_i64 = issue_number_i64(args.number)?;
    let client = ctx.sdk()?;
    let _ = client
        .issues()
        .post_issue_comment(repo.clone(), number_i64, &payload)
        .await?;
    eprintln!("✓ Commented on #{} in {repo}", args.number);
    Ok(())
}

async fn comment_edit(ctx: &mut Context, args: CommentEditArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let payload = PatchIssueCommentForm {
        body: Some(args.body.clone()),
    };
    let number_i64 = issue_number_i64(args.number)?;
    let comment_id_i64 = i64::try_from(args.comment_id)
        .map_err(|_| CliError::BadArgs(format!("comment id out of range: {}", args.comment_id)))?;
    let client = ctx.sdk()?;
    let _ = client
        .issues()
        .patch_issue_comment(repo.clone(), number_i64, comment_id_i64, &payload)
        .await?;
    eprintln!("✓ Edited comment {} on #{}", args.comment_id, args.number);
    Ok(())
}

async fn assign(ctx: &mut Context, args: AssignArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    if args.add.is_empty() && args.remove.is_empty() {
        return Err(CliError::BadArgs(
            "pass --add USER[,USER..] and/or --remove USER[,USER..]".into(),
        ));
    }
    // SDK's assignee endpoints take `number: String` (inconsistent with
    // the rest of issues, see SDK-I07). Build it once up front.
    let number_str = args.number.to_string();
    let client = ctx.sdk()?;
    if !args.add.is_empty() {
        let payload = PostIssueAssigneesForm {
            assignees: Some(args.add.clone()),
        };
        let _ = client
            .issues()
            .post_issue_assignees(repo.clone(), number_str.clone(), &payload)
            .await?;
        eprintln!("✓ Added assignees: {}", args.add.join(", "));
    }
    if !args.remove.is_empty() {
        let payload = DeleteIssueAssigneesForm {
            assignees: Some(args.remove.clone()),
        };
        let _ = client
            .issues()
            .delete_issue_assignees(repo.clone(), number_str, &payload)
            .await?;
        eprintln!("✓ Removed assignees: {}", args.remove.join(", "));
    }
    Ok(())
}

async fn label(ctx: &mut Context, args: LabelArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    if args.add.is_empty() && args.remove.is_none() {
        return Err(CliError::BadArgs(
            "pass --add LABEL[,LABEL..] and/or --remove LABEL".into(),
        ));
    }
    let number_i64 = issue_number_i64(args.number)?;
    let client = ctx.sdk()?;
    if !args.add.is_empty() {
        let payload = PostIssueLabelsForm {
            labels: Some(args.add.clone()),
        };
        let _ = client
            .issues()
            .post_issue_labels(repo.clone(), number_i64, &payload)
            .await?;
        eprintln!("✓ Added labels: {}", args.add.join(", "));
    }
    if let Some(name) = args.remove {
        // Keep the path-traversal guard that `cnb-api::labels::ensure_no_slash`
        // had — the SDK itself does not validate (SDK-I10).
        if name.contains('/') {
            return Err(CliError::BadArgs(format!("label name must not contain `/`: {name:?}")));
        }
        let _ = client
            .issues()
            .delete_issue_label(repo.clone(), number_i64, name.clone())
            .await?;
        eprintln!("✓ Removed label: {name}");
    }
    Ok(())
}

async fn comments(ctx: &mut Context, args: NumberArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let number_i64 = issue_number_i64(args.number)?;
    let client = ctx.sdk()?;
    let items = client
        .issues()
        .list_issue_comments(repo, number_i64, &cnb_sdk::issues::ListIssueCommentsQuery::new())
        .await?;
    let v = serde_json::to_value(&items).map_err(|e| CliError::Generic(format!("serialise comments: {e}")))?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn activity(ctx: &mut Context, args: ActivityArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let number_i64 = issue_number_i64(args.number)?;
    let q = ListIssueActivitiesQuery::new()
        .page(i64::from(args.page))
        .page_size(i64::from(args.limit.max(1)));
    let items = {
        let client = ctx.sdk()?;
        client.issues().list_issue_activities(repo, number_i64, &q).await?
    };
    let v = serde_json::to_value(&items).map_err(|e| CliError::Generic(format!("serialise activities: {e}")))?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(arr.len());
    for it in &arr {
        let t = it.get("type").and_then(Value::as_str).unwrap_or("");
        let actor = it
            .get("actor")
            .and_then(|a| a.get("username"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let at = it
            .get("submitted_at")
            .or_else(|| it.get("created_at"))
            .and_then(Value::as_str)
            .unwrap_or("");
        rows.push(vec![t.to_owned(), actor.to_owned(), at.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["TYPE", "ACTOR", "WHEN"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn properties(ctx: &mut Context, args: PropertiesArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let number_i64 = issue_number_i64(args.number)?;
    if args.set.is_empty() {
        // Read mode.
        let items = {
            let client = ctx.sdk()?;
            client.issues().get_issue_properties(repo, number_i64).await?
        };
        let v = serde_json::to_value(&items).map_err(|e| CliError::Generic(format!("serialise properties: {e}")))?;
        if render(ctx, &args.out, &v)? {
            return Ok(());
        }
        let arr = v.as_array().cloned().unwrap_or_default();
        let mut rows = Vec::with_capacity(arr.len());
        for it in &arr {
            let key = it.get("key").and_then(Value::as_str).unwrap_or("");
            let name = it.get("name").and_then(Value::as_str).unwrap_or("");
            let val = it.get("value").and_then(Value::as_str).unwrap_or("");
            rows.push(vec![key.to_owned(), name.to_owned(), val.to_owned()]);
        }
        let mut stdout = std::io::stdout().lock();
        table::write_table(&mut stdout, &["KEY", "NAME", "VALUE"], &rows, ctx.io.stdout_is_tty)?;
        return Ok(());
    }
    // Write mode.
    let mut updates = Vec::with_capacity(args.set.len());
    for kv in &args.set {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| CliError::BadArgs(format!("--set must be KEY=VALUE: {kv}")))?;
        updates.push(PropertyForm {
            key: Some(k.to_owned()),
            value: Some(v.to_owned()),
        });
    }
    let body = IssuePropertiesForm {
        properties: Some(updates),
    };
    let client = ctx.sdk()?;
    let _ = client.issues().update_issue_properties(repo, number_i64, &body).await?;
    eprintln!("✓ Updated {} property/properties on #{}", args.set.len(), args.number);
    Ok(())
}

/// Convert the CLI's `u64` issue-number parameter into the `i64` the
/// SDK expects, surfacing a clear `BadArgs` on overflow instead of a
/// `try_from` panic.
fn issue_number_i64(n: u64) -> Result<i64, CliError> {
    i64::try_from(n).map_err(|_| CliError::BadArgs(format!("issue number out of range: {n}")))
}

/// Stream every file in `paths` to CNB and append the returned URLs to `body`
/// as markdown (`![name](url)` for images, `[name](url)` for files).
///
/// `comment_number` selects the upload endpoint family:
/// - `None` → repo-scoped (`/{repo}/-/upload/{files,imgs}`)
/// - `Some(n)` → comment-scoped (`/{repo}/-/issues/{n}/comment-*-asset-upload-url`)
async fn append_attachments(
    ctx: &mut Context,
    repo: &str,
    comment_number: Option<u64>,
    mut body: String,
    paths: &[PathBuf],
) -> Result<String, CliError> {
    let scope = match comment_number {
        Some(n) => uploads::Scope::IssueComment { repo, number: n },
        None => uploads::Scope::Repo(repo),
    };
    for p in paths {
        let up = uploads::upload_one(ctx, scope.clone(), p, None).await?;
        // Append a blank line + link.
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push('\n');
        match up.kind {
            uploads::Kind::Image => {
                use std::fmt::Write;
                writeln!(&mut body, "![{}]({})", up.original_name, up.url).expect("write to String");
            }
            uploads::Kind::File => {
                use std::fmt::Write;
                writeln!(&mut body, "[{}]({})", up.original_name, up.url).expect("write to String");
            }
        }
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_issue_number_accepts_string() {
        assert_eq!(format_issue_number(Some(&json!("42"))), "#42");
        assert_eq!(format_issue_number(Some(&json!("1"))), "#1");
    }

    #[test]
    fn format_issue_number_accepts_integer() {
        assert_eq!(format_issue_number(Some(&json!(42))), "#42");
        assert_eq!(format_issue_number(Some(&json!(0))), "#0");
    }

    #[test]
    fn format_issue_number_falls_back_on_missing_or_null() {
        assert_eq!(format_issue_number(None), "#?");
        assert_eq!(format_issue_number(Some(&json!(null))), "#?");
        assert_eq!(format_issue_number(Some(&json!([]))), "#?");
    }
}
