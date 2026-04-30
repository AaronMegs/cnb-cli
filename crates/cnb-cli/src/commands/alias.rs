//! `cnb alias` — manage command aliases (M4 §8.14).
//!
//! Aliases are simple textual expansions stored in `config.toml::aliases`.
//! At runtime, `cnb` checks for aliases before matching subcommands and
//! re-invokes itself with the expanded argv. (Alias expansion in the dispatch
//! layer is wired in `crate::run`; here we only manage the storage.)

use std::collections::BTreeMap;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use cnb_config::Config;
use cnb_tty::table;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct AliasArgs {
    #[command(subcommand)]
    pub command: AliasCmd,
}

#[derive(Debug, Subcommand)]
pub enum AliasCmd {
    /// Add or replace an alias.
    Set(SetArgs),
    /// List all aliases.
    List,
    /// Delete an alias.
    Delete(DeleteArgs),
    /// Import aliases from a TOML file with `[aliases]` table or a flat
    /// JSON map of name→expansion.
    Import(ImportArgs),
}

#[derive(Debug, Args)]
pub struct SetArgs {
    pub name: String,
    /// Expansion string, e.g. `'issue list -l bug'`.
    pub expansion: String,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub name: String,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Path to TOML or JSON file. Use `-` to read from stdin.
    pub path: String,
}

#[allow(clippy::unused_async)]
pub async fn run(ctx: &mut Context, args: AliasArgs) -> Result<(), CliError> {
    match args.command {
        AliasCmd::Set(a) => set(ctx, a),
        AliasCmd::List => list(ctx),
        AliasCmd::Delete(a) => delete(ctx, a),
        AliasCmd::Import(a) => import(ctx, a),
    }
}

fn set(_ctx: &mut Context, args: SetArgs) -> Result<(), CliError> {
    let mut cfg = Config::load()?;
    cfg.aliases.insert(args.name.clone(), args.expansion.clone());
    cfg.save()?;
    eprintln!("✓ alias `{}` → {}", args.name, args.expansion);
    Ok(())
}

fn list(ctx: &mut Context) -> Result<(), CliError> {
    let cfg = Config::load()?;
    let rows: Vec<Vec<String>> = cfg.aliases.iter().map(|(k, v)| vec![k.clone(), v.clone()]).collect();
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["NAME", "EXPANSION"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

fn delete(_ctx: &mut Context, args: DeleteArgs) -> Result<(), CliError> {
    let mut cfg = Config::load()?;
    if cfg.aliases.remove(&args.name).is_none() {
        return Err(CliError::BadArgs(format!("no alias named `{}`", args.name)));
    }
    cfg.save()?;
    eprintln!("✓ deleted alias `{}`", args.name);
    Ok(())
}

fn import(_ctx: &mut Context, args: ImportArgs) -> Result<(), CliError> {
    let body = if args.path == "-" {
        use std::io::Read;
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(PathBuf::from(&args.path))?
    };
    let parsed = parse_alias_file(&body)?;
    let count = parsed.len();
    let mut cfg = Config::load()?;
    for (k, v) in parsed {
        cfg.aliases.insert(k, v);
    }
    cfg.save()?;
    eprintln!("✓ imported {count} alias(es)");
    Ok(())
}

/// TOML envelope for the `[aliases]` table.
#[derive(serde::Deserialize)]
struct AliasesTable {
    aliases: BTreeMap<String, String>,
}

/// Parse either a TOML doc with `[aliases]` table, a flat TOML doc, or a JSON
/// object mapping name→expansion.
fn parse_alias_file(body: &str) -> Result<BTreeMap<String, String>, CliError> {
    // Try JSON first.
    if let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(body) {
        return Ok(map);
    }
    // Try TOML with [aliases] table.
    if let Ok(parsed) = toml::from_str::<AliasesTable>(body) {
        return Ok(parsed.aliases);
    }
    // Try flat TOML doc (top-level is the alias map).
    if let Ok(map) = toml::from_str::<BTreeMap<String, String>>(body) {
        return Ok(map);
    }
    Err(CliError::BadArgs(
        "alias file must be JSON object or TOML with `[aliases]` table".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_json_object() {
        let body = r#"{"bugs": "issue list -l bug", "myrepo": "repo view cnb/feedback"}"#;
        let m = parse_alias_file(body).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m["bugs"], "issue list -l bug");
    }

    #[test]
    fn parse_toml_with_aliases_table() {
        let body = "[aliases]\nbugs = \"issue list -l bug\"\nmr = \"pr\"\n";
        let m = parse_alias_file(body).unwrap();
        assert_eq!(m["mr"], "pr");
    }

    #[test]
    fn parse_flat_toml() {
        let body = "bugs = \"issue list -l bug\"\n";
        let m = parse_alias_file(body).unwrap();
        assert_eq!(m["bugs"], "issue list -l bug");
    }

    #[test]
    fn parse_garbage_errors() {
        assert!(matches!(
            parse_alias_file("not <toml> or JSON").unwrap_err(),
            CliError::BadArgs(_)
        ));
    }
}
