//! `cnb org` — organizations & members (M4 §8.10, 7 subcommands incl. follower).
//!
//! Phase 2, step 2.9 of the cnb-api → typed SDK migration. The verbs
//! route through three different SDK resource clients:
//!
//! | subcommand              | SDK client                  |
//! |-------------------------|-----------------------------|
//! | `list`                  | `OrganizationsClient::list_top_groups` |
//! | `view`                  | `OrganizationsClient::get_group`       |
//! | `member list`           | `MembersClient::list_members_of_group` |
//! | `member add`            | `MembersClient::add_members_of_group`  |
//! | `member edit`           | `MembersClient::update_members_of_group` |
//! | `member remove`         | `MembersClient::delete_members_of_group` |
//! | `follower` / `following`| `FollowersClient::get_followers_by_user_id` / `get_following_by_user_id` |
//!
//! The `--user` fallback for `follower`/`following` queries the
//! current user via `UsersClient::get_user_info` (GET /user) →
//! `UsersResult.username`.
//!
//! **Body-shape change, member add/edit** (→ SDK-I16): the SDK's
//! `UpdateMembersRequest` carries `{access_level, is_outside_collaborator}`
//! rather than the `{username, role}` that the cnb-api facade used.
//! We forward the CLI's `--role` value verbatim into `access_level`.
//! If a real server rejects this shape, the ticket gets bumped to
//! blocker and we fall back to raw HTTP.

use clap::{Args, Subcommand};
use cnb_sdk::followers::{GetFollowersByUserIDQuery, GetFollowingByUserIDQuery};
use cnb_sdk::members::ListMembersOfGroupQuery;
use cnb_sdk::models::UpdateMembersRequest;
use cnb_sdk::organizations::ListTopGroupsQuery;
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

fn to_value<T: serde::Serialize>(t: &T) -> Result<Value, CliError> {
    serde_json::to_value(t).map_err(|e| CliError::Generic(format!("serialise response: {e}")))
}

async fn list(ctx: &mut Context, args: ListArgs) -> Result<(), CliError> {
    let q = ListTopGroupsQuery::new()
        .page(1_i64)
        .page_size(i64::from(args.limit.max(1)));
    let items = {
        let client = ctx.sdk()?;
        client.organizations().list_top_groups(&q).await?
    };
    let v = Value::Array(items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?);
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    // `OrganizationAccess` has no `slug` field — the server uses
    // `path` for the slug-like identifier. Same pattern as
    // `Repos4User.path` we rely on elsewhere.
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let slug = v_get_str(&to_value(it)?, "path")
            .or_else(|| v_get_str(&to_value(it).unwrap_or(Value::Null), "slug"))
            .unwrap_or_default();
        let name = it_name(it).unwrap_or_default();
        rows.push(vec![slug, name]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["SLUG", "NAME"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

fn v_get_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(|s| s.to_owned())
}

/// `OrganizationAccess` does not expose `name` as a typed `Option<String>`
/// (it's inside a broader struct with flattened custom fields). We
/// fall back to `Value::get("name")` for a stable lookup.
fn it_name(access: &cnb_sdk::models::OrganizationAccess) -> Option<String> {
    to_value(access).ok().and_then(|v| v_get_str(&v, "name"))
}

async fn view(ctx: &mut Context, args: ViewArgs) -> Result<(), CliError> {
    if args.web {
        let url = format!("https://{}/{}", ctx.host, args.group);
        eprintln!("→ Opening: {url}");
        let _ = open::that(&url);
        return Ok(());
    }
    let org = {
        let client = ctx.sdk()?;
        client.organizations().get_group(args.group.clone()).await?
    };
    let v = to_value(&org)?;
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let name = v_get_str(&v, "name").unwrap_or_else(|| args.group.clone());
    let desc = v_get_str(&v, "description").unwrap_or_default();
    println!("{name}");
    if !desc.is_empty() {
        println!("  {desc}");
    }
    Ok(())
}

async fn member_list(ctx: &mut Context, args: MemberListArgs) -> Result<(), CliError> {
    let mut q = ListMembersOfGroupQuery::new()
        .page(1_i64)
        .page_size(i64::from(args.limit.max(1)));
    if let Some(r) = args.role {
        q = q.role(r);
    }
    let items = {
        let client = ctx.sdk()?;
        client.members().list_members_of_group(args.group.clone(), &q).await?
    };
    let v = Value::Array(items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?);
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let user = it.username.as_deref().unwrap_or("").to_owned();
        // Typed SDK DTO uses `access_level: Option<String>` (via the
        // `AccessRole = String` alias). The previous cnb-api facade
        // read `role` from the wire; that key is gone after the port.
        // Servers that still only emit `role` (not `access_level`)
        // will render an empty column here — intentional, matching
        // every other "typed-first" port in Phase 2.
        let role = it.access_level.clone().unwrap_or_default();
        rows.push(vec![user, role]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["USER", "ROLE"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

async fn member_add(ctx: &mut Context, args: MemberAddArgs) -> Result<(), CliError> {
    let body = UpdateMembersRequest {
        access_level: Some(args.role.clone()),
        is_outside_collaborator: None,
    };
    let client = ctx.sdk()?;
    let _ = client
        .members()
        .add_members_of_group(args.group.clone(), args.username.clone(), &body)
        .await?;
    eprintln!("✓ Added {} to {} as {}", args.username, args.group, args.role);
    Ok(())
}

async fn member_remove(ctx: &mut Context, args: MemberRemoveArgs) -> Result<(), CliError> {
    ctx.confirm(
        &format!("Remove `{}` from `{}`? (y/N)", args.username, args.group),
        args.yes,
    )?;
    let client = ctx.sdk()?;
    let _ = client
        .members()
        .delete_members_of_group(args.group.clone(), args.username.clone())
        .await?;
    eprintln!("✓ Removed {} from {}", args.username, args.group);
    Ok(())
}

async fn member_edit(ctx: &mut Context, args: MemberEditArgs) -> Result<(), CliError> {
    let body = UpdateMembersRequest {
        access_level: Some(args.role.clone()),
        is_outside_collaborator: None,
    };
    let client = ctx.sdk()?;
    let _ = client
        .members()
        .update_members_of_group(args.group.clone(), args.username.clone(), &body)
        .await?;
    eprintln!("✓ {} in {} → role {}", args.username, args.group, args.role);
    Ok(())
}

async fn follower(ctx: &mut Context, args: UserListArgs, following_mode: bool) -> Result<(), CliError> {
    let user = match args.user {
        Some(u) => u,
        None => {
            // Resolve the current user via `users::get_user_info`
            // (GET /user → UsersResult). Fall back to an error if
            // the SDK returns a body without `username`.
            let me = {
                let client = ctx.sdk()?;
                client.users().get_user_info().await?
            };
            me.username
                .ok_or_else(|| CliError::Generic("server did not return a username for the current user".into()))?
        }
    };

    let page = 1_i64;
    let page_size = i64::from(args.limit.max(1));
    let items = {
        let client = ctx.sdk()?;
        if following_mode {
            let q = GetFollowingByUserIDQuery::new().page(page).page_size(page_size);
            client.followers().get_following_by_user_id(user.clone(), &q).await?
        } else {
            let q = GetFollowersByUserIDQuery::new().page(page).page_size(page_size);
            client.followers().get_followers_by_user_id(user.clone(), &q).await?
        }
    };

    let v = Value::Array(items.iter().map(to_value).collect::<Result<Vec<_>, _>>()?);
    if render(ctx, &args.out, &v)? {
        return Ok(());
    }
    let mut rows = Vec::with_capacity(items.len());
    for it in &items {
        let username = it.username.as_deref().unwrap_or("").to_owned();
        let nickname = it.nickname.as_deref().unwrap_or("").to_owned();
        rows.push(vec![username, nickname]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["USER", "NICKNAME"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}
