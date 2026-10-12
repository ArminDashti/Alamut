mod http_server;
mod service;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "Alamut", version, about = "Alamut — collectors + Web UI + HTTP/WebSocket API (http://alamut.local)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run in foreground (dev / console)
    Run,
    /// Install Windows service
    Install,
    /// Uninstall Windows service
    Uninstall,
    /// Start Windows service
    Start,
    /// Stop Windows service
    Stop,
    /// Print service install/running status
    Status,
    /// Print version
    Version,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Run) {
        Commands::Run => {
            #[cfg(windows)]
            {
                let service_result = tokio::task::spawn_blocking(move || {
                    service::run_as_service()
                })
                .await;

                match service_result {
                    Ok(Ok(())) => return Ok(()),
                    Ok(Err(_)) => {
                        tracing::info!("Not running as Windows service; starting in standalone console mode");
                    }
                    Err(join_err) => {
                        tracing::warn!("service dispatcher task join error: {join_err}; falling back to standalone mode");
                    }
                }
            }

            http_server::run_standalone().await
        }
        Commands::Install => service::install(),
        Commands::Uninstall => service::uninstall(),
        Commands::Start => service::start(),
        Commands::Stop => service::stop(),
        Commands::Status => service::status(),
        Commands::Version => {
            println!("alamut-api {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}
