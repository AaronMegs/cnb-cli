//! CLI command implementations.
//!
//! `cnb` (the bin) just parses argv via clap and calls into [`run`]; this lets
//! us write fast unit tests against the command tree without spawning processes.

pub mod cli;
pub mod commands;
pub mod context;
pub mod error;

pub use cli::{Cli, Commands};
pub use context::Context;
pub use error::CliError;

/// Entry point used by the binary.
pub async fn run(cli: Cli) -> Result<(), CliError> {
    let mut ctx = Context::from_cli(&cli)?;
    match cli.command {
        Commands::Auth(args) => commands::auth::run(&mut ctx, args).await,
        Commands::Api(args) => commands::api::run(&mut ctx, args).await,
        Commands::Repo(args) => commands::repo::run(&mut ctx, args).await,
        Commands::Issue(args) => commands::issue::run(&mut ctx, args).await,
        Commands::Label(args) => commands::label::run(&mut ctx, args).await,
        Commands::Pr(args) => commands::pr::run(&mut ctx, args).await,
        Commands::Build(args) => commands::build::run(&mut ctx, args).await,
        Commands::Workspace(args) => commands::workspace::run(&mut ctx, args).await,
        Commands::Release(args) => commands::release::run(&mut ctx, args).await,
        Commands::Registry(args) => commands::registry::run(&mut ctx, args).await,
        Commands::Mission(args) => commands::mission::run(&mut ctx, args).await,
        Commands::Org(args) => commands::org::run(&mut ctx, args).await,
        Commands::Browse(args) => commands::browse::run(&mut ctx, args).await,
        Commands::Completion(args) => commands::completion::run(&mut ctx, args).await,
        Commands::Config(args) => commands::config::run(&mut ctx, args).await,
        Commands::Alias(args) => commands::alias::run(&mut ctx, args).await,
        Commands::Update(args) => commands::update::run(&mut ctx, args).await,
    }
}
