//! `cnb label` — repository labels (M2 §8.3, 4 subcommands).
//!
//! Backed entirely by the typed SDK (`cnb_sdk::repo_labels`) as of Phase 2
//! step 2.5. This is the **first command group ported in full** —
//! including write paths (create / edit / delete) — and it doubles as the
//! reference implementation for the rest of Phase 2's write-path ports.

use clap::{Args, Subcommand};
use cnb_sdk::models::{PatchLabelForm, PostLabelForm};
use cnb_sdk::repo_labels::ListLabelsQuery;
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

/// Reject label names containing `/`.
///
/// The SDK builds delete / patch URLs by interpolating the name into
/// `/{repo}/-/labels/{name}` and then handing the string to
/// `url::Url::join`, which treats embedded `/` as path separators rather
/// than percent-encoding them. A label named `evil/..` would be silently
/// routed to a different endpoint instead of erroring out.
///
/// We mirror the guard the cnb-api facade had so the migration does not
/// regress security posture. See `docs/sdk-issues.md` SDK-I10.
fn ensure_label_name_safe(name: &str) -> Result<(), CliError> {
    if name.is_empty() {
        return Err(CliError::BadArgs("label name must not be empty".into()));
    }
    if name.contains('/') {
        return Err(CliError::BadArgs(format!("label name must not contain `/`: {name:?}")));
    }
    Ok(())
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let q = ListLabelsQuery::new();
    let items = {
        let client = ctx.sdk()?;
        client.repo_labels().list_labels(repo, &q).await?
    };
    let v = serde_json::to_value(&items).expect("Label serialises infallibly");
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let arr = v.as_array().cloned().unwrap_or_default();
    let mut rows = Vec::with_capacity(arr.len());
    for it in &arr {
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
    ensure_label_name_safe(&args.name)?;
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    // SDK form types every field as `Option<String>` even though `name`
    // is logically required; we always wire a `Some(...)` here.
    let body = PostLabelForm {
        name: Some(args.name.clone()),
        color: args.color.clone(),
        description: args.description.clone(),
    };
    let client = ctx.sdk()?;
    let _ = client.repo_labels().post_label(repo.clone(), &body).await?;
    eprintln!("✓ Created label `{}` in {}", args.name, repo);
    Ok(())
}

async fn edit(ctx: &mut Context, args: EditArgs) -> Result<(), CliError> {
    ensure_label_name_safe(&args.name)?;
    if let Some(new) = &args.new_name {
        // The new name will become the new path segment after a follow-up
        // PATCH or GET, so apply the same guard.
        ensure_label_name_safe(new)?;
    }
    if args.new_name.is_none() && args.color.is_none() && args.description.is_none() {
        return Err(CliError::BadArgs(
            "nothing to edit — pass at least one of --new-name/--color/--description".into(),
        ));
    }
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let body = PatchLabelForm {
        new_name: args.new_name.clone(),
        color: args.color.clone(),
        description: args.description.clone(),
    };
    let client = ctx.sdk()?;
    let _ = client.repo_labels().patch_label(repo, args.name.clone(), &body).await?;
    eprintln!("✓ Edited label `{}`", args.name);
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    ensure_label_name_safe(&args.name)?;
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    ctx.confirm(&format!("Delete label `{}` from `{repo}`? (y/N)", args.name), args.yes)?;
    let client = ctx.sdk()?;
    let _ = client.repo_labels().delete_label(repo, args.name.clone()).await?;
    eprintln!("✓ Deleted label `{}`", args.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_label_name_safe_accepts_normal_names() {
        ensure_label_name_safe("bug").unwrap();
        ensure_label_name_safe("good first issue").unwrap();
        ensure_label_name_safe("Type: enhancement").unwrap();
        // Punctuation that doesn't break URL paths is fine.
        ensure_label_name_safe("a:b").unwrap();
        ensure_label_name_safe("v1.0").unwrap();
    }

    #[test]
    fn ensure_label_name_safe_rejects_slash() {
        let err = ensure_label_name_safe("evil/../leak").unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
    }

    #[test]
    fn ensure_label_name_safe_rejects_empty() {
        let err = ensure_label_name_safe("").unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
    }

    #[test]
    fn ensure_label_name_safe_rejects_lone_slash() {
        let err = ensure_label_name_safe("/").unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
    }
}
