// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Machine-discoverable command contracts derived from the CLI declaration.
//!
//! This is the registry boundary for future GUI and MCP surfaces. It keeps the
//! executable command tree and discovery data together while dispatch migrates
//! incrementally out of the CLI adapter.

use crate::channel;
use crate::cli::{
    AudioCommand, ChannelCommand, CleanupCommand, CompletionCommand, EpisodeCommand,
    MetadataCommand, StorageCommand, ThumbnailCommand, TranscriptCommand, YoutubeCommand,
};
use crate::commands;
use crate::output::OutputFormat;
use crate::thumbnail::ComposeRequest;
use crate::youtube::publish;
use anyhow::{Context, Result};
use clap::{ArgAction, Command};
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Safety {
    ReadOnly,
    LocalWrite,
    RemoteWrite,
    DestructiveLocal,
}

#[derive(Debug, Serialize)]
pub struct CommandDefinition {
    pub id: String,
    pub path: Vec<String>,
    pub description: String,
    pub safety: Safety,
    pub schema: Value,
    pub template: Value,
}

#[derive(Debug)]
pub struct Registry {
    commands: Vec<CommandDefinition>,
}

impl Registry {
    pub fn from_clap(command: &Command) -> Self {
        let mut commands = Vec::new();
        collect(command, &[], &mut commands);
        Self { commands }
    }

    pub fn commands(&self) -> &[CommandDefinition] {
        &self.commands
    }

    pub fn get(&self, path: &[String]) -> Option<&CommandDefinition> {
        self.commands.iter().find(|command| command.path == path)
    }
}

#[derive(Debug)]
pub enum Discovery {
    Schema(Value),
    Template(Value),
}

pub(crate) fn run_version(format: OutputFormat) -> Result<()> {
    commands::version::run(format)
}

pub(crate) fn run_commands(clap: Command, format: OutputFormat) -> Result<()> {
    let registry = Registry::from_clap(&clap);
    let text = registry
        .commands()
        .iter()
        .map(|command| format!("{}\t{:?}", command.path.join(" "), command.safety))
        .collect::<Vec<_>>()
        .join("\n");
    crate::output::write(format, registry.commands(), &text)
}

pub(crate) fn run_doctor(config: Option<&Path>, format: OutputFormat) -> Result<()> {
    commands::doctor::run(config, format)
}

/// Execute metadata commands from their typed registry boundary. Other command
/// families migrate here incrementally so future surfaces gain one execution
/// route rather than duplicating the CLI's mutation logic.
pub(crate) fn run_metadata(
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

pub(crate) fn run_completions(
    command: CompletionCommand,
    dry_run: bool,
    format: OutputFormat,
    clap: Command,
) -> Result<()> {
    let (shell, directory) = match command {
        CompletionCommand::Bash(args) => (clap_complete::Shell::Bash, args.directory),
        CompletionCommand::Zsh(args) => (clap_complete::Shell::Zsh, args.directory),
        CompletionCommand::Fish(args) => (clap_complete::Shell::Fish, args.directory),
        CompletionCommand::Powershell(args) => (clap_complete::Shell::PowerShell, args.directory),
        CompletionCommand::Elvish(args) => (clap_complete::Shell::Elvish, args.directory),
        CompletionCommand::Install(args) => {
            return commands::completions::install(
                args.shell,
                args.directory.as_deref(),
                dry_run,
                clap,
                format,
            );
        }
    };
    commands::completions::generate_completion(shell, directory.as_deref(), dry_run, clap)
}

/// Execute selected-workspace inspection from the registry boundary.
pub(crate) fn run_channel(
    command: ChannelCommand,
    config: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    let path = channel::resolve_path(config)?;
    match command {
        ChannelCommand::List => commands::channel::list(&path, format),
        ChannelCommand::Show => commands::channel::show(&path, format),
        ChannelCommand::Validate => commands::channel::validate(&path, format),
    }
}

pub(crate) fn run_storage(
    command: StorageCommand,
    config: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    let path = channel::resolve_path(config)?;
    match command {
        StorageCommand::Audit { stages } => commands::retention::audit(&path, &stages, format),
    }
}

pub(crate) fn run_cleanup(
    command: CleanupCommand,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    match command {
        CleanupCommand::Plan {
            episode,
            include_review,
        } => commands::retention::cleanup(&episode, include_review, false, dry_run, format),
        CleanupCommand::Apply {
            episode,
            include_review,
            apply,
        } => commands::retention::cleanup(
            &episode,
            include_review,
            apply && !dry_run,
            dry_run,
            format,
        ),
    }
}

pub(crate) fn run_thumbnail(
    command: ThumbnailCommand,
    config: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    match command {
        ThumbnailCommand::Frames { video, count } => {
            commands::media::frames(&video, count, dry_run, format)
        }
        ThumbnailCommand::Compose {
            image,
            text,
            output,
            no_quotes,
            force,
        } => {
            let config = channel::resolve_path(config)?;
            commands::media::compose(
                &config,
                &ComposeRequest {
                    image: &image,
                    text: &text,
                    output: output.as_deref(),
                    no_quotes,
                    force,
                },
                dry_run,
                format,
            )
        }
    }
}

pub(crate) fn run_transcript(
    command: TranscriptCommand,
    config: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::resolve_path(config)?;
    match command {
        TranscriptCommand::Extract(args) => commands::media::extract(
            &config,
            &args.input,
            args.output.as_deref(),
            dry_run,
            format,
        ),
        TranscriptCommand::Transcribe { input, output_dir } => {
            commands::media::transcribe(&config, &input, output_dir.as_deref(), dry_run, format)
        }
    }
}

pub(crate) fn run_audio(
    command: AudioCommand,
    config: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::resolve_path(config)?;
    match command {
        AudioCommand::Analyse { input } => commands::media::analyse(&config, &input, format),
        AudioCommand::Process(args) => commands::media::process(
            &config,
            &args.input,
            args.output.as_deref(),
            dry_run,
            format,
        ),
    }
}

pub(crate) fn run_episode(
    command: EpisodeCommand,
    config: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    match command {
        EpisodeCommand::Inspect { path } => commands::episode::inspect(&path, format),
        EpisodeCommand::Validate { path } => commands::episode::validate(&path, format),
        EpisodeCommand::Set(args) => commands::episode::set(
            &args.path,
            &args.assignments,
            args.section.as_deref(),
            args.force,
            dry_run,
            format,
        ),
        EpisodeCommand::SetDraft(args) => {
            let text = read_input(args.from_file.as_deref(), "draft")?;
            commands::episode::set_draft(
                &args.path,
                &args.platform,
                &args.heading,
                &text,
                dry_run,
                format,
            )
        }
        EpisodeCommand::SetTags(args) => {
            let tags = read_input(args.from_file.as_deref(), "tags")?;
            commands::episode::set_tags(&args.path, &tags, dry_run, format)
        }
        EpisodeCommand::MarkPlaylist(args) => commands::episode::mark_playlist(
            &args.path,
            &args.playlist,
            args.note.as_deref(),
            dry_run,
            format,
        ),
        EpisodeCommand::AddPlatform(args) => {
            commands::episode::add_platform(&args.path, &args.platform, dry_run, format)
        }
        EpisodeCommand::New(args) => {
            let path = channel::resolve_path(config)?;
            commands::episode::new(
                &path,
                &args.date,
                &args.title,
                args.guest.as_deref(),
                dry_run,
                format,
            )
        }
        EpisodeCommand::List { stage } => {
            let path = channel::resolve_path(config)?;
            commands::episode::list(&path, stage.as_deref(), format)
        }
        EpisodeCommand::Promote(args) => {
            commands::episode::promote(&args.path, &args.stage, args.force, dry_run, format)
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

pub(crate) fn run_youtube(
    command: YoutubeCommand,
    config: Option<&Path>,
    dry_run: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::resolve_path(config)?;
    match command {
        YoutubeCommand::PlanUpload(args) => commands::youtube::plan_upload(
            &config,
            &args.episode,
            &args.playlists,
            args.privacy.as_deref(),
            format,
        ),
        YoutubeCommand::Upload(args) => commands::youtube::upload(
            &config,
            &args.episode,
            &args.playlists,
            args.privacy.as_deref(),
            args.apply && !dry_run,
            format,
        ),
        YoutubeCommand::Verify { episode } => commands::youtube::verify(&config, &episode, format),
        YoutubeCommand::Captions(args) => commands::youtube::repair(
            &config,
            &args.episode,
            publish::RepairKind::Captions,
            args.apply && !dry_run,
            format,
        ),
        YoutubeCommand::Thumbnail(args) => commands::youtube::repair(
            &config,
            &args.episode,
            publish::RepairKind::Thumbnail,
            args.apply && !dry_run,
            format,
        ),
        YoutubeCommand::Playlist {
            episode,
            playlist,
            apply,
        } => commands::youtube::repair(
            &config,
            &episode,
            publish::RepairKind::Playlist(playlist),
            apply && !dry_run,
            format,
        ),
        YoutubeCommand::Publish {
            episode,
            privacy,
            apply,
        } => {
            commands::youtube::publish_video(&config, &episode, &privacy, apply && !dry_run, format)
        }
    }
}

/// Recognise a schema/template request before clap validates required positional
/// arguments. Normal invocations still go through clap unchanged.
pub fn discover(args: &[OsString], command: &Command) -> Option<Discovery> {
    let request = args.last()?.to_str()?;
    let template = match request {
        "--schema" => false,
        "--template" => true,
        _ => return None,
    };
    let path = command_path_without_global_options(&args[1..args.len() - 1])?;
    let registry = Registry::from_clap(command);
    let definition = registry.get(&path)?;
    Some(if template {
        Discovery::Template(definition.template.clone())
    } else {
        Discovery::Schema(definition.schema.clone())
    })
}

fn command_path_without_global_options(args: &[OsString]) -> Option<Vec<String>> {
    let mut path = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].to_str()?;
        match argument {
            "--config" | "--format" => index += 2,
            "--dry-run" | "-n" => index += 1,
            _ if argument.starts_with("--config=") || argument.starts_with("--format=") => {
                index += 1
            }
            _ if argument.starts_with('-') => return None,
            _ => {
                path.push(argument.to_owned());
                index += 1;
            }
        }
    }
    Some(path)
}

fn collect(command: &Command, parent: &[String], definitions: &mut Vec<CommandDefinition>) {
    for child in command.get_subcommands() {
        let mut path = parent.to_vec();
        path.push(child.get_name().to_owned());
        if child.has_subcommands() {
            collect(child, &path, definitions);
        } else {
            definitions.push(definition(child, path));
        }
    }
}

fn definition(command: &Command, path: Vec<String>) -> CommandDefinition {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let mut template = Map::new();
    for argument in command.get_arguments() {
        if argument.is_hide_set() {
            continue;
        }
        let name = argument
            .get_long()
            .unwrap_or_else(|| argument.get_id().as_str())
            .to_owned();
        let schema = argument_schema(argument);
        if argument.is_required_set() {
            required.push(Value::String(name.clone()));
        }
        template.insert(name.clone(), template_value(&schema, &name));
        properties.insert(name, schema);
    }
    let mut schema = Map::new();
    schema.insert(
        "$schema".to_owned(),
        Value::String("https://json-schema.org/draft/2020-12/schema".to_owned()),
    );
    schema.insert(
        "$id".to_owned(),
        Value::String(format!("urn:episodic:command:{}", path.join("/"))),
    );
    schema.insert("type".to_owned(), Value::String("object".to_owned()));
    schema.insert("additionalProperties".to_owned(), Value::Bool(false));
    schema.insert("properties".to_owned(), Value::Object(properties));
    if !required.is_empty() {
        schema.insert("required".to_owned(), Value::Array(required));
    }
    CommandDefinition {
        id: path.join("/"),
        description: command
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        safety: safety(&path),
        path,
        schema: Value::Object(schema),
        template: Value::Object(template),
    }
}

fn argument_schema(argument: &clap::Arg) -> Value {
    let description = argument.get_help().map(ToString::to_string);
    let mut schema = match argument.get_action() {
        ArgAction::SetTrue | ArgAction::SetFalse => json!({ "type": "boolean" }),
        ArgAction::Append => json!({ "type": "array", "items": { "type": "string" } }),
        _ => json!({ "type": "string" }),
    };
    if let Some(description) = description {
        schema
            .as_object_mut()
            .unwrap()
            .insert("description".to_owned(), Value::String(description));
    }
    schema
}

fn template_value(schema: &Value, name: &str) -> Value {
    match schema.get("type").and_then(Value::as_str) {
        Some("boolean") => Value::Bool(false),
        Some("array") => Value::Array(Vec::new()),
        _ => Value::String(format!("<{name}>")),
    }
}

fn safety(path: &[String]) -> Safety {
    match path
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [
            "youtube",
            "upload" | "captions" | "thumbnail" | "playlist" | "publish",
        ] => Safety::RemoteWrite,
        ["cleanup", "apply"] => Safety::DestructiveLocal,
        ["audio", "process"]
        | ["transcript", "extract" | "transcribe"]
        | ["thumbnail", "frames" | "compose"]
        | [
            "metadata",
            "chapters" | "tags" | "quote" | "description" | "social",
        ]
        | [
            "episode",
            "set" | "set-draft" | "set-tags" | "mark-playlist" | "add-platform" | "new" | "promote",
        ]
        | ["completions", _] => Safety::LocalWrite,
        _ => Safety::ReadOnly,
    }
}
