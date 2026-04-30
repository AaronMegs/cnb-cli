//! `cnb pr` — pull requests (M2 §8.4, 12 subcommands). `cnb mr` is a CLI alias.

use std::process::Command;

use clap::{Args, Subcommand};
use cnb_api::services::pulls;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct PrArgs {
    #[command(subcommand)]
    pub command: PrCmd,
}

#[derive(Debug, Subcommand)]
pub enum PrCmd {
    /// List pull requests in a repository.
    List(ListArgs),
    /// View a PR.
    View(ViewArgs),
    /// Create a new PR.
    Create(CreateArgs),
    /// Edit a PR.
    Edit(EditArgs),
    /// Close a PR.
    Close(NumberArgs),
    /// Reopen a PR.
    Reopen(NumberArgs),
    /// Comment on a PR.
    Comment(CommentArgs),
    /// Show changed files in a PR.
    Diff(NumberArgs),
    /// List commits in a PR.
    Commits(NumberArgs),
    /// Check out a PR's source branch locally.
    Checkout(CheckoutArgs),
    /// Add or remove assignees.
    Assign(AssignArgs),
    /// Add labels (or remove a single label).
    Label(LabelArgs),
    /// Merge a PR.
    Merge(MergeArgs),
    /// Submit a review on a PR (M3).
    Review(ReviewArgs),
    /// Show CI/check statuses for a PR (M3).
    Checks(NumberArgs),
    /// Fetch multiple PRs by number in one round-trip (M3).
    Batch(BatchArgs),
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
    /// `open` (default) | `closed` | `merged` | `all`.
    #[arg(long, default_value = "open")]
    pub state: String,
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
    #[arg(long, value_name = "BRANCH")]
    pub base: String,
    /// Source branch (defaults to the current local branch).
    #[arg(long, value_name = "BRANCH")]
    pub head: Option<String>,
    pub repo: Option<String>,
    #[arg(long)]
    pub body: Option<String>,
    #[arg(long, value_delimiter = ',')]
    pub assignee: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    pub label: Vec<String>,
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
    pub base: Option<String>,
}

#[derive(Debug, Args)]
pub struct NumberArgs {
    pub number: u64,
    pub repo: Option<String>,
}

#[derive(Debug, Args)]
pub struct CommentArgs {
    pub number: u64,
    pub repo: Option<String>,
    #[arg(long)]
    pub body: String,
}

#[derive(Debug, Args)]
pub struct CheckoutArgs {
    pub number: u64,
    pub repo: Option<String>,
    /// Override the local branch name (defaults to `pr/<number>`).
    #[arg(long)]
    pub branch: Option<String>,
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
    #[arg(long)]
    pub remove: Option<String>,
}

#[derive(Debug, Args)]
pub struct MergeArgs {
    pub number: u64,
    pub repo: Option<String>,
    /// `merge` (default) | `squash` | `rebase`.
    #[arg(long, value_parser = ["merge", "squash", "rebase"], default_value = "merge")]
    pub method: String,
    #[arg(long)]
    pub commit_title: Option<String>,
    #[arg(long)]
    pub commit_message: Option<String>,
    /// Delete the source branch after a successful merge.
    #[arg(long)]
    pub delete_branch: bool,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ReviewArgs {
    pub number: u64,
    pub repo: Option<String>,
    /// Review event: must be exactly one.
    #[arg(long, conflicts_with_all = ["request_changes", "comment_event"])]
    pub approve: bool,
    #[arg(long = "request-changes", conflicts_with = "comment_event")]
    pub request_changes: bool,
    #[arg(long = "comment", conflicts_with_all = ["approve", "request_changes"])]
    pub comment_event: bool,
    #[arg(long)]
    pub body: Option<String>,
}

#[derive(Debug, Args)]
pub struct BatchArgs {
    /// One or more PR numbers.
    #[arg(required = true)]
    pub numbers: Vec<u64>,
    /// `OWNER/REPO[/SUBGROUP]` (or auto-detected from `git remote origin`).
    #[arg(long)]
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: PrArgs) -> Result<(), CliError> {
    match args.command {
        PrCmd::List(a) => list(ctx, a).await,
        PrCmd::View(a) => view(ctx, a).await,
        PrCmd::Create(a) => create(ctx, a).await,
        PrCmd::Edit(a) => edit(ctx, a).await,
        PrCmd::Close(a) => close_or_reopen(ctx, a, true).await,
        PrCmd::Reopen(a) => close_or_reopen(ctx, a, false).await,
        PrCmd::Comment(a) => comment(ctx, a).await,
        PrCmd::Diff(a) => diff(ctx, a).await,
        PrCmd::Commits(a) => commits(ctx, a).await,
        PrCmd::Checkout(a) => checkout(ctx, a).await,
        PrCmd::Assign(a) => assign(ctx, a).await,
        PrCmd::Label(a) => label(ctx, a).await,
        PrCmd::Merge(a) => merge(ctx, a).await,
        PrCmd::Review(a) => review(ctx, a).await,
        PrCmd::Checks(a) => checks(ctx, a).await,
        PrCmd::Batch(a) => batch(ctx, a).await,
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
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut q = format!("page={}&per_page={}", args.page, args.limit.max(1));
    if args.state != "all" {
        use std::fmt::Write;
        write!(&mut q, "&state={}", args.state).expect("write to String");
    }
    let client = ctx.api()?;
    let items = pulls::list(client, &repo, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let n = it.get("number").and_then(Value::as_i64).unwrap_or(0);
        let title = it.get("title").and_then(Value::as_str).unwrap_or("");
        let head = it.get("source_branch").and_then(Value::as_str).unwrap_or("");
        let base = it.get("target_branch").and_then(Value::as_str).unwrap_or("");
        let created = it.get("created_at").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![
            format!("#{n}"),
            title.to_owned(),
            format!("{head}->{base}"),
            created.to_owned(),
        ]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["#", "TITLE", "BRANCH", "CREATED"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = pulls::view(client, &repo, args.number).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let title = v.get("title").and_then(Value::as_str).unwrap_or("");
    let state = v.get("state").and_then(Value::as_str).unwrap_or("?");
    let head = v.get("source_branch").and_then(Value::as_str).unwrap_or("");
    let base = v.get("target_branch").and_then(Value::as_str).unwrap_or("");
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    println!("#{} {title}  [{state}]", args.number);
    println!("  {head} → {base}");
    if !body.is_empty() {
        println!();
        println!("{body}");
    }
    Ok(())
}

async fn create(ctx: &mut Context, args: CreateArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let head = match args.head {
        Some(h) => h,
        None => current_branch()?,
    };
    let payload = pulls::CreatePullBody {
        title: args.title,
        source_branch: head,
        target_branch: args.base,
        body: args.body,
        source_repo: None,
        assignees: args.assignee,
        labels: args.label,
    };
    let client = ctx.api()?;
    let v = pulls::create(client, &repo, &payload).await?;
    let n = v.get("number").and_then(Value::as_i64).unwrap_or(0);
    eprintln!("✓ Created PR #{n} in {repo}");
    Ok(())
}

async fn edit(ctx: &mut Context, args: EditArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    if args.title.is_none() && args.body.is_none() && args.state.is_none() && args.base.is_none() {
        return Err(CliError::BadArgs(
            "nothing to edit — pass --title/--body/--state/--base".into(),
        ));
    }
    let body = pulls::EditPullBody {
        title: args.title,
        body: args.body,
        state: args.state,
        target_branch: args.base,
    };
    let client = ctx.api()?;
    let _ = pulls::edit(client, &repo, args.number, &body).await?;
    eprintln!("✓ Updated PR #{} in {repo}", args.number);
    Ok(())
}

async fn close_or_reopen(ctx: &mut Context, args: NumberArgs, close: bool) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    if close {
        let _ = pulls::close(client, &repo, args.number).await?;
        eprintln!("✓ Closed PR #{} in {repo}", args.number);
    } else {
        let _ = pulls::reopen(client, &repo, args.number).await?;
        eprintln!("✓ Reopened PR #{} in {repo}", args.number);
    }
    Ok(())
}

async fn comment(ctx: &mut Context, args: CommentArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let _ = pulls::comment(client, &repo, args.number, &args.body).await?;
    eprintln!("✓ Commented on PR #{} in {repo}", args.number);
    Ok(())
}

async fn diff(ctx: &mut Context, args: NumberArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = pulls::files(client, &repo, args.number).await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn commits(ctx: &mut Context, args: NumberArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = pulls::commits(client, &repo, args.number).await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn checkout(ctx: &mut Context, args: CheckoutArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let pr = pulls::view(client, &repo, args.number).await?;
    let source_branch = pr
        .get("source_branch")
        .and_then(Value::as_str)
        .ok_or_else(|| CliError::Generic(format!("PR #{} has no source_branch", args.number)))?
        .to_owned();
    let local_branch = args.branch.unwrap_or_else(|| format!("pr/{}", args.number));

    // git fetch origin <source_branch>:<local_branch>
    let fetch_status = Command::new("git")
        .args(["fetch", "origin", &format!("{source_branch}:{local_branch}")])
        .status()?;
    if !fetch_status.success() {
        return Err(CliError::Generic(format!(
            "git fetch failed (status {})",
            fetch_status.code().unwrap_or(-1)
        )));
    }
    let checkout_status = Command::new("git").args(["checkout", &local_branch]).status()?;
    if !checkout_status.success() {
        return Err(CliError::Generic(format!(
            "git checkout failed (status {})",
            checkout_status.code().unwrap_or(-1)
        )));
    }
    eprintln!("✓ Checked out PR #{} as `{local_branch}`", args.number);
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
        let _ = pulls::add_assignees(client, &repo, args.number, &args.add).await?;
        eprintln!("✓ Added assignees: {}", args.add.join(", "));
    }
    if !args.remove.is_empty() {
        let _ = pulls::remove_assignees(client, &repo, args.number, &args.remove).await?;
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
        let _ = pulls::add_labels(client, &repo, args.number, &args.add).await?;
        eprintln!("✓ Added labels: {}", args.add.join(", "));
    }
    if let Some(name) = args.remove {
        let _ = pulls::remove_label(client, &repo, args.number, &name).await?;
        eprintln!("✓ Removed label: {name}");
    }
    Ok(())
}

async fn merge(ctx: &mut Context, args: MergeArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(
        &format!("Merge PR #{} into `{repo}` ({})? (y/N)", args.number, args.method),
        args.yes,
    )?;
    let body = pulls::MergeBody {
        merge_method: Some(args.method.clone()),
        commit_title: args.commit_title,
        commit_message: args.commit_message,
        remove_source_branch: if args.delete_branch { Some(true) } else { None },
    };
    let client = ctx.api()?;
    let _ = pulls::merge(client, &repo, args.number, &body).await?;
    eprintln!("✓ Merged PR #{} ({})", args.number, args.method);
    Ok(())
}

async fn review(ctx: &mut Context, args: ReviewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let event = match (args.approve, args.request_changes, args.comment_event) {
        (true, false, false) => "approve",
        (false, true, false) => "request_changes",
        (false, false, true) => "comment",
        (false, false, false) => {
            return Err(CliError::BadArgs(
                "pass exactly one of --approve / --request-changes / --comment".into(),
            ))
        }
        _ => unreachable!("clap conflicts_with prevents multi-select"),
    };
    let body = pulls::CreateReviewBody {
        event: event.to_owned(),
        body: args.body,
    };
    let client = ctx.api()?;
    let _ = pulls::create_review(client, &repo, args.number, &body).await?;
    eprintln!("✓ Submitted `{event}` review on PR #{}", args.number);
    Ok(())
}

async fn checks(ctx: &mut Context, args: NumberArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = pulls::checks(client, &repo, args.number).await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn batch(ctx: &mut Context, args: BatchArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = pulls::batch(client, &repo, &args.numbers).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

/// Best-effort current branch detection via `git symbolic-ref --short HEAD`.
fn current_branch() -> Result<String, CliError> {
    let out = Command::new("git").args(["symbolic-ref", "--short", "HEAD"]).output()?;
    if !out.status.success() {
        return Err(CliError::BadArgs(
            "could not detect current branch — pass --head explicitly".into(),
        ));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if s.is_empty() {
        return Err(CliError::BadArgs(
            "current branch is empty — pass --head explicitly".into(),
        ));
    }
    Ok(s)
}
