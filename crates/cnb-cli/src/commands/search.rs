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
use cnb_sdk::search::ListPublicReposQuery;
use serde_json::Value;

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
    // Build the SDK query. `--desc` is a CLI-ergonomic alias for the
    // underlying `desc: Option<bool>` field — we only set `Some(true)` when
    // the user explicitly asks, so the default (server-controlled) order is
    // preserved when the flag is absent.
    let query = ListPublicReposQuery {
        key: args.key.clone(),
        flags: args.flags.clone(),
        flags_match: args.flags_match.clone(),
        order_by: args.order_by.clone(),
        desc: if args.desc { Some(true) } else { None },
        top_n: args.top_n,
    };

    let client = ctx.sdk()?;
    let hits = client.search().list_public_repos(&query).await?;

    // Reinterpret as serde_json::Value for uniform --json/--jq/--template.
    // `Repos4UserBase` derives Serialize, so the conversion is infallible.
    let v = serde_json::to_value(&hits).expect("Repos4UserBase serialises infallibly");

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
    // `Repos4UserBase` DTO actually populates (the upstream search response
    // does NOT include `full_path` / `visibility` — that's the `Repos4User`
    // shape used by `repo list`. We surface `path`, `visibility_level` and
    // `updated_at` instead).
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(hits.len());
    if let Some(arr) = v.as_array() {
        for it in arr {
            let path = it.get("path").and_then(Value::as_str).unwrap_or("").to_owned();
            // `visibility_level` may serialise as either a string enum
            // (e.g. "public") or, depending on the spec variant, an int —
            // we coerce both to a display string.
            let vis = it
                .get("visibility_level")
                .map(|val| val.as_str().map_or_else(|| val.to_string(), ToOwned::to_owned))
                .unwrap_or_default();
            let updated = it.get("updated_at").and_then(Value::as_str).unwrap_or("").to_owned();
            rows.push(vec![path, vis, updated]);
        }
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
