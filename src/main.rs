mod commands;
mod config;
mod formatting;
mod google;
mod ui;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "edh", version, about = "Everything Digital Health setup CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create all resources for a new EDH episode.
    New {
        /// Episode title (Title Case)
        title: Option<String>,
        /// Proposed date (YYYY-MM-DD)
        date: Option<String>,
    },
    /// Run the editing workflow (not implemented yet).
    Edit,
    /// Run the post-production workflow (not implemented yet).
    Post,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_crypto_provider()?;
    let cli = Cli::parse();
    match cli.command {
        Commands::New { title, date } => commands::new_episode(title, date).await,
        Commands::Edit => {
            eprintln!("Editing workflow is not implemented yet.");
            Ok(())
        }
        Commands::Post => {
            eprintln!("Post-production workflow is not implemented yet.");
            Ok(())
        }
    }
}

fn install_crypto_provider() -> Result<()> {
    let provider = rustls::crypto::ring::default_provider();
    rustls::crypto::CryptoProvider::install_default(provider)
        .map_err(|_| anyhow!("Failed to install Rustls crypto provider (ring): already installed"))
}
