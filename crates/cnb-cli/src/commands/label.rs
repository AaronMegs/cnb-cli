//! `cnb label` — repository labels (M2 §8.3, 4 subcommands).

use clap::{Args, Subcommand};
use cnb_api::services::labels;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    pub command: LabelCmd,
}

#[derive(Debug, Subcommand)]
pub enum LabelCmd {
    /// List labels in a repository.
    List(ListArgs),
    /// Create a new label.
    Create(CreateArgs),
    /// Edit a label by name.
    Edit(EditArgs),
    /// Delete a label by name.
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
    pub repo: Option<String>,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    pub name: String,
    pub repo: Option<String>,
    /// Hex color without `#` (e.g. `ff0000`).
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    pub name: String,
    pub repo: Option<String>,
    #[arg(long)]
    pub new_name: Option<String>,
    #[arg(long)]
    pub color: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub name: String,
    pub repo: Option<String>,
    #[arg(long)]
    pub yes: bool,
}

pub async fn run(ctx: &mut Context, args: LabelArgs) -> Result<(), CliError> {
    match args.command {
        LabelCmd::List(a) => list(ctx, a).await,
        LabelCmd::Create(a) => create(ctx, a).await,
        LabelCmd::Edit(a) => edit(ctx, a).await,
        LabelCmd::Delete(a) => delete(ctx, a).await,
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
    let client = ctx.api()?;
    let items = labels::list(client, &repo).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let name = it.get("name").and_then(Value::as_str).unwrap_or("");
        let color = it.get("color").and_then(Value::as_str).unwrap_or("");
        let desc = it.get("description").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![name.to_owned(), color.to_owned(), desc.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["NAME", "COLOR", "DESCRIPTION"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}

async fn create(ctx: &mut Context, args: CreateArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let body = labels::CreateLabelBody {
        name: &args.name,
        color: args.color.as_deref(),
        description: args.description.as_deref(),
    };
    let client = ctx.api()?;
    let _ = labels::create(client, &repo, &body).await?;
    eprintln!("✓ Created label `{}` in {repo}", args.name);
    Ok(())
}

async fn edit(ctx: &mut Context, args: EditArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    if args.new_name.is_none() && args.color.is_none() && args.description.is_none() {
        return Err(CliError::BadArgs(
            "nothing to edit — pass at least one of --new-name/--color/--description".into(),
        ));
    }
    let body = labels::EditLabelBody {
        new_name: args.new_name.as_deref(),
        color: args.color.as_deref(),
        description: args.description.as_deref(),
    };
    let client = ctx.api()?;
    let _ = labels::edit(client, &repo, &args.name, &body).await?;
    eprintln!("✓ Edited label `{}` in {repo}", args.name);
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Delete label `{}` from `{repo}`? (y/N)", args.name), args.yes)?;
    let client = ctx.api()?;
    let _ = labels::delete(client, &repo, &args.name).await?;
    eprintln!("✓ Deleted label `{}` from {repo}", args.name);
    Ok(())
}
