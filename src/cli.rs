// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel;
use crate::command_registry::{self, Discovery};
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
    about = "Plan, produce, and publish video and podcast episodes from the command line",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Configuration file (defaults to eps.toml in the current directory).
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
    /// Analyse and process episode audio.
    Audio {
        #[command(subcommand)]
        command: Option<AudioCommand>,
    },
    /// Generate or install shell completion scripts.
    Completions {
        #[command(subcommand)]
        command: Option<CompletionCommand>,
    },
    /// List machine-discoverable command contracts.
    Commands,
    /// Check local workspace prerequisites without running tools or contacting services.
    Doctor,
    /// Inspect and validate episode state.
    Episode {
        #[command(subcommand)]
        command: Option<EpisodeCommand>,
    },
    /// Generate episode metadata with the configured model.
    Metadata {
        #[command(subcommand)]
        command: Option<MetadataCommand>,
    },
    /// Extract frames and compose thumbnails.
    Thumbnail {
        #[command(subcommand)]
        command: Option<ThumbnailCommand>,
    },
    /// Upload to and reconcile with YouTube.
    Youtube {
        #[command(subcommand)]
        command: Option<YoutubeCommand>,
    },
    /// Produce transcripts and captions.
    Transcript {
        #[command(subcommand)]
        command: Option<TranscriptCommand>,
    },
    /// Audit old episode media against the channel retention policy.
    Storage {
        #[command(subcommand)]
        command: Option<StorageCommand>,
    },
    /// Quarantine disposable artefacts from one episode without deleting them.
    Cleanup {
        #[command(subcommand)]
        command: Option<CleanupCommand>,
    },
    /// Report the installed eps version.
    Version,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AudioCommand {
    /// Measure loudness without changing anything.
    Analyse {
        /// Audio or video file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        input: PathBuf,
    },
    /// Apply the channel's loudness chain, producing the upload master.
    Process(MediaArgs),
}

#[derive(Debug, Subcommand)]
pub(crate) enum MetadataCommand {
    /// Generate YouTube chapter markers from a transcript.
    Chapters(TranscriptTaskArgs),
    /// Generate the tag list from a transcript.
    Tags(TranscriptTaskArgs),
    /// Generate thumbnail quote candidates from a transcript.
    Quote(TranscriptTaskArgs),
    /// Generate the YouTube description.
    Description(TaskArgs),
    /// Draft a post for one social platform.
    Social {
        /// Platform task, for example bluesky-draft.
        task: String,

        #[command(flatten)]
        args: TaskArgs,
    },
    /// Check an existing artefact against its task contract.
    Check {
        /// Task name, for example chapters.
        task: String,

        /// Artefact to check.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        input: PathBuf,

        /// Transcript, enabling the chapter coverage check.
        #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        transcript: Option<PathBuf>,
    },
}

#[derive(Debug, clap::Args)]
pub(crate) struct TaskArgs {
    /// Episode to take title, URL, chapters, and show notes from.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) episode: Option<PathBuf>,

    /// Write the result here instead of standard output.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) output: Option<PathBuf>,

    /// Transcript to condense into the prompt.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) transcript: Option<PathBuf>,

    /// Extra prompt variables, as NAME=VALUE.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) variables: Vec<String>,

    /// Regenerate even if the output already exists.
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, clap::Args)]
pub(crate) struct TranscriptTaskArgs {
    /// Episode to take title, URL, chapters, and show notes from.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) episode: Option<PathBuf>,

    /// Whisper SRT transcript.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) transcript: PathBuf,

    /// Write the result here instead of standard output.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) output: Option<PathBuf>,

    /// Extra prompt variables, as NAME=VALUE.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) variables: Vec<String>,

    /// Regenerate even if the output already exists.
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum YoutubeCommand {
    /// Show what an upload would do, contacting nothing.
    PlanUpload(UploadArgs),
    /// Upload the episode. Requires --apply.
    Upload(UploadArgs),
    /// Compare the episode record against the destination.
    Verify {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        episode: PathBuf,
    },
    /// Upload or replace the episode's caption track. Requires --apply.
    Captions(RepairArgs),
    /// Set the episode's thumbnail. Requires --apply.
    Thumbnail(RepairArgs),
    /// Add the episode to a playlist. Requires --apply.
    Playlist {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        episode: PathBuf,

        /// Playlist ID to add the video to.
        playlist: String,

        /// Perform the change. Without this the command only reports it.
        #[arg(long)]
        apply: bool,
    },
    /// Change a video's privacy, for example to make it public.
    Publish {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        episode: PathBuf,

        /// Privacy to set.
        #[arg(long, default_value = "public")]
        privacy: String,

        /// Perform the change. Without this the command only reports it.
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Debug, clap::Args)]
pub(crate) struct RepairArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) episode: PathBuf,

    /// Perform the change. Without this the command only reports it.
    #[arg(long)]
    pub(crate) apply: bool,
}

#[derive(Debug, clap::Args)]
pub(crate) struct UploadArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) episode: PathBuf,

    /// Playlist IDs to add the video to.
    #[arg(long = "playlist", value_name = "PLAYLIST_ID")]
    pub(crate) playlists: Vec<String>,

    /// Privacy for the new video. Defaults to the channel's, or unlisted.
    #[arg(long)]
    pub(crate) privacy: Option<String>,

    /// Perform the upload. Without this the command only reports the plan.
    #[arg(long)]
    pub(crate) apply: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ThumbnailCommand {
    /// Extract evenly spaced still frames for selection.
    Frames {
        /// Video to take frames from.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        video: PathBuf,

        /// How many frames to extract.
        #[arg(long, default_value_t = 8)]
        count: u32,
    },
    /// Overlay caption text on a frame to make the thumbnail.
    Compose {
        /// Source image, usually a chosen frame.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        image: PathBuf,

        /// Caption text.
        text: String,

        /// PNG output path. Defaults to the episode's thumbnail.
        #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        output: Option<PathBuf>,

        /// Do not wrap the caption in double quotes.
        #[arg(long)]
        no_quotes: bool,

        /// Recompose even if the thumbnail already exists.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum TranscriptCommand {
    /// Extract the standardised mono track used for transcription.
    Extract(MediaArgs),
    /// Transcribe audio into the srt, vtt, txt, json, and tsv family.
    Transcribe {
        /// Audio file to transcribe.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
        input: PathBuf,

        /// Directory for the transcript family. Defaults to the input's.
        #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::DirPath)]
        output_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum StorageCommand {
    /// Report old media candidates across the selected workspace without changing anything.
    Audit {
        /// Lifecycle stage to inspect. Repeat for more than one stage.
        #[arg(long = "stage")]
        stages: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum CleanupCommand {
    /// Show selected episode artefacts that would be moved to quarantine.
    Plan {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        episode: PathBuf,

        /// Include ambiguous source media and editor projects in the plan.
        #[arg(long)]
        include_review: bool,
    },
    /// Move selected episode artefacts to its local quarantine. Requires --apply.
    Apply {
        /// Episode directory or canonical metadata file.
        #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
        episode: PathBuf,

        /// Include ambiguous source media and editor projects in the cleanup.
        #[arg(long)]
        include_review: bool,

        /// Perform the move. Without this the command only reports the plan.
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Debug, clap::Args)]
pub(crate) struct MediaArgs {
    /// Audio or video file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) input: PathBuf,

    /// Output path. Defaults to the contract's name beside the input.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ChannelCommand {
    /// List the single selected workspace channel.
    List,
    /// Show the resolved channel configuration summary.
    Show,
    /// Validate the resolved channel configuration.
    Validate,
}

#[derive(Debug, Subcommand)]
pub(crate) enum EpisodeCommand {
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
    /// Set managed metadata fields, preserving all other content.
    Set(SetArgs),
    /// Replace a social draft block, preserving all other content.
    SetDraft(SetDraftArgs),
    /// Replace the generated tag list, preserving all other content.
    SetTags(SetTagsArgs),
    /// Record that one playlist was successfully added.
    MarkPlaylist(MarkPlaylistArgs),
    /// Add a missing social platform section.
    AddPlatform(AddPlatformArgs),
    /// Create a new episode folder in the planning stage.
    New(NewArgs),
    /// List episodes by lifecycle stage.
    List {
        /// Show only one lifecycle stage.
        #[arg(long)]
        stage: Option<String>,
    },
    /// Move an episode to another lifecycle stage.
    Promote(PromoteArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct NewArgs {
    /// Episode date, as YYYY-MM-DD.
    pub(crate) date: String,

    /// Episode title in human form; the folder slug is derived from it.
    pub(crate) title: String,

    /// Guest name, when the episode has one.
    #[arg(long)]
    pub(crate) guest: Option<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PromoteArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) path: PathBuf,

    /// Destination lifecycle stage.
    pub(crate) stage: String,

    /// Override the lifecycle guards.
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SetTagsArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) path: PathBuf,

    /// Read the tags from a file instead of standard input.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) from_file: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct MarkPlaylistArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) path: PathBuf,

    /// Playlist ID, matched against the entries in the playlist section.
    pub(crate) playlist: String,

    /// Detail to append, for example 'Added 2026-08-07T09:00:00Z'.
    #[arg(long)]
    pub(crate) note: Option<String>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct AddPlatformArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) path: PathBuf,

    /// Platform section to add, for example Mastodon.
    pub(crate) platform: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SetArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) path: PathBuf,

    /// Field assignments, as 'Label=value'.
    #[arg(required = true, value_name = "LABEL=VALUE")]
    pub(crate) assignments: Vec<String>,

    /// Enclosing heading, required when a label appears in several sections.
    #[arg(long)]
    pub(crate) section: Option<String>,

    /// Replace a field that already holds a value rather than a placeholder.
    #[arg(long)]
    pub(crate) force: bool,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SetDraftArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    pub(crate) path: PathBuf,

    /// Social platform section, for example Bluesky.
    pub(crate) platform: String,

    /// Draft heading within the platform section.
    #[arg(long, default_value = "Draft")]
    pub(crate) heading: String,

    /// Read the draft from a file instead of standard input.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    pub(crate) from_file: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct CompletionDirectoryArgs {
    /// Write the completion file into this directory instead of stdout.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::DirPath)]
    pub(crate) directory: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum CompletionCommand {
    /// Generate a bash completion script.
    Bash(CompletionDirectoryArgs),
    /// Generate a zsh completion script.
    Zsh(CompletionDirectoryArgs),
    /// Generate a fish completion script.
    Fish(CompletionDirectoryArgs),
    /// Generate a PowerShell completion script.
    Powershell(CompletionDirectoryArgs),
    /// Generate an elvish completion script.
    Elvish(CompletionDirectoryArgs),
    /// Install a completion script without editing shell startup files.
    Install(CompletionInstallArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct CompletionInstallArgs {
    /// Shell to install for. Defaults to the detected current shell.
    #[arg(long, value_enum)]
    pub(crate) shell: Option<Shell>,

    /// Completion directory. Defaults to the shell's standard user directory.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::DirPath)]
    pub(crate) directory: Option<PathBuf>,
}

pub fn run() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if is_version_alias(&args) {
        println!("eps {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if let Some(discovery) = command_registry::discover(&args, &Cli::command()) {
        return print_discovery(discovery);
    }

    let cli = Cli::parse_from(args);
    match cli.command {
        None => print_root_help(),
        Some(RootCommand::Version) => command_registry::run_version(cli.format),
        Some(RootCommand::Commands) => command_registry::run_commands(Cli::command(), cli.format),
        Some(RootCommand::Doctor) => {
            command_registry::run_doctor(cli.config.as_deref(), cli.format)
        }
        Some(RootCommand::Completions { command: None }) => print_family_help("completions"),
        Some(RootCommand::Completions {
            command: Some(command),
        }) => command_registry::run_completions(command, cli.dry_run, cli.format, Cli::command()),
        Some(RootCommand::Audio { command: None }) => print_family_help("audio"),
        Some(RootCommand::Audio {
            command: Some(command),
        }) => command_registry::run_audio(command, cli.config.as_deref(), cli.dry_run, cli.format),
        Some(RootCommand::Metadata { command: None }) => print_family_help("metadata"),
        Some(RootCommand::Metadata {
            command: Some(command),
        }) => {
            let config = channel::resolve_path(cli.config.as_deref())?;
            command_registry::run_metadata(command, &config, cli.dry_run, cli.format)
        }
        Some(RootCommand::Youtube { command: None }) => print_family_help("youtube"),
        Some(RootCommand::Youtube {
            command: Some(command),
        }) => {
            command_registry::run_youtube(command, cli.config.as_deref(), cli.dry_run, cli.format)
        }
        Some(RootCommand::Thumbnail { command: None }) => print_family_help("thumbnail"),
        Some(RootCommand::Thumbnail {
            command: Some(command),
        }) => {
            command_registry::run_thumbnail(command, cli.config.as_deref(), cli.dry_run, cli.format)
        }
        Some(RootCommand::Transcript { command: None }) => print_family_help("transcript"),
        Some(RootCommand::Transcript {
            command: Some(command),
        }) => command_registry::run_transcript(
            command,
            cli.config.as_deref(),
            cli.dry_run,
            cli.format,
        ),
        Some(RootCommand::Storage { command: None }) => print_family_help("storage"),
        Some(RootCommand::Storage {
            command: Some(command),
        }) => command_registry::run_storage(command, cli.config.as_deref(), cli.format),
        Some(RootCommand::Cleanup { command: None }) => print_family_help("cleanup"),
        Some(RootCommand::Cleanup {
            command: Some(command),
        }) => command_registry::run_cleanup(command, cli.dry_run, cli.format),
        Some(RootCommand::Channel { command: None }) => print_family_help("channel"),
        Some(RootCommand::Channel {
            command: Some(command),
        }) => command_registry::run_channel(command, cli.config.as_deref(), cli.format),
        Some(RootCommand::Episode { command: None }) => print_family_help("episode"),
        Some(RootCommand::Episode {
            command: Some(command),
        }) => {
            command_registry::run_episode(command, cli.config.as_deref(), cli.dry_run, cli.format)
        }
    }
}

fn print_root_help() -> Result<()> {
    Cli::command().print_help()?;
    println!();
    Ok(())
}

fn print_discovery(discovery: Discovery) -> Result<()> {
    match discovery {
        Discovery::Schema(value) | Discovery::Template(value) => {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
    }
    Ok(())
}

/// Print a command family's help by asking clap, so the listing can never
/// drift from the commands that actually exist.
fn print_family_help(family: &str) -> Result<()> {
    let mut command = Cli::command();
    let subcommand = command
        .find_subcommand_mut(family)
        .expect("every family printing help is a defined subcommand");
    // Without this the usage line reads "Usage: episode", losing the binary.
    subcommand
        .clone()
        .bin_name(format!("eps {family}"))
        .print_help()?;
    println!();
    Ok(())
}

fn is_version_alias(args: &[OsString]) -> bool {
    args.len() == 2
        && args[1]
            .to_str()
            .is_some_and(|argument| matches!(argument, "-v" | "--v" | "-version"))
}
