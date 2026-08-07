// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::channel;
use crate::commands;
use crate::output::OutputFormat;
use crate::paths::tilde_pathbuf;
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

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
    /// Analyse and process episode audio.
    Audio {
        #[command(subcommand)]
        command: Option<AudioCommand>,
    },
    /// Generate shell completion scripts.
    Completions(CompletionsArgs),
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
    /// Produce transcripts and captions.
    Transcript {
        #[command(subcommand)]
        command: Option<TranscriptCommand>,
    },
    /// Report the installed eps version.
    Version,
}

#[derive(Debug, Subcommand)]
enum AudioCommand {
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
enum MetadataCommand {
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
struct TaskArgs {
    /// Episode to take title, URL, chapters, and show notes from.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    episode: Option<PathBuf>,

    /// Write the result here instead of standard output.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,

    /// Transcript to condense into the prompt.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    transcript: Option<PathBuf>,

    /// Extra prompt variables, as NAME=VALUE.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    variables: Vec<String>,

    /// Regenerate even if the output already exists.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, clap::Args)]
struct TranscriptTaskArgs {
    /// Episode to take title, URL, chapters, and show notes from.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    episode: Option<PathBuf>,

    /// Whisper SRT transcript.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    transcript: PathBuf,

    /// Write the result here instead of standard output.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,

    /// Extra prompt variables, as NAME=VALUE.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    variables: Vec<String>,

    /// Regenerate even if the output already exists.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Subcommand)]
enum ThumbnailCommand {
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
    },
}

#[derive(Debug, Subcommand)]
enum TranscriptCommand {
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

#[derive(Debug, clap::Args)]
struct MediaArgs {
    /// Audio or video file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    input: PathBuf,

    /// Output path. Defaults to the contract's name beside the input.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    output: Option<PathBuf>,
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
struct NewArgs {
    /// Episode date, as YYYY-MM-DD.
    date: String,

    /// Episode title in human form; the folder slug is derived from it.
    title: String,

    /// Guest name, when the episode has one.
    #[arg(long)]
    guest: Option<String>,
}

#[derive(Debug, clap::Args)]
struct PromoteArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    path: PathBuf,

    /// Destination lifecycle stage.
    stage: String,

    /// Override the lifecycle guards.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, clap::Args)]
struct SetTagsArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    path: PathBuf,

    /// Read the tags from a file instead of standard input.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    from_file: Option<PathBuf>,
}

#[derive(Debug, clap::Args)]
struct MarkPlaylistArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    path: PathBuf,

    /// Playlist ID, matched against the entries in the playlist section.
    playlist: String,

    /// Detail to append, for example 'Added 2026-08-07T09:00:00Z'.
    #[arg(long)]
    note: Option<String>,
}

#[derive(Debug, clap::Args)]
struct AddPlatformArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    path: PathBuf,

    /// Platform section to add, for example Mastodon.
    platform: String,
}

#[derive(Debug, clap::Args)]
struct SetArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    path: PathBuf,

    /// Field assignments, as 'Label=value'.
    #[arg(required = true, value_name = "LABEL=VALUE")]
    assignments: Vec<String>,

    /// Enclosing heading, required when a label appears in several sections.
    #[arg(long)]
    section: Option<String>,

    /// Replace a field that already holds a value rather than a placeholder.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, clap::Args)]
struct SetDraftArgs {
    /// Episode directory or canonical metadata file.
    #[arg(value_parser = tilde_pathbuf, value_hint = ValueHint::AnyPath)]
    path: PathBuf,

    /// Social platform section, for example Bluesky.
    platform: String,

    /// Draft heading within the platform section.
    #[arg(long, default_value = "Draft")]
    heading: String,

    /// Read the draft from a file instead of standard input.
    #[arg(long, value_parser = tilde_pathbuf, value_hint = ValueHint::FilePath)]
    from_file: Option<PathBuf>,
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
        Some(RootCommand::Audio { command: None }) => print_family_help("audio"),
        Some(RootCommand::Audio {
            command: Some(AudioCommand::Analyse { input }),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::media::analyse(&path, &input, cli.format)
        }
        Some(RootCommand::Audio {
            command: Some(AudioCommand::Process(args)),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::media::process(
                &path,
                &args.input,
                args.output.as_deref(),
                cli.dry_run,
                cli.format,
            )
        }
        Some(RootCommand::Metadata { command: None }) => print_family_help("metadata"),
        Some(RootCommand::Metadata {
            command: Some(command),
        }) => {
            let config = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            run_metadata(command, &config, cli.dry_run, cli.format)
        }
        Some(RootCommand::Thumbnail { command: None }) => print_family_help("thumbnail"),
        Some(RootCommand::Thumbnail {
            command: Some(ThumbnailCommand::Frames { video, count }),
        }) => commands::media::frames(&video, count, cli.dry_run, cli.format),
        Some(RootCommand::Thumbnail {
            command:
                Some(ThumbnailCommand::Compose {
                    image,
                    text,
                    output,
                    no_quotes,
                }),
        }) => {
            let config = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::media::compose(
                &config,
                &image,
                &text,
                output.as_deref(),
                no_quotes,
                cli.dry_run,
                cli.format,
            )
        }
        Some(RootCommand::Transcript { command: None }) => print_family_help("transcript"),
        Some(RootCommand::Transcript {
            command: Some(TranscriptCommand::Extract(args)),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::media::extract(
                &path,
                &args.input,
                args.output.as_deref(),
                cli.dry_run,
                cli.format,
            )
        }
        Some(RootCommand::Transcript {
            command: Some(TranscriptCommand::Transcribe { input, output_dir }),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::media::transcribe(
                &path,
                &input,
                output_dir.as_deref(),
                cli.dry_run,
                cli.format,
            )
        }
        Some(RootCommand::Channel { command: None }) => print_family_help("channel"),
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
        Some(RootCommand::Episode { command: None }) => print_family_help("episode"),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::Inspect { path }),
        }) => commands::episode::inspect(&path, cli.format),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::Validate { path }),
        }) => commands::episode::validate(&path, cli.format),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::Set(args)),
        }) => commands::episode::set(
            &args.path,
            &args.assignments,
            args.section.as_deref(),
            args.force,
            cli.dry_run,
            cli.format,
        ),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::SetDraft(args)),
        }) => {
            let text = read_input(args.from_file.as_deref(), "draft")?;
            commands::episode::set_draft(
                &args.path,
                &args.platform,
                &args.heading,
                &text,
                cli.dry_run,
                cli.format,
            )
        }
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::SetTags(args)),
        }) => {
            let tags = read_input(args.from_file.as_deref(), "tags")?;
            commands::episode::set_tags(&args.path, &tags, cli.dry_run, cli.format)
        }
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::MarkPlaylist(args)),
        }) => commands::episode::mark_playlist(
            &args.path,
            &args.playlist,
            args.note.as_deref(),
            cli.dry_run,
            cli.format,
        ),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::AddPlatform(args)),
        }) => commands::episode::add_platform(&args.path, &args.platform, cli.dry_run, cli.format),
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::New(args)),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::episode::new(
                &path,
                &args.date,
                &args.title,
                args.guest.as_deref(),
                cli.dry_run,
                cli.format,
            )
        }
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::List { stage }),
        }) => {
            let path = channel::resolve_path(cli.config.as_deref(), cli.channel.as_deref())?;
            commands::episode::list(&path, stage.as_deref(), cli.format)
        }
        Some(RootCommand::Episode {
            command: Some(EpisodeCommand::Promote(args)),
        }) => {
            commands::episode::promote(&args.path, &args.stage, args.force, cli.dry_run, cli.format)
        }
    }
}

/// Read command input from a file or standard input.
fn read_input(from_file: Option<&Path>, what: &str) -> Result<String> {
    match from_file {
        Some(path) => fs::read_to_string(path)
            .with_context(|| format!("could not read {what} from {}", path.display())),
        None => {
            let mut text = String::new();
            io::stdin()
                .read_to_string(&mut text)
                .with_context(|| format!("could not read {what} from standard input"))?;
            Ok(text)
        }
    }
}

fn print_root_help() -> Result<()> {
    Cli::command().print_help()?;
    println!();
    Ok(())
}

fn run_metadata(
    command: MetadataCommand,
    config: &Path,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    use commands::metadata::Request;

    let (task, transcript, output, variables, force, episode) = match &command {
        MetadataCommand::Chapters(args) => (
            "chapters",
            Some(args.transcript.clone()),
            args.output.clone(),
            args.variables.clone(),
            args.force,
            args.episode.clone(),
        ),
        MetadataCommand::Tags(args) => (
            "tags",
            Some(args.transcript.clone()),
            args.output.clone(),
            args.variables.clone(),
            args.force,
            args.episode.clone(),
        ),
        MetadataCommand::Quote(args) => (
            "thumbnail-quote",
            Some(args.transcript.clone()),
            args.output.clone(),
            args.variables.clone(),
            args.force,
            args.episode.clone(),
        ),
        MetadataCommand::Description(args) => (
            "youtube-description",
            args.transcript.clone(),
            args.output.clone(),
            args.variables.clone(),
            args.force,
            args.episode.clone(),
        ),
        MetadataCommand::Social { task, args } => (
            task.as_str(),
            args.transcript.clone(),
            args.output.clone(),
            args.variables.clone(),
            args.force,
            args.episode.clone(),
        ),
        MetadataCommand::Check {
            task,
            input,
            transcript,
        } => {
            return commands::metadata::check(config, task, input, transcript.as_deref(), format);
        }
    };

    commands::metadata::generate(
        &Request {
            task,
            config_path: config,
            episode: episode.as_deref(),
            transcript_path: transcript.as_deref(),
            variables: &variables,
            output: output.as_deref(),
            force,
            dry_run,
        },
        format,
    )
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
