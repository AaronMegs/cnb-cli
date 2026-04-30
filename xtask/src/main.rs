//! `cargo xtask <subcommand>` — build helpers for the cnb workspace.
//!
//! Subcommands:
//!   - `sync-openapi`     fetch upstream Swagger 2.0 → OpenAPI 3.0
//!   - `gen-man`          render man pages from the clap definitions
//!   - `gen-completions`  render shell completions (bash/zsh/fish/powershell/elvish)
//!   - `gen-dist`         run gen-man + gen-completions and stage them under `dist/`
//!
//! These helpers are invoked by both contributors locally and the release CI
//! workflow, so they must remain offline-friendly (no network for gen-*).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate_to, Shell};
use cnb_cli::Cli as CnbCli;

const DEFAULT_SWAGGER_URL: &str = "https://api.cnb.cool/swagger.json";
const BIN_NAME: &str = "cnb";

#[derive(Parser, Debug)]
#[command(name = "xtask", about = "cnb workspace build helpers")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Fetch upstream Swagger 2.0 spec and convert it to OpenAPI 3.0.
    SyncOpenapi {
        /// Source URL of the Swagger 2.0 document.
        #[arg(long, default_value = DEFAULT_SWAGGER_URL)]
        url: String,
        /// Output directory (relative to workspace root).
        #[arg(long, default_value = "openapi")]
        out: PathBuf,
        /// Skip the Swagger 2.0 → OpenAPI 3.0 conversion (useful when offline).
        #[arg(long)]
        no_convert: bool,
    },

    /// Render man pages (roff) for the cnb CLI and all its subcommands.
    GenMan {
        /// Output directory (relative to workspace root).
        #[arg(long, default_value = "dist/man")]
        out: PathBuf,
    },

    /// Render shell completions for bash/zsh/fish/powershell/elvish.
    GenCompletions {
        /// Output directory (relative to workspace root).
        #[arg(long, default_value = "dist/completions")]
        out: PathBuf,
    },

    /// Run `gen-man` and `gen-completions` together (used by release CI).
    GenDist {
        /// Output directory (relative to workspace root).
        #[arg(long, default_value = "dist")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::SyncOpenapi { url, out, no_convert } => sync_openapi(&url, &out, no_convert),
        Cmd::GenMan { out } => gen_man(&out),
        Cmd::GenCompletions { out } => gen_completions(&out),
        Cmd::GenDist { out } => {
            gen_man(&out.join("man"))?;
            gen_completions(&out.join("completions"))
        }
    }
}

// ---------------------------------------------------------------------------
// sync-openapi
// ---------------------------------------------------------------------------

fn sync_openapi(url: &str, out_rel: &Path, no_convert: bool) -> Result<()> {
    let workspace_root = workspace_root()?;
    let out_dir = workspace_root.join(out_rel);
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let swagger_path = out_dir.join("cnb-swagger-2.0.json");
    let openapi_path = out_dir.join("cnb-openapi-3.0.json");

    eprintln!("Fetching {url} ...");
    let body = reqwest::blocking::get(url)
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("GET {url} returned non-2xx"))?
        .bytes()
        .context("reading swagger body")?;
    std::fs::write(&swagger_path, &body).with_context(|| format!("writing {}", swagger_path.display()))?;
    eprintln!("✓ wrote {} ({} bytes)", swagger_path.display(), body.len());

    if no_convert {
        eprintln!("(skipping OpenAPI 3.0 conversion)");
        return Ok(());
    }

    eprintln!("Converting → OpenAPI 3.0 via `npx swagger2openapi` ...");
    let status = Command::new("npx")
        .args([
            "-y",
            "swagger2openapi",
            swagger_path.to_str().ok_or_else(|| anyhow!("non-utf8 path"))?,
            "-o",
            openapi_path.to_str().ok_or_else(|| anyhow!("non-utf8 path"))?,
        ])
        .status()
        .context("spawning npx (is Node.js installed?)")?;
    if !status.success() {
        bail!("swagger2openapi exited with {status}");
    }
    eprintln!("✓ wrote {}", openapi_path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// gen-man
// ---------------------------------------------------------------------------

fn gen_man(out_rel: &Path) -> Result<()> {
    let out_dir = workspace_root()?.join(out_rel);
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let cmd = CnbCli::command();
    write_man_recursive(&cmd, BIN_NAME, &out_dir)?;
    eprintln!("✓ man pages written under {}", out_dir.display());
    Ok(())
}

fn write_man_recursive(cmd: &clap::Command, prefix: &str, out_dir: &Path) -> Result<()> {
    // Render this command. Use a synthetic display name "cnb foo bar" so the
    // page header reads naturally; the file name uses dashes ("cnb-foo-bar.1").
    let display = if prefix == BIN_NAME {
        prefix.to_string()
    } else {
        format!("{BIN_NAME} {prefix}")
    };
    let file_stem = if prefix == BIN_NAME {
        BIN_NAME.to_string()
    } else {
        format!("{BIN_NAME}-{}", prefix.replace(' ', "-"))
    };

    // Clone the subtree so we can override the name without mutating the
    // shared command graph used by other recursion branches.
    // clap::Command::name takes `impl Into<Str>` which is implemented for
    // `&'static str` but not `String`. We intentionally leak the small per-page
    // header string (a few dozen bytes); xtask is short-lived so this is fine.
    let display_static: &'static str = Box::leak(display.into_boxed_str());
    let mut shaped = cmd.clone().name(display_static);
    shaped.build();

    let man = clap_mangen::Man::new(shaped);
    let path = out_dir.join(format!("{file_stem}.1"));
    let mut buf: Vec<u8> = Vec::new();
    man.render(&mut buf).context("rendering man page")?;
    std::fs::write(&path, &buf).with_context(|| format!("writing {}", path.display()))?;

    // Recurse into subcommands (skip the implicit "help" leaves).
    for sub in cmd.get_subcommands() {
        if sub.get_name() == "help" {
            continue;
        }
        let next_prefix = if prefix == BIN_NAME {
            sub.get_name().to_string()
        } else {
            format!("{prefix} {}", sub.get_name())
        };
        write_man_recursive(sub, &next_prefix, out_dir)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// gen-completions
// ---------------------------------------------------------------------------

fn gen_completions(out_rel: &Path) -> Result<()> {
    let out_dir = workspace_root()?.join(out_rel);
    std::fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let mut cmd = CnbCli::command();
    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell, Shell::Elvish] {
        let path = generate_to(shell, &mut cmd, BIN_NAME, &out_dir)
            .with_context(|| format!("generating {shell} completion"))?;
        eprintln!("✓ {} → {}", shell, path.display());
    }
    Ok(())
}

// ---------------------------------------------------------------------------

fn workspace_root() -> Result<PathBuf> {
    // CARGO_MANIFEST_DIR points at xtask/. Workspace root is its parent.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR not set")?;
    Ok(PathBuf::from(manifest)
        .parent()
        .ok_or_else(|| anyhow!("xtask manifest has no parent"))?
        .to_path_buf())
}
