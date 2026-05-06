//! Top-level clap definitions.

use clap::{Parser, Subcommand};

use crate::commands::{
    alias::AliasArgs, api::ApiArgs, auth::AuthArgs, browse::BrowseArgs, build::BuildArgs, completion::CompletionArgs,
    config::ConfigArgs, issue::IssueArgs, label::LabelArgs, mission::MissionArgs, org::OrgArgs, pr::PrArgs,
    registry::RegistryArgs, release::ReleaseArgs, repo::RepoArgs, search::SearchArgs, update::UpdateArgs,
    workspace::WorkspaceArgs,
};

/// `cnb` — official command-line tool for CNB (cnb.cool).
#[derive(Debug, Parser)]
#[command(name = "cnb", version, about, long_about = None)]
pub struct Cli {
    /// CNB host (default: `cnb.cool`). Equivalent to `CNB_HOST` env.
    #[arg(long, global = true, env = "CNB_HOST")]
    pub hostname: Option<String>,

    /// Increase verbosity (`-v` info, `-vv` debug).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Authenticate with a CNB host.
    Auth(AuthArgs),
    /// Make an authenticated request to a CNB API endpoint.
    Api(ApiArgs),
    /// Manage repositories.
    Repo(RepoArgs),
    /// Manage issues.
    Issue(IssueArgs),
    /// Manage repository labels.
    Label(LabelArgs),
    /// Manage pull requests (alias: `mr`).
    #[command(alias = "mr")]
    Pr(PrArgs),
    /// Trigger and inspect pipeline builds.
    Build(BuildArgs),
    /// Manage cloud-native dev environments (alias: `ws`).
    #[command(alias = "ws")]
    Workspace(WorkspaceArgs),
    /// Manage releases and their assets.
    Release(ReleaseArgs),
    /// Manage artifact registries and packages (M4).
    Registry(RegistryArgs),
    /// Manage task collections / missions (M4).
    Mission(MissionArgs),
    /// Manage organizations and members (M4).
    Org(OrgArgs),
    /// Open a CNB page in the browser (M4).
    Browse(BrowseArgs),
    /// Generate shell completion scripts (M4).
    Completion(CompletionArgs),
    /// Read/write user preferences (M4).
    Config(ConfigArgs),
    /// Manage command aliases (M4).
    Alias(AliasArgs),
    /// Check for newer cnb releases on GitHub (M5.0).
    Update(UpdateArgs),
    /// Search public repositories (first consumer of the typed SDK).
    Search(SearchArgs),
}
