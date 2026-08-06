// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel;
use crate::commands;
use crate::output::OutputFormat;
use crate::paths::tilde_pathbuf;
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "eps",
    version,
    about = "Local-first episode production automation",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Channel ID when more than one config is available.
    #[arg(long, global = true)]
    channel: Option<String>,

    /// Explicit channel configuration file.
    #[arg(
        long,
        global = true,
        value_parser = tilde_pathbuf,
        value_hint = ValueHint::FilePath
    )]
    config: Option<PathBuf>,

    /// Output encoding.
    #[arg(long, global = true, value_enum, default_value_t)]
    format: OutputFormat,

    /// Print the complete plan without applying local or remote writes.
    #[arg(short = 'n', long, global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Option<RootCommand>,
}

#[derive(Debug, Subcommand)]
enum RootCommand {
    /// Inspect and validate channel configuration.
    Channel {
        #[command(subcommand)]
        command: Option<ChannelCommand>,
    },
    /// Generate shell completion scripts.
    Completions(CompletionsArgs),
    /// Inspect and validate episode state.
    Episode {
        #[command(subcommand)]
        command: Option<EpisodeCommand>,
    },
    /// Report the installed eps version.
    Version,
}

#[derive(Debug, Subcommand)]
enum ChannelCommand {
    /// List channel configs in a directory.
    List {
        /// Directory containing channel TOML files.
        #[arg(
            long,
            default_value = "channels",
            value_parser = tilde_pathbuf,
            value_hint = ValueHint::DirPath
        )]
        directory: PathBuf,
    },
    /// Show the resolved channel configuration summary.
    Show,
    /// Validate the resolved channel configuration.
    Validate,
}

#[derive(Debug, Subcommand)]
enum EpisodeCommand {
    /// Inspect local episode state without changing it.
    Inspect {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        path: PathBuf,
    },
    /// Validate local upload readiness and lifecycle consistency.
    Validate {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        path: PathBuf,
    },
}

#[derive(Debug, clap::Args)]
struct CompletionsArgs {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    shell: Shell,

    /// Write the completion file into this directory instead of stdout.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::DirPath)]
    directory: Option<PathBuf>,
}

pub fn run() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if is_version_alias(&args) {
        println!("eps {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let cli = Cli::parse_from(args);
    match cli.command {
        None => print_root_help(),
        Some(RootCommand::Version) => commands::version::run(cli.format),
        Some(RootCommand::Completions(args)) => commands::completions::run(
            args.shell,
            args.directory.as_deref(),
            cli.dry_run,
            Cli::command(),
        ),
        Some(RootCommand::Channel { command: None }) => print_channel_help(),
        Some(RootCommand::Channel {
            command: Some(ChannelCommand::List { directory }),
        }) => commands::channel::list(&directory, cli.format),
        Some(RootCommand::Channel {
            command: Some(ChannelCommand::Show),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::channel::show(&path, cli.format)
        }
        Some(RootCommand::Channel {
            command: Some(ChannelCommand::Validate),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::channel::validate(&path, cli.format)
        }
        Some(RootCommand::Episode { command: None }) => print_episode_help(),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::Inspect { path }),
        }) => commands::episode::inspect(&path, cli.format),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::Validate { path }),
        }) => commands::episode::validate(&path, cli.format),
    }
}

fn print_root_help() -> Result<()> {
    Cli::command().print_help()?;
    println!();
    Ok(())
}

fn print_channel_help() -> Result<()> {
    println!(
        "Inspect and validate channel configuration\n\nUsage: eps channel <COMMAND>\n\nCommands:\n  list      List channel configs in a directory\n  show      Show the resolved channel configuration summary\n  validate  Validate the resolved channel configuration"
    );
    Ok(())
}

fn print_episode_help() -> Result<()> {
    println!(
        "Inspect and validate episode state\n\nUsage: eps episode <COMMAND>\n\nCommands:\n  inspect   Inspect local episode state without changing it\n  validate  Validate local upload readiness and lifecycle consistency"
    );
    Ok(())
}

fn is_version_alias(args: &[OsString]) -> bool {
    args.len() == 2
        && args[1]
            .to_str()
            .is_some_and(|argument| matches!(argument, "-v" | "--v" | "-version"))
}
