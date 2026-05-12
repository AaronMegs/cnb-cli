//! `cnb search` — public repo search.
//!
//! **First consumer of the external typed SDK** (`cnb` crate, aliased as
//! `cnb-sdk`). The underlying endpoint is `GET /search/public-repos` which is
//! the only operation in the `search` tag group, making it an ideal pilot:
//! read-only, anonymous-friendly, typed DTO response.
//!
//! Once this command is proven in CI and on a real cnb.cool account, the
//! rest of the cnb-api facades will be ported over module-by-module in
//! Phase 2 of the migration (see CHANGELOG under `[Unreleased]`).

use clap::Args;
use serde_json::Value;

use crate::commands::repo::format_visibility;
use crate::context::Context;
use crate::error::CliError;
use cnb_tty::{jq, json_out, table, template};

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Full-text search key. When omitted, the server returns a default
    /// hot-list (exact semantics depend on the cnb.cool instance).
    #[arg(value_name = "KEY")]
    pub key: Option<String>,

    /// Comma-separated repository type flags (forwarded verbatim as `flags`).
    #[arg(long, value_name = "CSV")]
    pub flags: Option<String>,

    /// Match mode when multiple `--flags` are supplied (forwarded verbatim).
    #[arg(long, value_name = "MODE")]
    pub flags_match: Option<String>,

    /// Sort key (forwarded verbatim).
    #[arg(long, value_name = "FIELD")]
    pub order_by: Option<String>,

    /// Descending order.
    #[arg(long)]
    pub desc: bool,

    /// Cap the response to the top-N hits.
    #[arg(long, value_name = "N")]
    pub top_n: Option<i64>,

    // --- standard output flags (identical to other list commands) ---
    /// Emit raw JSON (full envelope).
    #[arg(long)]
    pub json: bool,
    /// Apply a jq expression (implies `--json` semantics).
    #[arg(long, value_name = "EXPR")]
    pub jq: Option<String>,
    /// Apply a tinytemplate string (implies `--json` semantics).
    #[arg(long, value_name = "TPL")]
    pub template: Option<String>,
}

pub async fn run(ctx: &mut Context, args: SearchArgs) -> Result<(), CliError> {
    // We *intentionally* bypass the typed `client.search().list_public_repos(&q)`
    // call here. Reason: cnb 0.2.2's `Repos4UserBase` DTO types `flags`
    // as `Option<crate::models::Repo>`, but the live server returns a
    // plain string at that field (e.g. `"Unknown"`), which makes serde
    // blow up with `invalid type: string "Unknown", expected struct Repo`.
    //
    // We build the same `GET /search/public-repos` URL by hand (using
    // the SDK's `client.http().url(path)` so base-URL precedence and
    // percent-encoding stay identical) and decode the body as raw
    // `serde_json::Value`. The default table only reads `path`,
    // `visibility_level`, and `updated_at`, so dropping the typed DTO
    // costs us nothing.
    //
    // Tracked as a follow-up SDK issue (sibling to SDK-I02 / SDK-I11);
    // revert to the typed call once upstream relaxes the field to
    // `Option<serde_json::Value>` or the server stops sending the
    // string form.
    let mut path = String::from("/search/public-repos");
    let mut sep = '?';
    let push = |path: &mut String, sep: &mut char, k: &str, v: &str| {
        path.push(*sep);
        path.push_str(k);
        path.push('=');
        path.extend(url::form_urlencoded::byte_serialize(v.as_bytes()));
        *sep = '&';
    };
    if let Some(v) = args.key.as_deref() {
        push(&mut path, &mut sep, "key", v);
    }
    if let Some(v) = args.flags.as_deref() {
        push(&mut path, &mut sep, "flags", v);
    }
    if let Some(v) = args.flags_match.as_deref() {
        push(&mut path, &mut sep, "flags_match", v);
    }
    if let Some(v) = args.order_by.as_deref() {
        push(&mut path, &mut sep, "order_by", v);
    }
    if args.desc {
        push(&mut path, &mut sep, "desc", "true");
    }
    if let Some(v) = args.top_n {
        // SDK 0.2.2 wire form: query parameter is `topN` (camelCase),
        // not `top_n`. Match it byte-for-byte so any shared wiremock
        // fixture / server-side log keeps recognising the request.
        push(&mut path, &mut sep, "topN", &v.to_string());
    }

    let v = ctx.sdk_raw_get(&path).await?;

    if let Some(tpl) = args.template.as_deref() {
        println!("{}", template::apply(&v, tpl)?);
        return Ok(());
    }
    if let Some(expr) = args.jq.as_deref() {
        let outs = jq::apply(&v, expr)?;
        let mut stdout = std::io::stdout().lock();
        for o in outs {
            json_out::write_json(&mut stdout, &o, false)?;
        }
        return Ok(());
    }
    if args.json {
        let mut stdout = std::io::stdout().lock();
        json_out::write_json(&mut stdout, &v, ctx.io.stdout_is_tty)?;
        return Ok(());
    }

    // Default human-readable table. Column choice mirrors the fields the
    // server actually populates for a search hit (the upstream search
    // response does NOT include `full_path` / `visibility` — that's the
    // `Repos4User` shape used by `repo list`. We surface `path`,
    // `visibility_level` and `updated_at` instead).
    let empty = Vec::<Value>::new();
    let arr = v.as_array().unwrap_or(&empty);
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(arr.len());
    for it in arr {
        let p = it.get("path").and_then(Value::as_str).unwrap_or("").to_owned();
        // Use the shared `format_visibility` helper so the search table
        // and the `repo list/view` table render visibility identically
        // (canonical SDK capitalisation: Public / Private / Secret).
        let vis = format_visibility(it.get("visibility_level")).to_owned();
        let updated = it.get("updated_at").and_then(Value::as_str).unwrap_or("").to_owned();
        rows.push(vec![p, vis, updated]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(
        &mut stdout,
        &["PATH", "VISIBILITY", "UPDATED"],
        &rows,
        ctx.io.stdout_is_tty,
    )?;
    Ok(())
}
