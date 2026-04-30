//! `cnb registry` — artifact registries & packages (M4 §8.8, 10 subcommands).

use clap::{Args, Subcommand};
use cnb_api::services::registries;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct RegistryArgs {
    #[command(subcommand)]
    pub command: RegistryCmd,
}

#[derive(Debug, Subcommand)]
pub enum RegistryCmd {
    /// List artifact registries under a group/org slug.
    List(ListArgs),
    /// Delete an artifact registry (destructive).
    Delete(DeleteArgs),
    /// Set visibility on a registry.
    SetVisibility(SetVisibilityArgs),
    /// Subcommands for packages (npm/maven/docker/...).
    #[command(subcommand)]
    Package(PackageCmd),
    /// Subcommands for package tags.
    #[command(subcommand)]
    Tag(TagCmd),
}

#[derive(Debug, Subcommand)]
pub enum PackageCmd {
    /// List packages under a group/org slug.
    List(PackageListArgs),
    /// View a package's metadata.
    View(PackageRefArgs),
    /// Delete a package (destructive).
    Delete(PackageDeleteArgs),
}

#[derive(Debug, Subcommand)]
pub enum TagCmd {
    /// List tags for a package.
    List(PackageRefArgs),
    /// View a single tag's metadata.
    View(TagRefArgs),
    /// Delete a tag (destructive).
    Delete(TagDeleteArgs),
    /// Show the tag's SLSA provenance.
    Provenance(TagRefArgs),
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
    /// Group/org slug.
    pub slug: String,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub registry: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct SetVisibilityArgs {
    pub registry: String,
    #[arg(value_parser = ["public", "internal", "private"])]
    pub visibility: String,
}

#[derive(Debug, Args)]
pub struct PackageListArgs {
    pub slug: String,
    /// Optional package type filter (npm/maven/docker/...).
    #[arg(long, value_name = "TYPE")]
    pub kind: Option<String>,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct PackageRefArgs {
    /// Group/org slug containing the package.
    pub slug: String,
    /// Package type. One of: docker, helm, dockermodel, maven, npm, ohpm,
    /// pypi, nuget, composer, conan, cargo.
    #[arg(long = "type", value_name = "TYPE")]
    pub kind: String,
    #[arg(long)]
    pub name: String,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct PackageDeleteArgs {
    pub slug: String,
    #[arg(long = "type", value_name = "TYPE")]
    pub kind: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct TagRefArgs {
    pub slug: String,
    #[arg(long = "type", value_name = "TYPE")]
    pub kind: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub tag: String,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct TagDeleteArgs {
    pub slug: String,
    #[arg(long = "type", value_name = "TYPE")]
    pub kind: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub tag: String,
    #[arg(long)]
    pub yes: bool,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: RegistryArgs) -> Result<(), CliError> {
    match args.command {
        RegistryCmd::List(a) => list(ctx, a).await,
        RegistryCmd::Delete(a) => delete(ctx, a).await,
        RegistryCmd::SetVisibility(a) => set_visibility(ctx, a).await,
        RegistryCmd::Package(c) => match c {
            PackageCmd::List(a) => pkg_list(ctx, a).await,
            PackageCmd::View(a) => pkg_view(ctx, a).await,
            PackageCmd::Delete(a) => pkg_delete(ctx, a).await,
        },
        RegistryCmd::Tag(c) => match c {
            TagCmd::List(a) => tag_list(ctx, a).await,
            TagCmd::View(a) => tag_view(ctx, a).await,
            TagCmd::Delete(a) => tag_delete(ctx, a).await,
            TagCmd::Provenance(a) => tag_provenance(ctx, a).await,
        },
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

fn validate_kind(kind: &str) -> Result<(), CliError> {
    if registries::PACKAGE_TYPES.contains(&kind) {
        return Ok(());
    }
    Err(CliError::BadArgs(format!(
        "unknown package type `{kind}`; expected one of: {}",
        registries::PACKAGE_TYPES.join(", ")
    )))
}

fn visibility_to_level(s: &str) -> i64 {
    match s {
        "public" => 0,
        "internal" => 10,
        _ => 20,
    }
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let q = format!("page=1&page_size={}", args.limit.max(1));
    let client = ctx.api()?;
    let items = registries::list(client, &args.slug, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let name = it.get("name").and_then(Value::as_str).unwrap_or("");
        let path = it.get("path").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![path.to_owned(), name.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["PATH", "NAME"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn delete(ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    ctx.confirm(
        &format!("Delete registry `{}` (destructive)? (y/N)", args.registry),
        args.yes,
    )?;
    let client = ctx.api()?;
    let _ = registries::delete(client, &args.registry).await?;
    eprintln!("✓ Deleted registry {}", args.registry);
    Ok(())
}

async fn set_visibility(ctx: &mut Context, args: SetVisibilityArgs) -> Result<(), CliError> {
    let level = visibility_to_level(&args.visibility);
    let client = ctx.api()?;
    let _ = registries::set_visibility(client, &args.registry, level).await?;
    eprintln!("✓ {}: visibility set to {}", args.registry, args.visibility);
    Ok(())
}

async fn pkg_list(ctx: &mut Context, args: PackageListArgs) -> Result<(), CliError> {
    if let Some(k) = &args.kind {
        validate_kind(k)?;
    }
    let mut q = format!("page=1&page_size={}", args.limit.max(1));
    if let Some(k) = &args.kind {
        use std::fmt::Write;
        write!(&mut q, "&type={k}").expect("write to String");
    }
    let client = ctx.api()?;
    let items = registries::list_packages(client, &args.slug, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let kind = it.get("type").and_then(Value::as_str).unwrap_or("");
        let name = it.get("name").and_then(Value::as_str).unwrap_or("");
        let updated = it.get("updated_at").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![kind.to_owned(), name.to_owned(), updated.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["TYPE", "NAME", "UPDATED"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn pkg_view(ctx: &mut Context, args: PackageRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    let client = ctx.api()?;
    let v = registries::view_package(client, &args.slug, &args.kind, &args.name).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn pkg_delete(ctx: &mut Context, args: PackageDeleteArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    ctx.confirm(
        &format!(
            "Delete package `{}/{}` ({} type)? (y/N)",
            args.slug, args.name, args.kind
        ),
        args.yes,
    )?;
    let client = ctx.api()?;
    let _ = registries::delete_package(client, &args.slug, &args.kind, &args.name).await?;
    eprintln!("✓ Deleted package {}/{} ({})", args.slug, args.name, args.kind);
    Ok(())
}

async fn tag_list(ctx: &mut Context, args: PackageRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    let client = ctx.api()?;
    let items = registries::list_tags(client, &args.slug, &args.kind, &args.name).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let tag = it
            .get("name")
            .or_else(|| it.get("tag"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let updated = it.get("updated_at").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![tag.to_owned(), updated.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["TAG", "UPDATED"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn tag_view(ctx: &mut Context, args: TagRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    let client = ctx.api()?;
    let v = registries::view_tag(client, &args.slug, &args.kind, &args.name, &args.tag).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn tag_delete(ctx: &mut Context, args: TagDeleteArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    ctx.confirm(
        &format!(
            "Delete tag `{}` of package `{}` (type {})? (y/N)",
            args.tag, args.name, args.kind
        ),
        args.yes,
    )?;
    let client = ctx.api()?;
    let _ = registries::delete_tag(client, &args.slug, &args.kind, &args.name, &args.tag).await?;
    eprintln!("✓ Deleted tag {} of {}/{}", args.tag, args.slug, args.name);
    Ok(())
}

async fn tag_provenance(ctx: &mut Context, args: TagRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    let client = ctx.api()?;
    let v = registries::provenance(client, &args.slug, &args.kind, &args.name, &args.tag).await?;
    let mut stdout = std::io::stdout().lock();
    json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_kind_accepts_known_types() {
        assert!(validate_kind("npm").is_ok());
        assert!(validate_kind("docker").is_ok());
        assert!(validate_kind("cargo").is_ok());
    }

    #[test]
    fn validate_kind_rejects_unknown() {
        let err = validate_kind("bogus").unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
    }

    #[test]
    fn visibility_levels_match_repo() {
        assert_eq!(visibility_to_level("public"), 0);
        assert_eq!(visibility_to_level("internal"), 10);
        assert_eq!(visibility_to_level("private"), 20);
    }
}
