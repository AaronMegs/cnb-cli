//! `cnb build` — pipeline builds (M3 §8.6, 8 subcommands).

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand};
use cnb_api::services::builds;
use cnb_tty::{jq, json_out, table, template};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[command(subcommand)]
    pub command: BuildCmd,
}

#[derive(Debug, Subcommand)]
pub enum BuildCmd {
    /// Trigger a build run.
    Run(RunArgs),
    /// List recent build runs.
    List(ListArgs),
    /// Query the status of a build by SN.
    Status(StatusArgs),
    /// View a single stage's logs in a build.
    View(ViewStageArgs),
    /// Download (or stream) the runner log for a pipeline.
    Logs(LogsArgs),
    /// Cancel a running build.
    Cancel(SnArgs),
    /// Delete logs of a build (destructive).
    DeleteLogs(SnArgs),
    /// Sync crontab pipelines for a branch.
    CrontabSync(CrontabArgs),
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
pub struct RunArgs {
    pub repo: Option<String>,
    /// Branch to build (defaults to repo's default branch).
    #[arg(long)]
    pub branch: Option<String>,
    /// Tag to build (precedence over branch).
    #[arg(long)]
    pub tag: Option<String>,
    /// Specific commit SHA (precedence over tag).
    #[arg(long)]
    pub sha: Option<String>,
    /// Inline pipeline yaml (alternative to `.cnb.yml`).
    #[arg(long, value_name = "FILE")]
    pub config_file: Option<PathBuf>,
    /// Custom event name (must start with `api_trigger`); defaults to `api_trigger`.
    #[arg(long)]
    pub event: Option<String>,
    /// `KEY=VALUE` env vars (multiple `--env` allowed).
    #[arg(long, value_name = "KEY=VAL")]
    pub env: Vec<String>,
    /// Block until the build is fully scheduled before returning.
    #[arg(long)]
    pub sync: bool,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    pub repo: Option<String>,
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
pub struct StatusArgs {
    pub sn: String,
    pub repo: Option<String>,
    /// Poll until the build reaches a terminal state.
    #[arg(long)]
    pub watch: bool,
    /// Polling interval in seconds (default 3).
    #[arg(long, default_value_t = 3u64)]
    pub interval: u64,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct ViewStageArgs {
    pub sn: String,
    /// Pipeline id within the build.
    #[arg(long)]
    pub pipeline_id: String,
    /// Stage id within the pipeline.
    #[arg(long)]
    pub stage_id: String,
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    /// Pipeline id whose runner log we want.
    pub pipeline_id: String,
    pub repo: Option<String>,
    /// Output to a file instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SnArgs {
    pub sn: String,
    pub repo: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct CrontabArgs {
    pub branch: String,
    pub repo: Option<String>,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: BuildArgs) -> Result<(), CliError> {
    match args.command {
        BuildCmd::Run(a) => start(ctx, a).await,
        BuildCmd::List(a) => list(ctx, a).await,
        BuildCmd::Status(a) => status(ctx, a).await,
        BuildCmd::View(a) => view_stage(ctx, a).await,
        BuildCmd::Logs(a) => logs(ctx, a).await,
        BuildCmd::Cancel(a) => cancel(ctx, a).await,
        BuildCmd::DeleteLogs(a) => delete_logs(ctx, a).await,
        BuildCmd::CrontabSync(a) => crontab_sync(ctx, a).await,
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

async fn start(ctx: &mut Context, args: RunArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut env_map = serde_json::Map::new();
    for kv in &args.env {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| CliError::BadArgs(format!("--env must be KEY=VAL: {kv}")))?;
        env_map.insert(k.to_owned(), Value::String(v.to_owned()));
    }
    let config = match args.config_file {
        Some(p) => Some(std::fs::read_to_string(p)?),
        None => None,
    };
    let body = builds::StartBuildBody {
        branch: args.branch,
        tag: args.tag,
        sha: args.sha,
        config,
        event: args.event,
        env: env_map,
        sync: if args.sync { Some("true".into()) } else { None },
    };
    let client = ctx.api()?;
    let v = builds::start(client, &repo, &body).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let sn = v.get("sn").and_then(Value::as_str).unwrap_or("(unknown)");
    let url = v.get("buildLogUrl").and_then(Value::as_str).unwrap_or("");
    eprintln!("✓ Build triggered: sn={sn}");
    if !url.is_empty() {
        eprintln!("  Logs: {url}");
    }
    Ok(())
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    use std::fmt::Write;
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut q = format!("page={}&page_size={}", args.page, args.limit.max(1));
    if let Some(s) = &args.status {
        write!(&mut q, "&status={s}").expect("write to String");
    }
    if let Some(b) = &args.branch {
        write!(&mut q, "&sourceRef={b}").expect("write to String");
    }
    let client = ctx.api()?;
    let v = builds::list(client, &repo, &q).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let arr = v.get("data").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(arr.len());
    for it in &arr {
        let sn = it.get("sn").and_then(Value::as_str).unwrap_or("");
        let status = it.get("status").and_then(Value::as_str).unwrap_or("");
        let branch = it
            .get("sourceRef")
            .or_else(|| it.get("targetRef"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let created = it.get("createTime").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![
            sn.to_owned(),
            status.to_owned(),
            branch.to_owned(),
            created.to_owned(),
        ]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["SN", "STATUS", "BRANCH", "CREATED"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn status(ctx: &mut Context, args: StatusArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;

    if !args.watch {
        let client = ctx.api()?;
        let v = builds::status(client, &repo, &args.sn).await?;
        if render(ctx, &args.out, &v)? {
            return Ok(());
        }
        let s = v.get("status").and_then(Value::as_str).unwrap_or("?");
        println!("status: {s}");
        return Ok(());
    }

    // --watch: poll until terminal state, with spinner + ctrl-c handling.
    let interval = Duration::from_secs(args.interval.max(1));
    let pb = if ctx.io.stderr_is_tty {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template("{spinner} build {prefix}: {msg}")
                .expect("static template parses")
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
        );
        pb.set_prefix(args.sn.clone());
        pb.enable_steady_tick(Duration::from_millis(120));
        Some(pb)
    } else {
        None
    };

    let client = ctx.api()?.clone();
    let sn = args.sn.clone();
    let result: Result<Value, CliError> = tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => Err(CliError::Interrupted),
        r = async {
            loop {
                let v = builds::status(&client, &repo, &sn).await?;
                let s = v.get("status").and_then(Value::as_str).unwrap_or("").to_owned();
                if let Some(pb) = &pb {
                    pb.set_message(s.clone());
                } else {
                    println!("status: {s}");
                }
                if is_terminal_status(&s) {
                    return Ok::<Value, CliError>(v);
                }
                tokio::time::sleep(interval).await;
            }
        } => r,
    };

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
    let v = result?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let s = v.get("status").and_then(Value::as_str).unwrap_or("?");
    eprintln!("✓ Build {} finished with status `{s}`", args.sn);
    Ok(())
}

fn is_terminal_status(s: &str) -> bool {
    matches!(
        s.to_ascii_lowercase().as_str(),
        "success" | "failure" | "failed" | "cancelled" | "canceled" | "skipped" | "timeout" | "error"
    )
}

async fn view_stage(ctx: &mut Context, args: ViewStageArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let v = builds::stage(client, &repo, &args.sn, &args.pipeline_id, &args.stage_id).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn logs(ctx: &mut Context, args: LogsArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let body = builds::download_log(client, &repo, &args.pipeline_id).await?;
    if let Some(out) = args.output {
        std::fs::write(&out, body.as_bytes())?;
        eprintln!("✓ Wrote {} bytes to {}", body.len(), out.display());
    } else {
        print!("{body}");
    }
    Ok(())
}

async fn cancel(ctx: &mut Context, args: SnArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Cancel build `{}` in `{repo}`? (y/N)", args.sn), args.yes)?;
    let client = ctx.api()?;
    let _ = builds::cancel(client, &repo, &args.sn).await?;
    eprintln!("✓ Cancelled build {}", args.sn);
    Ok(())
}

async fn delete_logs(ctx: &mut Context, args: SnArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Delete logs for `{}` in `{repo}`? (y/N)", args.sn), args.yes)?;
    let client = ctx.api()?;
    let _ = builds::delete_logs(client, &repo, &args.sn).await?;
    eprintln!("✓ Deleted logs for {}", args.sn);
    Ok(())
}

async fn crontab_sync(ctx: &mut Context, args: CrontabArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.api()?;
    let _ = builds::crontab_sync(client, &repo, &args.branch).await?;
    eprintln!("✓ Synced crontab pipelines for branch `{}` in {repo}", args.branch);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_recognised() {
        assert!(is_terminal_status("success"));
        assert!(is_terminal_status("FAILED"));
        assert!(is_terminal_status("Cancelled"));
        assert!(!is_terminal_status("running"));
        assert!(!is_terminal_status("pending"));
        assert!(!is_terminal_status(""));
    }
}
