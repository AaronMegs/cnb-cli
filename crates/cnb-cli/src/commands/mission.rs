//! `cnb mission` — task collections (M4 §8.9, 6 subcommands).

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_api::services::missions;
use cnb_tty::json_out;
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct MissionArgs {
    #[command(subcommand)]
    pub command: MissionCmd,
}

#[derive(Debug, Subcommand)]
pub enum MissionCmd {
    /// Delete a mission collection (destructive).
    Delete(DeleteArgs),
    /// List the views configured on a mission.
    ViewList(MissionRefArgs),
    /// Add or edit a view (PUT view-list).
    ViewEdit(ViewEditArgs),
    /// Reorder the view list.
    ViewSort(ViewSortArgs),
    /// Get the active view's configuration.
    ViewGet(MissionRefArgs),
    /// Set the active view's configuration.
    ViewSet(ViewSetArgs),
}

#[derive(Debug, Args)]
pub struct MissionRefArgs {
    pub mission: String,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub mission: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ViewEditArgs {
    pub mission: String,
    /// Path to a JSON file describing the view payload.
    #[arg(long, value_name = "PATH")]
    pub config_file: PathBuf,
}

#[derive(Debug, Args)]
pub struct ViewSortArgs {
    pub mission: String,
    /// Comma-separated view IDs in desired order.
    #[arg(long, value_delimiter = ',')]
    pub ids: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ViewSetArgs {
    pub mission: String,
    /// Path to a JSON config payload.
    #[arg(long, value_name = "PATH")]
    pub config_file: PathBuf,
}

pub async fn run(ctx: &mut Context, args: MissionArgs) -> Result<(), CliError> {
    match args.command {
        MissionCmd::Delete(a) => delete(ctx, a).await,
        MissionCmd::ViewList(a) => view_list(ctx, a).await,
        MissionCmd::ViewEdit(a) => view_edit(ctx, a).await,
        MissionCmd::ViewSort(a) => view_sort(ctx, a).await,
        MissionCmd::ViewGet(a) => view_get(ctx, a).await,
        MissionCmd::ViewSet(a) => view_set(ctx, a).await,
    }
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    ctx.confirm(
        &format!("Delete mission `{}` (destructive)? (y/N)", args.mission),
        args.yes,
    )?;
    let client = ctx.api()?;
    let _ = missions::delete(client, &args.mission).await?;
    eprintln!("✓ Deleted mission {}", args.mission);
    Ok(())
}

async fn view_list(ctx: &mut Context, args: MissionRefArgs) -> Result<(), CliError> {
    let client = ctx.api()?;
    let v = missions::view_list(client, &args.mission).await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

fn read_json_file(path: &std::path::Path) -> Result<Value, CliError> {
    let s = std::fs::read_to_string(path)?;
    serde_json::from_str(&s).map_err(|e| CliError::BadArgs(format!("{}: invalid JSON: {e}", path.display())))
}

async fn view_edit(ctx: &mut Context, args: ViewEditArgs) -> Result<(), CliError> {
    let body = read_json_file(&args.config_file)?;
    let client = ctx.api()?;
    let _ = missions::put_view_list(client, &args.mission, &body).await?;
    eprintln!("✓ Updated view-list for {}", args.mission);
    Ok(())
}

async fn view_sort(ctx: &mut Context, args: ViewSortArgs) -> Result<(), CliError> {
    if args.ids.is_empty() {
        return Err(CliError::BadArgs("--ids must contain at least one ID".into()));
    }
    let client = ctx.api()?;
    let _ = missions::sort_view_list(client, &args.mission, args.ids).await?;
    eprintln!("✓ Reordered views for {}", args.mission);
    Ok(())
}

async fn view_get(ctx: &mut Context, args: MissionRefArgs) -> Result<(), CliError> {
    let client = ctx.api()?;
    let v = missions::get_view(client, &args.mission, "").await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn view_set(ctx: &mut Context, args: ViewSetArgs) -> Result<(), CliError> {
    let body = read_json_file(&args.config_file)?;
    let client = ctx.api()?;
    let _ = missions::set_view(client, &args.mission, &body).await?;
    eprintln!("✓ Updated active view for {}", args.mission);
    Ok(())
}
