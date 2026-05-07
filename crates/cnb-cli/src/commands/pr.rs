//! `cnb pr` — pull requests (M2 §8.4, 12 subcommands). `cnb mr` is a CLI alias.
//!
//! `list` and `view` are backed by the typed SDK (`cnb-sdk`) as of Phase 2
//! step 2.4; the remaining subcommands still route through the hand-written
//! `cnb-api` facade and will be ported module-by-module.

use std::process::Command;

use clap::{Args, Subcommand};
use cnb_api::services::pulls;
use cnb_sdk::pulls::ListPullsQuery;
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
    let page = i64::from(args.page);
    let page_size = i64::from(args.limit.max(1));
    // Upstream treats `all` specially (omit the query param); `open`,
    // `closed` and `merged` pass through verbatim.
    let mut q = ListPullsQuery::new().page(page).page_size(page_size);
    if args.state != "all" {
        q = q.state(args.state.clone());
    }
    let items = {
        let client = ctx.sdk()?;
        client.pulls().list_pulls(repo, &q).await?
    };
    let v = serde_json::to_value(&items).expect("PullRequest serialises infallibly");
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(arr.len());
    for it in &arr {
        let n_display = format_pr_number(it.get("number"));
        let title = it.get("title").and_then(Value::as_str).unwrap_or("");
        let head = read_branch(it.get("head"), it.get("source_branch"));
        let base = read_branch(it.get("base"), it.get("target_branch"));
        let created = it.get("created_at").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![
            n_display,
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
    // SDK pins the PR number argument to `String` (unlike issues which use
    // `i64` — see SDK-I07 / SDK-I08 in docs/sdk-issues.md). Our CLI accepts
    // `u64` for ergonomics; the string formatting below performs the
    // conversion at the boundary.
    let number_str = args.number.to_string();
    let dto = {
        let client = ctx.sdk()?;
        client.pulls().get_pull(repo, number_str).await?
    };
    let v = serde_json::to_value(&dto).expect("Pull serialises infallibly");
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let title = v.get("title").and_then(Value::as_str).unwrap_or("");
    let state = v.get("state").and_then(Value::as_str).unwrap_or("?");
    let head = read_branch(v.get("head"), v.get("source_branch"));
    let base = read_branch(v.get("base"), v.get("target_branch"));
    let body = v.get("body").and_then(Value::as_str).unwrap_or("");
    println!("#{} {title}  [{state}]", args.number);
    println!("  {head} → {base}");
    if !body.is_empty() {
        println!();
        println!("{body}");
    }
    Ok(())
}

/// Display formatter for a PR's `number` field.
///
/// SDK DTOs (`Pull`, `PullRequest`) type `number` as `Option<String>`; the
/// legacy cnb-api facade emitted it as an integer. Accept both on the
/// display path so real-world responses keep rendering.
fn format_pr_number(raw: Option<&Value>) -> String {
    match raw {
        Some(Value::String(s)) => format!("#{s}"),
        Some(Value::Number(n)) => format!("#{n}"),
        _ => String::from("#?"),
    }
}

/// Extract a branch name from a PR's `head` / `base` field.
///
/// The SDK types these as `Option<serde_json::Value>` because the upstream
/// OpenAPI spec does not pin their schema. Real servers return one of:
///   * a flat `{head: {branch: "feat/x"}}` or `{head: {ref: "feat/x"}}`
///   * a slightly richer object with `{name, commit_id, …}` nested under
///   * or, on legacy deployments, a sibling top-level string field
///     (`source_branch` / `target_branch`) the SDK wholly drops.
///
/// We try each shape in order of specificity so the card always renders a
/// branch name when one is present, regardless of which encoding the
/// server chose.
fn read_branch(primary: Option<&Value>, fallback: Option<&Value>) -> String {
    if let Some(obj) = primary {
        for key in ["branch", "ref", "name"] {
            if let Some(s) = obj.get(key).and_then(Value::as_str) {
                if !s.is_empty() {
                    return s.to_owned();
                }
            }
        }
    }
    fallback.and_then(Value::as_str).unwrap_or("").to_owned()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_pr_number_accepts_string() {
        assert_eq!(format_pr_number(Some(&json!("7"))), "#7");
        assert_eq!(format_pr_number(Some(&json!("42"))), "#42");
    }

    #[test]
    fn format_pr_number_accepts_integer() {
        // Legacy encoding — some deployments still emit this.
        assert_eq!(format_pr_number(Some(&json!(7))), "#7");
        assert_eq!(format_pr_number(Some(&json!(0))), "#0");
    }

    #[test]
    fn format_pr_number_missing_or_null_falls_back() {
        assert_eq!(format_pr_number(None), "#?");
        assert_eq!(format_pr_number(Some(&json!(null))), "#?");
        assert_eq!(format_pr_number(Some(&json!([]))), "#?");
    }

    #[test]
    fn read_branch_prefers_primary_branch_subfield() {
        let primary = json!({"branch": "feat/shiny", "commit_id": "abc"});
        assert_eq!(read_branch(Some(&primary), None), "feat/shiny");
    }

    #[test]
    fn read_branch_tries_alternate_keys_in_order() {
        let r_only = json!({"ref": "main"});
        assert_eq!(read_branch(Some(&r_only), None), "main");
        let name_only = json!({"name": "release"});
        assert_eq!(read_branch(Some(&name_only), None), "release");
    }

    #[test]
    fn read_branch_falls_back_to_legacy_top_level_string() {
        // Primary is absent / empty object → fall back to the sibling
        // top-level string field (source_branch / target_branch on older
        // servers).
        assert_eq!(read_branch(None, Some(&json!("legacy-branch"))), "legacy-branch");
        let empty_obj = json!({});
        assert_eq!(
            read_branch(Some(&empty_obj), Some(&json!("legacy-branch"))),
            "legacy-branch"
        );
    }

    #[test]
    fn read_branch_returns_empty_when_nothing_present() {
        assert_eq!(read_branch(None, None), "");
        assert_eq!(read_branch(Some(&json!({})), None), "");
    }
}
