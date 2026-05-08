//! `cnb workspace` (alias `ws`) — cloud-native dev environments
//! (M3 §8.7, 5 subcommands).
//!
//! Phase 2, step 2.8 of the cnb-api → typed SDK migration. All 5
//! subcommands now route through `cnb_sdk::workspace::WorkspaceClient`.

use clap::{Args, Subcommand};
use cnb_sdk::models::{StartWorkspaceReq, WorkspaceDeleteReq, WorkspaceStopReq};
use cnb_sdk::workspace::ListWorkspacesQuery;
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

fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, CliError> {
    serde_json::to_value(t).map_err(|e| CliError::Generic(format!("serialise response: {e}")))
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let mut q = ListWorkspacesQuery::new()
        .page(i64::from(args.page))
        .page_size(i64::from(args.limit.max(1)));
    if let Some(s) = args.status {
        q = q.status(s);
    }
    if let Some(b) = args.branch {
        q = q.branch(b);
    }
    let result = {
        let client = ctx.sdk()?;
        client.workspace().list_workspaces(&q).await?
    };
    let v = to_value(&result)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let items = result.list.unwrap_or_default();
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let sn = it.sn.as_deref().unwrap_or("");
        let slug = it.slug.as_deref().unwrap_or("");
        let branch = it.branch.as_deref().unwrap_or("");
        let status = it.status.as_deref().unwrap_or("");
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
    // `StartWorkspaceReq.ref_` in the SDK serialises as `"ref"` via
    // serde rename, matching both the prior cnb-api facade and the
    // wiremock expectations.
    let body = StartWorkspaceReq {
        branch: args.branch,
        ref_: args.r#ref,
    };
    let result = {
        let client = ctx.sdk()?;
        client.workspace().start_workspace(repo.clone(), &body).await?
    };
    let url = result.url.as_deref().unwrap_or("");
    let sn = result.sn.as_deref().unwrap_or("");

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
    } else if let Some(msg) = result.message.as_deref() {
        eprintln!("  {msg}");
    }
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let result = {
        let client = ctx.sdk()?;
        client
            .workspace()
            .get_workspace_detail(repo.clone(), args.sn.clone())
            .await?
    };

    if args.web {
        if let Some(url) = result.webide.as_deref() {
            if !url.is_empty() {
                eprintln!("→ Opening: {url}");
                let _ = open::that(url);
                return Ok(());
            }
        }
        return Err(CliError::Generic("workspace has no webide URL".into()));
    }
    let v = to_value(&result)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    // Card-style: list each access channel. Keys below are the serde-
    // renamed wire names (matching the prior cnb-api facade output),
    // resolved through `Value::get` on the typed DTO serialisation.
    for key in ["webide", "remoteSsh", "ssh", "vscode", "cursor", "codebuddy", "jumpUrl"] {
        if let Some(s) = v.get(key).and_then(Value::as_str) {
            if !s.is_empty() {
                println!("  {key:14}: {s}");
            }
        }
    }
    Ok(())
}

/// Common body preparation for `stop` + `delete`: at least one of
/// `--sn` or `--pipeline-id` must be set. The SDK provides separate
/// typed bodies (`WorkspaceStopReq` vs `WorkspaceDeleteReq`) that
/// happen to share the same two fields — we build whichever one the
/// caller requests through the narrow wrapper below to avoid stringy
/// duplication.
fn ensure_target(sn: Option<&String>, pipeline_id: Option<&String>) -> Result<(), CliError> {
    if sn.is_none() && pipeline_id.is_none() {
        return Err(CliError::BadArgs("pass --sn or --pipeline-id".into()));
    }
    Ok(())
}

async fn stop(ctx: &mut Context, args: TargetArgs) -> Result<(), CliError> {
    ensure_target(args.sn.as_ref(), args.pipeline_id.as_ref())?;
    let body = WorkspaceStopReq {
        pipeline_id: args.pipeline_id.clone(),
        sn: args.sn.clone(),
    };
    let client = ctx.sdk()?;
    let _ = client.workspace().workspace_stop(&body).await?;
    eprintln!(
        "✓ Stopped workspace ({})",
        body.sn.as_deref().or(body.pipeline_id.as_deref()).unwrap_or("")
    );
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    ensure_target(args.sn.as_ref(), args.pipeline_id.as_ref())?;
    let id = args.sn.as_deref().or(args.pipeline_id.as_deref()).unwrap_or("");
    ctx.confirm(&format!("Delete workspace `{id}`? (y/N)"), args.yes)?;
    let body = WorkspaceDeleteReq {
        pipeline_id: args.pipeline_id.clone(),
        sn: args.sn.clone(),
    };
    let client = ctx.sdk()?;
    let _ = client.workspace().delete_workspace(&body).await?;
    eprintln!("✓ Deleted workspace ({id})");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_target_requires_sn_or_pipeline() {
        assert!(matches!(ensure_target(None, None), Err(CliError::BadArgs(_))));
        let a = "a".to_owned();
        let p = "p".to_owned();
        assert!(ensure_target(Some(&a), None).is_ok());
        assert!(ensure_target(None, Some(&p)).is_ok());
    }
}
