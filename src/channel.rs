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
    pub audio: Option<AudioPolicy>,
    #[serde(default)]
    pub metadata: Option<MetadataPolicy>,
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

/// What generated metadata must look like before it may be published.
///
/// These are the task contracts a model's answer is checked against. They are
/// configuration because a channel may reasonably want different lengths or
/// different required text; only structural rules live in code.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataPolicy {
    #[serde(default = "default_chapters_min")]
    pub chapters_min: u32,
    #[serde(default = "default_chapters_max")]
    pub chapters_max: u32,
    /// The exact first chapter line YouTube expects.
    #[serde(default = "default_first_chapter")]
    pub first_chapter: String,
    #[serde(default = "default_chapter_title_min")]
    pub chapter_title_min_words: u32,
    #[serde(default = "default_chapter_title_max")]
    pub chapter_title_max_words: u32,
    /// How much of the transcript the last chapter must reach. This is what
    /// catches a model that only saw part of the episode.
    #[serde(default = "default_chapter_coverage")]
    pub chapter_coverage: f64,
    #[serde(default = "default_tags_min")]
    pub tags_min: u32,
    #[serde(default = "default_tags_max")]
    pub tags_max: u32,
    #[serde(default = "default_tags_characters")]
    pub tags_max_characters: u32,
    /// A tag every episode must end with, if the channel wants one.
    #[serde(default)]
    pub tags_required_final: Option<String>,
    #[serde(default = "default_description_characters")]
    pub description_max_characters: u32,
    #[serde(default)]
    pub description_required_headings: Vec<String>,
    #[serde(default = "default_quote_count")]
    pub thumbnail_quote_count: u32,
    #[serde(default = "default_quote_min_words")]
    pub thumbnail_quote_min_words: u32,
    #[serde(default = "default_quote_max_words")]
    pub thumbnail_quote_max_words: u32,
    /// Per-platform draft limits, keyed by task name.
    #[serde(default)]
    pub social: BTreeMap<String, SocialLimits>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocialLimits {
    #[serde(default)]
    pub max_characters: Option<u32>,
    #[serde(default)]
    pub min_words: Option<u32>,
    #[serde(default)]
    pub max_words: Option<u32>,
    /// Exactly how many URLs the draft must contain, when it matters.
    #[serde(default)]
    pub urls: Option<u32>,
    /// Text the draft must include, such as a first-comment pointer.
    #[serde(default)]
    pub must_contain: Option<String>,
}

fn default_chapters_min() -> u32 {
    6
}
fn default_chapters_max() -> u32 {
    12
}
fn default_first_chapter() -> String {
    "0:00 Introduction".to_owned()
}
fn default_chapter_title_min() -> u32 {
    3
}
fn default_chapter_title_max() -> u32 {
    8
}
fn default_chapter_coverage() -> f64 {
    0.75
}
fn default_tags_min() -> u32 {
    10
}
fn default_tags_max() -> u32 {
    15
}
fn default_tags_characters() -> u32 {
    500
}
fn default_description_characters() -> u32 {
    5000
}
fn default_quote_count() -> u32 {
    5
}
fn default_quote_min_words() -> u32 {
    6
}
fn default_quote_max_words() -> u32 {
    13
}

impl Default for MetadataPolicy {
    fn default() -> Self {
        Self {
            chapters_min: default_chapters_min(),
            chapters_max: default_chapters_max(),
            first_chapter: default_first_chapter(),
            chapter_title_min_words: default_chapter_title_min(),
            chapter_title_max_words: default_chapter_title_max(),
            chapter_coverage: default_chapter_coverage(),
            tags_min: default_tags_min(),
            tags_max: default_tags_max(),
            tags_max_characters: default_tags_characters(),
            tags_required_final: None,
            description_max_characters: default_description_characters(),
            description_required_headings: Vec::new(),
            thumbnail_quote_count: default_quote_count(),
            thumbnail_quote_min_words: default_quote_min_words(),
            thumbnail_quote_max_words: default_quote_max_words(),
            social: BTreeMap::new(),
        }
    }
}

/// The loudness chain applied to an episode's audio.
///
/// Defaults are the podcast delivery targets used by YouTube and Spotify.
/// They are configuration rather than constants so a channel with different
/// material can retune without touching the code.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioPolicy {
    /// High-pass cutoff in Hz, removing rumble below speech.
    #[serde(default = "default_highpass")]
    pub highpass_hz: u32,
    /// Noise floor for the FFT denoiser, in dB.
    #[serde(default = "default_denoise")]
    pub denoise_floor_db: f64,
    /// Integrated loudness target, in LUFS.
    #[serde(default = "default_target_lufs")]
    pub target_lufs: f64,
    /// True-peak ceiling, in dBTP.
    #[serde(default = "default_true_peak")]
    pub true_peak: f64,
    /// Loudness range target, in LU. Lower is flatter and more compressed.
    #[serde(default = "default_loudness_range")]
    pub loudness_range: f64,
    #[serde(default = "default_aac_bitrate")]
    pub aac_bitrate_kbps: u32,
    #[serde(default = "default_output_rate")]
    pub output_sample_rate: u32,
    /// Sample rate of the mono track produced for transcription. Whisper
    /// expects 16 kHz, so this rarely changes.
    #[serde(default = "default_transcription_rate")]
    pub transcription_sample_rate: u32,
}

fn default_highpass() -> u32 {
    80
}
fn default_denoise() -> f64 {
    -25.0
}
fn default_target_lufs() -> f64 {
    -16.0
}
fn default_true_peak() -> f64 {
    -1.5
}
fn default_loudness_range() -> f64 {
    7.0
}
fn default_aac_bitrate() -> u32 {
    192
}
fn default_output_rate() -> u32 {
    48_000
}
fn default_transcription_rate() -> u32 {
    16_000
}

impl Default for AudioPolicy {
    fn default() -> Self {
        Self {
            highpass_hz: default_highpass(),
            denoise_floor_db: default_denoise(),
            target_lufs: default_target_lufs(),
            true_peak: default_true_peak(),
            loudness_range: default_loudness_range(),
            aac_bitrate_kbps: default_aac_bitrate(),
            output_sample_rate: default_output_rate(),
            transcription_sample_rate: default_transcription_rate(),
        }
    }
}

impl AudioPolicy {
    /// Everything applied before loudness normalisation. The dynaudnorm
    /// settings are tuned for speech: half-second frames, a wide gaussian
    /// window, and a bounded gain so quiet passages do not pump.
    pub fn preprocess_chain(&self) -> String {
        format!(
            "highpass=f={},afftdn=nf={},dynaudnorm=f=500:g=21:p=0.95:m=12",
            self.highpass_hz, self.denoise_floor_db
        )
    }

    pub fn loudnorm_targets(&self) -> String {
        format!(
            "loudnorm=I={}:TP={}:LRA={}",
            self.target_lufs, self.true_peak, self.loudness_range
        )
    }
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
        note(self.audio.is_some(), "audio");
        note(self.metadata.is_some(), "metadata");
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
pub fn workspace_root(config_path: &Path) -> Result<PathBuf> {
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
    if let Some(metadata) = &config.metadata {
        if metadata.chapters_min > metadata.chapters_max {
            issues.push("metadata.chapters_min must not exceed metadata.chapters_max".to_owned());
        }
        if metadata.tags_min > metadata.tags_max {
            issues.push("metadata.tags_min must not exceed metadata.tags_max".to_owned());
        }
        if !(0.0..=1.0).contains(&metadata.chapter_coverage) {
            issues.push("metadata.chapter_coverage must be a fraction between 0 and 1".to_owned());
        }
    }
    if let Some(audio) = &config.audio {
        if audio.target_lufs > 0.0 {
            issues.push("audio.target_lufs is a LUFS value and must not be positive".to_owned());
        }
        if audio.true_peak > 0.0 {
            issues.push("audio.true_peak is a dBTP ceiling and must not be positive".to_owned());
        }
        if audio.loudness_range <= 0.0 {
            issues.push("audio.loudness_range must be greater than zero".to_owned());
        }
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
                "audio",
                "credentials",
                "description",
                "hashtags",
                "llm",
                "metadata",
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
