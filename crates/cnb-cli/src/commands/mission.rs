//! `cnb mission` — task collections (M4 §8.9, 6 subcommands).
//!
//! Phase 2, step 2.9 of the cnb-api → typed SDK migration. All 6
//! subcommands route through `cnb_sdk::missions::MissionsClient`. The
//! `view-edit` and `view-set` commands still accept a JSON config file
//! as input, but we now parse it into the typed body
//! (`MissionView` / `MissionViewConfig`) before handing it to the SDK
//! — this surfaces schema errors early instead of letting the server
//! round-trip a malformed payload.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_sdk::missions::GetMissionViewConfigQuery;
use cnb_sdk::models::{MissionPostViewReq, MissionView, MissionViewConfig};
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
    /// Path to a JSON file describing the view payload
    /// (shape: `MissionView` — `{id, name, type}`).
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
    /// Path to a JSON config payload (shape: `MissionViewConfig` —
    /// `{id, type, fields[], group, selectors[], sorts[]}`).
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

fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, CliError> {
    serde_json::to_value(t).map_err(|e| CliError::Generic(format!("serialise response: {e}")))
}

/// Parse a JSON file into any typed body. Produces a `BadArgs` with a
/// clear message when the shape is wrong — preferable to letting the
/// server reject it five layers down.
fn read_typed_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, CliError> {
    let s = std::fs::read_to_string(path)?;
    serde_json::from_str::<T>(&s)
        .map_err(|e| CliError::BadArgs(format!("{}: invalid JSON for typed body: {e}", path.display())))
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    ctx.confirm(
        &format!("Delete mission `{}` (destructive)? (y/N)", args.mission),
        args.yes,
    )?;
    let client = ctx.sdk()?;
    let _ = client.missions().delete_mission(args.mission.clone()).await?;
    eprintln!("✓ Deleted mission {}", args.mission);
    Ok(())
}

async fn view_list(ctx: &mut Context, args: MissionRefArgs) -> Result<(), CliError> {
    let views = {
        let client = ctx.sdk()?;
        client.missions().get_mission_view_list(args.mission.clone()).await?
    };
    let v = to_value(&views)?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn view_edit(ctx: &mut Context, args: ViewEditArgs) -> Result<(), CliError> {
    let body: MissionView = read_typed_json(&args.config_file)?;
    let client = ctx.sdk()?;
    let _ = client
        .missions()
        .put_mission_view_list(args.mission.clone(), &body)
        .await?;
    eprintln!("✓ Updated view-list for {}", args.mission);
    Ok(())
}

async fn view_sort(ctx: &mut Context, args: ViewSortArgs) -> Result<(), CliError> {
    if args.ids.is_empty() {
        return Err(CliError::BadArgs("--ids must contain at least one ID".into()));
    }
    let body = MissionPostViewReq {
        ids: Some(args.ids.clone()),
    };
    let client = ctx.sdk()?;
    let _ = client
        .missions()
        .post_mission_view_list(args.mission.clone(), &body)
        .await?;
    eprintln!("✓ Reordered views for {}", args.mission);
    Ok(())
}

async fn view_get(ctx: &mut Context, args: MissionRefArgs) -> Result<(), CliError> {
    let query = GetMissionViewConfigQuery::new();
    let cfg = {
        let client = ctx.sdk()?;
        client
            .missions()
            .get_mission_view_config(args.mission.clone(), &query)
            .await?
    };
    let v = to_value(&cfg)?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn view_set(ctx: &mut Context, args: ViewSetArgs) -> Result<(), CliError> {
    let body: MissionViewConfig = read_typed_json(&args.config_file)?;
    let client = ctx.sdk()?;
    let _ = client
        .missions()
        .post_mission_view_config(args.mission.clone(), &body)
        .await?;
    eprintln!("✓ Updated active view for {}", args.mission);
    Ok(())
}
