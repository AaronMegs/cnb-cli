//! `cnb build` — pipeline builds (M3 §8.6, 8 subcommands).
//!
//! Phase 2, step 2.8 of the cnb-api → typed SDK migration. All 8
//! subcommands now route through `cnb_sdk::build::BuildClient`. The
//! runner-log download is the one exception: the SDK models the
//! endpoint (`build_runner_download_log`) as JSON-returning, but the
//! real response is plain text, so we fall back to a side-car
//! `reqwest::Client` — same pattern as `cnb release download`, tracked
//! under SDK-I14.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand};
use cnb_sdk::build::GetBuildLogsQuery;
use cnb_sdk::models::StartBuildReq;
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

fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, CliError> {
    serde_json::to_value(t).map_err(|e| CliError::Generic(format!("serialise response: {e}")))
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
    // Note: SDK's `env` is typed as `Option<Value>` rather than a map.
    // We feed an object when the user passed at least one `--env`,
    // otherwise leave it `None` so the field is omitted from the
    // request body (matches the prior cnb-api facade shape).
    let env = if env_map.is_empty() {
        None
    } else {
        Some(Value::Object(env_map))
    };
    let body = StartBuildReq {
        branch: args.branch,
        tag: args.tag,
        sha: args.sha,
        config,
        event: args.event,
        env,
        sync: if args.sync { Some("true".into()) } else { None },
    };
    let result = {
        let client = ctx.sdk()?;
        client.build().start_build(repo.clone(), &body).await?
    };
    let v = to_value(&result)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let sn = result.sn.as_deref().unwrap_or("(unknown)");
    let url = result.build_log_url.as_deref().unwrap_or("");
    eprintln!("✓ Build triggered: sn={sn}");
    if !url.is_empty() {
        eprintln!("  Logs: {url}");
    }
    Ok(())
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut q = GetBuildLogsQuery::new()
        .page(i64::from(args.page))
        .page_size(i64::from(args.limit.max(1)));
    if let Some(s) = args.status {
        q = q.status(s);
    }
    if let Some(b) = args.branch {
        q = q.source_ref(b);
    }
    let result = {
        let client = ctx.sdk()?;
        client.build().get_build_logs(repo.clone(), &q).await?
    };
    let v = to_value(&result)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let items = result.data.unwrap_or_default();
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let sn = it.sn.as_deref().unwrap_or("");
        let status = it.status.as_deref().unwrap_or("");
        let branch = it.source_ref.as_deref().or(it.target_ref.as_deref()).unwrap_or("");
        let created = it.create_time.as_deref().unwrap_or("");
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
        let result = {
            let client = ctx.sdk()?;
            client.build().get_build_status(repo.clone(), args.sn.clone()).await?
        };
        let v = to_value(&result)?;
        if render(ctx, &args.out, &v)? {
            return Ok(());
        }
        let s = result.status.as_deref().unwrap_or("?");
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

    // ApiClient is Clone (Arc-backed), so we can move it into the poll
    // future without borrowing ctx across .await points.
    let client = ctx.sdk()?.clone();
    let sn = args.sn.clone();
    let repo2 = repo.clone();
    let result: Result<cnb_sdk::models::BuildStatusResult, CliError> = tokio::select! {
        biased;
        _ = tokio::signal::ctrl_c() => Err(CliError::Interrupted),
        r = async {
            loop {
                let v = client.build().get_build_status(repo2.clone(), sn.clone()).await?;
                let s = v.status.clone().unwrap_or_default();
                if let Some(pb) = &pb {
                    pb.set_message(s.clone());
                } else {
                    println!("status: {s}");
                }
                if is_terminal_status(&s) {
                    return Ok::<cnb_sdk::models::BuildStatusResult, CliError>(v);
                }
                tokio::time::sleep(interval).await;
            }
        } => r,
    };

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
    let result = result?;
    let v = to_value(&result)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let s = result.status.as_deref().unwrap_or("?");
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
    let result = {
        let client = ctx.sdk()?;
        client
            .build()
            .get_build_stage(
                repo.clone(),
                args.sn.clone(),
                args.pipeline_id.clone(),
                args.stage_id.clone(),
            )
            .await?
    };
    let v = to_value(&result)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

/// Download the runner log. The SDK's typed `build_runner_download_log`
/// returns `serde_json::Value`, but the real endpoint replies with plain
/// text, so we use a side-car `reqwest::Client` (same pattern as
/// `cnb release download`). See SDK-I14.
async fn logs(ctx: &mut Context, args: LogsArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    if args.pipeline_id.contains('/') {
        return Err(CliError::BadArgs(format!(
            "pipeline id must not contain `/`: {:?}",
            args.pipeline_id
        )));
    }
    let body = download_log_bytes(&repo, &args.pipeline_id).await?;
    if let Some(out) = args.output {
        std::fs::write(&out, body.as_bytes())?;
        eprintln!("✓ Wrote {} bytes to {}", body.len(), out.display());
    } else {
        print!("{body}");
    }
    Ok(())
}

async fn download_log_bytes(repo: &str, pipeline_id: &str) -> Result<String, CliError> {
    let mut base = std::env::var("CNB_API_BASE")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| cnb_sdk::DEFAULT_BASE_URL.to_owned());
    if !base.ends_with('/') {
        base.push('/');
    }
    let base_url =
        url::Url::parse(&base).map_err(|e| CliError::Generic(format!("invalid SDK base url `{base}`: {e}")))?;
    let full = base_url
        .join(&format!("{repo}/-/build/runner/download/log/{pipeline_id}"))
        .map_err(|e| CliError::Generic(format!("could not build log URL: {e}")))?;
    let token = std::env::var("CNB_TOKEN").unwrap_or_default();
    let mut req = reqwest::Client::new().get(full);
    if !token.is_empty() {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| CliError::Generic(format!("log download GET failed: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(CliError::Generic(format!(
            "log download GET {}: {text}",
            status.as_u16()
        )));
    }
    resp.text()
        .await
        .map_err(|e| CliError::Generic(format!("log body: {e}")))
}

async fn cancel(ctx: &mut Context, args: SnArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Cancel build `{}` in `{repo}`? (y/N)", args.sn), args.yes)?;
    let client = ctx.sdk()?;
    let _ = client.build().stop_build(repo.clone(), args.sn.clone()).await?;
    eprintln!("✓ Cancelled build {}", args.sn);
    Ok(())
}

async fn delete_logs(ctx: &mut Context, args: SnArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Delete logs for `{}` in `{repo}`? (y/N)", args.sn), args.yes)?;
    let client = ctx.sdk()?;
    let _ = client.build().build_logs_delete(repo.clone(), args.sn.clone()).await?;
    eprintln!("✓ Deleted logs for {}", args.sn);
    Ok(())
}

async fn crontab_sync(ctx: &mut Context, args: CrontabArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let client = ctx.sdk()?;
    let _ = client
        .build()
        .build_crontab_sync(repo.clone(), args.branch.clone())
        .await?;
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
