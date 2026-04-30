//! `cnb org` — organizations & members (M4 §8.10, 7 subcommands incl. follower).

use clap::{Args, Subcommand};
use cnb_api::services::orgs;
use cnb_tty::{jq, json_out, table, template};
use serde_json::Value;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct OrgArgs {
    #[command(subcommand)]
    pub command: OrgCmd,
}

#[derive(Debug, Subcommand)]
pub enum OrgCmd {
    /// List groups/orgs the current user belongs to.
    List(ListArgs),
    /// View an organization.
    View(ViewArgs),
    /// Member management.
    #[command(subcommand)]
    Member(MemberCmd),
    /// User followers.
    Follower(UserListArgs),
    /// Users a person is following.
    Following(UserListArgs),
}

#[derive(Debug, Subcommand)]
pub enum MemberCmd {
    /// List members of an organization.
    List(MemberListArgs),
    /// Add a member with a role.
    Add(MemberAddArgs),
    /// Remove a member (destructive).
    Remove(MemberRemoveArgs),
    /// Edit a member's role.
    Edit(MemberEditArgs),
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
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct ViewArgs {
    pub group: String,
    /// Open the org's web page in a browser.
    #[arg(long)]
    pub web: bool,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct MemberListArgs {
    pub group: String,
    /// Filter by role (read / write / admin).
    #[arg(long)]
    pub role: Option<String>,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

#[derive(Debug, Args)]
pub struct MemberAddArgs {
    pub group: String,
    pub username: String,
    #[arg(long, value_parser = ["read", "write", "admin"], default_value = "read")]
    pub role: String,
}

#[derive(Debug, Args)]
pub struct MemberRemoveArgs {
    pub group: String,
    pub username: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct MemberEditArgs {
    pub group: String,
    pub username: String,
    #[arg(long, value_parser = ["read", "write", "admin"])]
    pub role: String,
}

#[derive(Debug, Args)]
pub struct UserListArgs {
    /// Username (defaults to current authenticated user).
    pub user: Option<String>,
    #[arg(long, default_value_t = 30u32)]
    pub limit: u32,
    #[command(flatten)]
    pub out: OutputOpts,
}

// ============================================================================

pub async fn run(ctx: &mut Context, args: OrgArgs) -> Result<(), CliError> {
    match args.command {
        OrgCmd::List(a) => list(ctx, a).await,
        OrgCmd::View(a) => view(ctx, a).await,
        OrgCmd::Member(c) => match c {
            MemberCmd::List(a) => member_list(ctx, a).await,
            MemberCmd::Add(a) => member_add(ctx, a).await,
            MemberCmd::Remove(a) => member_remove(ctx, a).await,
            MemberCmd::Edit(a) => member_edit(ctx, a).await,
        },
        OrgCmd::Follower(a) => follower(ctx, a, false).await,
        OrgCmd::Following(a) => follower(ctx, a, true).await,
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
    let q = format!("page=1&page_size={}", args.limit.max(1));
    let client = ctx.api()?;
    let items = orgs::list(client, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let slug = it.get("slug").and_then(Value::as_str).unwrap_or("");
        let name = it.get("name").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![slug.to_owned(), name.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["SLUG", "NAME"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    if args.web {
        let url = format!("https://{}/{}", ctx.host, args.group);
        eprintln!("→ Opening: {url}");
        let _ = open::that(&url);
        return Ok(());
    }
    let client = ctx.api()?;
    let v = orgs::view(client, &args.group).await?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let name = v.get("name").and_then(Value::as_str).unwrap_or(&args.group);
    let desc = v.get("description").and_then(Value::as_str).unwrap_or("");
    println!("{name}");
    if !desc.is_empty() {
        println!("  {desc}");
    }
    Ok(())
}

async fn member_list(ctx: &mut Context, args: MemberListArgs) -> Result<(), CliError> {
    use std::fmt::Write;
    let mut q = format!("page=1&page_size={}", args.limit.max(1));
    if let Some(r) = &args.role {
        write!(&mut q, "&role={r}").expect("write to String");
    }
    let client = ctx.api()?;
    let items = orgs::list_members(client, &args.group, &q).await?;
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let user = it.get("username").and_then(Value::as_str).unwrap_or("");
        let role = it.get("role").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![user.to_owned(), role.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["USER", "ROLE"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn member_add(ctx: &mut Context, args: MemberAddArgs) -> Result<(), CliError> {
    let client = ctx.api()?;
    let _ = orgs::add_member(client, &args.group, &args.username, &args.role).await?;
    eprintln!("✓ Added {} to {} as {}", args.username, args.group, args.role);
    Ok(())
}

async fn member_remove(ctx: &mut Context, args: MemberRemoveArgs) -> Result<(), CliError> {
    ctx.confirm(
        &format!("Remove `{}` from `{}`? (y/N)", args.username, args.group),
        args.yes,
    )?;
    let client = ctx.api()?;
    let _ = orgs::remove_member(client, &args.group, &args.username).await?;
    eprintln!("✓ Removed {} from {}", args.username, args.group);
    Ok(())
}

async fn member_edit(ctx: &mut Context, args: MemberEditArgs) -> Result<(), CliError> {
    let client = ctx.api()?;
    let _ = orgs::edit_member(client, &args.group, &args.username, &args.role).await?;
    eprintln!("✓ {} in {} → role {}", args.username, args.group, args.role);
    Ok(())
}

async fn follower(ctx: &mut Context, args: UserListArgs, following_mode: bool) -> Result<(), CliError> {
    let user = match args.user {
        Some(u) => u,
        None => {
            // Probe `/user` for current username.
            let client = ctx.api()?;
            let me = cnb_api::services::users::get_self(client).await?;
            me.username
        }
    };
    let q = format!("page=1&page_size={}", args.limit.max(1));
    let client = ctx.api()?;
    let items = if following_mode {
        orgs::following(client, &user, &q).await?
    } else {
        orgs::followers(client, &user, &q).await?
    };
    let v = Value::Array(items.clone());
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let username = it.get("username").and_then(Value::as_str).unwrap_or("");
        let nickname = it.get("nickname").and_then(Value::as_str).unwrap_or("");
        rows.push(vec![username.to_owned(), nickname.to_owned()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["USER", "NICKNAME"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}
