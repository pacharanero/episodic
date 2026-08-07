// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A channel configuration is a small generic core (`channel`, `paths`, `llm`,
/// `storage`, `transcription`, `thumbnail`) plus typed integration records for
/// the destinations a given channel happens to use. A channel that publishes
/// nowhere but YouTube carries no social or archive sections, and no integration
/// section is required to load or validate a config.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelConfig {
    pub channel: ChannelIdentity,
    #[serde(default)]
    pub paths: ChannelPaths,
    #[serde(default)]
    pub llm: Option<LlmPolicy>,
    #[serde(default)]
    pub storage: Option<StoragePolicy>,
    #[serde(default)]
    pub transcription: Option<TranscriptionPolicy>,
    #[serde(default)]
    pub thumbnail: Option<ThumbnailPolicy>,
    #[serde(default)]
    pub description: Option<DescriptionPolicy>,
    #[serde(default)]
    pub hashtags: Option<HashtagPolicy>,
    #[serde(default)]
    pub youtube: Option<YoutubeIntegration>,
    #[serde(default)]
    pub playlists: Option<PlaylistPolicy>,
    #[serde(default)]
    pub credentials: BTreeMap<String, CredentialReference>,
    #[serde(default)]
    pub recording: BTreeMap<String, RecordingProfile>,
    #[serde(default)]
    pub archive: BTreeMap<String, ArchiveTarget>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelIdentity {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub voice: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelPaths {
    pub episodes: Option<PathBuf>,
}

/// Provider-neutral model routing. `profiles` name a way to reach a model;
/// `tasks` route a named generation task to one of those profiles.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmPolicy {
    pub default_profile: String,
    #[serde(default)]
    pub profiles: BTreeMap<String, LlmProfile>,
    #[serde(default)]
    pub tasks: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LlmProfile {
    pub provider: String,
    pub model: String,
    /// Executable for `provider = "cli"`; unused by provider-native routing.
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoragePolicy {
    pub raw_media_retention_days: Option<u32>,
    /// Template for the file actually uploaded, with `<base>` substituted.
    pub canonical_uploaded_video: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionPolicy {
    pub backend: String,
    pub model: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThumbnailPolicy {
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub font_path: Option<PathBuf>,
    #[serde(default)]
    pub point_size: Option<u32>,
    #[serde(default)]
    pub fill: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
    #[serde(default)]
    pub stroke_width: Option<u32>,
    #[serde(default)]
    pub uppercase: Option<bool>,
    #[serde(default)]
    pub quote_by_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptionPolicy {
    #[serde(default)]
    pub contact_email: Option<String>,
    #[serde(default)]
    pub boilerplate: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HashtagPolicy {
    #[serde(default)]
    pub default: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YoutubeIntegration {
    pub channel_id: String,
    #[serde(default)]
    pub handle: Option<String>,
    #[serde(default)]
    pub subscribe_url: Option<String>,
    #[serde(default)]
    pub default_privacy: Option<String>,
    /// Defaults recorded on a new episode, so upload settings are channel
    /// policy rather than constants inside a script. `category` and `license`
    /// are the human strings written to the episode record; `category_id` and
    /// `license_id` are what the API expects.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub category_id: Option<u32>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub license_id: Option<String>,
}

/// Non-secret joining details for a recording tool.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingProfile {
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub guest_url: Option<String>,
    #[serde(default)]
    pub host_url: Option<String>,
    #[serde(default)]
    pub director_url: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaylistPolicy {
    #[serde(default)]
    pub binge_watch_id: Option<String>,
    #[serde(default)]
    pub audio_only_id: Option<String>,
    /// Human-readable topic name to playlist ID.
    #[serde(default)]
    pub topic: BTreeMap<String, String>,
}

/// A pointer to where a credential lives. Never the credential itself.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialReference {
    #[serde(default)]
    pub client_secret_path: Option<PathBuf>,
    #[serde(default)]
    pub token_path: Option<PathBuf>,
    #[serde(default)]
    pub env: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveTarget {
    #[serde(default)]
    pub topic_url: Option<String>,
    #[serde(default)]
    pub topic_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ChannelSummary {
    pub config_path: PathBuf,
    pub id: String,
    pub name: String,
    pub host: String,
    pub timezone: String,
    pub episodes_path: Option<PathBuf>,
    pub sections: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ChannelValidation {
    pub config_path: PathBuf,
    pub valid: bool,
    pub issues: Vec<String>,
    pub channel: Option<ChannelSummary>,
}

#[derive(Debug, Serialize)]
pub struct ChannelListEntry {
    pub config_path: PathBuf,
    pub id: Option<String>,
    pub name: Option<String>,
    pub valid: bool,
    pub issues: Vec<String>,
}

impl ChannelConfig {
    /// Section names actually present, so `channel show` reports what a channel
    /// configures rather than the full schema.
    fn present_sections(&self) -> Vec<String> {
        let mut sections = Vec::new();
        let mut note = |present: bool, name: &str| {
            if present {
                sections.push(name.to_owned());
            }
        };
        note(self.paths.episodes.is_some(), "paths");
        note(self.llm.is_some(), "llm");
        note(self.storage.is_some(), "storage");
        note(self.transcription.is_some(), "transcription");
        note(self.thumbnail.is_some(), "thumbnail");
        note(self.description.is_some(), "description");
        note(self.hashtags.is_some(), "hashtags");
        note(self.youtube.is_some(), "youtube");
        note(self.playlists.is_some(), "playlists");
        note(!self.credentials.is_empty(), "credentials");
        note(!self.recording.is_empty(), "recording");
        note(!self.archive.is_empty(), "archive");
        sections.sort();
        sections
    }

    pub fn summary(&self, config_path: &Path) -> ChannelSummary {
        let sections = self.present_sections();
        ChannelSummary {
            config_path: config_path.to_path_buf(),
            id: self.channel.id.clone(),
            name: self.channel.name.clone(),
            host: self.channel.host.clone(),
            timezone: self.channel.timezone.clone(),
            episodes_path: self.paths.episodes.clone(),
            sections,
        }
    }
}

impl ChannelConfig {
    /// Resolve the episodes directory against the workspace root.
    ///
    /// Relative paths in a config are relative to the workspace, not to the
    /// config file: a config at `<workspace>/channels/<id>.toml` saying
    /// `episodes = "episodes"` means `<workspace>/episodes`. A config kept
    /// outside a `channels` directory is resolved against its own directory.
    pub fn episodes_path(&self, config_path: &Path) -> Result<PathBuf> {
        let episodes = self
            .paths
            .episodes
            .clone()
            .unwrap_or_else(|| PathBuf::from("episodes"));
        if episodes.is_absolute() {
            return Ok(episodes);
        }
        Ok(workspace_root(config_path)?.join(episodes))
    }
}

/// The directory a relative config path is resolved against.
fn workspace_root(config_path: &Path) -> Result<PathBuf> {
    let directory = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    if directory.file_name().is_some_and(|name| name == "channels")
        && let Some(parent) = directory.parent()
    {
        return Ok(if parent.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            parent.to_path_buf()
        });
    }
    Ok(directory)
}

pub fn load(path: &Path) -> Result<ChannelConfig> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("could not read channel config {}", path.display()))?;
    toml::from_str(&source)
        .with_context(|| format!("could not parse channel config {}", path.display()))
}

pub fn validate(path: &Path) -> ChannelValidation {
    match load(path) {
        Ok(config) => {
            let issues = config_issues(&config);
            ChannelValidation {
                config_path: path.to_path_buf(),
                valid: issues.is_empty(),
                issues,
                channel: Some(config.summary(path)),
            }
        }
        Err(error) => ChannelValidation {
            config_path: path.to_path_buf(),
            valid: false,
            issues: vec![format!("{error:#}")],
            channel: None,
        },
    }
}

pub fn list(directory: &Path) -> Result<Vec<ChannelListEntry>> {
    let mut paths = fs::read_dir(directory)
        .with_context(|| format!("could not read channel directory {}", directory.display()))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect::<Vec<_>>();
    paths.sort();

    Ok(paths
        .into_iter()
        .map(|path| {
            let result = validate(&path);
            let (id, name) = result
                .channel
                .as_ref()
                .map(|channel| (Some(channel.id.clone()), Some(channel.name.clone())))
                .unwrap_or_default();
            ChannelListEntry {
                config_path: path,
                id,
                name,
                valid: result.valid,
                issues: result.issues,
            }
        })
        .collect())
}

pub fn resolve_path(explicit: Option<&Path>, channel: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_path_buf());
    }

    let directory = PathBuf::from("channels");
    if let Some(id) = channel {
        let path = directory.join(format!("{id}.toml"));
        if path.is_file() {
            return Ok(path);
        }
        bail!(
            "channel config not found: {} (pass --config explicitly)",
            path.display()
        );
    }

    let entries = list(&directory)?;
    match entries.as_slice() {
        [only] => Ok(only.config_path.clone()),
        [] => bail!(
            "no channel configs found in {} (pass --config explicitly)",
            directory.display()
        ),
        _ => bail!("multiple channel configs found; pass --channel or --config"),
    }
}

fn config_issues(config: &ChannelConfig) -> Vec<String> {
    let mut issues = Vec::new();
    if !valid_id(&config.channel.id) {
        issues.push(
            "channel.id must use lowercase ASCII letters, digits, and single hyphens".to_owned(),
        );
    }
    if config.channel.name.trim().is_empty() {
        issues.push("channel.name must not be empty".to_owned());
    }
    if config.channel.timezone.trim().is_empty() {
        issues.push("channel.timezone must not be empty".to_owned());
    }
    if let Some(youtube) = &config.youtube {
        if youtube.channel_id.trim().is_empty() {
            issues.push("youtube.channel_id must not be empty".to_owned());
        }
        if let Some(privacy) = &youtube.default_privacy
            && !matches!(privacy.as_str(), "private" | "unlisted" | "public")
        {
            issues.push("youtube.default_privacy must be private, unlisted, or public".to_owned());
        }
    }
    if let Some(llm) = &config.llm {
        if !llm.profiles.contains_key(&llm.default_profile) {
            issues.push(format!(
                "llm.default_profile references undefined profile '{}'",
                llm.default_profile
            ));
        }
        for (task, profile) in &llm.tasks {
            if !llm.profiles.contains_key(profile) {
                issues.push(format!(
                    "llm.tasks.{task} references undefined profile '{profile}'"
                ));
            }
        }
        for (name, profile) in &llm.profiles {
            if profile.provider == "cli" && profile.command.is_none() {
                issues.push(format!(
                    "llm.profiles.{name} uses provider 'cli' and must set command"
                ));
            }
        }
    }
    if let Some(storage) = &config.storage
        && let Some(template) = &storage.canonical_uploaded_video
        && !template.contains("<base>")
    {
        issues.push("storage.canonical_uploaded_video must contain <base>".to_owned());
    }
    if let Some(thumbnail) = &config.thumbnail
        && (thumbnail.width == 0 || thumbnail.height == 0)
    {
        issues.push("thumbnail.width and thumbnail.height must be greater than zero".to_owned());
    }
    issues
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && !id.contains("--")
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::{ChannelConfig, config_issues};

    #[test]
    fn accepts_minimal_valid_channel() {
        let config: ChannelConfig = toml::from_str(
            "[channel]\nid = 'test-channel'\nname = 'Test Channel'\ntimezone = 'Europe/London'\n",
        )
        .unwrap();
        assert!(config_issues(&config).is_empty());
    }

    #[test]
    fn rejects_invalid_channel_id() {
        let config: ChannelConfig =
            toml::from_str("[channel]\nid = 'Test_Channel'\nname = 'Test'\ntimezone = 'UTC'\n")
                .unwrap();
        assert_eq!(config_issues(&config).len(), 1);
    }

    fn identity() -> &'static str {
        "[channel]\nid = 'test-channel'\nname = 'Test'\ntimezone = 'UTC'\n"
    }

    #[test]
    fn rejects_task_routed_to_undefined_profile() {
        let config: ChannelConfig = toml::from_str(&format!(
            "{}\n[llm]\ndefault_profile = 'main'\n\n[llm.profiles.main]\nprovider = 'anthropic'\nmodel = 'claude-sonnet-5'\n\n[llm.tasks]\nchapters = 'missing'\n",
            identity()
        ))
        .unwrap();
        assert_eq!(config_issues(&config).len(), 1);
    }

    #[test]
    fn rejects_cli_profile_without_command() {
        let config: ChannelConfig = toml::from_str(&format!(
            "{}\n[llm]\ndefault_profile = 'main'\n\n[llm.profiles.main]\nprovider = 'cli'\nmodel = 'some-model'\n",
            identity()
        ))
        .unwrap();
        assert_eq!(config_issues(&config).len(), 1);
    }

    #[test]
    fn rejects_upload_template_without_base_placeholder() {
        let config: ChannelConfig = toml::from_str(&format!(
            "{}\n[storage]\ncanonical_uploaded_video = 'final.mp4'\n",
            identity()
        ))
        .unwrap();
        assert_eq!(config_issues(&config).len(), 1);
    }

    #[test]
    fn rejects_unknown_top_level_section() {
        let result =
            toml::from_str::<ChannelConfig>(&format!("{}\n[typo]\nvalue = 1\n", identity()));
        assert!(result.is_err());
    }

    #[test]
    fn resolves_episode_paths_against_the_workspace_not_the_config() {
        let config: ChannelConfig =
            toml::from_str(&format!("{}\n[paths]\nepisodes = 'episodes'\n", identity())).unwrap();
        assert_eq!(
            config
                .episodes_path(std::path::Path::new("/work/channels/example.toml"))
                .unwrap(),
            std::path::PathBuf::from("/work/episodes")
        );
        assert_eq!(
            config
                .episodes_path(std::path::Path::new("/elsewhere/example.toml"))
                .unwrap(),
            std::path::PathBuf::from("/elsewhere/episodes")
        );
    }

    #[test]
    fn defaults_the_episodes_directory_when_unset() {
        let config: ChannelConfig = toml::from_str(identity()).unwrap();
        assert_eq!(
            config
                .episodes_path(std::path::Path::new("/work/channels/example.toml"))
                .unwrap(),
            std::path::PathBuf::from("/work/episodes")
        );
    }

    #[test]
    fn accepts_every_documented_section() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("channels/example.toml"),
        )
        .unwrap();
        let config: ChannelConfig = toml::from_str(&source).unwrap();
        assert!(config_issues(&config).is_empty());
        assert_eq!(
            config.present_sections(),
            vec![
                "archive",
                "credentials",
                "description",
                "hashtags",
                "llm",
                "paths",
                "playlists",
                "recording",
                "storage",
                "thumbnail",
                "transcription",
                "youtube",
            ]
        );
    }
}
