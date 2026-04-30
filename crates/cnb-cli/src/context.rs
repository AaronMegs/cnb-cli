//! Lazy CLI runtime context.

use std::path::PathBuf;

use cnb_api::Client;
use cnb_auth::{resolve_token, AuthService, KeyringBackend, RealKeyring};
use cnb_config::{hosts as hosts_mod, paths};
use cnb_git::{parse_remote_url, RepoSlug};
use cnb_tty::IoStreams;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;

use crate::cli::Cli;
use crate::error::CliError;

/// Backend selection (env-driven for tests).
const ENV_KEYRING_BACKEND: &str = "CNB_KEYRING_BACKEND";

pub struct Context {
    pub host: String,
    pub io: IoStreams,
    pub hosts_path: PathBuf,
    keyring: Box<dyn KeyringBackend>,
    api: Option<Client>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("host", &self.host)
            .field("hosts_path", &self.hosts_path)
            .field("api", &self.api.is_some())
            .finish()
    }
}

impl Context {
    pub fn from_cli(cli: &Cli) -> Result<Self, CliError> {
        let host = cli
            .hostname
            .clone()
            .unwrap_or_else(|| hosts_mod::DEFAULT_HOST.to_owned());
        let io = IoStreams::default();
        let hosts_path = paths::hosts_file()?;
        let keyring = pick_keyring();
        Ok(Self {
            host,
            io,
            hosts_path,
            keyring,
            api: None,
        })
    }

    pub fn keyring(&self) -> &dyn KeyringBackend {
        self.keyring.as_ref()
    }

    pub fn auth_service(&self) -> AuthService<'_> {
        AuthService::new(self.keyring.as_ref(), self.hosts_path.clone())
    }

    /// Build (or reuse) the API client for the active host.
    /// Resolves the token via env > keyring > file.
    pub fn api(&mut self) -> Result<&Client, CliError> {
        if self.api.is_none() {
            let (token, _src) = resolve_token(&self.host, None, self.keyring.as_ref(), Some(&self.hosts_path))?;
            let client = Client::builder().token(token).build()?;
            self.api = Some(client);
        }
        Ok(self.api.as_ref().expect("just inserted"))
    }

    /// Build an API client with no token (for `auth login`'s validation step,
    /// where the token isn't yet stored).
    pub fn api_with_token(&self, token: &str) -> Result<Client, CliError> {
        Ok(Client::builder().token(token).build()?)
    }

    /// Resolve the repository slug to operate on (M2 §8.2 contract).
    ///
    /// Resolution order:
    ///   1. Explicit `OWNER/REPO[/SUBPATH]` if provided.
    ///   2. `git remote get-url origin` in the current directory, then parsed.
    ///
    /// Returns the slug as a flat path (`owner_path/repo`) suitable for
    /// substituting into URL templates like `/{repo}/-/issues`.
    pub fn resolve_repo(&self, explicit: Option<&str>) -> Result<String, CliError> {
        if let Some(s) = explicit {
            let cleaned = s.trim().trim_start_matches('/').trim_end_matches('/');
            if cleaned.is_empty() {
                return Err(CliError::BadArgs("empty --repo value".into()));
            }
            // Sanity check: must contain at least one slash (owner/repo).
            if !cleaned.contains('/') {
                return Err(CliError::BadArgs(format!(
                    "invalid repo `{cleaned}`: expected `OWNER/REPO`"
                )));
            }
            return Ok(cleaned.to_owned());
        }

        // Auto-detect from cwd.
        let cwd = std::env::current_dir()?;
        let url = cnb_git::git_cmd::remote_url(&cwd, "origin")?;
        let slug: RepoSlug = parse_remote_url(&url)?;
        Ok(slug.full_path())
    }

    /// Prompt the user for a destructive action. Returns `Cancelled` if denied.
    ///
    /// `--yes` short-circuits both the prompt and the cancellation.
    pub fn confirm(&self, prompt: &str, yes: bool) -> Result<(), CliError> {
        if yes {
            return Ok(());
        }
        if !self.io.stdin_is_tty {
            return Err(CliError::BadArgs(
                "destructive action requires either a TTY or `--yes`".into(),
            ));
        }
        let ok = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt)
            .default(false)
            .interact()
            .map_err(|e| CliError::Generic(e.to_string()))?;
        if ok {
            Ok(())
        } else {
            Err(CliError::Cancelled)
        }
    }
}

fn pick_keyring() -> Box<dyn KeyringBackend> {
    match std::env::var(ENV_KEYRING_BACKEND).as_deref() {
        Ok("memory" | "inmemory") => Box::new(cnb_auth::InMemoryKeyring::new()),
        Ok("none" | "disabled") => Box::new(NullKeyring),
        _ => Box::new(RealKeyring),
    }
}

/// Backend that always reports "not stored" — used in CI environments
/// when `CNB_KEYRING_BACKEND=none`. Forces fallback to file storage.
#[derive(Debug, Default)]
struct NullKeyring;

impl KeyringBackend for NullKeyring {
    fn get(&self, _service: &str, _account: &str) -> Result<Option<String>, cnb_auth::AuthError> {
        Ok(None)
    }
    fn set(&self, _service: &str, _account: &str, _secret: &str) -> Result<(), cnb_auth::AuthError> {
        Err(cnb_auth::AuthError::Keyring(
            "keyring disabled via CNB_KEYRING_BACKEND".into(),
        ))
    }
    fn delete(&self, _service: &str, _account: &str) -> Result<(), cnb_auth::AuthError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cnb_config::hosts;

    fn ctx() -> Context {
        let cli = Cli {
            hostname: Some(hosts::DEFAULT_HOST.into()),
            verbose: 0,
            command: crate::cli::Commands::Auth(crate::commands::auth::AuthArgs {
                command: crate::commands::auth::AuthCmd::Token(crate::commands::auth::TokenArgs { user: None }),
            }),
        };
        Context::from_cli(&cli).unwrap()
    }

    #[test]
    fn explicit_repo_with_owner_and_name_passes_through() {
        let c = ctx();
        let r = c.resolve_repo(Some("alice/widgets")).unwrap();
        assert_eq!(r, "alice/widgets");
    }

    #[test]
    fn explicit_repo_with_subgroup_passes_through() {
        let c = ctx();
        let r = c.resolve_repo(Some("cnb/sub/widgets")).unwrap();
        assert_eq!(r, "cnb/sub/widgets");
    }

    #[test]
    fn explicit_repo_strips_leading_and_trailing_slash() {
        let c = ctx();
        let r = c.resolve_repo(Some("/alice/widgets/")).unwrap();
        assert_eq!(r, "alice/widgets");
    }

    #[test]
    fn explicit_repo_without_slash_is_rejected() {
        let c = ctx();
        let err = c.resolve_repo(Some("widgets")).unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
    }

    #[test]
    fn explicit_empty_repo_is_rejected() {
        let c = ctx();
        let err = c.resolve_repo(Some("/")).unwrap_err();
        assert!(matches!(err, CliError::BadArgs(_)));
    }
}
