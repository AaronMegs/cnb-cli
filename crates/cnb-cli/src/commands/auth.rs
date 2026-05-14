//! `cnb auth login | logout | status | token`.

use std::io::{IsTerminal, Read};

use clap::{Args, Subcommand};
use cnb_auth::TokenSource;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Password;

use crate::context::Context;
use crate::error::CliError;

#[derive(Debug, Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCmd,
}

#[derive(Debug, Subcommand)]
pub enum AuthCmd {
    /// Authenticate with a CNB host using a Personal Access Token.
    Login(LoginArgs),
    /// Remove credentials for a host/user.
    Logout(LogoutArgs),
    /// Display the current authentication status.
    Status(StatusArgs),
    /// Print the active token (for piping into other commands).
    Token(TokenArgs),
    /// Configure git to use the cnb token as a credential helper (M4).
    SetupGit(SetupGitArgs),
    /// Internal: act as a git credential helper (called by git, not humans).
    ///
    /// Implements the git-credential protocol over stdin/stdout. Wired up
    /// by `cnb auth setup-git`; you should not need to invoke this
    /// directly. Hidden from `--help` to avoid confusing users.
    #[command(name = "git-credential", hide = true)]
    GitCredential(GitCredentialArgs),
}

#[derive(Debug, Args)]
pub struct LoginArgs {
    /// Read the token from stdin (one line) instead of prompting.
    #[arg(long)]
    pub with_token: bool,
    /// Default git protocol to record.
    #[arg(long, value_parser = ["https", "ssh"], default_value = "https")]
    pub git_protocol: String,
}

#[derive(Debug, Args)]
pub struct LogoutArgs {
    #[arg(long)]
    pub user: Option<String>,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Reveal the token (TTY only; ignored when stdout is piped).
    #[arg(long)]
    pub show_token: bool,
}

#[derive(Debug, Args)]
pub struct TokenArgs {
    #[arg(long)]
    pub user: Option<String>,
}

#[derive(Debug, Args)]
pub struct SetupGitArgs {
    /// Print the proposed `git config` commands instead of executing them.
    #[arg(long)]
    pub print_only: bool,
    /// Override host (defaults to active --hostname).
    #[arg(long)]
    pub hostname: Option<String>,
}

#[derive(Debug, Args)]
pub struct GitCredentialArgs {
    /// First positional arg passed by git (`get` / `store` / `erase`).
    /// Only `get` produces output; other operations exit silently.
    pub operation: String,
    /// Username whose token to surface. Wired in by `setup-git` so the
    /// helper is bound to a specific cnb account on this machine.
    #[arg(long)]
    pub user: String,
}

pub async fn run(ctx: &mut Context, args: AuthArgs) -> Result<(), CliError> {
    match args.command {
        AuthCmd::Login(a) => login(ctx, a).await,
        AuthCmd::Logout(a) => logout(ctx, a),
        AuthCmd::Status(a) => status(ctx, a).await,
        AuthCmd::Token(a) => token(ctx, a),
        AuthCmd::SetupGit(a) => setup_git(ctx, a),
        AuthCmd::GitCredential(a) => git_credential(ctx, a),
    }
}

async fn login(ctx: &mut Context, args: LoginArgs) -> Result<(), CliError> {
    let token = if args.with_token {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        let trimmed = s.trim().to_owned();
        if trimmed.is_empty() {
            return Err(CliError::BadArgs("empty token from stdin".into()));
        }
        trimmed
    } else {
        if !std::io::stdin().is_terminal() {
            return Err(CliError::BadArgs(
                "no TTY detected; pass `--with-token` and pipe the token via stdin".into(),
            ));
        }
        Password::with_theme(&ColorfulTheme::default())
            .with_prompt(format!("Paste your CNB Personal Access Token for {}", ctx.host))
            .interact()
            .map_err(|e| CliError::Generic(e.to_string()))?
    };

    // Validate against /user via the typed SDK. The SDK's `UsersResult`
    // DTO types `username` as `Option<String>`, so we extract it
    // defensively — a `GET /user` response without a username would be
    // a server-side contract violation, not an auth failure.
    let probe = ctx.sdk_with_token(&token)?;
    let user = probe.users().get_user_info().await?;
    let username = user
        .username
        .as_deref()
        .ok_or_else(|| CliError::Generic("`/user` response omitted `username`".into()))?;

    let svc = ctx.auth_service();
    let record = svc.login(&ctx.host, username, &token, &args.git_protocol)?;

    let where_stored = if record.stored_in_keyring {
        "system keyring"
    } else {
        "config file"
    };
    eprintln!(
        "✓ Logged in to {} as {} (token stored in {})",
        record.host, record.user, where_stored
    );
    Ok(())
}

fn logout(ctx: &mut Context, args: LogoutArgs) -> Result<(), CliError> {
    let svc = ctx.auth_service();
    let user = match args.user {
        Some(u) => u,
        None => {
            // Find a default user from hosts.toml.
            let hosts = cnb_config::Hosts::load_from(&ctx.hosts_path)?;
            hosts
                .default_user(&ctx.host)
                .map(str::to_owned)
                .ok_or_else(|| CliError::BadArgs(format!("no user known for {}", ctx.host)))?
        }
    };
    svc.logout(&ctx.host, &user)?;
    eprintln!("✓ Logged out of {} (user {})", ctx.host, user);
    Ok(())
}

async fn status(ctx: &mut Context, args: StatusArgs) -> Result<(), CliError> {
    let svc = ctx.auth_service();
    let st = svc.status(&ctx.host, None)?;

    // Optionally re-validate by hitting /user via the typed SDK. We
    // only care about the request-level success/failure; the response
    // body isn't displayed here.
    let token = svc.token(&ctx.host, None)?;
    let probe = ctx.sdk_with_token(&token)?;
    let user_check = probe.users().get_user_info().await;

    let source = match &st.source {
        TokenSource::Env => "env (CNB_TOKEN)".to_owned(),
        TokenSource::Keyring { .. } => "system keyring".to_owned(),
        TokenSource::File { .. } => "config file".to_owned(),
    };

    println!("Logged in to {} as {}", st.host, st.user);
    println!("  Token source: {source}");
    match user_check {
        Ok(_) => println!("  Token: ✓ valid"),
        Err(e) => println!("  Token: ✗ invalid ({e})"),
    }
    if args.show_token {
        if ctx.io.stdout_is_tty {
            println!("  Token value: {token}");
        } else {
            eprintln!("  (refusing to print token: stdout is not a TTY)");
        }
    }
    Ok(())
}

fn token(ctx: &mut Context, args: TokenArgs) -> Result<(), CliError> {
    let svc = ctx.auth_service();
    let t = svc.token(&ctx.host, args.user.as_deref())?;
    println!("{t}");
    Ok(())
}

/// Configure a git credential helper that delegates to `cnb auth token`.
///
/// We follow the `gh auth setup-git` model: register a custom helper at the
/// global git config level for the specific cnb host. Git will invoke the
/// helper on `git push/pull/clone over HTTPS` and we'll respond with the
/// active token. **No token is written to disk** — this matches gh and avoids
/// the `~/.git-credentials` plain-text fallback.
fn setup_git(ctx: &mut Context, args: SetupGitArgs) -> Result<(), CliError> {
    use std::process::Command;

    let host = args.hostname.as_deref().unwrap_or(&ctx.host);

    // The helper command git will invoke: `cnb auth git-credential`. Since
    // git's credential protocol is "exec helper, exchange key=value over
    // stdin/stdout", and we don't ship a `git-credential` adapter yet, we
    // register the simpler "cache → helper-line that delegates to a script".
    //
    // For maximum portability without shelling out to a separate adapter, we
    // configure the well-known `store` helper but **do not** persist anything;
    // instead we ask git to call `!cnb auth token --user <USER>` (gh-style
    // shell helper notation).
    //
    // Lookup the username from hosts.toml. Without a recorded user we cannot
    // automate this (the helper needs to know which credentials to fetch).
    let svc = ctx.auth_service();
    let st = svc.status(host, None).map_err(|_| {
        CliError::BadArgs(format!(
            "no recorded credentials for `{host}`; run `cnb auth login` first"
        ))
    })?;

    // Helper value uses git's `!`-prefixed exec-command form. The body
    // is a **plain command line, not a shell script** — git splits on
    // whitespace and execs directly, which means it works identically
    // on Linux/macOS *and* on Windows native cmd/PowerShell (no MSYS
    // bash dependency). The previous `!f() { ... }; f` shell-function
    // form required Git for Windows' bundled bash and silently broke
    // anywhere git was installed without one.
    //
    // The actual credential-protocol I/O is handled by the hidden
    // `cnb auth git-credential` subcommand, which reads git's stdin
    // KV stream and writes back `username=` / `password=` lines.
    let helper_value = format!("!cnb auth git-credential --user {}", st.user);
    let key = format!("credential.https://{host}.helper");

    if args.print_only {
        println!("# Run these to enable cnb as a git credential helper:");
        println!("git config --global --replace-all '{key}' '{helper_value}'");
        return Ok(());
    }

    // Replace any prior cnb helper for this host (idempotent).
    let status = Command::new("git")
        .args(["config", "--global", "--replace-all", &key, &helper_value])
        .status()?;
    if !status.success() {
        return Err(CliError::Generic(format!(
            "git config exited with status {}",
            status.code().unwrap_or(-1)
        )));
    }

    eprintln!("✓ Configured git credential helper for {host} (user: {})", st.user);
    eprintln!("  git push/pull over https://{host}/... will now use your cnb token.");
    eprintln!("  To remove:  git config --global --unset {key}");
    Ok(())
}

/// Hidden subcommand wired into git as a credential helper by `setup-git`.
///
/// Implements the [git credential helper protocol]:
/// - git invokes us with one positional arg (`get` / `store` / `erase`)
/// - git writes a list of `key=value\n` lines on our stdin, terminated
///   by a blank line
/// - on `get`, we MUST write `username=...\n` + `password=...\n` + a
///   blank line back on stdout (other keys are optional and we omit
///   them; git already filled them in from the URL)
/// - `store` and `erase` are no-ops for us: we never persist anything
///   git asked us to save (the cnb token is the source of truth and is
///   managed via `cnb auth login` / `logout`), and we cannot meaningfully
///   "erase" a token whose lifecycle git does not own.
///
/// Why a real subcommand and not a shell function: the previous
/// `!f() { ... }; f` form embedded in git config required POSIX shell
/// (or Git for Windows' MSYS bash) to evaluate. Native Windows cmd /
/// PowerShell users — and any other minimal-git environment — silently
/// got "helper invoked but produced no output". A direct executable
/// works on every platform git itself runs on.
///
/// [git credential helper protocol]: https://git-scm.com/docs/gitcredentials#_custom_helpers
fn git_credential(ctx: &mut Context, args: GitCredentialArgs) -> Result<(), CliError> {
    use std::io::BufRead;

    // git only expects output for `get`. We deliberately tolerate any
    // other op name (`store`, `erase`, future additions) by exiting 0
    // without printing — git treats no-output as "helper had nothing
    // to contribute" rather than an error, which is exactly what we
    // want for a read-only adapter on top of the cnb keyring.
    if args.operation != "get" {
        return Ok(());
    }

    // Drain git's stdin per the protocol — even if we don't need the
    // values, leaving the pipe full causes git to wait. We stop at the
    // first empty line, matching git's framing.
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear();
        let n = handle.read_line(&mut line)?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
    }

    // Look up the active token for the bound user. We mirror the
    // resolution chain that `cnb auth token --user X` uses (env >
    // keyring > hosts.toml). Any failure here means git can't auth —
    // we surface it via stderr (visible with GIT_TRACE) and exit
    // non-zero so the user sees the prompt fall back to interactive
    // entry rather than silently shipping an empty password.
    let svc = ctx.auth_service();
    let token = svc.token(&ctx.host, Some(&args.user))?;

    // Order matches what `git credential fill` expects on stdout: any
    // missing key triggers a fallback prompt. We write both, then a
    // blank line to terminate the response.
    println!("username={}", args.user);
    println!("password={token}");
    println!();
    Ok(())
}
