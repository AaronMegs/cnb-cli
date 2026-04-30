//! `cnb browse` — open repo / branch / issue / PR / release in the browser
//! (M4 §8.12).

use std::path::PathBuf;

use clap::Args;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct BrowseArgs {
    /// File or directory path inside the repo.
    pub path: Option<PathBuf>,
    /// `OWNER/REPO[/SUBGROUP]` (or auto-detected from `git remote origin`).
    #[arg(long)]
    pub repo: Option<String>,
    /// Open the branch tree view.
    #[arg(long)]
    pub branch: Option<String>,
    /// Jump to the issue with NUMBER.
    #[arg(long, conflicts_with_all = ["pr", "release"])]
    pub issue: Option<u64>,
    /// Jump to the PR with NUMBER.
    #[arg(long, conflicts_with_all = ["issue", "release"])]
    pub pr: Option<u64>,
    /// Open the release page; pass an optional TAG.
    #[arg(long, value_name = "TAG", num_args = 0..=1, default_missing_value = "")]
    pub release: Option<String>,
    /// Print the URL instead of opening a browser.
    #[arg(long, alias = "no-browser")]
    pub url_only: bool,
}

#[allow(clippy::unused_async)]
pub async fn run(ctx: &mut Context, args: BrowseArgs) -> Result<(), CliError> {
    use std::fmt::Write;
    let repo = ctx.resolve_repo(args.repo.as_deref())?;
    let mut url = format!("https://{}/{}", ctx.host, repo);

    if let Some(num) = args.issue {
        write!(&mut url, "/-/issues/{num}").expect("write to String");
    } else if let Some(num) = args.pr {
        write!(&mut url, "/-/pulls/{num}").expect("write to String");
    } else if let Some(tag) = args.release {
        if tag.is_empty() {
            url.push_str("/-/releases");
        } else {
            write!(&mut url, "/-/releases/tag/{tag}").expect("write to String");
        }
    } else if let Some(branch) = args.branch.as_deref() {
        let path_seg = args
            .path
            .as_ref()
            .map(|p| format!("/{}", p.display()))
            .unwrap_or_default();
        write!(&mut url, "/-/tree/{branch}{path_seg}").expect("write to String");
    } else if let Some(p) = args.path.as_ref() {
        // Default branch is implicit; let the server redirect.
        write!(&mut url, "/-/blob/HEAD/{}", p.display()).expect("write to String");
    }

    if args.url_only || !ctx.io.stdout_is_tty {
        println!("{url}");
    } else {
        eprintln!("→ Opening: {url}");
        let _ = open::that(&url);
    }
    Ok(())
}
