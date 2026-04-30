//! `cnb` CLI binary entrypoint.

use clap::Parser;
use cnb_cli::Cli;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match cnb_cli::run(cli).await {
        Ok(()) => {}
        Err(e) => {
            // Print to stderr; tracing already captured detail at debug level.
            eprintln!("error: {e}");
            std::process::exit(e.exit_code());
        }
    }
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => LevelFilter::WARN,
        1 => LevelFilter::INFO,
        _ => LevelFilter::DEBUG,
    };
    let filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .try_init();
}
