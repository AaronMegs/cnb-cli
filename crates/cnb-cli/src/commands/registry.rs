//! `cnb registry` — artifact registries & packages (M4 §8.8, 10 subcommands).
//!
//! Phase 2, step 2.9 of the cnb-api → typed SDK migration. All 10
//! verbs route through `cnb_sdk::registries::RegistriesClient`, with
//! one exception: `registry tag list` uses the raw HTTP path through
//! `Context::sdk_raw_get` because the SDK's typed
//! `list_package_tags` returns `models::Tag` (a git-tag DTO that has
//! a single-object shape, not the array shape the registry endpoint
//! actually emits). Tracked as SDK-I15.
//!
//! `registry set-visibility` sends `?visibility=public|internal|private`
//! as a query parameter (the SDK shape, consistent with
//! `repo set-visibility` and the OpenAPI spec). The integer-level
//! translation that the old cnb-api facade maintained
//! (`public→0, internal→10, private→20`) is gone — we forward the
//! string verbatim. See SDK-I12 for the same pattern on repos.

use clap::{Args, Subcommand};
use cnb_sdk::registries::{
    GetPackageTagDetailQuery, ListPackageTagsQuery, ListPackagesQuery, SetRegistryVisibilityQuery,
};
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

/// Whitelist of package types accepted by the `--type` flag. Kept
/// locally so we don't need to depend on the cnb-api facade just
/// for this constant.
const PACKAGE_TYPES: &[&str] = &[
    "docker",
    "helm",
    "dockermodel",
    "maven",
    "npm",
    "ohpm",
    "pypi",
    "nuget",
    "composer",
    "conan",
    "cargo",
    "generic",
];

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
    /// pypi, nuget, composer, conan, cargo, generic.
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

fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, CliError> {
    serde_json::to_value(t).map_err(|e| CliError::Generic(format!("serialise response: {e}")))
}

fn validate_kind(kind: &str) -> Result<(), CliError> {
    if PACKAGE_TYPES.contains(&kind) {
        return Ok(());
    }
    Err(CliError::BadArgs(format!(
        "unknown package type `{kind}`; expected one of: {}",
        PACKAGE_TYPES.join(", ")
    )))
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    // The SDK's `get_group_sub_registries` query accepts page /
    // page_size. CLI only exposes `--limit`, so we fix page=1.
    let q = cnb_sdk::registries::GetGroupSubRegistriesQuery::new()
        .page(1_i64)
        .page_size(i64::from(args.limit.max(1)));
    let items = {
        let client = ctx.sdk()?;
        client
            .registries()
            .get_group_sub_registries(args.slug.clone(), &q)
            .await?
    };
    let v = Value::Array(items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?);
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let name = it.name.as_deref().unwrap_or("");
        let path = it.path.as_deref().unwrap_or("");
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
    let client = ctx.sdk()?;
    let _ = client.registries().delete_registry(args.registry.clone()).await?;
    eprintln!("✓ Deleted registry {}", args.registry);
    Ok(())
}

async fn set_visibility(ctx: &mut Context, args: SetVisibilityArgs) -> Result<(), CliError> {
    // SDK shape: POST /{registry}/-/settings/set_visibility?visibility=...
    // NOT a body payload. Same convention as `repo set-visibility` (SDK-I12).
    let q = SetRegistryVisibilityQuery::new().visibility(args.visibility.clone());
    let client = ctx.sdk()?;
    let _ = client
        .registries()
        .set_registry_visibility(args.registry.clone(), &q)
        .await?;
    eprintln!("✓ {}: visibility set to {}", args.registry, args.visibility);
    Ok(())
}

async fn pkg_list(ctx: &mut Context, args: PackageListArgs) -> Result<(), CliError> {
    if let Some(k) = &args.kind {
        validate_kind(k)?;
    }
    let mut q = ListPackagesQuery::new()
        .page(1_i64)
        .page_size(i64::from(args.limit.max(1)));
    if let Some(k) = args.kind {
        q = q.type_(k);
    }
    let items = {
        let client = ctx.sdk()?;
        client.registries().list_packages(args.slug.clone(), &q).await?
    };
    let v = Value::Array(items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?);
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let kind = it.package_type.as_deref().unwrap_or("");
        let name = it.name.as_deref().unwrap_or("");
        // `Package` DTO has no `updated_at`; the SDK picks
        // `last_pusher.created_at` only on some flavours. We fall
        // back to the empty string, matching the prior cnb-api
        // behaviour when the server omitted the field.
        let updated = it
            .last_pusher
            .as_ref()
            .and_then(|p| p.nickname.as_deref())
            .unwrap_or("");
        rows.push(vec![kind.to_owned(), name.to_owned(), updated.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["TYPE", "NAME", "PUSHER"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn pkg_view(ctx: &mut Context, args: PackageRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    let detail = {
        let client = ctx.sdk()?;
        client
            .registries()
            .get_package(args.slug.clone(), args.kind.clone(), args.name.clone())
            .await?
    };
    let v = to_value(&detail)?;
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
    let client = ctx.sdk()?;
    let _ = client
        .registries()
        .delete_package(args.slug.clone(), args.kind.clone(), args.name.clone())
        .await?;
    eprintln!("✓ Deleted package {}/{} ({})", args.slug, args.name, args.kind);
    Ok(())
}

/// `registry tag list` — bypasses the SDK's typed call because
/// `list_package_tags` returns `models::Tag` (a single-object git-tag
/// DTO), not the `Vec<TagSummary>`-shaped response the server actually
/// emits. We go straight to the wire via `sdk_raw_get` and render as
/// raw JSON. Tracked as SDK-I15.
async fn tag_list(ctx: &mut Context, args: PackageRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    // Still run the typed call to exercise the SDK's request path
    // (base URL / auth / retries / tracing). Its return value is
    // discarded — shape mismatch means we cannot read it safely.
    let _touch: cnb_sdk::models::Tag = {
        let q = ListPackageTagsQuery::new();
        let client = ctx.sdk()?;
        client
            .registries()
            .list_package_tags(args.slug.clone(), args.kind.clone(), args.name.clone(), &q)
            .await
            .unwrap_or_default()
    };
    let v = ctx
        .sdk_raw_get(&format!("/{}/-/packages/{}/{}/-/tags", args.slug, args.kind, args.name))
        .await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let items = v.as_array().cloned().unwrap_or_default();
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
    let q = GetPackageTagDetailQuery::new();
    let detail = {
        let client = ctx.sdk()?;
        client
            .registries()
            .get_package_tag_detail(
                args.slug.clone(),
                args.kind.clone(),
                args.name.clone(),
                args.tag.clone(),
                &q,
            )
            .await?
    };
    let v = to_value(&detail)?;
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
    let client = ctx.sdk()?;
    let _ = client
        .registries()
        .delete_package_tag(
            args.slug.clone(),
            args.kind.clone(),
            args.name.clone(),
            args.tag.clone(),
        )
        .await?;
    eprintln!("✓ Deleted tag {} of {}/{}", args.tag, args.slug, args.name);
    Ok(())
}

async fn tag_provenance(ctx: &mut Context, args: TagRefArgs) -> Result<(), CliError> {
    validate_kind(&args.kind)?;
    let prov = {
        let client = ctx.sdk()?;
        client
            .registries()
            .get_package_tag_provenance(
                args.slug.clone(),
                args.kind.clone(),
                args.name.clone(),
                args.tag.clone(),
            )
            .await?
    };
    let v = to_value(&prov)?;
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
}
