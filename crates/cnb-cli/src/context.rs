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
    /// Typed SDK client (external crate `cnb`, aliased as `cnb-sdk`).
    /// Introduced in Phase 1 of the cnb-api → cnb SDK migration and shared
    /// with new commands (starting with `cnb search`) while legacy facades
    /// continue to use `api` above. Once every command has migrated,
    /// `api` and the local `cnb-api` crate can be removed.
    sdk: Option<cnb_sdk::ApiClient>,
    /// Base URL override for the SDK client — set by tests (wiremock) via
    /// [`Context::set_sdk_base_url`]. Production code leaves this `None` and
    /// the SDK uses its built-in `https://api.cnb.cool` default.
    sdk_base_url: Option<String>,
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
            sdk: None,
            sdk_base_url: None,
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

    /// Build (or reuse) the typed SDK client (`cnb` crate / `cnb-sdk`).
    ///
    /// The previous `api_with_token(token)` helper (last caller: `auth
    /// login/status`) has been removed together with its callers — see
    /// [`Context::sdk_with_token`] for the typed-SDK replacement. `api()`
    /// (no suffix) stays because `cnb api` raw passthrough still depends
    /// on `cnb_api::Client` (SDK-I14 JSON-only transport gap).
    ///
    /// Phase 1 of the cnb-api → SDK migration. Token resolution keeps the
    /// project's three-tier contract (env > keyring > file) by running
    /// `cnb_auth::resolve_token` here and **only** feeding the resolved
    /// string into `ClientBuilder::token()` — we never rely on the SDK's own
    /// `CNB_TOKEN` env-var fallback so behaviour stays identical on
    /// CI / remote-container machines where the env var is unset.
    ///
    /// The base URL honours the same `CNB_API_BASE` test override that
    /// `cnb-api::Client` already supports, keeping all existing wiremock
    /// fixtures usable for SDK-backed commands too.
    pub fn sdk(&mut self) -> Result<&cnb_sdk::ApiClient, CliError> {
        if self.sdk.is_none() {
            let (token, _src) = resolve_token(&self.host, None, self.keyring.as_ref(), Some(&self.hosts_path))?;
            let mut builder = cnb_sdk::ApiClient::builder()
                .token(token)
                .user_agent(concat!("cnb/", env!("CARGO_PKG_VERSION")));
            // Effective base URL precedence: explicit override (set via
            // `set_sdk_base_url`, used by unit tests) > `CNB_API_BASE` env
            // (used by integration tests / wiremock) > SDK default
            // (`https://api.cnb.cool`).
            let base_override = self
                .sdk_base_url
                .clone()
                .or_else(|| std::env::var("CNB_API_BASE").ok().filter(|v| !v.is_empty()));
            if let Some(base) = base_override {
                builder = builder.base_url(base);
            }
            let client = builder.build().map_err(CliError::from)?;
            self.sdk = Some(client);
        }
        Ok(self.sdk.as_ref().expect("just inserted"))
    }

    /// Test-only hook: override the SDK base URL (e.g. a wiremock server).
    /// Must be called before the first [`Context::sdk`] invocation.
    #[doc(hidden)]
    pub fn set_sdk_base_url(&mut self, url: impl Into<String>) {
        self.sdk_base_url = Some(url.into());
        // Drop any previously-built client so the override takes effect on
        // the next `sdk()` call. Cheap: `ApiClient` is just `Arc`s under the
        // hood, so there's no real teardown cost.
        self.sdk = None;
    }

    /// Build a **one-shot** SDK client using an explicit token, bypassing
    /// the usual env > keyring > file resolution.
    ///
    /// Used by `cnb auth login` to validate a freshly-pasted token before
    /// it is persisted — at that point `resolve_token` would return
    /// `NotLoggedIn`. Also used by `cnb auth status` when it wants to
    /// re-validate against a specific token source rather than re-derive
    /// through the resolver.
    ///
    /// The base URL honours the same precedence as [`Context::sdk`]:
    /// explicit override (tests) > `CNB_API_BASE` env (wiremock) >
    /// SDK default. The returned client is **not** cached: callers that
    /// need the shared SDK instance should use [`Context::sdk`] instead.
    pub fn sdk_with_token(&self, token: &str) -> Result<cnb_sdk::ApiClient, CliError> {
        let mut builder = cnb_sdk::ApiClient::builder()
            .token(token.to_owned())
            .user_agent(concat!("cnb/", env!("CARGO_PKG_VERSION")));
        let base_override = self
            .sdk_base_url
            .clone()
            .or_else(|| std::env::var("CNB_API_BASE").ok().filter(|v| !v.is_empty()));
        if let Some(base) = base_override {
            builder = builder.base_url(base);
        }
        builder.build().map_err(CliError::from)
    }

    /// Resolve the base URL that the SDK client will actually use. Mirrors
    /// the precedence used inside [`Context::sdk`]: explicit override
    /// (tests) > `CNB_API_BASE` env (wiremock fixtures) > SDK default.
    fn effective_sdk_base_url(&self) -> String {
        self.sdk_base_url
            .clone()
            .or_else(|| std::env::var("CNB_API_BASE").ok().filter(|v| !v.is_empty()))
            .unwrap_or_else(|| cnb_sdk::DEFAULT_BASE_URL.to_owned())
    }

    /// Low-level GET returning `serde_json::Value`, routed through the SDK's
    /// HTTP layer (shares its reqwest pool, retry config, auth header, and
    /// tracing instrumentation).
    ///
    /// Useful for commands whose endpoint **is** modelled by the SDK but
    /// whose rendering logic still needs fields the typed DTO does not
    /// expose (e.g. `default_branch` on a single-repo view). Prefer the
    /// typed `client.<resource>().<op>()` call wherever the DTO is
    /// sufficient.
    pub async fn sdk_raw_get(&mut self, path: &str) -> Result<serde_json::Value, CliError> {
        let base = self.effective_sdk_base_url();
        // `url::Url::join` treats an absolute path correctly but requires
        // the base to end with `/`. We normalise defensively.
        let mut base_with_slash = base;
        if !base_with_slash.ends_with('/') {
            base_with_slash.push('/');
        }
        let base_url = url::Url::parse(&base_with_slash)
            .map_err(|e| CliError::Generic(format!("invalid SDK base url `{base_with_slash}`: {e}")))?;
        let full = base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| CliError::Generic(format!("could not join path `{path}` onto base: {e}")))?;
        let client = self.sdk()?;
        let v: serde_json::Value = client.http().execute(reqwest::Method::GET, full).await?;
        Ok(v)
    }

    /// Low-level JSON-body request (`PUT` / `POST` / `PATCH`) returning
    /// `serde_json::Value`, routed through the SDK's HTTP layer. Useful
    /// for endpoints the SDK exposes only as a GET (e.g. `pinned-repos`,
    /// which the SDK types for `GET` but leaves the `PUT` counterpart
    /// unmodelled).
    ///
    /// Prefer the typed `client.<resource>().<op>(body)` call wherever
    /// the SDK has it — this helper exists to unblock consumers until
    /// the SDK grows the missing verbs.
    pub async fn sdk_raw_json(
        &mut self,
        method: reqwest::Method,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, CliError> {
        let base = self.effective_sdk_base_url();
        let mut base_with_slash = base;
        if !base_with_slash.ends_with('/') {
            base_with_slash.push('/');
        }
        let base_url = url::Url::parse(&base_with_slash)
            .map_err(|e| CliError::Generic(format!("invalid SDK base url `{base_with_slash}`: {e}")))?;
        let full = base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| CliError::Generic(format!("could not join path `{path}` onto base: {e}")))?;
        let client = self.sdk()?;
        let v: serde_json::Value = client.http().execute_with_body(method, full, body).await?;
        Ok(v)
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
