//! `cnb workspace` (alias `ws`) — cloud-native dev environments (M3 §8.7, 5 subcommands).

use clap::{Args, Subcommand};
use cnb_api::services::workspaces;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub command: WorkspaceCmd,
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCmd {
    /// List my workspaces.
    List(ListArgs),
    /// Start (or open existing) workspace for a repository.
    Start(StartArgs),
    /// View access URLs for a workspace by SN.
    View(ViewArgs),
    /// Stop a workspace.
    Stop(TargetArgs),
    /// Delete a workspace (destructive).
    Delete(DeleteArgs),
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
    #[arg(long)]
    pub branch: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[arg(long, default_value_t = 1u32)]
    pub page: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct StartArgs {
    pub repo: Option<String>,
    #[arg(long)]
    pub branch: Option<String>,
    /// Full git ref (precedence over `--branch`).
    #[arg(long, value_name = "REF")]
    pub r#ref: Option<String>,
    /// Suppress the auto-open of webide URL in your browser.
    #[arg(long)]
    pub no_open: bool,
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    #[arg(long)]
    pub sn: String,
    pub repo: Option<String>,
    /// Open the webide URL in the default browser.
    #[arg(long)]
    pub web: bool,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct TargetArgs {
    /// Workspace SN (alternative: `--pipeline-id`).
    #[arg(long)]
    pub sn: Option<String>,
    /// Pipeline id (server prefers this when both are present).
    #[arg(long)]
    pub pipeline_id: Option<String>,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[arg(long)]
    pub sn: Option<String>,
    #[arg(long)]
    pub pipeline_id: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: WorkspaceArgs) -> Result<(), CliError> {
    match args.command {
        WorkspaceCmd::List(a) => list(ctx, a).await,
        WorkspaceCmd::Start(a) => start(ctx, a).await,
        WorkspaceCmd::View(a) => view(ctx, a).await,
        WorkspaceCmd::Stop(a) => stop(ctx, a).await,
        WorkspaceCmd::Delete(a) => delete(ctx, a).await,
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
    use std::fmt::Write;
    let mut q = format!("page={}&page_size={}", args.page, args.limit.max(1));
    if let Some(s) = &args.status {
        write!(&mut q, "&status={s}").expect("write to String");
    }
    if let Some(b) = &args.branch {
        write!(&mut q, "&branch={b}").expect("write to String");
    }
    let client = ctx.api()?;
    let v = workspaces::list(client, &q).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let arr = v.get("list").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(arr.len());
    for it in &arr {
        let sn = it.get("sn").and_then(Value::as_str).unwrap_or("");
        let slug = it.get("slug").and_then(Value::as_str).unwrap_or("");
        let branch = it.get("branch").and_then(Value::as_str).unwrap_or("");
        let status = it.get("status").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![
            sn.to_owned(),
            slug.to_owned(),
            branch.to_owned(),
            status.to_owned(),
        ]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["SN", "REPO", "BRANCH", "STATUS"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn start(ctx: &mut Context, args: StartArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let body = workspaces::StartWorkspaceBody {
        branch: args.branch,
        r#ref: args.r#ref,
    };
    let client = ctx.api()?;
    let v = workspaces::start(client, &repo, &body).await?;
    let url = v.get("url").and_then(Value::as_str).unwrap_or("");
    let sn = v.get("sn").and_then(Value::as_str).unwrap_or("");

    if !sn.is_empty() {
        eprintln!("✓ Workspace pipeline triggered: sn={sn}");
    }
    if !url.is_empty() {
        if args.no_open {
            println!("{url}");
        } else {
            eprintln!("→ Opening: {url}");
            let _ = open::that(url);
        }
    } else if let Some(msg) = v.get("message").and_then(Value::as_str) {
        eprintln!("  {msg}");
    }
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = workspaces::view(client, &repo, &args.sn).await?;

    if args.web {
        if let Some(url) = v.get("webide").and_then(Value::as_str) {
            eprintln!("→ Opening: {url}");
            let _ = open::that(url);
            return Ok(());
        }
        return Err(CliError::Generic("workspace has no webide URL".into()));
    }
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    // Card-style: list each access channel.
    for key in ["webide", "remoteSsh", "ssh", "vscode", "cursor", "codebuddy", "jumpUrl"] {
        if let Some(s) = v.get(key).and_then(Value::as_str) {
            if !s.is_empty() {
                println!("  {key:14}: {s}");
            }
        }
    }
    Ok(())
}

fn pick_target(sn: Option<String>, pipeline_id: Option<String>) -> Result<workspaces::WorkspaceTargetBody, CliError> {
    if sn.is_none() && pipeline_id.is_none() {
        return Err(CliError::BadArgs("pass --sn or --pipeline-id".into()));
    }
    Ok(workspaces::WorkspaceTargetBody { pipeline_id, sn })
}

async fn stop(ctx: &mut Context, args: TargetArgs) -> Result<(), CliError> {
    let body = pick_target(args.sn.clone(), args.pipeline_id.clone())?;
    let client = ctx.api()?;
    let _ = workspaces::stop(client, &body).await?;
    eprintln!(
        "✓ Stopped workspace ({})",
        body.sn.as_deref().or(body.pipeline_id.as_deref()).unwrap_or("")
    );
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    let body = pick_target(args.sn.clone(), args.pipeline_id.clone())?;
    let id = body.sn.as_deref().or(body.pipeline_id.as_deref()).unwrap_or("");
    ctx.confirm(&format!("Delete workspace `{id}`? (y/N)"), args.yes)?;
    let client = ctx.api()?;
    let _ = workspaces::delete(client, &body).await?;
    eprintln!("✓ Deleted workspace ({id})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_target_requires_sn_or_pipeline() {
        assert!(matches!(pick_target(None, None), Err(CliError::BadArgs(_))));
        assert!(pick_target(Some("a".into()), None).is_ok());
        assert!(pick_target(None, Some("p".into())).is_ok());
    }
}
