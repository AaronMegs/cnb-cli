//! `cnb update` — version check against GitHub Releases (M5.0).
//!
//! By design we **never** make this network call without explicit user
//! invocation; it does not run on cli startup, only when the user types
//! `cnb update [--check]`. This honors the DESIGN principle of "no telemetry".

use clap::Args;
use serde::Deserialize;

use crate::context::Context;
use crate::error::CliError;

const DEFAULT_REPO: &str = "cnb-cool/cnb";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Only check; do not print install instructions.
    #[arg(long)]
    pub check: bool,
    /// Override release source (default: `cnb-cool/cnb`).
    #[arg(long, default_value = DEFAULT_REPO)]
    pub repo: String,
}

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
    #[serde(default)]
    html_url: String,
}

pub async fn run(_ctx: &mut Context, args: UpdateArgs) -> Result<(), CliError> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", args.repo);
    let client = reqwest::Client::builder()
        .user_agent(format!("cnb-cli/{CURRENT_VERSION}"))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| CliError::Generic(format!("http client: {e}")))?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CliError::Generic(format!("could not reach {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(CliError::Generic(format!("{} returned HTTP {}", url, resp.status())));
    }
    let latest: LatestRelease = resp
        .json()
        .await
        .map_err(|e| CliError::Generic(format!("could not parse release JSON: {e}")))?;

    let cmp = compare_versions(CURRENT_VERSION, latest.tag_name.trim_start_matches('v'));
    match cmp {
        std::cmp::Ordering::Less => {
            eprintln!(
                "• A newer release is available: {} (you have {CURRENT_VERSION})",
                latest.tag_name
            );
            if !args.check {
                eprintln!("  Release notes: {}", latest.html_url);
                eprintln!("  Upgrade:");
                eprintln!(
                    "    curl -fsSL https://raw.githubusercontent.com/{}/main/scripts/install.sh | bash",
                    args.repo
                );
            }
        }
        std::cmp::Ordering::Equal => {
            eprintln!("✓ cnb {CURRENT_VERSION} is the latest release");
        }
        std::cmp::Ordering::Greater => {
            eprintln!(
                "• cnb {CURRENT_VERSION} is newer than the latest published release ({})",
                latest.tag_name
            );
        }
    }
    Ok(())
}

/// Lexicographic version compare with numeric awareness on dotted segments.
/// Strips a leading `v` and trims pre-release suffixes (`-alpha.X`, `+build.Y`).
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parts(v: &str) -> Vec<u64> {
        v.trim_start_matches('v')
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|s| s.parse::<u64>().unwrap_or(0))
            .collect()
    }
    parts(a).cmp(&parts(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn compare_normal() {
        assert_eq!(compare_versions("0.4.0", "0.5.0"), Ordering::Less);
        assert_eq!(compare_versions("0.4.0", "0.4.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "0.9.0"), Ordering::Greater);
    }

    #[test]
    fn compare_strips_prefix_and_suffix() {
        assert_eq!(compare_versions("v0.4.0", "0.4.0"), Ordering::Equal);
        assert_eq!(
            compare_versions("0.4.0-alpha.1", "0.4.0"),
            Ordering::Equal,
            "pre-release strips for compare"
        );
    }
}
