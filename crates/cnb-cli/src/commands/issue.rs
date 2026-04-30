//! `cnb issue` — issues, comments, assignees, labels (M2 §8.3, 11 subcommands).
//!
//! `create` and `comment` support `--attach FILE...` which streams each file
//! through CNB's upload endpoints and appends the returned URL to the issue
//! body / comment body as markdown.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_api::services::{issues, uploads};
use cnb_api::Client;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

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
    let mut q = format!("page={}&per_page={}", args.page, args.limit.max(1));
    if args.state != "all" {
        use std::fmt::Write;
        write!(&mut q, "&state={}", args.state).expect("write to String");
    }
    let client = ctx.api()?;
    let items = if args.mine {
        issues::list_self(client, &q).await?
    } else {
        let repo = ctx.resolve_repo(args.repo.as_deref())?;
        let client = ctx.api()?; // re-borrow OK
        issues::list(client, &repo, &q).await?
    };
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let n = it.get("number").and_then(Value::as_i64).unwrap_or(0);
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
        rows.push(vec![format!("#{n}"), title.to_owned(), labels, upd.to_owned()]);
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
    let client = ctx.api()?;
    let v = issues::view(client, &repo, args.number).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let title = v.get("title").and_then(Value::as_str).unwrap_or("");
    let state = v.get("state").and_then(Value::as_str).unwrap_or("?");
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    let author = v
        .get("author")
        .and_then(|a| a.get("username"))
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

async fn create(ctx: &mut Context, args: CreateArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut body = args.body.unwrap_or_default();
    if !args.attach.is_empty() {
        let client = ctx.api()?;
        body = append_attachments(client, &repo, None, body, &args.attach).await?;
    }
    let payload = issues::CreateIssueBody {
        title: args.title,
        body: if body.is_empty() { None } else { Some(body) },
        assignees: args.assignee,
        labels: args.label,
        priority: args.priority,
    };
    let client = ctx.api()?;
    let v = issues::create(client, &repo, &payload).await?;
    let n = v.get("number").and_then(Value::as_i64).unwrap_or(0);
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
    let body = issues::EditIssueBody {
        title: args.title,
        body: args.body,
        state: args.state,
        priority: args.priority,
    };
    let client = ctx.api()?;
    let _ = issues::edit(client, &repo, args.number, &body).await?;
    eprintln!("✓ Updated #{} in {repo}", args.number);
    Ok(())
}

async fn close_or_reopen(ctx: &mut Context, args: NumberArgs, close: bool) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    if close {
        let _ = issues::close(client, &repo, args.number).await?;
        eprintln!("✓ Closed #{} in {repo}", args.number);
    } else {
        let _ = issues::reopen(client, &repo, args.number).await?;
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
        let client = ctx.api()?;
        body = append_attachments(client, &repo, Some(args.number), body, &args.attach).await?;
    }
    if body.trim().is_empty() {
        return Err(CliError::BadArgs(
            "comment body is empty — pass --body, --body-file, or --attach".into(),
        ));
    }
    let client = ctx.api()?;
    let _ = issues::comment(client, &repo, args.number, &body).await?;
    eprintln!("✓ Commented on #{} in {repo}", args.number);
    Ok(())
}

async fn comment_edit(ctx: &mut Context, args: CommentEditArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let _ = issues::edit_comment(client, &repo, args.number, args.comment_id, &args.body).await?;
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
    let client = ctx.api()?;
    if !args.add.is_empty() {
        let _ = issues::add_assignees(client, &repo, args.number, &args.add).await?;
        eprintln!("✓ Added assignees: {}", args.add.join(", "));
    }
    if !args.remove.is_empty() {
        let _ = issues::remove_assignees(client, &repo, args.number, &args.remove).await?;
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
    let client = ctx.api()?;
    if !args.add.is_empty() {
        let _ = issues::add_labels(client, &repo, args.number, &args.add).await?;
        eprintln!("✓ Added labels: {}", args.add.join(", "));
    }
    if let Some(name) = args.remove {
        let _ = issues::remove_label(client, &repo, args.number, &name).await?;
        eprintln!("✓ Removed label: {name}");
    }
    Ok(())
}

async fn comments(ctx: &mut Context, args: NumberArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let items = issues::list_comments(client, &repo, args.number).await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &Value::Array(items), ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn activity(ctx: &mut Context, args: ActivityArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let q = format!("page={}&page_size={}", args.page, args.limit.max(1));
    let client = ctx.api()?;
    let items = issues::list_activities(client, &repo, args.number, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
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
    if args.set.is_empty() {
        // Read mode.
        let client = ctx.api()?;
        let items = issues::list_properties(client, &repo, args.number).await?;
        let v = Value::Array(items.clone());
        if render(ctx, &args.out, &v)? {
            return Ok(());
        }
        let mut rows = Vec::with_capacity(items.len());
        for it in &items {
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
        updates.push(issues::PropertyUpdate {
            key: k.to_owned(),
            value: v.to_owned(),
        });
    }
    let client = ctx.api()?;
    let _ = issues::set_properties(client, &repo, args.number, updates).await?;
    eprintln!("✓ Updated {} property/properties on #{}", args.set.len(), args.number);
    Ok(())
}

/// Stream every file in `paths` to CNB and append the returned URLs to `body`
/// as markdown (`![name](url)` for images, `[name](url)` for files).
///
/// `comment_number` selects the upload endpoint family:
/// - `None` → repo-scoped (`/{repo}/-/upload/{files,imgs}`)
/// - `Some(n)` → comment-scoped (`/{repo}/-/issues/{n}/comment-*-asset-upload-url`)
async fn append_attachments(
    client: &Client,
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
        let up = uploads::upload_one(client, scope.clone(), p, None).await?;
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
