//! `cnb config` — read/write user preferences (M4 §8.14).
//!
//! Supports dotted keys for nested fields:
//! - `core.editor`, `core.pager`, `core.git_protocol`, `core.prompt`
//! - `output.color`, `output.default_json_indent`
//!
//! Aliases live under `aliases.<name>` (use `cnb alias` for ergonomic access).

use std::process::Command;

use clap::{Args, Subcommand};
use cnb_config::{paths, Config};
use cnb_tty::table;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCmd,
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Get a configuration value.
    Get(GetArgs),
    /// Set a configuration value.
    Set(SetArgs),
    /// List all configuration entries.
    List,
    /// Open the config file in $EDITOR.
    Edit,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub key: String,
}

#[derive(Debug, Args)]
pub struct SetArgs {
    pub key: String,
    pub value: String,
}

#[allow(clippy::unused_async)]
pub async fn run(ctx: &mut Context, args: ConfigArgs) -> Result<(), CliError> {
    match args.command {
        ConfigCmd::Get(a) => get(ctx, a),
        ConfigCmd::Set(a) => set(ctx, a),
        ConfigCmd::List => list(ctx),
        ConfigCmd::Edit => edit(ctx),
    }
}

fn read_value(cfg: &Config, key: &str) -> Result<String, CliError> {
    Ok(match key {
        "core.editor" => cfg.core.editor.clone().unwrap_or_default(),
        "core.pager" => cfg.core.pager.clone().unwrap_or_default(),
        "core.git_protocol" => cfg.core.git_protocol.clone(),
        "core.prompt" => cfg.core.prompt.clone(),
        "output.color" => cfg.output.color.clone(),
        "output.default_json_indent" => cfg.output.default_json_indent.to_string(),
        k if k.starts_with("aliases.") => {
            let name = &k["aliases.".len()..];
            cfg.aliases.get(name).cloned().unwrap_or_default()
        }
        other => {
            return Err(CliError::BadArgs(format!(
                "unknown config key `{other}` (try: core.editor, core.pager, core.git_protocol, core.prompt, output.color, output.default_json_indent, aliases.NAME)"
            )));
        }
    })
}

fn write_value(cfg: &mut Config, key: &str, value: &str) -> Result<(), CliError> {
    match key {
        "core.editor" => cfg.core.editor = if value.is_empty() { None } else { Some(value.into()) },
        "core.pager" => cfg.core.pager = if value.is_empty() { None } else { Some(value.into()) },
        "core.git_protocol" => {
            if !matches!(value, "https" | "ssh") {
                return Err(CliError::BadArgs("git_protocol must be `https` or `ssh`".into()));
            }
            cfg.core.git_protocol = value.into();
        }
        "core.prompt" => {
            if !matches!(value, "enabled" | "disabled") {
                return Err(CliError::BadArgs("prompt must be `enabled` or `disabled`".into()));
            }
            cfg.core.prompt = value.into();
        }
        "output.color" => {
            if !matches!(value, "auto" | "always" | "never") {
                return Err(CliError::BadArgs("color must be `auto`/`always`/`never`".into()));
            }
            cfg.output.color = value.into();
        }
        "output.default_json_indent" => {
            cfg.output.default_json_indent = value
                .parse()
                .map_err(|_| CliError::BadArgs("default_json_indent must be 0-255".into()))?;
        }
        k if k.starts_with("aliases.") => {
            let name = k["aliases.".len()..].to_owned();
            if value.is_empty() {
                cfg.aliases.remove(&name);
            } else {
                cfg.aliases.insert(name, value.to_owned());
            }
        }
        other => {
            return Err(CliError::BadArgs(format!("unknown config key `{other}`")));
        }
    }
    Ok(())
}

fn get(_ctx: &mut Context, args: GetArgs) -> Result<(), CliError> {
    let cfg = Config::load()?;
    let v = read_value(&cfg, &args.key)?;
    println!("{v}");
    Ok(())
}

fn set(_ctx: &mut Context, args: SetArgs) -> Result<(), CliError> {
    let mut cfg = Config::load()?;
    write_value(&mut cfg, &args.key, &args.value)?;
    cfg.save()?;
    eprintln!("✓ {} = {}", args.key, args.value);
    Ok(())
}

fn list(ctx: &mut Context) -> Result<(), CliError> {
    let cfg = Config::load()?;
    let mut rows: Vec<Vec<String>> = vec![
        vec!["core.editor".into(), cfg.core.editor.unwrap_or_default()],
        vec!["core.pager".into(), cfg.core.pager.unwrap_or_default()],
        vec!["core.git_protocol".into(), cfg.core.git_protocol],
        vec!["core.prompt".into(), cfg.core.prompt],
        vec!["output.color".into(), cfg.output.color],
        vec![
            "output.default_json_indent".into(),
            cfg.output.default_json_indent.to_string(),
        ],
    ];
    for (k, v) in &cfg.aliases {
        rows.push(vec![format!("aliases.{k}"), v.clone()]);
    }
    let mut stdout = std::io::stdout().lock();
    table::write_table(&mut stdout, &["KEY", "VALUE"], &rows, ctx.io.stdout_is_tty)?;
    Ok(())
}

fn edit(_ctx: &mut Context) -> Result<(), CliError> {
    let editor = std::env::var("EDITOR")
        .ok()
        .or_else(|| {
            // Fall back to config.core.editor if set.
            Config::load().ok().and_then(|c| c.core.editor)
        })
        .unwrap_or_else(|| "vi".to_owned());
    let path = paths::config_file()?;
    let status = Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        return Err(CliError::Generic(format!(
            "editor `{editor}` exited with status {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_known_keys_returns_default() {
        let cfg = Config::default();
        assert_eq!(read_value(&cfg, "core.git_protocol").unwrap(), "https");
        assert_eq!(read_value(&cfg, "output.color").unwrap(), "auto");
    }

    #[test]
    fn read_unknown_key_errors() {
        let cfg = Config::default();
        assert!(matches!(
            read_value(&cfg, "nope.what").unwrap_err(),
            CliError::BadArgs(_)
        ));
    }

    #[test]
    fn write_validates_git_protocol_enum() {
        let mut cfg = Config::default();
        assert!(write_value(&mut cfg, "core.git_protocol", "ssh").is_ok());
        assert_eq!(cfg.core.git_protocol, "ssh");
        assert!(matches!(
            write_value(&mut cfg, "core.git_protocol", "ftp").unwrap_err(),
            CliError::BadArgs(_)
        ));
    }

    #[test]
    fn write_alias_round_trips() {
        let mut cfg = Config::default();
        write_value(&mut cfg, "aliases.bugs", "issue list -l bug").unwrap();
        assert_eq!(read_value(&cfg, "aliases.bugs").unwrap(), "issue list -l bug");
        write_value(&mut cfg, "aliases.bugs", "").unwrap();
        assert_eq!(read_value(&cfg, "aliases.bugs").unwrap(), "");
    }
}
