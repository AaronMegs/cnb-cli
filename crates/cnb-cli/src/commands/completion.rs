//! `cnb completion` — generate shell completion scripts (M4 §8.13).

use clap::{Args, CommandFactory, ValueEnum};
use clap_complete::{generate, Shell};

use crate::cli::Cli;
use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct CompletionArgs {
    /// Shell type. If omitted, attempt to auto-detect from `$SHELL`.
    pub shell: Option<ShellChoice>,
    /// Print install instructions instead of writing the script.
    #[arg(long)]
    pub install: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ShellChoice {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
}

impl ShellChoice {
    fn into_clap(self) -> Shell {
        match self {
            Self::Bash => Shell::Bash,
            Self::Zsh => Shell::Zsh,
            Self::Fish => Shell::Fish,
            Self::Powershell => Shell::PowerShell,
            Self::Elvish => Shell::Elvish,
        }
    }

    fn install_hint(self) -> &'static str {
        match self {
            Self::Bash => "add to ~/.bashrc:  source <(cnb completion bash)",
            Self::Zsh => "add to ~/.zshrc:   source <(cnb completion zsh)",
            Self::Fish => "save to ~/.config/fish/completions/cnb.fish:  cnb completion fish > ~/.config/fish/completions/cnb.fish",
            Self::Powershell => "PowerShell:  cnb completion powershell | Out-String | Invoke-Expression",
            Self::Elvish => "elvish:  use cnb-completion (cnb completion elvish | str:split-from-stdin)",
        }
    }
}

fn detect_shell() -> Option<ShellChoice> {
    let shell = std::env::var("SHELL").ok()?;
    let s = shell.rsplit('/').next().unwrap_or("");
    match s {
        "bash" => Some(ShellChoice::Bash),
        "zsh" => Some(ShellChoice::Zsh),
        "fish" => Some(ShellChoice::Fish),
        "elvish" => Some(ShellChoice::Elvish),
        _ => None,
    }
}

#[allow(clippy::unused_async)]
pub async fn run(_ctx: &mut Context, args: CompletionArgs) -> Result<(), CliError> {
    let choice = match args.shell {
        Some(c) => c,
        None => detect_shell()
            .ok_or_else(|| CliError::BadArgs("could not detect shell from $SHELL; pass one explicitly".into()))?,
    };

    if args.install {
        eprintln!("# Install hint:");
        eprintln!("# {}", choice.install_hint());
        return Ok(());
    }

    let mut cmd = Cli::command();
    let mut stdout = std::io::stdout().lock();
    generate(choice.into_clap(), &mut cmd, "cnb", &mut stdout);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_hints_are_distinct() {
        assert!(ShellChoice::Bash.install_hint().contains("bashrc"));
        assert!(ShellChoice::Zsh.install_hint().contains("zshrc"));
        assert!(ShellChoice::Fish.install_hint().contains("fish"));
    }
}
